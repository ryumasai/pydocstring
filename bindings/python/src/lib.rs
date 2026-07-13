use pyo3::PyClass;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};

use pydocstring_core::matcher::Match;
use pydocstring_core::parse::Style as CoreStyle;
use pydocstring_core::pattern::Pattern;

use pydocstring_core::edit::Edits as CoreEdits;
use pydocstring_core::emit::EmitOptions;
use pydocstring_core::model;
use pydocstring_core::parse::text_block::TextBlock;
use pydocstring_core::parse::token_ref::TokenRef;
use pydocstring_core::parse::unified as uv;
use pydocstring_core::syntax::{Parsed, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
use pydocstring_core::text::TextRange;
use pydocstring_core::text::TextSize;

use std::convert::{TryFrom, TryInto};
use std::ops::Deref;
use std::sync::Arc;

// ─── TextRange ──────────────────────────────────────────────────────────────

#[pyclass(eq, hash, frozen, skip_from_py_object, module = "pydocstring", name = "TextRange")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PyTextRange {
    #[pyo3(get)]
    start: u32,
    #[pyo3(get)]
    end: u32,
}

impl From<TextRange> for PyTextRange {
    fn from(r: TextRange) -> Self {
        Self {
            start: r.start().raw(),
            end: r.end().raw(),
        }
    }
}

#[pymethods]
impl PyTextRange {
    /// Whether the range is empty (``start == end``).
    ///
    /// An empty range is used as a zero-length placeholder for tokens that are
    /// missing from the source (e.g. the type in ``arg ():``).
    fn is_empty(&self) -> bool {
        self.start == self.end
    }
    fn __repr__(&self) -> String {
        format!("TextRange({}..{})", self.start, self.end)
    }
}

// ─── LineColumn ─────────────────────────────────────────────────────────────

#[pyclass(eq, hash, frozen, skip_from_py_object, module = "pydocstring", name = "LineColumn")]
#[derive(PartialEq, Eq, Hash)]
struct PyLineColumn {
    #[pyo3(get)]
    lineno: u32,
    #[pyo3(get)]
    col: u32,
}

#[pymethods]
impl PyLineColumn {
    fn __repr__(&self) -> String {
        format!("LineColumn(lineno={}, col={})", self.lineno, self.col)
    }
}

fn build_line_starts(source: &str) -> Vec<u32> {
    let mut starts = vec![0u32];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            starts.push((i + 1) as u32);
        }
    }
    starts
}

// ─── NodeRef — lazy node addressing ─────────────────────────────────────────

/// Lazy address of a node in the immutable syntax tree: the shared parse
/// result plus the child-index path from the root.
///
/// The tree never changes after parsing, so a path is a stable address:
/// every property access re-resolves it (depth ≤ ~5, negligible) and reads
/// through the core typed accessors instead of materializing eager copies.
/// The parent's address is the path prefix (drop the last segment) — no
/// parent pointers or core changes needed.
#[derive(Clone)]
struct NodeRef {
    parsed: Arc<Parsed>,
    path: Vec<u32>,
}

/// Resolve a child-index `path` (node segments only) starting at `root`.
fn resolve_node_path<'a>(root: &'a SyntaxNode, path: &[u32]) -> &'a SyntaxNode {
    let mut node = root;
    for &idx in path {
        node = match &node.children()[idx as usize] {
            SyntaxElement::Node(n) => n,
            SyntaxElement::Token(_) => unreachable!("NodeRef path segment addresses a token"),
        };
    }
    node
}

/// Locate node `target` under `root` by pointer identity, appending its
/// child-index path to `path`. Returns whether it was found.
fn find_node_path(root: &SyntaxNode, target: &SyntaxNode, path: &mut Vec<u32>) -> bool {
    if std::ptr::eq(root, target) {
        return true;
    }
    for (idx, child) in root.children().iter().enumerate() {
        if let SyntaxElement::Node(n) = child {
            path.push(idx as u32);
            if find_node_path(n, target, path) {
                return true;
            }
            path.pop();
        }
    }
    false
}

/// Locate token `target` under `root` by pointer identity. Appends the
/// child-index path of the token's **parent node** to `path` and returns the
/// token's child index within that parent.
fn find_token_path(root: &SyntaxNode, target: &SyntaxToken, path: &mut Vec<u32>) -> Option<u32> {
    for (idx, child) in root.children().iter().enumerate() {
        match child {
            SyntaxElement::Token(t) => {
                if std::ptr::eq(t, target) {
                    return Some(idx as u32);
                }
            }
            SyntaxElement::Node(n) => {
                path.push(idx as u32);
                if let Some(found) = find_token_path(n, target, path) {
                    return Some(found);
                }
                path.pop();
            }
        }
    }
    None
}

impl NodeRef {
    /// Address the root of `parsed` (empty path).
    fn root(parsed: Arc<Parsed>) -> Self {
        Self {
            parsed,
            path: Vec::new(),
        }
    }

    /// Resolve to the addressed `SyntaxNode`.
    fn node(&self) -> &SyntaxNode {
        resolve_node_path(self.parsed.root(), &self.path)
    }

    /// Address a descendant node returned by a core accessor on this node.
    fn child_ref(&self, node: &SyntaxNode) -> NodeRef {
        let mut path = self.path.clone();
        let found = find_node_path(self.node(), node, &mut path);
        debug_assert!(found, "node does not belong to this subtree");
        NodeRef {
            parsed: Arc::clone(&self.parsed),
            path,
        }
    }

    /// Wrap a descendant node in its lazy pyclass wrapper.
    fn wrap_child<T>(&self, py: Python<'_>, node: &SyntaxNode) -> PyResult<Py<T>>
    where
        T: PyClass + From<NodeRef> + Into<pyo3::PyClassInitializer<T>>,
    {
        Py::new(py, T::from(self.child_ref(node)))
    }

    fn py_range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.node().range()))
    }

    /// Wrap the addressed node itself in the raw-CST `Node` view (#126).
    fn py_node(&self, py: Python<'_>) -> PyResult<Py<PyNode>> {
        Py::new(py, PyNode::from(self.clone()))
    }

    // ── Token helpers ────────────────────────────────────────────────────

    /// Address a token returned by a core accessor on this node.
    fn token(&self, py: Python<'_>, token: &SyntaxToken) -> PyResult<Py<PyToken>> {
        let mut path = self.path.clone();
        let index = find_token_path(self.node(), token, &mut path).expect("token does not belong to this subtree");
        Py::new(
            py,
            PyToken {
                parent: NodeRef {
                    parsed: Arc::clone(&self.parsed),
                    path,
                },
                index,
            },
        )
    }

    fn token_opt(&self, py: Python<'_>, token: Option<TokenRef<'_>>) -> PyResult<Option<Py<PyToken>>> {
        token.map(|t| self.token(py, t.syntax())).transpose()
    }

    fn tokens<'a>(&self, py: Python<'_>, tokens: impl Iterator<Item = TokenRef<'a>>) -> PyResult<Vec<Py<PyToken>>> {
        tokens.map(|t| self.token(py, t.syntax())).collect()
    }

    // ── TextBlock helpers ────────────────────────────────────────────────

    fn block(&self, py: Python<'_>, block: &TextBlock<'_>) -> PyResult<Py<PyTextBlock>> {
        Py::new(
            py,
            PyTextBlock {
                nr: self.child_ref(block.syntax()),
            },
        )
    }

    fn block_opt(&self, py: Python<'_>, block: Option<TextBlock<'_>>) -> PyResult<Option<Py<PyTextBlock>>> {
        block.map(|b| self.block(py, &b)).transpose()
    }

    fn blocks<'a>(
        &self,
        py: Python<'_>,
        blocks: impl Iterator<Item = TextBlock<'a>>,
    ) -> PyResult<Vec<Py<PyTextBlock>>> {
        blocks.map(|b| self.block(py, &b)).collect()
    }

}

// ─── Token ──────────────────────────────────────────────────────────────────

/// A typed token: a text fragment plus its byte range in the source.
///
/// Lazy view of a tree leaf — holds the parent node's [`NodeRef`] plus the
/// token's child index, and resolves `kind` / `text` / `range` through the core
/// tree on access.
#[pyclass(frozen, skip_from_py_object, module = "pydocstring", name = "Token")]
struct PyToken {
    /// Address of the token's parent node.
    parent: NodeRef,
    /// Child index of the token within the parent node.
    index: u32,
}

impl PyToken {
    fn resolve(&self) -> &SyntaxToken {
        match &self.parent.node().children()[self.index as usize] {
            SyntaxElement::Token(t) => t,
            SyntaxElement::Node(_) => unreachable!("PyToken index addresses a node"),
        }
    }
}

#[pymethods]
impl PyToken {
    #[getter]
    fn kind(&self) -> PySyntaxKind {
        py_syntax_kind_of(self.resolve().kind())
    }
    #[getter]
    fn text(&self) -> &str {
        self.resolve().text(self.parent.parsed.source())
    }
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.resolve().range()))
    }
    /// Whether this token is a zero-length placeholder inserted by the parser
    /// to represent a syntactically missing element.
    ///
    /// For example, ``arg (int)`` without a closing ``)`` produces a missing
    /// CLOSE_BRACKET token; ``arg ():`` produces a missing TYPE token.
    /// Equivalent to ``token.range.is_empty()``.
    fn is_missing(&self) -> bool {
        self.resolve().is_missing()
    }
    /// Two `Token`s are equal when they are the same kind over the same range
    /// of the same source.
    ///
    /// `kind` is load-bearing, not decoration: an entry like `x (:` produces a
    /// missing TYPE *and* a missing CLOSE_BRACKET, both zero-length at the same
    /// offset. On text and range alone they compare equal and collapse into one
    /// element in a set.
    fn __eq__(&self, other: &PyToken) -> bool {
        self.resolve().kind() == other.resolve().kind()
            && self.resolve().range() == other.resolve().range()
            && self.text() == other.text()
    }
    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let token = self.resolve();
        token.kind().hash(&mut hasher);
        token.range().hash(&mut hasher);
        token.text(self.parent.parsed.source()).hash(&mut hasher);
        hasher.finish()
    }
    fn __repr__(&self) -> String {
        format!("Token({:?})", self.text())
    }
}

// ─── TextBlock ──────────────────────────────────────────────────────────────

/// A multi-line text content block (summary, extended summary, description,
/// stray paragraph, free-text section body, or reference content).
///
/// Wraps one `Token` per content line; `text` is the raw source slice of the
/// block's range (byte-identical to the pre-#38 token text), `logical_text`
/// is the dedented/joined convenience form.
#[pyclass(frozen, skip_from_py_object, module = "pydocstring", name = "TextBlock")]
struct PyTextBlock {
    nr: NodeRef,
}

impl PyTextBlock {
    /// Resolve the lazy address into the core typed view.
    fn view(&self) -> TextBlock<'_> {
        TextBlock::cast(&self.nr.parsed, self.nr.node()).expect("NodeRef addresses a text block node")
    }
}

#[pymethods]
impl PyTextBlock {
    /// Raw source slice of the block's range, including interior
    /// indentation and newlines.
    #[getter]
    fn text(&self) -> &str {
        self.view().text()
    }
    /// Logical text: continuation lines dedented by their common
    /// indentation and joined with newlines.
    #[getter]
    fn logical_text(&self) -> String {
        self.view().logical_text()
    }
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    /// One `Token` per content line.
    #[getter]
    fn lines(&self, py: Python<'_>) -> PyResult<Vec<Py<PyToken>>> {
        self.nr.tokens(py, self.view().lines())
    }
    /// Whether this block is a zero-length placeholder inserted by the
    /// parser to represent a syntactically missing element (e.g. the
    /// description in ``arg (int):``). Equivalent to ``range.is_empty()``.
    fn is_missing(&self) -> bool {
        self.view().is_missing()
    }
    fn __repr__(&self) -> String {
        format!("TextBlock({:?})", self.text())
    }
}

// ─── Style ──────────────────────────────────────────────────────────────────

#[pyclass(eq, frozen, skip_from_py_object, hash, module = "pydocstring", name = "Style")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum PyStyle {
    #[pyo3(name = "GOOGLE")]
    Google,
    #[pyo3(name = "NUMPY")]
    NumPy,
    #[pyo3(name = "PLAIN")]
    Plain,
}

#[pymethods]
impl PyStyle {
    fn __repr__(&self) -> &'static str {
        match self {
            PyStyle::Google => "Style.GOOGLE",
            PyStyle::NumPy => "Style.NUMPY",
            PyStyle::Plain => "Style.PLAIN",
        }
    }
    fn __str__(&self) -> &'static str {
        match self {
            PyStyle::Google => "google",
            PyStyle::NumPy => "numpy",
            PyStyle::Plain => "plain",
        }
    }
}

// =============================================================================
// Parsed — the parse result (#119)
// =============================================================================

/// A parsed docstring, whatever its style.
///
/// One type for every style: `parse()`, `parse_google()`, `parse_numpy()` and
/// `parse_plain()` all return this, so nothing downstream branches on which
/// one you called — the tree's vocabulary is style-independent. Read it through
/// `Document(parsed)` (the semantic lens), `parsed.syntax` (the faithful CST),
/// or `parsed.to_model()` (the normalized IR); edit it through `parsed.edit()`.
#[pyclass(frozen, skip_from_py_object, module = "pydocstring", name = "Parsed")]
struct PyParsed {
    nr: NodeRef,
}

impl From<NodeRef> for PyParsed {
    fn from(nr: NodeRef) -> Self {
        Self { nr }
    }
}

/// Map a core style onto the Python enum.
fn py_style_of(style: CoreStyle) -> PyStyle {
    match style {
        CoreStyle::Google => PyStyle::Google,
        CoreStyle::NumPy => PyStyle::NumPy,
        // `Style` is #[non_exhaustive]; surface future styles as PLAIN until the
        // Python enum grows a matching member.
        _ => PyStyle::Plain,
    }
}

#[pymethods]
impl PyParsed {
    /// The style this docstring was parsed as.
    #[getter]
    fn style(&self) -> PyStyle {
        py_style_of(self.nr.parsed.style())
    }
    #[getter]
    fn source(&self) -> &str {
        self.nr.parsed.source()
    }
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    /// The root CST node — the faithful lens.
    #[getter]
    fn syntax(&self, py: Python<'_>) -> PyResult<Py<PyNode>> {
        self.nr.py_node(py)
    }
    /// A debug rendering of the syntax tree.
    fn pretty_print(&self) -> String {
        self.nr.parsed.pretty_print()
    }
    /// Convert to the normalized, position-free model IR.
    fn to_model(&self) -> PyResult<PyModelDocstring> {
        PyModelDocstring::try_from(&self.nr.parsed.to_model())
    }
    /// Start an empty edit list anchored on this parse result.
    ///
    /// Anchor edits on the ``range`` of any view; everything an edit does not
    /// touch is preserved byte-for-byte.
    fn edit(&self) -> PyEdits {
        PyEdits::new(Arc::clone(&self.nr.parsed))
    }
    /// Replace every match of ``pattern`` (a pattern of this docstring's style,
    /// with ``$NAME`` / ``$$$NAME`` metavariables) with ``template``, returning
    /// the new source. Captured content is substituted byte-for-byte; everything
    /// else is preserved. Raises ``PatternError`` for an invalid pattern.
    fn replace(&self, pattern: &str, template: &str) -> PyResult<String> {
        rewrite_replace(&self.nr, self.nr.parsed.style(), pattern, template)
    }
    /// Find every match of ``pattern`` in document order (non-overlapping).
    fn findall(&self, py: Python<'_>, pattern: &str) -> PyResult<Vec<Py<PyMatch>>> {
        rewrite_findall(py, &self.nr, self.nr.parsed.style(), pattern)
    }
    /// Like ``replace``, but scoped to ``anchor``'s subtree.
    ///
    /// ``anchor`` is a ``Document``, ``Section``, or ``Entry`` view of *this*
    /// parse result — a plain docstring has no sections, so only a ``Document``
    /// anchor applies there. Raises ``TypeError`` for anything else, and
    /// ``ValueError`` for a view of a different parse result.
    ///
    /// The anchor also selects the *reading*: an entry line is a ``$NAME`` under
    /// a parameters section and a ``$TYPE`` under a raises section, so the same
    /// pattern reads differently depending on where it is scoped.
    fn replace_in(&self, anchor: &Bound<'_, PyAny>, pattern: &str, template: &str) -> PyResult<String> {
        rewrite_replace_in(&self.nr, self.nr.parsed.style(), anchor, pattern, template)
    }
    /// Like ``findall``, but scoped to ``anchor``'s subtree.
    ///
    /// Same anchor rules as ``replace_in``: a ``Document``, ``Section``, or
    /// ``Entry`` view of *this* parse result. Raises ``TypeError`` for anything
    /// else, and ``ValueError`` for a view of a different parse result.
    fn findall_in(&self, py: Python<'_>, anchor: &Bound<'_, PyAny>, pattern: &str) -> PyResult<Vec<Py<PyMatch>>> {
        rewrite_findall_in(py, &self.nr, self.nr.parsed.style(), anchor, pattern)
    }
    fn __repr__(&self) -> String {
        format!("Parsed(style={:?})", self.nr.parsed.style())
    }
}

fn build_parsed(py: Python<'_>, parsed: Parsed) -> PyResult<Py<PyParsed>> {
    Py::new(py, PyParsed::from(NodeRef::root(Arc::new(parsed))))
}

// =============================================================================
// Raw CST — the fidelity lens (#126)
// =============================================================================
//
// The tree's vocabulary is already style-independent, so one `Node` type walks
// any docstring. This is the lens that keeps *everything*: punctuation, trivia,
// and the zero-length missing placeholders the unified view deliberately hides
// (`find_missing(SyntaxKind.TYPE)` is what distinguishes `x ():` from `x:`).

/// The kind of a CST node or token.
#[pyclass(from_py_object, eq, frozen, hash, module = "pydocstring", name = "SyntaxKind")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum PySyntaxKind {
    #[pyo3(name = "NAME")]
    Name,
    #[pyo3(name = "TYPE")]
    Type,
    #[pyo3(name = "COLON")]
    Colon,
    #[pyo3(name = "COMMA")]
    Comma,
    #[pyo3(name = "DESCRIPTION")]
    Description,
    #[pyo3(name = "OPEN_BRACKET")]
    OpenBracket,
    #[pyo3(name = "CLOSE_BRACKET")]
    CloseBracket,
    #[pyo3(name = "OPTIONAL")]
    Optional,
    #[pyo3(name = "SUMMARY")]
    Summary,
    #[pyo3(name = "EXTENDED_SUMMARY")]
    ExtendedSummary,
    #[pyo3(name = "TEXT_LINE")]
    TextLine,
    #[pyo3(name = "WHITESPACE")]
    Whitespace,
    #[pyo3(name = "NEWLINE")]
    Newline,
    #[pyo3(name = "BLANK_LINE")]
    BlankLine,
    #[pyo3(name = "UNDERLINE")]
    Underline,
    #[pyo3(name = "DIRECTIVE_MARKER")]
    DirectiveMarker,
    #[pyo3(name = "DIRECTIVE_NAME")]
    DirectiveName,
    #[pyo3(name = "DOUBLE_COLON")]
    DoubleColon,
    #[pyo3(name = "ARGUMENT")]
    Argument,
    #[pyo3(name = "DEFAULT_KEYWORD")]
    DefaultKeyword,
    #[pyo3(name = "DEFAULT_SEPARATOR")]
    DefaultSeparator,
    #[pyo3(name = "DEFAULT_VALUE")]
    DefaultValue,
    #[pyo3(name = "LABEL")]
    Label,
    #[pyo3(name = "DOCUMENT")]
    Document,
    #[pyo3(name = "SECTION")]
    Section,
    #[pyo3(name = "SECTION_HEADER")]
    SectionHeader,
    #[pyo3(name = "ENTRY")]
    Entry,
    #[pyo3(name = "DIRECTIVE")]
    Directive,
    #[pyo3(name = "CITATION")]
    Citation,
    #[pyo3(name = "DEFAULT")]
    Default,
    #[pyo3(name = "PARAGRAPH")]
    Paragraph,
    /// A kind this build of the Python bindings does not know about.
    ///
    /// `SyntaxKind` is `#[non_exhaustive]` in the crate, so a newer core can
    /// produce kinds this enum has no member for. Reading one yields `UNKNOWN`;
    /// it cannot be used as a query argument.
    #[pyo3(name = "UNKNOWN")]
    Unknown,
}

macro_rules! syntax_kind_map {
    ($($py:ident <=> $core:ident),+ $(,)?) => {
        /// Core kind → Python kind. Total: an unrecognised kind reads as `UNKNOWN`.
        fn py_syntax_kind_of(kind: SyntaxKind) -> PySyntaxKind {
            match kind {
                $(SyntaxKind::$core => PySyntaxKind::$py,)+
                // `SyntaxKind` is #[non_exhaustive].
                _ => PySyntaxKind::Unknown,
            }
        }

        /// Python kind → core kind, for query arguments.
        fn core_syntax_kind_of(kind: PySyntaxKind) -> PyResult<SyntaxKind> {
            Ok(match kind {
                $(PySyntaxKind::$py => SyntaxKind::$core,)+
                PySyntaxKind::Unknown => {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "SyntaxKind.UNKNOWN is a read-only result and cannot be used as a query",
                    ));
                }
            })
        }
    };
}

syntax_kind_map! {
    Name <=> NAME,
    Type <=> TYPE,
    Colon <=> COLON,
    Comma <=> COMMA,
    Description <=> DESCRIPTION,
    OpenBracket <=> OPEN_BRACKET,
    CloseBracket <=> CLOSE_BRACKET,
    Optional <=> OPTIONAL,
    Summary <=> SUMMARY,
    ExtendedSummary <=> EXTENDED_SUMMARY,
    TextLine <=> TEXT_LINE,
    Whitespace <=> WHITESPACE,
    Newline <=> NEWLINE,
    BlankLine <=> BLANK_LINE,
    Underline <=> UNDERLINE,
    DirectiveMarker <=> DIRECTIVE_MARKER,
    DirectiveName <=> DIRECTIVE_NAME,
    DoubleColon <=> DOUBLE_COLON,
    Argument <=> ARGUMENT,
    DefaultKeyword <=> DEFAULT_KEYWORD,
    DefaultSeparator <=> DEFAULT_SEPARATOR,
    DefaultValue <=> DEFAULT_VALUE,
    Label <=> LABEL,
    Document <=> DOCUMENT,
    Section <=> SECTION,
    SectionHeader <=> SECTION_HEADER,
    Entry <=> ENTRY,
    Directive <=> DIRECTIVE,
    Citation <=> CITATION,
    Default <=> DEFAULT,
    Paragraph <=> PARAGRAPH,
}

/// A node of the concrete syntax tree.
///
/// The faithful lens: it keeps every byte, including punctuation, trivia, and
/// zero-length missing placeholders. Reach it from any unified view with
/// `.syntax`, or from a parse result with `.syntax`.
#[pyclass(frozen, skip_from_py_object, module = "pydocstring", name = "Node")]
struct PyNode {
    nr: NodeRef,
}

impl From<NodeRef> for PyNode {
    fn from(nr: NodeRef) -> Self {
        Self { nr }
    }
}

#[pymethods]
impl PyNode {
    #[getter]
    fn kind(&self) -> PySyntaxKind {
        py_syntax_kind_of(self.nr.node().kind())
    }
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    /// The raw source slice of this node's range.
    #[getter]
    fn text(&self) -> &str {
        self.nr.node().range().source_text(self.nr.parsed.source())
    }
    /// Two `Node`s are equal when they are the same kind over the same range
    /// of the same source — accessors hand out a fresh wrapper each time, so
    /// identity would make even `n == n` false. Mirrors `Token`.
    fn __eq__(&self, other: &PyNode) -> bool {
        self.nr.node().kind() == other.nr.node().kind()
            && self.nr.node().range() == other.nr.node().range()
            && self.text() == other.text()
    }
    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let node = self.nr.node();
        node.kind().hash(&mut hasher);
        node.range().hash(&mut hasher);
        self.text().hash(&mut hasher);
        hasher.finish()
    }
    /// Every child, in source order: a mix of `Node` and `Token`.
    #[getter]
    fn children(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.nr
            .node()
            .children()
            .iter()
            .enumerate()
            .map(|(index, child)| match child {
                SyntaxElement::Node(n) => Ok(self.nr.wrap_child::<PyNode>(py, n)?.into_any()),
                SyntaxElement::Token(_) => Py::new(
                    py,
                    PyToken {
                        parent: self.nr.clone(),
                        index: index as u32,
                    },
                )
                .map(|t| t.into_any()),
            })
            .collect()
    }
    /// Direct child nodes of `kind`, in source order.
    fn nodes(&self, py: Python<'_>, kind: PySyntaxKind) -> PyResult<Vec<Py<PyNode>>> {
        let kind = core_syntax_kind_of(kind)?;
        self.nr
            .node()
            .nodes(kind)
            .map(|n| self.nr.wrap_child::<PyNode>(py, n))
            .collect()
    }
    /// Direct child tokens of `kind`, in source order. Missing placeholders are
    /// excluded — use `find_missing()` for those.
    fn tokens(&self, py: Python<'_>, kind: PySyntaxKind) -> PyResult<Vec<Py<PyToken>>> {
        let kind = core_syntax_kind_of(kind)?;
        self.nr
            .node()
            .tokens(kind)
            .map(|t| self.nr.token(py, t))
            .collect()
    }
    /// The first direct child node of `kind`.
    fn find_node(&self, py: Python<'_>, kind: PySyntaxKind) -> PyResult<Option<Py<PyNode>>> {
        let kind = core_syntax_kind_of(kind)?;
        self.nr
            .node()
            .find_node(kind)
            .map(|n| self.nr.wrap_child::<PyNode>(py, n))
            .transpose()
    }
    /// The first present (non-missing) direct child token of `kind`.
    fn find_token(&self, py: Python<'_>, kind: PySyntaxKind) -> PyResult<Option<Py<PyToken>>> {
        let kind = core_syntax_kind_of(kind)?;
        self.nr
            .node()
            .find_token(kind)
            .map(|t| self.nr.token(py, t))
            .transpose()
    }
    /// The first *missing* (zero-length) direct child token of `kind`.
    ///
    /// This is what tells `x ():` — an empty type between brackets, so a
    /// placeholder exists — apart from `x:`, where the grammar produced no type
    /// token at all. The placeholder's range is the insertion anchor.
    fn find_missing(&self, py: Python<'_>, kind: PySyntaxKind) -> PyResult<Option<Py<PyToken>>> {
        let kind = core_syntax_kind_of(kind)?;
        self.nr
            .node()
            .find_missing(kind)
            .map(|t| self.nr.token(py, t))
            .transpose()
    }
    fn __repr__(&self) -> String {
        let node = self.nr.node();
        format!("Node({:?}, {}..{})", node.kind(), node.range().start(), node.range().end())
    }
}

// =============================================================================
// Unified (style-independent) views — #116
// =============================================================================
//
// One code path over any docstring style. These wrap `parse::unified`, which
// names things in the style-independent vocabulary: a section's role is `kind`
// (data), not a nominal type. They hold no conversion knowledge — each getter
// delegates straight to the core accessor.
//
// Missing (zero-length) placeholder tokens are *not* surfaced: these views
// mirror the core exactly, where `find_token` excludes them, so `None` means
// "not present" rather than "present but empty". `parsed.syntax` — the raw CST
// — is the lens that keeps them.

/// Extract the shared `Arc<Parsed>` from a `Parsed` or any unified view.
fn parsed_of(obj: &Bound<'_, PyAny>) -> PyResult<Arc<Parsed>> {
    match obj.cast::<PyParsed>() {
        Ok(p) => Ok(Arc::clone(&p.get().nr.parsed)),
        Err(_) => Err(pyo3::exceptions::PyTypeError::new_err(
            "Document() expects a Parsed (the result of parse() / parse_google() / \
             parse_numpy() / parse_plain())",
        )),
    }
}

// ─── Document ────────────────────────────────────────────────────────────────

/// Style-independent view of a parsed docstring.
///
/// Construct from any parse result, whatever the style::
///
///     doc = pydocstring.Document(pydocstring.parse(src))
///     for section in doc.sections:
///         if section.kind == pydocstring.SectionKind.PARAMETERS:
///             for entry in section.entries:
///                 print(entry.name.text)
///
/// The same loop works for Google and NumPy sources: `"Args:"` and
/// `"Parameters"` both resolve to `SectionKind.PARAMETERS`.
#[pyclass(frozen, skip_from_py_object, module = "pydocstring", name = "Document")]
struct PyDocument {
    nr: NodeRef,
}

impl PyDocument {
    fn view(&self) -> uv::Document<'_> {
        uv::Document::new(&self.nr.parsed)
    }
}

#[pymethods]
impl PyDocument {
    #[new]
    fn new(parsed: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            nr: NodeRef::root(parsed_of(parsed)?),
        })
    }
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    /// The style the docstring was parsed as.
    #[getter]
    fn style(&self) -> PyStyle {
        match self.view().style() {
            CoreStyle::Google => PyStyle::Google,
            CoreStyle::NumPy => PyStyle::NumPy,
            // `Style` is #[non_exhaustive]; surface future styles as PLAIN
            // until the Python enum grows a matching member.
            _ => PyStyle::Plain,
        }
    }
    #[getter]
    fn source(&self) -> &str {
        self.nr.parsed.source()
    }
    #[getter]
    fn summary(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().summary())
    }
    #[getter]
    fn extended_summary(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().extended_summary())
    }
    #[getter]
    fn sections(&self, py: Python<'_>) -> PyResult<Vec<Py<PySection>>> {
        self.view()
            .sections()
            .map(|s| self.nr.wrap_child(py, s.syntax()))
            .collect()
    }
    #[getter]
    fn directives(&self, py: Python<'_>) -> PyResult<Vec<Py<PyDirective>>> {
        self.view()
            .directives()
            .map(|d| self.nr.wrap_child(py, d.syntax()))
            .collect()
    }
    /// Stray-prose paragraph blocks between sections, in source order.
    #[getter]
    fn paragraphs(&self, py: Python<'_>) -> PyResult<Vec<Py<PyTextBlock>>> {
        self.nr.blocks(py, self.view().paragraphs())
    }
    /// Start an empty edit list anchored on this docstring.
    fn edit(&self) -> PyEdits {
        PyEdits::new(Arc::clone(&self.nr.parsed))
    }
    /// The underlying CST node — the escape hatch down to the faithful lens.
    #[getter]
    fn syntax(&self, py: Python<'_>) -> PyResult<Py<PyNode>> {
        self.nr.py_node(py)
    }
    fn __repr__(&self) -> String {
        format!("Document(style={:?})", self.view().style())
    }
}

// ─── Section ─────────────────────────────────────────────────────────────────

/// Style-independent view of one section.
///
/// `kind` is the section's role as *data* — `"Args:"` (Google) and
/// `"Parameters"` (NumPy) both resolve to `SectionKind.PARAMETERS` — so callers
/// never branch on style.
#[pyclass(frozen, skip_from_py_object, module = "pydocstring", name = "Section")]
struct PySection {
    nr: NodeRef,
}

impl From<NodeRef> for PySection {
    fn from(nr: NodeRef) -> Self {
        Self { nr }
    }
}

impl PySection {
    fn view(&self) -> uv::Section<'_> {
        uv::Section::cast(&self.nr.parsed, self.nr.node()).expect("NodeRef addresses a SECTION node")
    }
}

#[pymethods]
impl PySection {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    /// The header text as written (e.g. `"Args"`, `"Parameters"`).
    #[getter]
    fn header_name(&self) -> &str {
        self.view().header_name()
    }
    /// The style-independent role of this section.
    #[getter]
    fn kind(&self) -> PySectionKind {
        py_section_kind_of(&self.view().kind()).0
    }
    /// The header text of an unrecognised section (`kind == UNKNOWN`), else
    /// `None`.
    #[getter]
    fn unknown_name(&self) -> Option<String> {
        py_section_kind_of(&self.view().kind()).1
    }
    #[getter]
    fn entries(&self, py: Python<'_>) -> PyResult<Vec<Py<PyEntry>>> {
        self.view()
            .entries()
            .map(|e| self.nr.wrap_child(py, e.syntax()))
            .collect()
    }
    /// Free-text body block, for sections that carry prose rather than entries.
    #[getter]
    fn body(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().body())
    }
    #[getter]
    fn citations(&self, py: Python<'_>) -> PyResult<Vec<Py<PyCitation>>> {
        self.view()
            .citations()
            .map(|c| self.nr.wrap_child(py, c.syntax()))
            .collect()
    }
    /// The underlying CST node — the escape hatch down to the faithful lens.
    #[getter]
    fn syntax(&self, py: Python<'_>) -> PyResult<Py<PyNode>> {
        self.nr.py_node(py)
    }
    fn __repr__(&self) -> String {
        format!("Section({:?})", self.view().header_name())
    }
}

// ─── Entry ───────────────────────────────────────────────────────────────────

/// Style-independent view of one entry: a parameter, return, yield, exception,
/// warning, attribute, method, or "See Also" item.
///
/// All roles share one type — the role is the parent section's `kind`. Every
/// accessor is optional, so reading an entry never raises for a role that does
/// not carry that piece: a `Raises:` entry has `name is None` and its exception
/// type in `type_annotation`.
#[pyclass(frozen, skip_from_py_object, module = "pydocstring", name = "Entry")]
struct PyEntry {
    nr: NodeRef,
}

impl From<NodeRef> for PyEntry {
    fn from(nr: NodeRef) -> Self {
        Self { nr }
    }
}

impl PyEntry {
    fn view(&self) -> uv::Entry<'_> {
        uv::Entry::cast(&self.nr.parsed, self.nr.node()).expect("NodeRef addresses an ENTRY node")
    }
}

#[pymethods]
impl PyEntry {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    /// The first name, if any. Exception / warning entries carry a `type`
    /// instead of a name.
    #[getter]
    fn name(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().name())
    }
    /// All names — an entry can declare several comma-separated ones.
    #[getter]
    fn names(&self, py: Python<'_>) -> PyResult<Vec<Py<PyToken>>> {
        self.nr.tokens(py, self.view().names())
    }
    /// The type annotation: a parameter / attribute type, a return / yield
    /// type, or the exception type of a `Raises` entry.
    #[getter]
    fn type_annotation(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().type_annotation())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().description())
    }
    /// Whether the entry carries at least one `optional` marker.
    #[getter]
    fn is_optional(&self) -> bool {
        self.view().is_optional()
    }
    /// Every `optional` marker, one per occurrence, in source order.
    #[getter]
    fn optionals(&self, py: Python<'_>) -> PyResult<Vec<Py<PyToken>>> {
        self.nr.tokens(py, self.view().optionals())
    }
    /// Every `default …` marker, one per occurrence, in source order.
    #[getter]
    fn defaults(&self, py: Python<'_>) -> PyResult<Vec<Py<PyDefaultMarker>>> {
        self.view()
            .defaults()
            .map(|d| self.nr.wrap_child(py, d.syntax()))
            .collect()
    }
    /// The first `default …` marker's value — the same first-occurrence-wins
    /// rule the model layer applies. Use `defaults` to see every occurrence.
    #[getter]
    fn default_value(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().default_value())
    }
    /// The underlying CST node — the escape hatch down to the faithful lens.
    #[getter]
    fn syntax(&self, py: Python<'_>) -> PyResult<Py<PyNode>> {
        self.nr.py_node(py)
    }
    fn __repr__(&self) -> String {
        let view = self.view();
        let names = view.names().map(|n| n.text()).collect::<Vec<_>>().join(", ");
        format!("Entry({names:?})")
    }
}

// ─── DefaultMarker ───────────────────────────────────────────────────────────

/// One `default …` marker inside a type annotation (`default X`, `default=X`,
/// `default: X`).
#[pyclass(frozen, skip_from_py_object, module = "pydocstring", name = "DefaultMarker")]
struct PyDefaultMarker {
    nr: NodeRef,
}

impl From<NodeRef> for PyDefaultMarker {
    fn from(nr: NodeRef) -> Self {
        Self { nr }
    }
}

impl PyDefaultMarker {
    fn view(&self) -> uv::DefaultMarker<'_> {
        uv::DefaultMarker::cast(&self.nr.parsed, self.nr.node()).expect("NodeRef addresses a DEFAULT node")
    }
}

#[pymethods]
impl PyDefaultMarker {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    /// The `default` keyword token.
    #[getter]
    fn keyword(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().keyword().syntax())
    }
    /// The `=` / `:` separator, if written.
    #[getter]
    fn separator(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().separator())
    }
    #[getter]
    fn value(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().value())
    }
    /// The underlying CST node — the escape hatch down to the faithful lens.
    #[getter]
    fn syntax(&self, py: Python<'_>) -> PyResult<Py<PyNode>> {
        self.nr.py_node(py)
    }
    fn __repr__(&self) -> String {
        format!(
            "DefaultMarker({:?})",
            self.view().value().map(|v| v.text()).unwrap_or_default()
        )
    }
}

// ─── Directive ───────────────────────────────────────────────────────────────

/// Style-independent view of a directive (e.g. `.. deprecated:: 1.6.0`).
#[pyclass(frozen, skip_from_py_object, module = "pydocstring", name = "Directive")]
struct PyDirective {
    nr: NodeRef,
}

impl From<NodeRef> for PyDirective {
    fn from(nr: NodeRef) -> Self {
        Self { nr }
    }
}

impl PyDirective {
    fn view(&self) -> uv::Directive<'_> {
        uv::Directive::cast(&self.nr.parsed, self.nr.node()).expect("NodeRef addresses a DIRECTIVE node")
    }
}

#[pymethods]
impl PyDirective {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    /// The directive name (e.g. `deprecated`).
    #[getter]
    fn name(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().name().syntax())
    }
    /// The directive argument (e.g. the version of a `.. deprecated::`).
    #[getter]
    fn argument(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().argument())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().description())
    }
    /// The underlying CST node — the escape hatch down to the faithful lens.
    #[getter]
    fn syntax(&self, py: Python<'_>) -> PyResult<Py<PyNode>> {
        self.nr.py_node(py)
    }
    fn __repr__(&self) -> String {
        format!("Directive({:?})", self.view().name().text())
    }
}

// ─── Citation ────────────────────────────────────────────────────────────────

/// Style-independent view of a citation in a References section
/// (`.. [label] content`, or a plain reference line).
#[pyclass(frozen, skip_from_py_object, module = "pydocstring", name = "Citation")]
struct PyCitation {
    nr: NodeRef,
}

impl From<NodeRef> for PyCitation {
    fn from(nr: NodeRef) -> Self {
        Self { nr }
    }
}

impl PyCitation {
    fn view(&self) -> uv::Citation<'_> {
        uv::Citation::cast(&self.nr.parsed, self.nr.node()).expect("NodeRef addresses a CITATION node")
    }
}

#[pymethods]
impl PyCitation {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    /// The citation label (`1`, `CIT2002`, `#f1`, …), if present.
    #[getter]
    fn label(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().label())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().description())
    }
    /// The underlying CST node — the escape hatch down to the faithful lens.
    #[getter]
    fn syntax(&self, py: Python<'_>) -> PyResult<Py<PyNode>> {
        self.nr.py_node(py)
    }
    fn __repr__(&self) -> String {
        format!(
            "Citation({:?})",
            self.view().label().map(|l| l.text()).unwrap_or_default()
        )
    }
}

// =============================================================================
// Edits — anchored splice edits (#117)
// =============================================================================

pyo3::create_exception!(
    _pydocstring,
    EditError,
    pyo3::exceptions::PyValueError,
    "Raised by Edits.apply() when the edit list is invalid — a range is out of \
     bounds, or two edits overlap (a ValueError subclass)."
);

/// One recorded operation. Kept as an operation rather than a resolved splice
/// so that `remove_lines`'s line-extent expansion stays in the core.
enum Splice {
    Replace(TextRange, String),
    RemoveLines(TextRange),
}

/// A list of pending edits anchored on one parse result.
///
/// Everything an edit does not touch is preserved byte-for-byte: an empty edit
/// list reproduces the source exactly, and replacing an element with its own
/// text is the identity. Anchor edits on the `range` of any view::
///
///     doc = pydocstring.Document(pydocstring.parse(src))
///     edits = doc.edit()
///     for section in doc.sections:
///         if section.kind == pydocstring.SectionKind.PARAMETERS:
///             for entry in section.entries:
///                 edits.replace(entry.description.range, "Better.")
///     result = edits.apply()
///
/// `Edits` borrows `&Parsed` in Rust, but what it accumulates is position plus
/// text — nothing that needs a borrow to *store*. So this holds the shared
/// `Arc<Parsed>` and the recorded operations, and rebuilds the borrowed core
/// builder inside `apply()`, replaying them into it. Validation therefore stays
/// exactly where it is: in the core `apply()`.
#[pyclass(module = "pydocstring", name = "Edits")]
struct PyEdits {
    parsed: Arc<Parsed>,
    splices: Vec<Splice>,
}

impl PyEdits {
    fn new(parsed: Arc<Parsed>) -> Self {
        Self {
            parsed,
            splices: Vec::new(),
        }
    }

    /// Rebuild the core builder and replay the recorded operations into it.
    fn build(&self) -> CoreEdits<'_> {
        let mut edits = self.parsed.edit();
        for splice in &self.splices {
            match splice {
                Splice::Replace(range, text) => edits.replace(*range, text.clone()),
                Splice::RemoveLines(range) => edits.remove_lines_range(*range),
            };
        }
        edits
    }
}

/// Read a Python `TextRange` back into the core type.
fn core_range(range: &Bound<'_, PyTextRange>) -> TextRange {
    let r = range.get();
    TextRange::new(TextSize::from(r.start), TextSize::from(r.end))
}

#[pymethods]
impl PyEdits {
    /// Replace the bytes of ``range`` with ``text``.
    ///
    /// A zero-length range inserts at that offset — which is how a missing
    /// placeholder token (``token.is_missing()``) works as an insertion anchor.
    /// Empty ``text`` deletes. Ranges are validated by ``apply()``, not here.
    fn replace(&mut self, range: &Bound<'_, PyTextRange>, text: String) {
        self.splices.push(Splice::Replace(core_range(range), text));
    }
    /// Insert ``text`` at byte offset ``at``.
    ///
    /// Multiple inserts at the same offset are applied in call order.
    fn insert(&mut self, at: u32, text: String) {
        let at = TextSize::from(at);
        self.splices.push(Splice::Replace(TextRange::new(at, at), text));
    }
    /// Delete the bytes of ``range``.
    ///
    /// To remove a construct together with its line layout, use
    /// ``remove_lines()``.
    fn delete(&mut self, range: &Bound<'_, PyTextRange>) {
        self.splices.push(Splice::Replace(core_range(range), String::new()));
    }
    /// Delete ``range`` together with the whole line(s) it occupies: its
    /// leading indentation, its trailing newline, and one adjacent trailing
    /// blank line if the tree has one there.
    fn remove_lines(&mut self, range: &Bound<'_, PyTextRange>) {
        self.splices.push(Splice::RemoveLines(core_range(range)));
    }
    /// Validate the edit list and splice it into a new source string.
    ///
    /// Non-consuming: the list can be applied again or added to afterwards.
    /// Raises ``EditError`` if a range is out of bounds or two edits overlap
    /// (touching ranges are fine).
    fn apply(&self) -> PyResult<String> {
        self.build().apply().map_err(|e| EditError::new_err(e.to_string()))
    }
    /// ``apply()`` the edits, then re-parse the result.
    ///
    /// The style is deliberately **not** re-detected: editing must not silently
    /// reinterpret the docstring as another style, even if the edited text would
    /// auto-detect differently. Returns the same wrapper type as the original.
    fn apply_reparsed(&self, py: Python<'_>) -> PyResult<Py<PyParsed>> {
        let parsed = self
            .build()
            .apply_reparsed()
            .map_err(|e| EditError::new_err(e.to_string()))?;
        build_parsed(py, parsed)
    }
    /// The number of pending edits.
    fn __len__(&self) -> usize {
        self.splices.len()
    }
    fn __repr__(&self) -> String {
        format!("Edits({} pending)", self.splices.len())
    }
}

// =============================================================================
// Model IR types
// =============================================================================

/// Ensure every item of `list` is an instance of `T`.
fn ensure_all_instance_of<T: pyo3::PyTypeCheck>(py: Python<'_>, list: &Py<PyList>, message: &str) -> PyResult<()> {
    if list.bind(py).into_iter().any(|item| !item.is_instance_of::<T>()) {
        return Err(pyo3::exceptions::PyTypeError::new_err(message.to_string()));
    }
    Ok(())
}

/// Ensure every item of `list` is a Python `str`.
fn ensure_str_list(py: Python<'_>, list: &Py<PyList>, message: &str) -> PyResult<()> {
    ensure_all_instance_of::<PyString>(py, list, message)
}

/// A document-level rST directive (`.. name:: argument` + indented body).
///
/// Mirrors the core `model::Directive`: a deprecation notice is a directive
/// with `name == "deprecated"` whose `argument` is the version.
#[pyclass(module = "pydocstring.model", name = "Directive")]
struct PyModelDirective {
    #[pyo3(get, set)]
    name: Py<PyString>,
    #[pyo3(get, set)]
    argument: Option<Py<PyString>>,
    #[pyo3(get, set)]
    description: Option<Py<PyString>>,
}

#[pymethods]
impl PyModelDirective {
    #[new]
    #[pyo3(signature = (name, *, argument=None, description=None))]
    fn new(name: Py<PyString>, argument: Option<Py<PyString>>, description: Option<Py<PyString>>) -> Self {
        Self {
            name,
            argument,
            description,
        }
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        format!("Directive({:?})", self.name.bind(py).to_string_lossy())
    }
}

impl TryFrom<&model::Directive> for PyModelDirective {
    type Error = PyErr;

    fn try_from(dir: &model::Directive) -> Result<Self, Self::Error> {
        Python::attach(|py| {
            Ok(Self {
                name: (&dir.name).into_pyobject(py)?.unbind(),
                argument: dir
                    .argument
                    .as_ref()
                    .map(|a| -> PyResult<_> { Ok(a.into_pyobject(py)?.unbind()) })
                    .transpose()?,
                description: dir
                    .description
                    .as_ref()
                    .map(|d| -> PyResult<_> { Ok(d.into_pyobject(py)?.unbind()) })
                    .transpose()?,
            })
        })
    }
}

impl TryInto<model::Directive> for &PyModelDirective {
    type Error = PyErr;

    fn try_into(self) -> Result<model::Directive, Self::Error> {
        Python::attach(|py| {
            Ok(model::Directive {
                name: self.name.extract(py)?,
                argument: self.argument.as_ref().map(|a| a.extract(py)).transpose()?,
                description: self.description.as_ref().map(|d| d.extract(py)).transpose()?,
            })
        })
    }
}

#[pyclass(module = "pydocstring.model", name = "Parameter")]
struct PyModelParameter {
    names: Py<PyList>,
    #[pyo3(get, set)]
    type_annotation: Option<Py<PyString>>,
    #[pyo3(get, set)]
    description: Option<Py<PyString>>,
    #[pyo3(get, set)]
    is_optional: bool,
    #[pyo3(get, set)]
    default_value: Option<Py<PyString>>,
}

#[pymethods]
impl PyModelParameter {
    #[new]
    #[pyo3(signature = (names, *, type_annotation=None, description=None, is_optional=false, default_value=None))]
    fn new(
        py: Python<'_>,
        names: Py<PyList>,
        type_annotation: Option<Py<PyString>>,
        description: Option<Py<PyString>>,
        is_optional: bool,
        default_value: Option<Py<PyString>>,
    ) -> PyResult<Self> {
        ensure_str_list(py, &names, "Parameter names must be strings.")?;
        Ok(Self {
            names,
            type_annotation,
            description,
            is_optional,
            default_value,
        })
    }
    #[getter]
    fn names<'py>(&self, py: Python<'py>) -> &Bound<'py, PyList> {
        self.names.bind(py)
    }
    #[setter]
    fn set_names(&mut self, py: Python<'_>, names: Py<PyList>) -> PyResult<()> {
        ensure_str_list(py, &names, "Parameter names must be strings.")?;
        self.names = names;
        Ok(())
    }
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!("Parameter(names={})", self.names.bind(py).repr()?))
    }
}

impl TryFrom<&model::Parameter> for PyModelParameter {
    type Error = PyErr;

    fn try_from(param: &model::Parameter) -> Result<Self, Self::Error> {
        Python::attach(|py| {
            Ok(Self {
                names: (&param.names).into_pyobject(py)?.cast_into::<PyList>()?.unbind(),
                type_annotation: param
                    .type_annotation
                    .as_ref()
                    .map(|a| -> PyResult<_> { Ok(a.into_pyobject(py)?.unbind()) })
                    .transpose()?,
                description: param
                    .description
                    .as_ref()
                    .map(|a| -> PyResult<_> { Ok(a.into_pyobject(py)?.unbind()) })
                    .transpose()?,
                is_optional: param.is_optional,
                default_value: param
                    .default_value
                    .as_ref()
                    .map(|a| -> PyResult<_> { Ok(a.into_pyobject(py)?.unbind()) })
                    .transpose()?,
            })
        })
    }
}

impl TryInto<model::Parameter> for &PyModelParameter {
    type Error = PyErr;

    fn try_into(self) -> Result<model::Parameter, Self::Error> {
        Python::attach(|py| {
            Ok(model::Parameter {
                names: self.names.extract(py)?,
                type_annotation: self
                    .type_annotation
                    .as_ref()
                    .map(|a| -> PyResult<_> { a.extract(py) })
                    .transpose()?,
                description: self
                    .description
                    .as_ref()
                    .map(|d| -> PyResult<_> { d.extract(py) })
                    .transpose()?,
                is_optional: self.is_optional,
                default_value: self
                    .default_value
                    .as_ref()
                    .map(|d| -> PyResult<_> { d.extract(py) })
                    .transpose()?,
            })
        })
    }
}

#[pyclass(module = "pydocstring.model", name = "Return")]
struct PyModelReturn {
    #[pyo3(get, set)]
    name: Option<Py<PyString>>,
    #[pyo3(get, set)]
    type_annotation: Option<Py<PyString>>,
    #[pyo3(get, set)]
    description: Option<Py<PyString>>,
}

#[pymethods]
impl PyModelReturn {
    #[new]
    #[pyo3(signature = (*, name=None, type_annotation=None, description=None))]
    fn new(
        name: Option<Py<PyString>>,
        type_annotation: Option<Py<PyString>>,
        description: Option<Py<PyString>>,
    ) -> Self {
        Self {
            name,
            type_annotation,
            description,
        }
    }
    fn __repr__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.name.as_ref().map_or_else(
            || Ok("Return(...)".into_pyobject(py)?.into_any()),
            |n| "Return({})".into_pyobject(py)?.call_method("format", (n,), None),
        )
    }
}

impl TryFrom<&model::Return> for PyModelReturn {
    type Error = PyErr;

    fn try_from(ret: &model::Return) -> Result<Self, Self::Error> {
        Python::attach(|py| {
            Ok(Self {
                name: ret
                    .name
                    .as_ref()
                    .map(|n| -> PyResult<_> { Ok(n.into_pyobject(py)?.unbind()) })
                    .transpose()?,
                type_annotation: ret
                    .type_annotation
                    .as_ref()
                    .map(|n| -> PyResult<_> { Ok(n.into_pyobject(py)?.unbind()) })
                    .transpose()?,
                description: ret
                    .description
                    .as_ref()
                    .map(|n| -> PyResult<_> { Ok(n.into_pyobject(py)?.unbind()) })
                    .transpose()?,
            })
        })
    }
}

impl TryInto<model::Return> for &PyModelReturn {
    type Error = PyErr;

    fn try_into(self) -> Result<model::Return, Self::Error> {
        Python::attach(|py| {
            Ok(model::Return {
                name: self
                    .name
                    .as_ref()
                    .map(|n| -> PyResult<_> { n.extract(py) })
                    .transpose()?,
                type_annotation: self
                    .type_annotation
                    .as_ref()
                    .map(|n| -> PyResult<_> { n.extract(py) })
                    .transpose()?,
                description: self
                    .description
                    .as_ref()
                    .map(|n| -> PyResult<_> { n.extract(py) })
                    .transpose()?,
            })
        })
    }
}

#[pyclass(module = "pydocstring.model", name = "ExceptionEntry")]
struct PyModelExceptionEntry {
    #[pyo3(get, set)]
    type_name: Py<PyString>,
    #[pyo3(get, set)]
    description: Option<Py<PyString>>,
}

#[pymethods]
impl PyModelExceptionEntry {
    #[new]
    #[pyo3(signature = (type_name, *, description=None))]
    fn new(type_name: Py<PyString>, description: Option<Py<PyString>>) -> Self {
        Self { type_name, description }
    }
    fn __repr__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        "ExceptionEntry({})"
            .into_pyobject(py)?
            .call_method("format", (&self.type_name,), None)
    }
}

impl TryFrom<&model::ExceptionEntry> for PyModelExceptionEntry {
    type Error = PyErr;

    fn try_from(exception: &model::ExceptionEntry) -> Result<Self, Self::Error> {
        Python::attach(|py| {
            Ok(Self {
                type_name: (&exception.type_name).into_pyobject(py)?.unbind(),
                description: exception
                    .description
                    .as_ref()
                    .map(|d| -> PyResult<_> { Ok(d.into_pyobject(py)?.unbind()) })
                    .transpose()?,
            })
        })
    }
}

impl TryInto<model::ExceptionEntry> for &PyModelExceptionEntry {
    type Error = PyErr;

    fn try_into(self) -> Result<model::ExceptionEntry, Self::Error> {
        Python::attach(|py| {
            Ok(model::ExceptionEntry {
                type_name: self.type_name.extract(py)?,
                description: self
                    .description
                    .as_ref()
                    .map(|d| -> PyResult<_> { d.extract(py) })
                    .transpose()?,
            })
        })
    }
}

#[pyclass(module = "pydocstring.model", name = "SeeAlsoEntry")]
struct PyModelSeeAlsoEntry {
    names: Py<PyList>,
    #[pyo3(get, set)]
    description: Option<Py<PyString>>,
}

#[pymethods]
impl PyModelSeeAlsoEntry {
    #[new]
    #[pyo3(signature = (names, *, description=None))]
    fn new(py: Python<'_>, names: Py<PyList>, description: Option<Py<PyString>>) -> PyResult<Self> {
        ensure_str_list(py, &names, "Names must be strings.")?;
        Ok(Self { names, description })
    }
    #[getter]
    fn names<'py>(&self, py: Python<'py>) -> &Bound<'py, PyList> {
        self.names.bind(py)
    }
    #[setter]
    fn set_names(&mut self, py: Python<'_>, names: Py<PyList>) -> PyResult<()> {
        ensure_str_list(py, &names, "Names must be strings.")?;
        self.names = names;
        Ok(())
    }

    fn __repr__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let names = ", "
            .into_pyobject(py)?
            .call_method("join", (self.names.bind(py),), None)?;
        "SeeAlsoEntry({})"
            .into_pyobject(py)?
            .call_method("format", (names,), None)
    }
}

impl TryFrom<&model::SeeAlsoEntry> for PyModelSeeAlsoEntry {
    type Error = PyErr;

    fn try_from(seealso: &model::SeeAlsoEntry) -> Result<Self, Self::Error> {
        Python::attach(|py| {
            Ok(Self {
                names: (&seealso.names).into_pyobject(py)?.cast_into::<PyList>()?.unbind(),
                description: seealso
                    .description
                    .as_ref()
                    .map(|d| -> PyResult<_> { Ok(d.into_pyobject(py)?.unbind()) })
                    .transpose()?,
            })
        })
    }
}

impl TryInto<model::SeeAlsoEntry> for &PyModelSeeAlsoEntry {
    type Error = PyErr;

    fn try_into(self) -> Result<model::SeeAlsoEntry, Self::Error> {
        Python::attach(|py| {
            Ok(model::SeeAlsoEntry {
                names: self.names.extract(py)?,
                description: self
                    .description
                    .as_ref()
                    .map(|d| -> PyResult<_> { d.extract(py) })
                    .transpose()?,
            })
        })
    }
}

#[pyclass(module = "pydocstring.model", name = "Reference")]
struct PyModelReference {
    #[pyo3(get, set)]
    label: Option<Py<PyString>>,
    #[pyo3(get, set)]
    content: Option<Py<PyString>>,
}

#[pymethods]
impl PyModelReference {
    #[new]
    #[pyo3(signature = (*, label=None, content=None))]
    fn new(label: Option<Py<PyString>>, content: Option<Py<PyString>>) -> Self {
        Self { label, content }
    }
    fn __repr__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.label.as_ref().map_or_else(
            || Ok("Reference(...)".into_pyobject(py)?.into_any()),
            |n| "Reference({})".into_pyobject(py)?.call_method("format", (n,), None),
        )
    }
}

impl TryFrom<&model::Reference> for PyModelReference {
    type Error = PyErr;

    fn try_from(reference: &model::Reference) -> Result<Self, Self::Error> {
        Python::attach(|py| {
            Ok(Self {
                label: reference
                    .label
                    .as_ref()
                    .map(|d| -> PyResult<_> { Ok(d.into_pyobject(py)?.unbind()) })
                    .transpose()?,
                content: reference
                    .content
                    .as_ref()
                    .map(|d| -> PyResult<_> { Ok(d.into_pyobject(py)?.unbind()) })
                    .transpose()?,
            })
        })
    }
}

impl TryInto<model::Reference> for &PyModelReference {
    type Error = PyErr;

    fn try_into(self) -> Result<model::Reference, Self::Error> {
        Python::attach(|py| {
            Ok(model::Reference {
                label: self
                    .label
                    .as_ref()
                    .map(|d| -> PyResult<_> { d.extract(py) })
                    .transpose()?,
                content: self
                    .content
                    .as_ref()
                    .map(|d| -> PyResult<_> { d.extract(py) })
                    .transpose()?,
            })
        })
    }
}

#[pyclass(module = "pydocstring.model", name = "Attribute")]
struct PyModelAttribute {
    names: Py<PyList>,
    #[pyo3(get, set)]
    type_annotation: Option<Py<PyString>>,
    #[pyo3(get, set)]
    description: Option<Py<PyString>>,
}

#[pymethods]
impl PyModelAttribute {
    #[new]
    #[pyo3(signature = (names, *, type_annotation=None, description=None))]
    fn new(
        py: Python<'_>,
        names: Py<PyList>,
        type_annotation: Option<Py<PyString>>,
        description: Option<Py<PyString>>,
    ) -> PyResult<Self> {
        ensure_str_list(py, &names, "Attribute names must be strings.")?;
        Ok(Self {
            names,
            type_annotation,
            description,
        })
    }
    #[getter]
    fn names<'py>(&self, py: Python<'py>) -> &Bound<'py, PyList> {
        self.names.bind(py)
    }
    #[setter]
    fn set_names(&mut self, py: Python<'_>, names: Py<PyList>) -> PyResult<()> {
        ensure_str_list(py, &names, "Attribute names must be strings.")?;
        self.names = names;
        Ok(())
    }
    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(format!("Attribute(names={})", self.names.bind(py).repr()?))
    }
}

impl TryFrom<&model::Attribute> for PyModelAttribute {
    type Error = PyErr;

    fn try_from(attribute: &model::Attribute) -> Result<Self, Self::Error> {
        Python::attach(|py| {
            Ok(Self {
                names: (&attribute.names).into_pyobject(py)?.cast_into::<PyList>()?.unbind(),
                type_annotation: attribute
                    .type_annotation
                    .as_ref()
                    .map(|d| -> PyResult<_> { Ok(d.into_pyobject(py)?.unbind()) })
                    .transpose()?,
                description: attribute
                    .description
                    .as_ref()
                    .map(|d| -> PyResult<_> { Ok(d.into_pyobject(py)?.unbind()) })
                    .transpose()?,
            })
        })
    }
}

impl TryInto<model::Attribute> for &PyModelAttribute {
    type Error = PyErr;

    fn try_into(self) -> Result<model::Attribute, Self::Error> {
        Python::attach(|py| {
            Ok(model::Attribute {
                names: self.names.extract(py)?,
                type_annotation: self
                    .type_annotation
                    .as_ref()
                    .map(|d| -> PyResult<_> { d.extract(py) })
                    .transpose()?,
                description: self
                    .description
                    .as_ref()
                    .map(|d| -> PyResult<_> { d.extract(py) })
                    .transpose()?,
            })
        })
    }
}

#[pyclass(module = "pydocstring.model", name = "Method")]
struct PyModelMethod {
    #[pyo3(get, set)]
    name: Py<PyString>,
    #[pyo3(get, set)]
    type_annotation: Option<Py<PyString>>,
    #[pyo3(get, set)]
    description: Option<Py<PyString>>,
}

#[pymethods]
impl PyModelMethod {
    #[new]
    #[pyo3(signature = (name, *, type_annotation=None, description=None))]
    fn new(name: Py<PyString>, type_annotation: Option<Py<PyString>>, description: Option<Py<PyString>>) -> Self {
        Self {
            name,
            type_annotation,
            description,
        }
    }
    fn __repr__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        "Method({})"
            .into_pyobject(py)?
            .call_method("format", (&self.name,), None)
    }
}

impl TryFrom<&model::Method> for PyModelMethod {
    type Error = PyErr;

    fn try_from(method: &model::Method) -> Result<Self, Self::Error> {
        Python::attach(|py| {
            Ok(Self {
                name: (&method.name).into_pyobject(py)?.unbind(),
                type_annotation: method
                    .type_annotation
                    .as_ref()
                    .map(|d| -> PyResult<_> { Ok(d.into_pyobject(py)?.unbind()) })
                    .transpose()?,
                description: method
                    .description
                    .as_ref()
                    .map(|d| -> PyResult<_> { Ok(d.into_pyobject(py)?.unbind()) })
                    .transpose()?,
            })
        })
    }
}

impl TryInto<model::Method> for &PyModelMethod {
    type Error = PyErr;

    fn try_into(self) -> Result<model::Method, Self::Error> {
        Python::attach(|py| {
            Ok(model::Method {
                name: self.name.extract(py)?,
                type_annotation: self
                    .type_annotation
                    .as_ref()
                    .map(|d| -> PyResult<_> { d.extract(py) })
                    .transpose()?,
                description: self
                    .description
                    .as_ref()
                    .map(|d| -> PyResult<_> { d.extract(py) })
                    .transpose()?,
            })
        })
    }
}

// ─── SectionKind ─────────────────────────────────────────────────────────────

#[pyclass(from_py_object, eq, frozen, hash, module = "pydocstring", name = "SectionKind")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum PySectionKind {
    #[pyo3(name = "PARAMETERS")]
    Parameters,
    #[pyo3(name = "KEYWORD_PARAMETERS")]
    KeywordParameters,
    #[pyo3(name = "OTHER_PARAMETERS")]
    OtherParameters,
    #[pyo3(name = "RECEIVES")]
    Receives,
    #[pyo3(name = "RETURNS")]
    Returns,
    #[pyo3(name = "YIELDS")]
    Yields,
    #[pyo3(name = "RAISES")]
    Raises,
    #[pyo3(name = "WARNS")]
    Warns,
    #[pyo3(name = "ATTRIBUTES")]
    Attributes,
    #[pyo3(name = "METHODS")]
    Methods,
    #[pyo3(name = "SEE_ALSO")]
    SeeAlso,
    #[pyo3(name = "REFERENCES")]
    References,
    #[pyo3(name = "NOTES")]
    Notes,
    #[pyo3(name = "EXAMPLES")]
    Examples,
    #[pyo3(name = "WARNINGS")]
    Warnings,
    #[pyo3(name = "TODO")]
    Todo,
    #[pyo3(name = "ATTENTION")]
    Attention,
    #[pyo3(name = "CAUTION")]
    Caution,
    #[pyo3(name = "DANGER")]
    Danger,
    #[pyo3(name = "ERROR")]
    Error,
    #[pyo3(name = "HINT")]
    Hint,
    #[pyo3(name = "IMPORTANT")]
    Important,
    #[pyo3(name = "TIP")]
    Tip,
    #[pyo3(name = "UNKNOWN")]
    Unknown,
}

fn py_section_kind_name(kind: PySectionKind) -> &'static str {
    match kind {
        PySectionKind::Parameters => "PARAMETERS",
        PySectionKind::KeywordParameters => "KEYWORD_PARAMETERS",
        PySectionKind::OtherParameters => "OTHER_PARAMETERS",
        PySectionKind::Receives => "RECEIVES",
        PySectionKind::Returns => "RETURNS",
        PySectionKind::Yields => "YIELDS",
        PySectionKind::Raises => "RAISES",
        PySectionKind::Warns => "WARNS",
        PySectionKind::Attributes => "ATTRIBUTES",
        PySectionKind::Methods => "METHODS",
        PySectionKind::SeeAlso => "SEE_ALSO",
        PySectionKind::References => "REFERENCES",
        PySectionKind::Notes => "NOTES",
        PySectionKind::Examples => "EXAMPLES",
        PySectionKind::Warnings => "WARNINGS",
        PySectionKind::Todo => "TODO",
        PySectionKind::Attention => "ATTENTION",
        PySectionKind::Caution => "CAUTION",
        PySectionKind::Danger => "DANGER",
        PySectionKind::Error => "ERROR",
        PySectionKind::Hint => "HINT",
        PySectionKind::Important => "IMPORTANT",
        PySectionKind::Tip => "TIP",
        PySectionKind::Unknown => "UNKNOWN",
    }
}

#[pymethods]
impl PySectionKind {
    fn __repr__(&self) -> String {
        format!("SectionKind.{}", py_section_kind_name(*self))
    }
}

// ─── Model SectionKind conversions ───────────────────────────────────────────

/// Map a core [`model::SectionKind`] to the Python `SectionKind` enum plus the
/// unknown section name, if the kind is an unrecognised free-text section.
fn py_section_kind_of(kind: &model::SectionKind) -> (PySectionKind, Option<String>) {
    use model::SectionKind as K;
    match kind {
        K::Parameters => (PySectionKind::Parameters, None),
        K::KeywordParameters => (PySectionKind::KeywordParameters, None),
        K::OtherParameters => (PySectionKind::OtherParameters, None),
        K::Receives => (PySectionKind::Receives, None),
        K::Returns => (PySectionKind::Returns, None),
        K::Yields => (PySectionKind::Yields, None),
        K::Raises => (PySectionKind::Raises, None),
        K::Warns => (PySectionKind::Warns, None),
        K::Attributes => (PySectionKind::Attributes, None),
        K::Methods => (PySectionKind::Methods, None),
        K::SeeAlso => (PySectionKind::SeeAlso, None),
        K::References => (PySectionKind::References, None),
        K::FreeText(free) => py_free_section_kind_of(free),
        // model::SectionKind is #[non_exhaustive]; surface future kinds as Unknown.
        _ => (PySectionKind::Unknown, None),
    }
}

fn py_free_section_kind_of(kind: &model::FreeSectionKind) -> (PySectionKind, Option<String>) {
    use model::FreeSectionKind as F;
    match kind {
        F::Notes => (PySectionKind::Notes, None),
        F::Examples => (PySectionKind::Examples, None),
        F::Warnings => (PySectionKind::Warnings, None),
        F::Todo => (PySectionKind::Todo, None),
        F::Attention => (PySectionKind::Attention, None),
        F::Caution => (PySectionKind::Caution, None),
        F::Danger => (PySectionKind::Danger, None),
        F::Error => (PySectionKind::Error, None),
        F::Hint => (PySectionKind::Hint, None),
        F::Important => (PySectionKind::Important, None),
        F::Tip => (PySectionKind::Tip, None),
        F::Unknown(name) => (PySectionKind::Unknown, Some(name.clone())),
        // model::FreeSectionKind is #[non_exhaustive].
        _ => (PySectionKind::Unknown, None),
    }
}

/// Map a Python `SectionKind` (plus optional unknown name) back to a core
/// [`model::SectionKind`].
fn model_section_kind_of(kind: PySectionKind, unknown_name: Option<String>) -> PyResult<model::SectionKind> {
    use model::FreeSectionKind as F;
    use model::SectionKind as K;
    Ok(match kind {
        PySectionKind::Parameters => K::Parameters,
        PySectionKind::KeywordParameters => K::KeywordParameters,
        PySectionKind::OtherParameters => K::OtherParameters,
        PySectionKind::Receives => K::Receives,
        PySectionKind::Returns => K::Returns,
        PySectionKind::Yields => K::Yields,
        PySectionKind::Raises => K::Raises,
        PySectionKind::Warns => K::Warns,
        PySectionKind::Attributes => K::Attributes,
        PySectionKind::Methods => K::Methods,
        PySectionKind::SeeAlso => K::SeeAlso,
        PySectionKind::References => K::References,
        PySectionKind::Notes => K::FreeText(F::Notes),
        PySectionKind::Examples => K::FreeText(F::Examples),
        PySectionKind::Warnings => K::FreeText(F::Warnings),
        PySectionKind::Todo => K::FreeText(F::Todo),
        PySectionKind::Attention => K::FreeText(F::Attention),
        PySectionKind::Caution => K::FreeText(F::Caution),
        PySectionKind::Danger => K::FreeText(F::Danger),
        PySectionKind::Error => K::FreeText(F::Error),
        PySectionKind::Hint => K::FreeText(F::Hint),
        PySectionKind::Important => K::FreeText(F::Important),
        PySectionKind::Tip => K::FreeText(F::Tip),
        PySectionKind::Unknown => K::FreeText(F::Unknown(unknown_name.ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err("Section(SectionKind.UNKNOWN) requires 'unknown_name'")
        })?)),
    })
}

// ─── Model Block ─────────────────────────────────────────────────────────────

/// A single body block within a [`Section`], mirroring the core
/// [`model::Block`]: a prose paragraph or a typed entry, in source order.
#[pyclass(module = "pydocstring.model", name = "Block")]
enum PyModelBlock {
    Paragraph { text: Py<PyString> },
    Parameter { value: Py<PyModelParameter> },
    Return { value: Py<PyModelReturn> },
    Exception { value: Py<PyModelExceptionEntry> },
    Attribute { value: Py<PyModelAttribute> },
    Method { value: Py<PyModelMethod> },
    SeeAlso { value: Py<PyModelSeeAlsoEntry> },
    Reference { value: Py<PyModelReference> },
}

impl PyModelBlock {
    fn from_model(py: Python<'_>, block: &model::Block) -> PyResult<Self> {
        Ok(match block {
            model::Block::Paragraph(text) => PyModelBlock::Paragraph {
                text: PyString::new(py, text).unbind(),
            },
            model::Block::Parameter(p) => PyModelBlock::Parameter {
                value: Py::new(py, PyModelParameter::try_from(p)?)?,
            },
            model::Block::Return(r) => PyModelBlock::Return {
                value: Py::new(py, PyModelReturn::try_from(r)?)?,
            },
            model::Block::Exception(e) => PyModelBlock::Exception {
                value: Py::new(py, PyModelExceptionEntry::try_from(e)?)?,
            },
            model::Block::Attribute(a) => PyModelBlock::Attribute {
                value: Py::new(py, PyModelAttribute::try_from(a)?)?,
            },
            model::Block::Method(m) => PyModelBlock::Method {
                value: Py::new(py, PyModelMethod::try_from(m)?)?,
            },
            model::Block::SeeAlso(s) => PyModelBlock::SeeAlso {
                value: Py::new(py, PyModelSeeAlsoEntry::try_from(s)?)?,
            },
            model::Block::Reference(r) => PyModelBlock::Reference {
                value: Py::new(py, PyModelReference::try_from(r)?)?,
            },
            // model::Block is #[non_exhaustive].
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "unsupported block kind (update the pydocstring-rs bindings)",
                ));
            }
        })
    }

    fn to_model(&self, py: Python<'_>) -> PyResult<model::Block> {
        Ok(match self {
            PyModelBlock::Paragraph { text } => model::Block::Paragraph(text.extract(py)?),
            PyModelBlock::Parameter { value } => model::Block::Parameter((&*value.borrow(py)).try_into()?),
            PyModelBlock::Return { value } => model::Block::Return((&*value.borrow(py)).try_into()?),
            PyModelBlock::Exception { value } => model::Block::Exception((&*value.borrow(py)).try_into()?),
            PyModelBlock::Attribute { value } => model::Block::Attribute((&*value.borrow(py)).try_into()?),
            PyModelBlock::Method { value } => model::Block::Method((&*value.borrow(py)).try_into()?),
            PyModelBlock::SeeAlso { value } => model::Block::SeeAlso((&*value.borrow(py)).try_into()?),
            PyModelBlock::Reference { value } => model::Block::Reference((&*value.borrow(py)).try_into()?),
        })
    }
}

// ─── Model Section ───────────────────────────────────────────────────────────

/// A docstring section: a [`SectionKind`] paired with a flat sequence of
/// [`Block`]s in source order, mirroring the core [`model::Section`].
#[pyclass(module = "pydocstring.model", name = "Section")]
struct PyModelSection {
    kind: PySectionKind,
    blocks: Py<PyList>,
    unknown_name: Option<Py<PyString>>,
}

#[pymethods]
impl PyModelSection {
    #[new]
    #[pyo3(signature = (kind, blocks=None, *, unknown_name=None))]
    fn new(
        py: Python<'_>,
        kind: PySectionKind,
        blocks: Option<Py<PyList>>,
        unknown_name: Option<Py<PyString>>,
    ) -> PyResult<Self> {
        if matches!(kind, PySectionKind::Unknown) {
            if unknown_name.is_none() {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "Section(SectionKind.UNKNOWN) requires 'unknown_name'",
                ));
            }
        } else if unknown_name.is_some() {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "'unknown_name' is only valid for Section(SectionKind.UNKNOWN)",
            ));
        }
        let blocks = blocks.unwrap_or_else(|| PyList::empty(py).unbind());
        ensure_all_instance_of::<PyModelBlock>(py, &blocks, "Section only accepts Blocks in the 'blocks' argument.")?;
        Ok(Self {
            kind,
            blocks,
            unknown_name,
        })
    }

    #[getter]
    fn kind(&self) -> PySectionKind {
        self.kind
    }

    #[getter]
    fn blocks<'py>(&self, py: Python<'py>) -> &Bound<'py, PyList> {
        self.blocks.bind(py)
    }

    #[getter]
    fn unknown_name<'py>(&self, py: Python<'py>) -> Option<&Bound<'py, PyString>> {
        self.unknown_name.as_ref().map(|n| n.bind(py))
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        // Include unknown_name: it is the only thing distinguishing one
        // UNKNOWN section from another in debug output.
        match &self.unknown_name {
            Some(name) => format!(
                "Section(SectionKind.{}, unknown_name={:?})",
                py_section_kind_name(self.kind),
                name.bind(py).to_string()
            ),
            None => format!("Section(SectionKind.{})", py_section_kind_name(self.kind)),
        }
    }
}

impl TryFrom<&model::Section> for PyModelSection {
    type Error = PyErr;

    fn try_from(section: &model::Section) -> Result<Self, Self::Error> {
        Python::attach(|py| -> Result<Self, Self::Error> {
            let (kind, unknown_name) = py_section_kind_of(&section.kind);
            // Build each block via `into_pyobject` (not `Py::new`) so the
            // instance is the variant *subclass* (`Block.Paragraph`, …) that
            // Python `isinstance` checks against, not the base `Block`.
            let mut items: Vec<Bound<'_, PyAny>> = Vec::with_capacity(section.blocks.len());
            for block in &section.blocks {
                items.push(PyModelBlock::from_model(py, block)?.into_pyobject(py)?.into_any());
            }
            let blocks = PyList::new(py, items)?.unbind();
            let unknown_name = unknown_name.map(|s| PyString::new(py, &s).unbind());
            Ok(Self {
                kind,
                blocks,
                unknown_name,
            })
        })
    }
}

impl TryInto<model::Section> for &PyModelSection {
    type Error = PyErr;

    fn try_into(self) -> Result<model::Section, Self::Error> {
        Python::attach(|py| -> Result<model::Section, Self::Error> {
            let unknown_name = self
                .unknown_name
                .as_ref()
                .map(|s| s.extract::<String>(py))
                .transpose()?;
            let kind = model_section_kind_of(self.kind, unknown_name)?;
            let mut blocks = Vec::new();
            for item in self.blocks.bind(py).iter() {
                let block = item.cast::<PyModelBlock>()?;
                blocks.push(block.borrow().to_model(py)?);
            }
            Ok(model::Section::new(kind, blocks))
        })
    }
}

// ─── Model Docstring ─────────────────────────────────────────────────────────

#[pyclass(module = "pydocstring.model", name = "Docstring")]
struct PyModelDocstring {
    summary: Option<Py<PyString>>,
    extended_summary: Option<Py<PyString>>,
    directives: Py<PyList>,
    sections: Py<PyList>,
}

impl PyModelDocstring {
    fn verify_sections(py: Python<'_>, sections: &Py<PyList>) -> PyResult<()> {
        ensure_all_instance_of::<PyModelSection>(
            py,
            sections,
            "Docstring only accepts Sections in the 'sections' argument.",
        )
    }

    fn verify_directives(py: Python<'_>, directives: &Py<PyList>) -> PyResult<()> {
        ensure_all_instance_of::<PyModelDirective>(
            py,
            directives,
            "Docstring only accepts Directives in the 'directives' argument.",
        )
    }
}

#[pymethods]
impl PyModelDocstring {
    #[new]
    #[pyo3(signature = (*, summary=None, extended_summary=None, directives=None, sections=None))]
    fn new(
        py: Python<'_>,
        summary: Option<Py<PyString>>,
        extended_summary: Option<Py<PyString>>,
        directives: Option<Py<PyList>>,
        sections: Option<Py<PyList>>,
    ) -> PyResult<Self> {
        let directives = if let Some(dir) = directives {
            Self::verify_directives(py, &dir)?;
            dir
        } else {
            PyList::empty(py).unbind()
        };
        let sections = if let Some(sec) = sections {
            Self::verify_sections(py, &sec)?;
            sec
        } else {
            PyList::empty(py).unbind()
        };
        Ok(Self {
            summary,
            extended_summary,
            directives,
            sections,
        })
    }

    #[getter]
    fn summary<'py>(&self, py: Python<'py>) -> Option<&Bound<'py, PyString>> {
        match &self.summary {
            Some(summary) => Some(summary.bind(py)),
            _ => None,
        }
    }
    #[setter]
    fn set_summary(&mut self, v: Option<Py<PyString>>) {
        self.summary = v;
    }
    #[getter]
    fn extended_summary<'py>(&self, py: Python<'py>) -> Option<&Bound<'py, PyString>> {
        match &self.extended_summary {
            Some(summary) => Some(summary.bind(py)),
            _ => None,
        }
    }
    #[setter]
    fn set_extended_summary(&mut self, v: Option<Py<PyString>>) {
        self.extended_summary = v;
    }

    /// Document-level rST directives, in source order.
    #[getter]
    fn directives<'py>(&self, py: Python<'py>) -> &Bound<'py, PyList> {
        self.directives.bind(py)
    }
    #[setter]
    fn set_directives(&mut self, py: Python<'_>, directives: Py<PyList>) -> PyResult<()> {
        Self::verify_directives(py, &directives)?;
        self.directives = directives;
        Ok(())
    }

    /// Computed convenience: the first directive named ``deprecated``, if any.
    ///
    /// Read-only — edit ``directives`` to change it.
    #[getter]
    fn deprecation(&self, py: Python<'_>) -> PyResult<Option<Py<PyModelDirective>>> {
        for item in self.directives.bind(py) {
            let directive = item.cast::<PyModelDirective>()?;
            if directive.borrow().name.bind(py).to_str()? == "deprecated" {
                return Ok(Some(directive.clone().unbind()));
            }
        }
        Ok(None)
    }

    #[getter]
    fn sections<'py>(&self, py: Python<'py>) -> &Bound<'py, PyList> {
        self.sections.bind(py)
    }
    #[setter]
    fn set_sections(&mut self, py: Python<'_>, sections: Py<PyList>) -> PyResult<()> {
        Self::verify_sections(py, &sections)?;
        self.sections = sections;
        Ok(())
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        match &self.summary {
            Some(s) => format!("Docstring(summary={:?})", s.bind(py).to_string_lossy()),
            None => "Docstring(summary=None)".to_string(),
        }
    }
}

impl TryFrom<&model::Docstring> for PyModelDocstring {
    type Error = PyErr;

    fn try_from(docstr: &model::Docstring) -> Result<Self, Self::Error> {
        Python::attach(|py| {
            Ok(Self {
                summary: docstr
                    .summary
                    .as_ref()
                    .map(|x| -> PyResult<_> { Ok(x.into_pyobject(py)?.unbind()) })
                    .transpose()?,
                extended_summary: docstr
                    .extended_summary
                    .as_ref()
                    .map(|x| -> PyResult<_> { Ok(x.into_pyobject(py)?.unbind()) })
                    .transpose()?,
                directives: PyList::new(
                    py,
                    docstr
                        .directives
                        .iter()
                        .map(PyModelDirective::try_from)
                        .collect::<PyResult<Vec<_>>>()?,
                )?
                .unbind(),
                sections: PyList::new(
                    py,
                    docstr
                        .sections
                        .iter()
                        .map(PyModelSection::try_from)
                        .collect::<PyResult<Vec<_>>>()?,
                )?
                .unbind(),
            })
        })
    }
}

impl TryInto<model::Docstring> for &PyModelDocstring {
    type Error = PyErr;

    fn try_into(self) -> Result<model::Docstring, Self::Error> {
        Python::attach(|py| {
            Ok(model::Docstring {
                summary: self.summary.as_ref().map(|x| x.extract(py)).transpose()?,
                extended_summary: self.extended_summary.as_ref().map(|x| x.extract(py)).transpose()?,
                directives: self
                    .directives
                    .bind(py)
                    .iter()
                    .map(|dir| dir.cast::<PyModelDirective>()?.borrow().deref().try_into())
                    .collect::<Result<Vec<_>, _>>()?,
                sections: self
                    .sections
                    .bind(py)
                    .iter()
                    .map(|sec| sec.cast::<PyModelSection>()?.borrow().deref().try_into())
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })
    }
}

// =============================================================================
// Pattern matching & rewriting (#47)
// =============================================================================

pyo3::create_exception!(
    _pydocstring,
    PatternError,
    pyo3::exceptions::PyValueError,
    "Raised when a pattern string has no valid reading (a ValueError subclass)."
);

/// Build a core `Pattern`, surfacing `PatternError` on failure.
fn build_pattern(style: CoreStyle, pattern: &str) -> PyResult<Pattern> {
    Pattern::new(style, pattern).map_err(|e| PatternError::new_err(e.to_string()))
}

/// Immutable value snapshot of a matcher `Capture` (byte range + original
/// target bytes); cloned into every `Capture`/`captures` access.
#[derive(Clone)]
struct CaptureData {
    start: u32,
    end: u32,
    text: String,
    multi: bool,
}

/// One metavariable capture of a `Match`: the original target bytes it bound
/// (`text`) and their byte range. Frozen, read-only.
#[pyclass(frozen, skip_from_py_object, module = "pydocstring", name = "Capture")]
struct PyCapture {
    data: CaptureData,
}

#[pymethods]
impl PyCapture {
    /// Byte range of the captured span, in the target's coordinates.
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(
            py,
            PyTextRange {
                start: self.data.start,
                end: self.data.end,
            },
        )
    }
    /// The original target bytes of the captured span (never reformatted).
    #[getter]
    fn text(&self) -> &str {
        &self.data.text
    }
    /// Whether this capture was bound by a ``$$$NAME`` sequence variable.
    fn is_multi(&self) -> bool {
        self.data.multi
    }
    fn __repr__(&self) -> String {
        format!("Capture({:?})", self.data.text)
    }
}

/// One non-overlapping match of a pattern against a docstring: the matched
/// span (`range` / `text`) and its metavariable `captures`. Frozen value
/// snapshot — safe to keep after the source is dropped.
#[pyclass(frozen, skip_from_py_object, module = "pydocstring", name = "Match")]
struct PyMatch {
    start: u32,
    end: u32,
    text: String,
    captures: Vec<(String, CaptureData)>,
}

impl PyMatch {
    fn from_match(m: &Match<'_>) -> Self {
        let captures = m
            .captures()
            .map(|(name, capture)| {
                (
                    name.to_owned(),
                    CaptureData {
                        start: capture.range().start().raw(),
                        end: capture.range().end().raw(),
                        text: capture.text().to_owned(),
                        multi: capture.is_multi(),
                    },
                )
            })
            .collect();
        PyMatch {
            start: m.range().start().raw(),
            end: m.range().end().raw(),
            text: m.text().to_owned(),
            captures,
        }
    }
}

#[pymethods]
impl PyMatch {
    /// Byte range of the matched span, in the target's coordinates.
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(
            py,
            PyTextRange {
                start: self.start,
                end: self.end,
            },
        )
    }
    /// The matched span's original target bytes.
    #[getter]
    fn text(&self) -> &str {
        &self.text
    }
    /// The captures as a ``dict[str, Capture]``, keyed by metavariable name
    /// (first occurrence order preserved by insertion).
    #[getter]
    fn captures(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        for (name, data) in &self.captures {
            dict.set_item(name, Py::new(py, PyCapture { data: data.clone() })?)?;
        }
        Ok(dict.unbind())
    }
    /// The capture bound to metavariable ``name`` (without the ``$`` sigil),
    /// or ``None`` if the pattern has no such metavariable.
    fn capture(&self, py: Python<'_>, name: &str) -> PyResult<Option<Py<PyCapture>>> {
        self.captures
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, data)| Py::new(py, PyCapture { data: data.clone() }))
            .transpose()
    }
    fn __repr__(&self) -> String {
        format!("Match({:?})", self.text)
    }
}

/// Shared implementation of ``doc.replace``.
fn rewrite_replace(nr: &NodeRef, style: CoreStyle, pattern: &str, template: &str) -> PyResult<String> {
    let pattern = build_pattern(style, pattern)?;
    nr.parsed
        .replace(&pattern, template)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Shared implementation of ``doc.findall``.
fn rewrite_findall(py: Python<'_>, nr: &NodeRef, style: CoreStyle, pattern: &str) -> PyResult<Vec<Py<PyMatch>>> {
    let pattern = build_pattern(style, pattern)?;
    pattern
        .matches(&nr.parsed)
        .iter()
        .map(|m| Py::new(py, PyMatch::from_match(m)))
        .collect()
}

/// Resolve a scope anchor: a unified `Document`, `Section`, or `Entry` view.
///
/// The anchor must come from the *same* parse result. A `NodeRef` addresses a
/// node by child-index path, so a foreign anchor would not simply "match
/// nothing" as it does in the Rust API — the same path would resolve to some
/// unrelated node of this tree. Hence the identity check.
fn anchor_ref(nr: &NodeRef, anchor: &Bound<'_, PyAny>) -> PyResult<NodeRef> {
    let anchor_nr = if let Ok(s) = anchor.cast::<PySection>() {
        s.get().nr.clone()
    } else if let Ok(e) = anchor.cast::<PyEntry>() {
        e.get().nr.clone()
    } else if let Ok(d) = anchor.cast::<PyDocument>() {
        d.get().nr.clone()
    } else {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "anchor must be a Document, Section, or Entry view",
        ));
    };
    if !Arc::ptr_eq(&nr.parsed, &anchor_nr.parsed) {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "anchor belongs to a different parse result",
        ));
    }
    Ok(anchor_nr)
}

/// Shared implementation of ``doc.replace_in``.
fn rewrite_replace_in(
    nr: &NodeRef,
    style: CoreStyle,
    anchor: &Bound<'_, PyAny>,
    pattern: &str,
    template: &str,
) -> PyResult<String> {
    let anchor_nr = anchor_ref(nr, anchor)?;
    let pattern = build_pattern(style, pattern)?;
    nr.parsed
        .replace_in(&pattern, anchor_nr.node(), template)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
}

/// Shared implementation of ``doc.findall_in``.
fn rewrite_findall_in(
    py: Python<'_>,
    nr: &NodeRef,
    style: CoreStyle,
    anchor: &Bound<'_, PyAny>,
    pattern: &str,
) -> PyResult<Vec<Py<PyMatch>>> {
    let anchor_nr = anchor_ref(nr, anchor)?;
    let pattern = build_pattern(style, pattern)?;
    pattern
        .matches_in(&nr.parsed, anchor_nr.node())
        .iter()
        .map(|m| Py::new(py, PyMatch::from_match(m)))
        .collect()
}

// =============================================================================
// Module functions
// =============================================================================

/// Parse a Google-style docstring.
#[pyfunction]
fn parse_google(py: Python<'_>, input: &str) -> PyResult<Py<PyParsed>> {
    build_parsed(py, pydocstring_core::parse::parse_google(input))
}

/// Parse a NumPy-style docstring.
#[pyfunction]
fn parse_numpy(py: Python<'_>, input: &str) -> PyResult<Py<PyParsed>> {
    build_parsed(py, pydocstring_core::parse::parse_numpy(input))
}

/// Parse a plain docstring (no section markers).
#[pyfunction]
fn parse_plain(py: Python<'_>, input: &str) -> PyResult<Py<PyParsed>> {
    build_parsed(py, pydocstring_core::parse::parse_plain(input))
}

/// Auto-detect the docstring style and parse it.
///
/// Returns a `Parsed`; check `.style` to see which style was detected.
#[pyfunction]
fn parse(py: Python<'_>, input: &str) -> PyResult<Py<PyParsed>> {
    build_parsed(py, pydocstring_core::parse::parse(input))
}

/// Detect the docstring style without fully parsing.
#[pyfunction]
fn detect_style(input: &str) -> PyStyle {
    py_style_of(pydocstring_core::parse::detect_style(input))
}

/// Emit a model `Docstring` as Google-style text.
#[pyfunction]
#[pyo3(name = "emit_google", signature = (doc, base_indent=0))]
fn py_emit_google(py: Python<'_>, doc: Py<PyModelDocstring>, base_indent: usize) -> PyResult<String> {
    Ok(pydocstring_core::emit::google::emit_google(
        &doc.borrow(py).deref().try_into()?,
        &EmitOptions::default().with_base_indent(base_indent),
    ))
}

/// Emit a model `Docstring` as NumPy-style text.
#[pyfunction]
#[pyo3(name = "emit_numpy", signature = (doc, base_indent=0))]
fn py_emit_numpy(py: Python<'_>, doc: Py<PyModelDocstring>, base_indent: usize) -> PyResult<String> {
    Ok(pydocstring_core::emit::numpy::emit_numpy(
        &doc.borrow(py).deref().try_into()?,
        &EmitOptions::default().with_base_indent(base_indent),
    ))
}

/// Emit a model `Docstring` as Sphinx-style (reStructuredText) text.
#[pyfunction]
#[pyo3(name = "emit_sphinx", signature = (doc, base_indent=0))]
fn py_emit_sphinx(py: Python<'_>, doc: Py<PyModelDocstring>, base_indent: usize) -> PyResult<String> {
    Ok(pydocstring_core::emit::sphinx::emit_sphinx(
        &doc.borrow(py).deref().try_into()?,
        &EmitOptions::default().with_base_indent(base_indent),
    ))
}

// =============================================================================
// walk() — generic CST traversal (#119)
// =============================================================================
//
// The traversal used to dispatch into per-style typed callbacks
// (`enter_google_arg`, `enter_numpy_parameter`, …), which is what coupled it to
// the 26 wrapper classes. The tree has no per-style structure, so the traversal
// need not either: it now walks nodes and tokens, and the visitor decides what
// it cares about by looking at `kind`.

// ─── WalkContext ─────────────────────────────────────────────────────────────

/// Context passed to every visitor method during a ``walk()`` call.
///
/// Provides source-location helpers for the docstring currently being walked.
#[pyclass(frozen, skip_from_py_object, module = "pydocstring", name = "WalkContext")]
struct PyWalkContext {
    source: String,
    line_starts: Vec<u32>,
}

#[pymethods]
impl PyWalkContext {
    /// Convert a byte offset into a ``LineColumn``.
    ///
    /// Returns a 1-based line number and 0-based column offset.
    fn line_col(&self, py: Python<'_>, offset: u32) -> PyResult<Py<PyLineColumn>> {
        let offset_usize = offset as usize;
        if offset_usize > self.source.len() {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "offset {} is out of bounds (source length: {})",
                offset,
                self.source.len()
            )));
        }
        let line = self.line_starts.partition_point(|&s| s <= offset) - 1;
        let line_start = self.line_starts[line] as usize;
        if !self.source.is_char_boundary(offset_usize) || !self.source.is_char_boundary(line_start) {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "offset is not on a UTF-8 character boundary",
            ));
        }
        let col = self.source[line_start..offset_usize].chars().count() as u32;
        Py::new(
            py,
            PyLineColumn {
                lineno: line as u32 + 1,
                col,
            },
        )
    }
    fn __repr__(&self) -> &'static str {
        "WalkContext(...)"
    }
}

// ─── walk ────────────────────────────────────────────────────────────────────

/// Which visitor methods are defined, collected **once per `walk()` call** so
/// the reflection is never repeated per-node.
struct ActiveMethods {
    enter_node: bool,
    leave_node: bool,
    visit_token: bool,
}

/// Whether the visitor actually implements `name`.
///
/// `Visitor`'s base hooks are no-ops tagged with `__pydocstring_noop__`, so a
/// plain `hasattr` would report every hook as present and call back into Python
/// for every node and token. Only an override counts.
fn is_overridden(visitor: &Bound<'_, PyAny>, name: &str) -> PyResult<bool> {
    match visitor.getattr(name) {
        Ok(method) => Ok(!method.hasattr("__pydocstring_noop__")?),
        Err(_) => Ok(false),
    }
}

fn walk_subtree(
    py: Python<'_>,
    nr: &NodeRef,
    visitor: &Py<PyAny>,
    ctx: &Py<PyWalkContext>,
    active: &ActiveMethods,
) -> PyResult<()> {
    let visitor = visitor.bind(py);
    if active.enter_node {
        let node = Py::new(py, PyNode::from(nr.clone()))?;
        visitor.call_method1("enter_node", (node, ctx))?;
    }
    // Collect the child node addresses first: `nr.node()` re-resolves the path
    // on each access, and the borrow must not straddle the Python callbacks.
    let child_kinds: Vec<(u32, bool)> = nr
        .node()
        .children()
        .iter()
        .enumerate()
        .map(|(i, c)| (i as u32, matches!(c, SyntaxElement::Node(_))))
        .collect();
    for (index, is_node) in child_kinds {
        if is_node {
            let mut path = nr.path.clone();
            path.push(index);
            let child = NodeRef {
                parsed: Arc::clone(&nr.parsed),
                path,
            };
            walk_subtree(py, &child, &visitor.clone().unbind(), ctx, active)?;
        } else if active.visit_token {
            let token = Py::new(
                py,
                PyToken {
                    parent: nr.clone(),
                    index,
                },
            )?;
            visitor.call_method1("visit_token", (token, ctx))?;
        }
    }
    if active.leave_node {
        let node = Py::new(py, PyNode::from(nr.clone()))?;
        visitor.call_method1("leave_node", (node, ctx))?;
    }
    Ok(())
}

/// Walk a parse result or a subtree depth-first, calling the visitor's methods.
///
/// The visitor may define any of `enter_node(node, ctx)`,
/// `leave_node(node, ctx)`, and `visit_token(token, ctx)`; whichever are absent
/// are simply not called. Dispatch on `node.kind` / `token.kind` to decide what
/// to do. Returns the visitor, so state can be read straight off the call.
///
/// Exceptions raised inside a visitor method propagate out of `walk`.
#[pyfunction]
fn walk(py: Python<'_>, target: &Bound<'_, PyAny>, visitor: Py<PyAny>) -> PyResult<Py<PyAny>> {
    let nr = if let Ok(p) = target.cast::<PyParsed>() {
        p.get().nr.clone()
    } else if let Ok(n) = target.cast::<PyNode>() {
        n.get().nr.clone()
    } else {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "walk() expects a Parsed or a Node",
        ));
    };

    let bound = visitor.bind(py);
    // Duck-typing here would silently do nothing for an object that is not a
    // visitor at all, so the subclass contract is checked, as it was before the
    // traversal became generic.
    let visitor_cls = py.import("pydocstring")?.getattr("Visitor")?;
    if !bound.is_instance(&visitor_cls)? {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "visitor must subclass pydocstring.Visitor",
        ));
    }
    let active = ActiveMethods {
        enter_node: is_overridden(bound, "enter_node")?,
        leave_node: is_overridden(bound, "leave_node")?,
        visit_token: is_overridden(bound, "visit_token")?,
    };

    let source = nr.parsed.source().to_string();
    let line_starts = build_line_starts(&source);
    let ctx = Py::new(py, PyWalkContext { source, line_starts })?;

    walk_subtree(py, &nr, &visitor, &ctx, &active)?;
    Ok(visitor)
}

// =============================================================================
// Module
// =============================================================================

/// `Visitor` is defined in `python/pydocstring/_visitor.py`. Its three hooks
/// are no-ops tagged with `__pydocstring_noop__`, so `is_overridden` probes for
/// that tag once per walk and records the answer in `ActiveMethods` — an
/// un-overridden hook then costs no Python call per node or token.

#[pymodule]
fn _pydocstring(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Functions
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(parse_google, m)?)?;
    m.add_function(wrap_pyfunction!(parse_numpy, m)?)?;
    m.add_function(wrap_pyfunction!(parse_plain, m)?)?;
    m.add_function(wrap_pyfunction!(detect_style, m)?)?;
    m.add_function(wrap_pyfunction!(py_emit_google, m)?)?;
    m.add_function(wrap_pyfunction!(py_emit_numpy, m)?)?;
    m.add_function(wrap_pyfunction!(py_emit_sphinx, m)?)?;
    m.add_function(wrap_pyfunction!(walk, m)?)?;
    // Core types
    m.add_class::<PyStyle>()?;
    m.add_class::<PyParsed>()?;
    m.add_class::<PyTextRange>()?;
    m.add_class::<PyLineColumn>()?;
    m.add_class::<PyToken>()?;
    m.add_class::<PyTextBlock>()?;
    m.add_class::<PyWalkContext>()?;
    // Pattern matching & rewriting (#47)
    m.add_class::<PyMatch>()?;
    m.add_class::<PyCapture>()?;
    m.add("PatternError", m.py().get_type::<PatternError>())?;
    // Raw CST — the fidelity lens (#126)
    m.add_class::<PySyntaxKind>()?;
    m.add_class::<PyNode>()?;
    // Unified (style-independent) views — the recommended read lens (#116)
    m.add_class::<PyDocument>()?;
    m.add_class::<PySection>()?;
    m.add_class::<PyEntry>()?;
    m.add_class::<PyDefaultMarker>()?;
    m.add_class::<PyDirective>()?;
    m.add_class::<PyCitation>()?;
    // Anchored splice edits (#117)
    m.add_class::<PyEdits>()?;
    m.add("EditError", m.py().get_type::<EditError>())?;
    // Section vocabulary — shared by the unified view (`section.kind`) and the
    // model, so it stays at the top level and is re-exported from `model`.
    m.add_class::<PySectionKind>()?;

    // Model IR — namespaced under `model`, mirroring the Rust crate's module
    // split. The two layers each define a `Section` and a `Directive`; Rust
    // keeps them apart with `model::` vs `parse::unified::`, and flattening
    // both into one Python namespace is what made them collide. The top level
    // is the CST / unified / edit surface; the model is a separate layer.
    let model_mod = PyModule::new(m.py(), "model")?;
    model_mod.add_class::<PySectionKind>()?;
    model_mod.add_class::<PyModelDocstring>()?;
    model_mod.add_class::<PyModelSection>()?;
    model_mod.add_class::<PyModelBlock>()?;
    model_mod.add_class::<PyModelParameter>()?;
    model_mod.add_class::<PyModelReturn>()?;
    model_mod.add_class::<PyModelExceptionEntry>()?;
    model_mod.add_class::<PyModelSeeAlsoEntry>()?;
    model_mod.add_class::<PyModelReference>()?;
    model_mod.add_class::<PyModelAttribute>()?;
    model_mod.add_class::<PyModelMethod>()?;
    model_mod.add_class::<PyModelDirective>()?;
    m.add_submodule(&model_mod)?;
    // A submodule is only an attribute until it is in `sys.modules`; without
    // this, `from pydocstring._pydocstring.model import Docstring` fails.
    m.py()
        .import("sys")?
        .getattr("modules")?
        .set_item("pydocstring._pydocstring.model", &model_mod)?;
    Ok(())
}
