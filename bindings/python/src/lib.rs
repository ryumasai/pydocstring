use pyo3::PyClass;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};

use pydocstring_core::matcher::Match;
use pydocstring_core::parse::Style as CoreStyle;
use pydocstring_core::pattern::Pattern;

use pydocstring_core::emit::EmitOptions;
use pydocstring_core::model;
use pydocstring_core::parse::DefaultMarker;
use pydocstring_core::parse::google;
use pydocstring_core::parse::google::kind::GoogleSectionKind;
use pydocstring_core::parse::google::nodes as gn;
use pydocstring_core::parse::numpy::kind::NumPySectionKind;
use pydocstring_core::parse::numpy::nodes as nn;
use pydocstring_core::parse::plain::nodes as pn;
use pydocstring_core::parse::text_block::TextBlock;
use pydocstring_core::parse::token_ref::TokenRef;
use pydocstring_core::parse::visitor::{DocstringVisitor, walk as core_walk, walk_children};
use pydocstring_core::syntax::{Parsed, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
use pydocstring_core::text::TextRange;

use std::convert::{TryFrom, TryInto};
use std::ops::Deref;
use std::sync::Arc;

// ─── TextRange ──────────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "TextRange")]
#[derive(Clone, Copy)]
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

#[pyclass(frozen, skip_from_py_object, name = "LineColumn")]
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

    /// Address `node`, which must belong to `parsed`'s tree.
    fn for_node(parsed: &Arc<Parsed>, node: &SyntaxNode) -> Self {
        let mut path = Vec::new();
        let found = find_node_path(parsed.root(), node, &mut path);
        debug_assert!(found, "node does not belong to this parse result");
        Self {
            parsed: Arc::clone(parsed),
            path,
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
        Py::new(py, PyTextRange::from(*self.node().range()))
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

    /// The present token from a typed accessor, else the zero-length
    /// "missing" placeholder of `kind` (excluded by the typed accessors) so
    /// Python callers can distinguish e.g. `a ()` from `a`.
    fn token_or_missing(
        &self,
        py: Python<'_>,
        present: Option<TokenRef<'_>>,
        kind: SyntaxKind,
    ) -> PyResult<Option<Py<PyToken>>> {
        match present {
            Some(t) => Ok(Some(self.token(py, t.syntax())?)),
            None => self.node().find_missing(kind).map(|t| self.token(py, t)).transpose(),
        }
    }

    /// First DEFAULT occurrence wins (markers are repeatable, #41); a missing
    /// (zero-length) value token lives inside that node.
    fn first_default_value<'a>(
        &self,
        py: Python<'_>,
        mut defaults: impl Iterator<Item = DefaultMarker<'a>>,
    ) -> PyResult<Option<Py<PyToken>>> {
        match defaults.next() {
            Some(d) => match d.value() {
                Some(t) => Ok(Some(self.token(py, t.syntax())?)),
                None => d
                    .syntax()
                    .find_missing(SyntaxKind::DEFAULT_VALUE)
                    .map(|t| self.token(py, t))
                    .transpose(),
            },
            None => Ok(None),
        }
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

    /// Like [`NodeRef::block_opt`], but falls back to the zero-length
    /// "missing" placeholder block of `kind` (excluded by the typed
    /// accessors) so Python callers can distinguish e.g. `a (int):` from
    /// `a (int)`.
    fn block_or_missing(
        &self,
        py: Python<'_>,
        present: Option<TextBlock<'_>>,
        kind: SyntaxKind,
    ) -> PyResult<Option<Py<PyTextBlock>>> {
        match present {
            Some(b) => Ok(Some(self.block(py, &b)?)),
            None => self
                .node()
                .nodes(kind)
                .next()
                .map(|n| Py::new(py, PyTextBlock { nr: self.child_ref(n) }))
                .transpose(),
        }
    }
}

// ─── Token ──────────────────────────────────────────────────────────────────

/// A typed token: a text fragment plus its byte range in the source.
///
/// Lazy view of a tree leaf — holds the parent node's [`NodeRef`] plus the
/// token's child index, and resolves `text` / `range` through the core tree
/// on access.
///
/// The field name on the parent object (e.g. `.name`, `.description`) implies
/// the semantic kind; no redundant `kind` field is exposed.
#[pyclass(frozen, skip_from_py_object, name = "Token")]
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
    fn text(&self) -> &str {
        self.resolve().text(self.parent.parsed.source())
    }
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(*self.resolve().range()))
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
    fn __eq__(&self, other: &PyToken) -> bool {
        self.text() == other.text() && self.resolve().range() == other.resolve().range()
    }
    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let token = self.resolve();
        token.text(self.parent.parsed.source()).hash(&mut hasher);
        token.range().start().raw().hash(&mut hasher);
        token.range().end().raw().hash(&mut hasher);
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
#[pyclass(frozen, skip_from_py_object, name = "TextBlock")]
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

#[pyclass(eq, frozen, skip_from_py_object, hash, name = "Style")]
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

// ─── GoogleSectionKind ───────────────────────────────────────────────────────

#[pyclass(eq, eq_int, frozen, skip_from_py_object, hash, name = "GoogleSectionKind")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum PyGoogleSectionKind {
    #[pyo3(name = "ARGS")]
    Args,
    #[pyo3(name = "KEYWORD_ARGS")]
    KeywordArgs,
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
    #[pyo3(name = "NOTES")]
    Notes,
    #[pyo3(name = "EXAMPLES")]
    Examples,
    #[pyo3(name = "TODO")]
    Todo,
    #[pyo3(name = "REFERENCES")]
    References,
    #[pyo3(name = "WARNINGS")]
    Warnings,
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

#[pymethods]
impl PyGoogleSectionKind {
    fn __repr__(&self) -> String {
        format!(
            "GoogleSectionKind.{}",
            match self {
                Self::Args => "ARGS",
                Self::KeywordArgs => "KEYWORD_ARGS",
                Self::OtherParameters => "OTHER_PARAMETERS",
                Self::Receives => "RECEIVES",
                Self::Returns => "RETURNS",
                Self::Yields => "YIELDS",
                Self::Raises => "RAISES",
                Self::Warns => "WARNS",
                Self::Attributes => "ATTRIBUTES",
                Self::Methods => "METHODS",
                Self::SeeAlso => "SEE_ALSO",
                Self::Notes => "NOTES",
                Self::Examples => "EXAMPLES",
                Self::Todo => "TODO",
                Self::References => "REFERENCES",
                Self::Warnings => "WARNINGS",
                Self::Attention => "ATTENTION",
                Self::Caution => "CAUTION",
                Self::Danger => "DANGER",
                Self::Error => "ERROR",
                Self::Hint => "HINT",
                Self::Important => "IMPORTANT",
                Self::Tip => "TIP",
                Self::Unknown => "UNKNOWN",
            }
        )
    }
}

fn google_section_kind_to_py(kind: GoogleSectionKind) -> PyGoogleSectionKind {
    match kind {
        GoogleSectionKind::Args => PyGoogleSectionKind::Args,
        GoogleSectionKind::KeywordArgs => PyGoogleSectionKind::KeywordArgs,
        GoogleSectionKind::OtherParameters => PyGoogleSectionKind::OtherParameters,
        GoogleSectionKind::Receives => PyGoogleSectionKind::Receives,
        GoogleSectionKind::Returns => PyGoogleSectionKind::Returns,
        GoogleSectionKind::Yields => PyGoogleSectionKind::Yields,
        GoogleSectionKind::Raises => PyGoogleSectionKind::Raises,
        GoogleSectionKind::Warns => PyGoogleSectionKind::Warns,
        GoogleSectionKind::Attributes => PyGoogleSectionKind::Attributes,
        GoogleSectionKind::Methods => PyGoogleSectionKind::Methods,
        GoogleSectionKind::SeeAlso => PyGoogleSectionKind::SeeAlso,
        GoogleSectionKind::Notes => PyGoogleSectionKind::Notes,
        GoogleSectionKind::Examples => PyGoogleSectionKind::Examples,
        GoogleSectionKind::Todo => PyGoogleSectionKind::Todo,
        GoogleSectionKind::References => PyGoogleSectionKind::References,
        GoogleSectionKind::Warnings => PyGoogleSectionKind::Warnings,
        GoogleSectionKind::Attention => PyGoogleSectionKind::Attention,
        GoogleSectionKind::Caution => PyGoogleSectionKind::Caution,
        GoogleSectionKind::Danger => PyGoogleSectionKind::Danger,
        GoogleSectionKind::Error => PyGoogleSectionKind::Error,
        GoogleSectionKind::Hint => PyGoogleSectionKind::Hint,
        GoogleSectionKind::Important => PyGoogleSectionKind::Important,
        GoogleSectionKind::Tip => PyGoogleSectionKind::Tip,
        GoogleSectionKind::Unknown => PyGoogleSectionKind::Unknown,
        // GoogleSectionKind is #[non_exhaustive]; surface future kinds as Unknown.
        _ => PyGoogleSectionKind::Unknown,
    }
}

// ─── NumPySectionKind ────────────────────────────────────────────────────────

#[pyclass(eq, eq_int, frozen, skip_from_py_object, hash, name = "NumPySectionKind")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum PyNumPySectionKind {
    #[pyo3(name = "PARAMETERS")]
    Parameters,
    #[pyo3(name = "RETURNS")]
    Returns,
    #[pyo3(name = "YIELDS")]
    Yields,
    #[pyo3(name = "RECEIVES")]
    Receives,
    #[pyo3(name = "OTHER_PARAMETERS")]
    OtherParameters,
    #[pyo3(name = "KEYWORD_PARAMETERS")]
    KeywordParameters,
    #[pyo3(name = "RAISES")]
    Raises,
    #[pyo3(name = "WARNS")]
    Warns,
    #[pyo3(name = "WARNINGS")]
    Warnings,
    #[pyo3(name = "SEE_ALSO")]
    SeeAlso,
    #[pyo3(name = "NOTES")]
    Notes,
    #[pyo3(name = "REFERENCES")]
    References,
    #[pyo3(name = "EXAMPLES")]
    Examples,
    #[pyo3(name = "ATTRIBUTES")]
    Attributes,
    #[pyo3(name = "METHODS")]
    Methods,
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

#[pymethods]
impl PyNumPySectionKind {
    fn __repr__(&self) -> String {
        format!(
            "NumPySectionKind.{}",
            match self {
                Self::Parameters => "PARAMETERS",
                Self::Returns => "RETURNS",
                Self::Yields => "YIELDS",
                Self::Receives => "RECEIVES",
                Self::OtherParameters => "OTHER_PARAMETERS",
                Self::KeywordParameters => "KEYWORD_PARAMETERS",
                Self::Raises => "RAISES",
                Self::Warns => "WARNS",
                Self::Warnings => "WARNINGS",
                Self::SeeAlso => "SEE_ALSO",
                Self::Notes => "NOTES",
                Self::References => "REFERENCES",
                Self::Examples => "EXAMPLES",
                Self::Attributes => "ATTRIBUTES",
                Self::Methods => "METHODS",
                Self::Todo => "TODO",
                Self::Attention => "ATTENTION",
                Self::Caution => "CAUTION",
                Self::Danger => "DANGER",
                Self::Error => "ERROR",
                Self::Hint => "HINT",
                Self::Important => "IMPORTANT",
                Self::Tip => "TIP",
                Self::Unknown => "UNKNOWN",
            }
        )
    }
}

fn numpy_section_kind_to_py(kind: NumPySectionKind) -> PyNumPySectionKind {
    match kind {
        NumPySectionKind::Parameters => PyNumPySectionKind::Parameters,
        NumPySectionKind::Returns => PyNumPySectionKind::Returns,
        NumPySectionKind::Yields => PyNumPySectionKind::Yields,
        NumPySectionKind::Receives => PyNumPySectionKind::Receives,
        NumPySectionKind::OtherParameters => PyNumPySectionKind::OtherParameters,
        NumPySectionKind::KeywordParameters => PyNumPySectionKind::KeywordParameters,
        NumPySectionKind::Raises => PyNumPySectionKind::Raises,
        NumPySectionKind::Warns => PyNumPySectionKind::Warns,
        NumPySectionKind::Warnings => PyNumPySectionKind::Warnings,
        NumPySectionKind::SeeAlso => PyNumPySectionKind::SeeAlso,
        NumPySectionKind::Notes => PyNumPySectionKind::Notes,
        NumPySectionKind::References => PyNumPySectionKind::References,
        NumPySectionKind::Examples => PyNumPySectionKind::Examples,
        NumPySectionKind::Attributes => PyNumPySectionKind::Attributes,
        NumPySectionKind::Methods => PyNumPySectionKind::Methods,
        NumPySectionKind::Todo => PyNumPySectionKind::Todo,
        NumPySectionKind::Attention => PyNumPySectionKind::Attention,
        NumPySectionKind::Caution => PyNumPySectionKind::Caution,
        NumPySectionKind::Danger => PyNumPySectionKind::Danger,
        NumPySectionKind::Error => PyNumPySectionKind::Error,
        NumPySectionKind::Hint => PyNumPySectionKind::Hint,
        NumPySectionKind::Important => PyNumPySectionKind::Important,
        NumPySectionKind::Tip => PyNumPySectionKind::Tip,
        NumPySectionKind::Unknown => PyNumPySectionKind::Unknown,
        // NumPySectionKind is #[non_exhaustive]; surface future kinds as Unknown.
        _ => PyNumPySectionKind::Unknown,
    }
}

// =============================================================================
// Lazy CST wrappers
// =============================================================================

/// Define a frozen pyclass CST wrapper holding a [`NodeRef`], resolving into
/// the core typed view `$mod::$view` on every property access.
///
/// THE POINT of these wrappers is that they contain no conversion knowledge:
/// every getter delegates to the corresponding core accessor, so grammar and
/// kind knowledge lives in one place (the Rust crate).
macro_rules! lazy_node {
    ($py:ident, $name:literal, $mod:ident :: $view:ident) => {
        #[pyclass(frozen, skip_from_py_object, name = $name)]
        struct $py {
            nr: NodeRef,
        }

        impl From<NodeRef> for $py {
            fn from(nr: NodeRef) -> Self {
                Self { nr }
            }
        }

        impl $py {
            /// Resolve the lazy address into the core typed view.
            fn view(&self) -> $mod::$view<'_> {
                $mod::$view::cast(&self.nr.parsed, self.nr.node())
                    .expect("NodeRef addresses a node of the wrapped kind")
            }
        }
    };
}

// =============================================================================
// Google typed wrappers
// =============================================================================

// ─── GoogleArg ───────────────────────────────────────────────────────────────

lazy_node!(PyGoogleArg, "GoogleArg", gn::GoogleArg);

#[pymethods]
impl PyGoogleArg {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn name(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().name().syntax())
    }
    #[getter]
    fn names(&self, py: Python<'_>) -> PyResult<Vec<Py<PyToken>>> {
        self.nr.tokens(py, self.view().names())
    }
    #[getter]
    fn open_bracket(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().open_bracket())
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr
            .token_or_missing(py, self.view().type_annotation(), SyntaxKind::TYPE)
    }
    #[getter]
    fn close_bracket(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr
            .token_or_missing(py, self.view().close_bracket(), SyntaxKind::CLOSE_BRACKET)
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_or_missing(py, self.view().colon(), SyntaxKind::COLON)
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr
            .block_or_missing(py, self.view().description(), SyntaxKind::DESCRIPTION)
    }
    #[getter]
    fn optional(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().optional_marker())
    }
    #[getter]
    fn default_keyword(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().default_keyword())
    }
    #[getter]
    fn default_separator(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().default_separator())
    }
    #[getter]
    fn default_value(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.first_default_value(py, self.view().defaults())
    }
    fn __repr__(&self) -> String {
        format!(
            "GoogleArg({:?})",
            self.view().names().map(|n| n.text()).collect::<Vec<_>>().join(", ")
        )
    }
}

// ─── GoogleReturn ────────────────────────────────────────────────────────────

lazy_node!(PyGoogleReturn, "GoogleReturn", gn::GoogleReturn);

#[pymethods]
impl PyGoogleReturn {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn return_type(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().type_annotation())
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().colon())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().description())
    }
    fn __repr__(&self) -> &'static str {
        "GoogleReturn(...)"
    }
}

// ─── GoogleYield ─────────────────────────────────────────────────────────────

lazy_node!(PyGoogleYield, "GoogleYield", gn::GoogleYield);

#[pymethods]
impl PyGoogleYield {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn return_type(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().type_annotation())
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().colon())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().description())
    }
    fn __repr__(&self) -> &'static str {
        "GoogleYield(...)"
    }
}

// ─── GoogleException ─────────────────────────────────────────────────────────

lazy_node!(PyGoogleException, "GoogleException", gn::GoogleException);

#[pymethods]
impl PyGoogleException {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().type_annotation().syntax())
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().colon())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr
            .block_or_missing(py, self.view().description(), SyntaxKind::DESCRIPTION)
    }
    fn __repr__(&self) -> String {
        format!("GoogleException({:?})", self.view().type_annotation().text())
    }
}

// ─── GoogleWarning ───────────────────────────────────────────────────────────

lazy_node!(PyGoogleWarning, "GoogleWarning", gn::GoogleWarning);

#[pymethods]
impl PyGoogleWarning {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().type_annotation().syntax())
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().colon())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr
            .block_or_missing(py, self.view().description(), SyntaxKind::DESCRIPTION)
    }
    fn __repr__(&self) -> String {
        format!("GoogleWarning({:?})", self.view().type_annotation().text())
    }
}

// ─── GoogleSeeAlsoItem ───────────────────────────────────────────────────────

lazy_node!(PyGoogleSeeAlsoItem, "GoogleSeeAlsoItem", gn::GoogleSeeAlsoItem);

#[pymethods]
impl PyGoogleSeeAlsoItem {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn names(&self, py: Python<'_>) -> PyResult<Vec<Py<PyToken>>> {
        self.nr.tokens(py, self.view().names())
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().colon())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr
            .block_or_missing(py, self.view().description(), SyntaxKind::DESCRIPTION)
    }
    fn __repr__(&self) -> &'static str {
        "GoogleSeeAlsoItem(...)"
    }
}

// ─── GoogleReference ─────────────────────────────────────────────────────────

lazy_node!(PyGoogleReference, "GoogleReference", gn::GoogleReference);

#[pymethods]
impl PyGoogleReference {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn directive_marker(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().directive_marker())
    }
    #[getter]
    fn open_bracket(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().open_bracket())
    }
    #[getter]
    fn label(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().label())
    }
    #[getter]
    fn close_bracket(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().close_bracket())
    }
    #[getter]
    fn content(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().content())
    }
    fn __repr__(&self) -> &'static str {
        "GoogleReference(...)"
    }
}

// ─── GoogleAttribute ─────────────────────────────────────────────────────────

lazy_node!(PyGoogleAttribute, "GoogleAttribute", gn::GoogleAttribute);

#[pymethods]
impl PyGoogleAttribute {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    /// First name token (convenience for ``names[0]``).
    #[getter]
    fn name(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().name().syntax())
    }
    #[getter]
    fn names(&self, py: Python<'_>) -> PyResult<Vec<Py<PyToken>>> {
        self.nr.tokens(py, self.view().names())
    }
    #[getter]
    fn open_bracket(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().open_bracket())
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr
            .token_or_missing(py, self.view().type_annotation(), SyntaxKind::TYPE)
    }
    #[getter]
    fn close_bracket(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr
            .token_or_missing(py, self.view().close_bracket(), SyntaxKind::CLOSE_BRACKET)
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_or_missing(py, self.view().colon(), SyntaxKind::COLON)
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr
            .block_or_missing(py, self.view().description(), SyntaxKind::DESCRIPTION)
    }
    fn __repr__(&self) -> String {
        format!(
            "GoogleAttribute({:?})",
            self.view().names().map(|n| n.text()).collect::<Vec<_>>().join(", ")
        )
    }
}

// ─── GoogleMethod ────────────────────────────────────────────────────────────

lazy_node!(PyGoogleMethod, "GoogleMethod", gn::GoogleMethod);

#[pymethods]
impl PyGoogleMethod {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn name(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().name().syntax())
    }
    #[getter]
    fn open_bracket(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().open_bracket())
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr
            .token_or_missing(py, self.view().type_annotation(), SyntaxKind::TYPE)
    }
    #[getter]
    fn close_bracket(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr
            .token_or_missing(py, self.view().close_bracket(), SyntaxKind::CLOSE_BRACKET)
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_or_missing(py, self.view().colon(), SyntaxKind::COLON)
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr
            .block_or_missing(py, self.view().description(), SyntaxKind::DESCRIPTION)
    }
    fn __repr__(&self) -> String {
        format!("GoogleMethod({:?})", self.view().name().text())
    }
}

// ─── GoogleSection ───────────────────────────────────────────────────────────

lazy_node!(PyGoogleSection, "GoogleSection", gn::GoogleSection);

#[pymethods]
impl PyGoogleSection {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn section_kind(&self) -> PyGoogleSectionKind {
        google_section_kind_to_py(self.view().section_kind())
    }
    #[getter]
    fn header_name(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().header().name().syntax())
    }
    fn __repr__(&self) -> String {
        format!("GoogleSection({:?})", self.view().header().name().text())
    }
}

// ─── GoogleDeprecation ───────────────────────────────────────────────────────

lazy_node!(PyGoogleDeprecation, "GoogleDeprecation", gn::GoogleDeprecation);

#[pymethods]
impl PyGoogleDeprecation {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn directive_marker(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().directive_marker())
    }
    #[getter]
    fn keyword(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().keyword())
    }
    #[getter]
    fn double_colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().double_colon())
    }
    #[getter]
    fn version(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().version().syntax())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().description())
    }
    fn __repr__(&self) -> String {
        format!("GoogleDeprecation({:?})", self.view().version().text())
    }
}

// ─── GoogleDirective ─────────────────────────────────────────────────────────

lazy_node!(PyGoogleDirective, "GoogleDirective", gn::GoogleDirective);

#[pymethods]
impl PyGoogleDirective {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn directive_marker(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().directive_marker())
    }
    #[getter]
    fn name(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().name().syntax())
    }
    #[getter]
    fn double_colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().double_colon())
    }
    #[getter]
    fn argument(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().argument())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().description())
    }
    fn __repr__(&self) -> String {
        format!("GoogleDirective({:?})", self.view().name().text())
    }
}

// ─── GoogleDocstring ─────────────────────────────────────────────────────────

lazy_node!(PyGoogleDocstring, "GoogleDocstring", gn::GoogleDocstring);

#[pymethods]
impl PyGoogleDocstring {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
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
    fn deprecation(&self, py: Python<'_>) -> PyResult<Option<Py<PyGoogleDeprecation>>> {
        self.view()
            .deprecation()
            .map(|d| self.nr.wrap_child(py, d.syntax()))
            .transpose()
    }
    /// Stray-prose paragraph blocks between sections, in source order.
    #[getter]
    fn paragraphs(&self, py: Python<'_>) -> PyResult<Vec<Py<PyTextBlock>>> {
        self.nr.blocks(py, self.view().paragraphs())
    }
    #[getter]
    fn sections(&self, py: Python<'_>) -> PyResult<Vec<Py<PyGoogleSection>>> {
        self.view()
            .sections()
            .map(|s| self.nr.wrap_child(py, s.syntax()))
            .collect()
    }
    #[getter]
    fn source(&self) -> &str {
        self.nr.parsed.source()
    }
    #[getter]
    fn style(&self) -> PyStyle {
        PyStyle::Google
    }
    fn pretty_print(&self) -> String {
        self.nr.parsed.pretty_print()
    }
    fn to_model(&self) -> PyResult<PyModelDocstring> {
        pydocstring_core::parse::google::to_model::to_model(&self.nr.parsed)
            .map(|doc| PyModelDocstring::try_from(&doc))
            .transpose()?
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("failed to convert to model"))
    }
    /// Replace every match of ``pattern`` (a Google-style pattern with
    /// ``$NAME`` / ``$$$NAME`` metavariables) with ``template``, returning the
    /// new source. Captured content is substituted byte-for-byte; everything
    /// else is preserved. Raises ``PatternError`` for an invalid pattern.
    fn replace(&self, pattern: &str, template: &str) -> PyResult<String> {
        rewrite_replace(&self.nr, CoreStyle::Google, pattern, template)
    }
    /// Find every match of ``pattern`` in document order (non-overlapping),
    /// returning a list of ``Match``. Raises ``PatternError`` for an invalid
    /// pattern.
    fn findall(&self, py: Python<'_>, pattern: &str) -> PyResult<Vec<Py<PyMatch>>> {
        rewrite_findall(py, &self.nr, CoreStyle::Google, pattern)
    }
    fn __repr__(&self) -> &'static str {
        "GoogleDocstring(...)"
    }
}

fn build_google_docstring(py: Python<'_>, parsed: Parsed) -> PyResult<Py<PyGoogleDocstring>> {
    let arc = Arc::new(parsed);
    gn::GoogleDocstring::cast(&arc, arc.root())
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("root is not a DOCUMENT node"))?;
    Py::new(py, PyGoogleDocstring::from(NodeRef::root(arc)))
}

// =============================================================================
// NumPy typed wrappers
// =============================================================================

// ─── NumPyDeprecation ────────────────────────────────────────────────────────

lazy_node!(PyNumPyDeprecation, "NumPyDeprecation", nn::NumPyDeprecation);

#[pymethods]
impl PyNumPyDeprecation {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn directive_marker(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().directive_marker())
    }
    #[getter]
    fn keyword(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().keyword())
    }
    #[getter]
    fn double_colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().double_colon())
    }
    #[getter]
    fn version(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().version().syntax())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().description())
    }
    fn __repr__(&self) -> String {
        format!("NumPyDeprecation({:?})", self.view().version().text())
    }
}

// ─── NumPyDirective ──────────────────────────────────────────────────────────

lazy_node!(PyNumPyDirective, "NumPyDirective", nn::NumPyDirective);

#[pymethods]
impl PyNumPyDirective {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn directive_marker(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().directive_marker())
    }
    #[getter]
    fn name(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().name().syntax())
    }
    #[getter]
    fn double_colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().double_colon())
    }
    #[getter]
    fn argument(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().argument())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().description())
    }
    fn __repr__(&self) -> String {
        format!("NumPyDirective({:?})", self.view().name().text())
    }
}

// ─── NumPyParameter ──────────────────────────────────────────────────────────

lazy_node!(PyNumPyParameter, "NumPyParameter", nn::NumPyParameter);

#[pymethods]
impl PyNumPyParameter {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    /// First name token (convenience for ``names[0]``); ``None`` when the
    /// entry has no name tokens.
    #[getter]
    fn name(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().names().next())
    }
    #[getter]
    fn names(&self, py: Python<'_>) -> PyResult<Vec<Py<PyToken>>> {
        self.nr.tokens(py, self.view().names())
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().colon())
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr
            .token_or_missing(py, self.view().type_annotation(), SyntaxKind::TYPE)
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr
            .block_or_missing(py, self.view().description(), SyntaxKind::DESCRIPTION)
    }
    #[getter]
    fn optional(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().optional_marker())
    }
    #[getter]
    fn default_keyword(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().default_keyword())
    }
    #[getter]
    fn default_separator(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().default_separator())
    }
    #[getter]
    fn default_value(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.first_default_value(py, self.view().defaults())
    }
    fn __repr__(&self) -> String {
        let names = self.view().names().map(|n| n.text()).collect::<Vec<_>>().join(", ");
        format!("NumPyParameter({:?})", names)
    }
}

// ─── NumPyReturns ────────────────────────────────────────────────────────────

lazy_node!(PyNumPyReturns, "NumPyReturns", nn::NumPyReturns);

#[pymethods]
impl PyNumPyReturns {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn name(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().name())
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().colon())
    }
    #[getter]
    fn return_type(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().type_annotation())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().description())
    }
    fn __repr__(&self) -> &'static str {
        "NumPyReturns(...)"
    }
}

// ─── NumPyYields ─────────────────────────────────────────────────────────────

lazy_node!(PyNumPyYields, "NumPyYields", nn::NumPyYields);

#[pymethods]
impl PyNumPyYields {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn name(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().name())
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().colon())
    }
    #[getter]
    fn return_type(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().type_annotation())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().description())
    }
    fn __repr__(&self) -> &'static str {
        "NumPyYields(...)"
    }
}

// ─── NumPyException ──────────────────────────────────────────────────────────

lazy_node!(PyNumPyException, "NumPyException", nn::NumPyException);

#[pymethods]
impl PyNumPyException {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().type_annotation().syntax())
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().colon())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr
            .block_or_missing(py, self.view().description(), SyntaxKind::DESCRIPTION)
    }
    fn __repr__(&self) -> String {
        format!("NumPyException({:?})", self.view().type_annotation().text())
    }
}

// ─── NumPyWarning ────────────────────────────────────────────────────────────

lazy_node!(PyNumPyWarning, "NumPyWarning", nn::NumPyWarning);

#[pymethods]
impl PyNumPyWarning {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().type_annotation().syntax())
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().colon())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr
            .block_or_missing(py, self.view().description(), SyntaxKind::DESCRIPTION)
    }
    fn __repr__(&self) -> String {
        format!("NumPyWarning({:?})", self.view().type_annotation().text())
    }
}

// ─── NumPySeeAlsoItem ────────────────────────────────────────────────────────

lazy_node!(PyNumPySeeAlsoItem, "NumPySeeAlsoItem", nn::NumPySeeAlsoItem);

#[pymethods]
impl PyNumPySeeAlsoItem {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn names(&self, py: Python<'_>) -> PyResult<Vec<Py<PyToken>>> {
        self.nr.tokens(py, self.view().names())
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().colon())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr
            .block_or_missing(py, self.view().description(), SyntaxKind::DESCRIPTION)
    }
    fn __repr__(&self) -> &'static str {
        "NumPySeeAlsoItem(...)"
    }
}

// ─── NumPyReference ──────────────────────────────────────────────────────────

lazy_node!(PyNumPyReference, "NumPyReference", nn::NumPyReference);

#[pymethods]
impl PyNumPyReference {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn directive_marker(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().directive_marker())
    }
    #[getter]
    fn open_bracket(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().open_bracket())
    }
    #[getter]
    fn label(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().label())
    }
    #[getter]
    fn close_bracket(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().close_bracket())
    }
    #[getter]
    fn content(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr.block_opt(py, self.view().content())
    }
    fn __repr__(&self) -> &'static str {
        "NumPyReference(...)"
    }
}

// ─── NumPyAttribute ──────────────────────────────────────────────────────────

lazy_node!(PyNumPyAttribute, "NumPyAttribute", nn::NumPyAttribute);

#[pymethods]
impl PyNumPyAttribute {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    /// First name token (convenience for ``names[0]``).
    #[getter]
    fn name(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().name().syntax())
    }
    #[getter]
    fn names(&self, py: Python<'_>) -> PyResult<Vec<Py<PyToken>>> {
        self.nr.tokens(py, self.view().names())
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().colon())
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr
            .token_or_missing(py, self.view().type_annotation(), SyntaxKind::TYPE)
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr
            .block_or_missing(py, self.view().description(), SyntaxKind::DESCRIPTION)
    }
    fn __repr__(&self) -> String {
        format!(
            "NumPyAttribute({:?})",
            self.view().names().map(|n| n.text()).collect::<Vec<_>>().join(", ")
        )
    }
}

// ─── NumPyMethod ─────────────────────────────────────────────────────────────

lazy_node!(PyNumPyMethod, "NumPyMethod", nn::NumPyMethod);

#[pymethods]
impl PyNumPyMethod {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn name(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().name().syntax())
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> PyResult<Option<Py<PyToken>>> {
        self.nr.token_opt(py, self.view().colon())
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> PyResult<Option<Py<PyTextBlock>>> {
        self.nr
            .block_or_missing(py, self.view().description(), SyntaxKind::DESCRIPTION)
    }
    fn __repr__(&self) -> String {
        format!("NumPyMethod({:?})", self.view().name().text())
    }
}

// ─── NumPySection ────────────────────────────────────────────────────────────

lazy_node!(PyNumPySection, "NumPySection", nn::NumPySection);

#[pymethods]
impl PyNumPySection {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
    }
    #[getter]
    fn section_kind(&self) -> PyNumPySectionKind {
        numpy_section_kind_to_py(self.view().section_kind())
    }
    #[getter]
    fn header_name(&self, py: Python<'_>) -> PyResult<Py<PyToken>> {
        self.nr.token(py, self.view().header().name().syntax())
    }
    fn __repr__(&self) -> String {
        format!("NumPySection({:?})", self.view().header().name().text())
    }
}

// ─── NumPyDocstring ──────────────────────────────────────────────────────────

lazy_node!(PyNumPyDocstring, "NumPyDocstring", nn::NumPyDocstring);

#[pymethods]
impl PyNumPyDocstring {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
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
    fn deprecation(&self, py: Python<'_>) -> PyResult<Option<Py<PyNumPyDeprecation>>> {
        self.view()
            .deprecation()
            .map(|d| self.nr.wrap_child(py, d.syntax()))
            .transpose()
    }
    /// Stray-prose paragraph blocks between sections, in source order.
    #[getter]
    fn paragraphs(&self, py: Python<'_>) -> PyResult<Vec<Py<PyTextBlock>>> {
        self.nr.blocks(py, self.view().paragraphs())
    }
    #[getter]
    fn sections(&self, py: Python<'_>) -> PyResult<Vec<Py<PyNumPySection>>> {
        self.view()
            .sections()
            .map(|s| self.nr.wrap_child(py, s.syntax()))
            .collect()
    }
    #[getter]
    fn source(&self) -> &str {
        self.nr.parsed.source()
    }
    #[getter]
    fn style(&self) -> PyStyle {
        PyStyle::NumPy
    }
    fn pretty_print(&self) -> String {
        self.nr.parsed.pretty_print()
    }
    fn to_model(&self) -> PyResult<PyModelDocstring> {
        pydocstring_core::parse::numpy::to_model::to_model(&self.nr.parsed)
            .map(|doc| PyModelDocstring::try_from(&doc))
            .transpose()?
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("failed to convert to model"))
    }
    /// Replace every match of ``pattern`` (a NumPy-style pattern with
    /// ``$NAME`` / ``$$$NAME`` metavariables) with ``template``, returning the
    /// new source. Captured content is substituted byte-for-byte; everything
    /// else is preserved. Raises ``PatternError`` for an invalid pattern.
    fn replace(&self, pattern: &str, template: &str) -> PyResult<String> {
        rewrite_replace(&self.nr, CoreStyle::NumPy, pattern, template)
    }
    /// Find every match of ``pattern`` in document order (non-overlapping),
    /// returning a list of ``Match``. Raises ``PatternError`` for an invalid
    /// pattern.
    fn findall(&self, py: Python<'_>, pattern: &str) -> PyResult<Vec<Py<PyMatch>>> {
        rewrite_findall(py, &self.nr, CoreStyle::NumPy, pattern)
    }
    fn __repr__(&self) -> &'static str {
        "NumPyDocstring(...)"
    }
}

fn build_numpy_docstring(py: Python<'_>, parsed: Parsed) -> PyResult<Py<PyNumPyDocstring>> {
    let arc = Arc::new(parsed);
    nn::NumPyDocstring::cast(&arc, arc.root())
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("root is not a DOCUMENT node"))?;
    Py::new(py, PyNumPyDocstring::from(NodeRef::root(arc)))
}

// =============================================================================
// Plain docstring
// =============================================================================

lazy_node!(PyPlainDocstring, "PlainDocstring", pn::PlainDocstring);

#[pymethods]
impl PyPlainDocstring {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        self.nr.py_range(py)
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
    fn source(&self) -> &str {
        self.nr.parsed.source()
    }
    #[getter]
    fn style(&self) -> PyStyle {
        PyStyle::Plain
    }
    fn pretty_print(&self) -> String {
        self.nr.parsed.pretty_print()
    }
    fn to_model(&self) -> PyResult<PyModelDocstring> {
        pydocstring_core::parse::plain::to_model::to_model(&self.nr.parsed)
            .map(|doc| PyModelDocstring::try_from(&doc))
            .transpose()?
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("failed to convert to model"))
    }
    /// Replace every match of ``pattern`` (a plain-style pattern with
    /// ``$NAME`` / ``$$$NAME`` metavariables) with ``template``, returning the
    /// new source. Captured content is substituted byte-for-byte; everything
    /// else is preserved. Raises ``PatternError`` for an invalid pattern.
    fn replace(&self, pattern: &str, template: &str) -> PyResult<String> {
        rewrite_replace(&self.nr, CoreStyle::Plain, pattern, template)
    }
    /// Find every match of ``pattern`` in document order (non-overlapping),
    /// returning a list of ``Match``. Raises ``PatternError`` for an invalid
    /// pattern.
    fn findall(&self, py: Python<'_>, pattern: &str) -> PyResult<Vec<Py<PyMatch>>> {
        rewrite_findall(py, &self.nr, CoreStyle::Plain, pattern)
    }
    fn __repr__(&self) -> &'static str {
        "PlainDocstring(...)"
    }
}

fn build_plain_docstring(py: Python<'_>, parsed: Parsed) -> PyResult<Py<PyPlainDocstring>> {
    let arc = Arc::new(parsed);
    pn::PlainDocstring::cast(&arc, arc.root())
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("root is not a DOCUMENT node"))?;
    Py::new(py, PyPlainDocstring::from(NodeRef::root(arc)))
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
#[pyclass(name = "Directive")]
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

#[pyclass(name = "Parameter")]
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

#[pyclass(name = "Return")]
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

#[pyclass(name = "ExceptionEntry")]
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

#[pyclass(name = "SeeAlsoEntry")]
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

#[pyclass(name = "Reference")]
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

#[pyclass(name = "Attribute")]
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

#[pyclass(name = "Method")]
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

#[pyclass(from_py_object, eq, eq_int, frozen, hash, name = "SectionKind")]
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
#[pyclass(name = "Block")]
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
#[pyclass(name = "Section")]
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

    fn __repr__(&self) -> String {
        format!("Section(SectionKind.{})", py_section_kind_name(self.kind))
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
            Ok(model::Section { kind, blocks })
        })
    }
}

// ─── Model Docstring ─────────────────────────────────────────────────────────

#[pyclass(name = "Docstring")]
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
// ParsedDocstring — typed return value for parse()
// =============================================================================

/// The three possible return values of [`parse`].
///
/// Implementing [`pyo3::IntoPyObject`] lets `parse` return a concrete Rust
/// type while still handing Python a `GoogleDocstring`, `NumPyDocstring`, or
/// `PlainDocstring` object at runtime.
enum ParsedDocstring {
    Google(Py<PyGoogleDocstring>),
    NumPy(Py<PyNumPyDocstring>),
    Plain(Py<PyPlainDocstring>),
}

impl<'py> pyo3::IntoPyObject<'py> for ParsedDocstring {
    type Target = pyo3::types::PyAny;
    type Output = pyo3::Bound<'py, pyo3::types::PyAny>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        match self {
            ParsedDocstring::Google(d) => Ok(d.into_pyobject(py)?.into_any()),
            ParsedDocstring::NumPy(d) => Ok(d.into_pyobject(py)?.into_any()),
            ParsedDocstring::Plain(d) => Ok(d.into_pyobject(py)?.into_any()),
        }
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
#[pyclass(frozen, skip_from_py_object, name = "Capture")]
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
#[pyclass(frozen, skip_from_py_object, name = "Match")]
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

// =============================================================================
// Module functions
// =============================================================================

/// Parse a Google-style docstring.
#[pyfunction]
fn parse_google(py: Python<'_>, input: &str) -> PyResult<Py<PyGoogleDocstring>> {
    build_google_docstring(py, google::parse_google(input))
}

/// Parse a NumPy-style docstring.
#[pyfunction]
fn parse_numpy(py: Python<'_>, input: &str) -> PyResult<Py<PyNumPyDocstring>> {
    build_numpy_docstring(py, pydocstring_core::parse::numpy::parse_numpy(input))
}

/// Parse a plain docstring (no section markers).
#[pyfunction]
fn parse_plain(py: Python<'_>, input: &str) -> PyResult<Py<PyPlainDocstring>> {
    build_plain_docstring(py, pydocstring_core::parse::plain::parse_plain(input))
}

/// Auto-detect the docstring style and parse it.
///
/// Returns a `GoogleDocstring`, `NumPyDocstring`, or `PlainDocstring`.
/// Use `.style` on the result to distinguish them without `isinstance` checks.
#[pyfunction]
fn parse(py: Python<'_>, input: &str) -> PyResult<ParsedDocstring> {
    use pydocstring_core::parse::Style;
    let parsed = pydocstring_core::parse::parse(input);
    match parsed.style() {
        Style::Google => Ok(ParsedDocstring::Google(build_google_docstring(py, parsed)?)),
        Style::NumPy => Ok(ParsedDocstring::NumPy(build_numpy_docstring(py, parsed)?)),
        Style::Plain => Ok(ParsedDocstring::Plain(build_plain_docstring(py, parsed)?)),
        // `Style` is #[non_exhaustive]; surface future styles as Plain until
        // the Python surface grows a matching wrapper.
        _ => Ok(ParsedDocstring::Plain(build_plain_docstring(py, parsed)?)),
    }
}

/// Detect the docstring style without fully parsing.
#[pyfunction]
fn detect_style(input: &str) -> PyStyle {
    match pydocstring_core::parse::detect_style(input) {
        pydocstring_core::parse::Style::Google => PyStyle::Google,
        pydocstring_core::parse::Style::NumPy => PyStyle::NumPy,
        pydocstring_core::parse::Style::Plain => PyStyle::Plain,
        // `Style` is #[non_exhaustive]; surface future styles as PLAIN until
        // the Python surface grows a matching enum member.
        _ => PyStyle::Plain,
    }
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
// walk() — CST-direct Python dispatch
// =============================================================================

// ─── WalkContext ─────────────────────────────────────────────────────────────

/// Context passed to every ``enter_*` / `exit_*`` method during a ``walk()`` call.
///
/// Provides source-location helpers for the docstring currently being walked.
#[pyclass(frozen, skip_from_py_object, name = "WalkContext")]
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

/// Which `enter_*` / `exit_*` / `leave_*` methods the Python visitor defines.
///
/// Collected **once per `walk()` call** by inspecting the visitor object,
/// so `hasattr` is never called per-node.
struct ActiveMethods {
    // Google (enter)
    google_docstring: bool,
    google_directive: bool,
    google_deprecation: bool,
    google_section: bool,
    google_arg: bool,
    google_return: bool,
    google_yield: bool,
    google_exception: bool,
    google_warning: bool,
    google_see_also_item: bool,
    google_reference: bool,
    google_attribute: bool,
    google_method: bool,
    // Google (exit)
    exit_google_docstring: bool,
    exit_google_directive: bool,
    exit_google_deprecation: bool,
    exit_google_section: bool,
    exit_google_arg: bool,
    exit_google_return: bool,
    exit_google_yield: bool,
    exit_google_exception: bool,
    exit_google_warning: bool,
    exit_google_see_also_item: bool,
    exit_google_reference: bool,
    exit_google_attribute: bool,
    exit_google_method: bool,
    // NumPy (enter)
    numpy_docstring: bool,
    numpy_directive: bool,
    numpy_deprecation: bool,
    numpy_section: bool,
    numpy_parameter: bool,
    numpy_returns: bool,
    numpy_yields: bool,
    numpy_exception: bool,
    numpy_warning: bool,
    numpy_see_also_item: bool,
    numpy_reference: bool,
    numpy_attribute: bool,
    numpy_method: bool,
    // NumPy (exit)
    exit_numpy_docstring: bool,
    exit_numpy_directive: bool,
    exit_numpy_deprecation: bool,
    exit_numpy_section: bool,
    exit_numpy_parameter: bool,
    exit_numpy_returns: bool,
    exit_numpy_yields: bool,
    exit_numpy_exception: bool,
    exit_numpy_warning: bool,
    exit_numpy_see_also_item: bool,
    exit_numpy_reference: bool,
    exit_numpy_attribute: bool,
    exit_numpy_method: bool,
    // Plain
    plain_docstring: bool,
    exit_plain_docstring: bool,
}

/// Inspect `visitor` once and return which `enter_*` / `exit_*` methods it defines.
///
/// Fast path: if the visitor has `__pydocstring_active__` (set by `Visitor.__init__`),
/// extract the frozenset to a Rust `HashSet` in one PyO3 call, then do pure-Rust
/// membership tests — no further Python attribute lookups.
fn collect_active(py: Python<'_>, visitor: &Py<PyAny>) -> PyResult<ActiveMethods> {
    let b = visitor.bind(py);

    let attr = b
        .getattr("__pydocstring_active__")
        .map_err(|_| pyo3::exceptions::PyTypeError::new_err("visitor must subclass pydocstring.Visitor"))?;
    // One extraction converts the Python frozenset into a Rust HashSet.
    let active: std::collections::HashSet<String> = attr.extract()?;
    let has = |name: &str| active.contains(name);
    Ok(ActiveMethods {
        // Google (enter)
        google_docstring: has("enter_google_docstring"),
        google_directive: has("enter_google_directive"),
        google_deprecation: has("enter_google_deprecation"),
        google_section: has("enter_google_section"),
        google_arg: has("enter_google_arg"),
        google_return: has("enter_google_return"),
        google_yield: has("enter_google_yield"),
        google_exception: has("enter_google_exception"),
        google_warning: has("enter_google_warning"),
        google_see_also_item: has("enter_google_see_also_item"),
        google_reference: has("enter_google_reference"),
        google_attribute: has("enter_google_attribute"),
        google_method: has("enter_google_method"),
        // Google (exit)
        exit_google_docstring: has("exit_google_docstring"),
        exit_google_directive: has("exit_google_directive"),
        exit_google_deprecation: has("exit_google_deprecation"),
        exit_google_section: has("exit_google_section"),
        exit_google_arg: has("exit_google_arg"),
        exit_google_return: has("exit_google_return"),
        exit_google_yield: has("exit_google_yield"),
        exit_google_exception: has("exit_google_exception"),
        exit_google_warning: has("exit_google_warning"),
        exit_google_see_also_item: has("exit_google_see_also_item"),
        exit_google_reference: has("exit_google_reference"),
        exit_google_attribute: has("exit_google_attribute"),
        exit_google_method: has("exit_google_method"),
        // NumPy (enter)
        numpy_docstring: has("enter_numpy_docstring"),
        numpy_directive: has("enter_numpy_directive"),
        numpy_deprecation: has("enter_numpy_deprecation"),
        numpy_section: has("enter_numpy_section"),
        numpy_parameter: has("enter_numpy_parameter"),
        numpy_returns: has("enter_numpy_returns"),
        numpy_yields: has("enter_numpy_yields"),
        numpy_exception: has("enter_numpy_exception"),
        numpy_warning: has("enter_numpy_warning"),
        numpy_see_also_item: has("enter_numpy_see_also_item"),
        numpy_reference: has("enter_numpy_reference"),
        numpy_attribute: has("enter_numpy_attribute"),
        numpy_method: has("enter_numpy_method"),
        // NumPy (exit)
        exit_numpy_docstring: has("exit_numpy_docstring"),
        exit_numpy_directive: has("exit_numpy_directive"),
        exit_numpy_deprecation: has("exit_numpy_deprecation"),
        exit_numpy_section: has("exit_numpy_section"),
        exit_numpy_parameter: has("exit_numpy_parameter"),
        exit_numpy_returns: has("exit_numpy_returns"),
        exit_numpy_yields: has("exit_numpy_yields"),
        exit_numpy_exception: has("exit_numpy_exception"),
        exit_numpy_warning: has("exit_numpy_warning"),
        exit_numpy_see_also_item: has("exit_numpy_see_also_item"),
        exit_numpy_reference: has("exit_numpy_reference"),
        exit_numpy_attribute: has("exit_numpy_attribute"),
        exit_numpy_method: has("exit_numpy_method"),
        // Plain
        plain_docstring: has("enter_plain_docstring"),
        exit_plain_docstring: has("exit_plain_docstring"),
    })
}

/// Call `visitor.method(arg, ctx)`.  The caller has already confirmed the method exists.
#[inline]
fn dispatch_with_ctx<T: pyo3::PyClass>(
    py: Python<'_>,
    visitor: &Py<PyAny>,
    method: &str,
    arg: Py<T>,
    ctx: &Py<PyWalkContext>,
) -> PyResult<()> {
    visitor.bind(py).call_method1(method, (arg.bind(py), ctx.bind(py)))?;
    Ok(())
}

// =============================================================================
// PyDispatcher — ANTLR-style Python dispatch via DocstringVisitor
// =============================================================================

/// Implements `DocstringVisitor` from the core crate.
///
/// For every node kind the pattern is:
/// 1. Call Python `enter_*` / `exit_*` (enter) if the visitor defines it.
/// 2. Recurse into children via `core_walk`.
/// 3. Call Python `leave_*` (exit) if the visitor defines it.
struct PyDispatcher<'py> {
    py: Python<'py>,
    arc: Arc<Parsed>,
    visitor: Py<PyAny>,
    active: ActiveMethods,
    ctx: Py<PyWalkContext>,
}

/// Generates a `DocstringVisitor` method body for `PyDispatcher`.
///
/// Variant with children:
///   `visit_node!(self, parsed, ENTER_FIELD, EXIT_FIELD, build_expr, syntax_expr)`
///
/// Variant without children (Plain):
///   `visit_node!(self, parsed, ENTER_FIELD, EXIT_FIELD, build_expr)`
///
/// The method name strings are derived automatically via `concat!` / `stringify!`.
macro_rules! visit_node {
    // ── with children ────────────────────────────────────────────────────
    ($self:ident, $parsed:expr, $enter:ident, $exit:ident, $build:expr, $syntax:expr) => {{
        let need = $self.active.$enter || $self.active.$exit;
        let obj: Option<_> = if need { Some($build?) } else { None };
        if $self.active.$enter {
            if let Some(ref o) = obj {
                dispatch_with_ctx(
                    $self.py,
                    &$self.visitor,
                    concat!("enter_", stringify!($enter)),
                    o.clone_ref($self.py),
                    &$self.ctx,
                )?;
            }
        }
        walk_children($parsed, $syntax, $self)?;
        if $self.active.$exit {
            if let Some(ref o) = obj {
                dispatch_with_ctx(
                    $self.py,
                    &$self.visitor,
                    concat!("exit_", stringify!($enter)),
                    o.clone_ref($self.py),
                    &$self.ctx,
                )?;
            }
        }
        Ok(())
    }};
    // ── without children (Plain) ─────────────────────────────────────────
    ($self:ident, $parsed:expr, $enter:ident, $exit:ident, $build:expr) => {{
        let need = $self.active.$enter || $self.active.$exit;
        let obj: Option<_> = if need { Some($build?) } else { None };
        if $self.active.$enter {
            if let Some(ref o) = obj {
                dispatch_with_ctx(
                    $self.py,
                    &$self.visitor,
                    concat!("enter_", stringify!($enter)),
                    o.clone_ref($self.py),
                    &$self.ctx,
                )?;
            }
        }
        if $self.active.$exit {
            if let Some(ref o) = obj {
                dispatch_with_ctx(
                    $self.py,
                    &$self.visitor,
                    concat!("exit_", stringify!($enter)),
                    o.clone_ref($self.py),
                    &$self.ctx,
                )?;
            }
        }
        Ok(())
    }};
}

impl PyDispatcher<'_> {
    /// Wrap `node` (from `self.arc`'s tree) in its lazy pyclass wrapper.
    ///
    /// Construction is cheap — a child-index path — so `walk()` can build
    /// wrappers during traversal without materializing any content.
    fn wrap<T>(&self, node: &SyntaxNode) -> PyResult<Py<T>>
    where
        T: PyClass + From<NodeRef> + Into<pyo3::PyClassInitializer<T>>,
    {
        Py::new(self.py, T::from(NodeRef::for_node(&self.arc, node)))
    }
}

impl<'py> DocstringVisitor for PyDispatcher<'py> {
    type Error = PyErr;

    // ── Google ────────────────────────────────────────────────────────────
    fn visit_google_docstring(&mut self, parsed: &Parsed, doc: &gn::GoogleDocstring<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            google_docstring,
            exit_google_docstring,
            self.wrap::<PyGoogleDocstring>(doc.syntax()),
            doc.syntax()
        )
    }

    fn visit_google_directive(&mut self, parsed: &Parsed, dir: &gn::GoogleDirective<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            google_directive,
            exit_google_directive,
            self.wrap::<PyGoogleDirective>(dir.syntax()),
            dir.syntax()
        )
    }

    fn visit_google_deprecation(&mut self, parsed: &Parsed, dep: &gn::GoogleDeprecation<'_>) -> Result<(), PyErr> {
        // Notification specialization: the generic directive hook already
        // descended the body, so fire the deprecation hooks WITHOUT walking
        // children again (no-children macro arm) — mirrors the core contract.
        visit_node!(
            self,
            parsed,
            google_deprecation,
            exit_google_deprecation,
            self.wrap::<PyGoogleDeprecation>(dep.syntax())
        )
    }

    fn visit_google_section(&mut self, parsed: &Parsed, sec: &gn::GoogleSection<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            google_section,
            exit_google_section,
            self.wrap::<PyGoogleSection>(sec.syntax()),
            sec.syntax()
        )
    }

    fn visit_google_arg(&mut self, parsed: &Parsed, arg: &gn::GoogleArg<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            google_arg,
            exit_google_arg,
            self.wrap::<PyGoogleArg>(arg.syntax()),
            arg.syntax()
        )
    }

    fn visit_google_return(&mut self, parsed: &Parsed, rtn: &gn::GoogleReturn<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            google_return,
            exit_google_return,
            self.wrap::<PyGoogleReturn>(rtn.syntax()),
            rtn.syntax()
        )
    }

    fn visit_google_yield(&mut self, parsed: &Parsed, yld: &gn::GoogleYield<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            google_yield,
            exit_google_yield,
            self.wrap::<PyGoogleYield>(yld.syntax()),
            yld.syntax()
        )
    }

    fn visit_google_exception(&mut self, parsed: &Parsed, exc: &gn::GoogleException<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            google_exception,
            exit_google_exception,
            self.wrap::<PyGoogleException>(exc.syntax()),
            exc.syntax()
        )
    }

    fn visit_google_warning(&mut self, parsed: &Parsed, wrn: &gn::GoogleWarning<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            google_warning,
            exit_google_warning,
            self.wrap::<PyGoogleWarning>(wrn.syntax()),
            wrn.syntax()
        )
    }

    fn visit_google_see_also_item(&mut self, parsed: &Parsed, sai: &gn::GoogleSeeAlsoItem<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            google_see_also_item,
            exit_google_see_also_item,
            self.wrap::<PyGoogleSeeAlsoItem>(sai.syntax()),
            sai.syntax()
        )
    }

    fn visit_google_reference(&mut self, parsed: &Parsed, r#ref: &gn::GoogleReference<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            google_reference,
            exit_google_reference,
            self.wrap::<PyGoogleReference>(r#ref.syntax()),
            r#ref.syntax()
        )
    }

    fn visit_google_attribute(&mut self, parsed: &Parsed, att: &gn::GoogleAttribute<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            google_attribute,
            exit_google_attribute,
            self.wrap::<PyGoogleAttribute>(att.syntax()),
            att.syntax()
        )
    }

    fn visit_google_method(&mut self, parsed: &Parsed, mtd: &gn::GoogleMethod<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            google_method,
            exit_google_method,
            self.wrap::<PyGoogleMethod>(mtd.syntax()),
            mtd.syntax()
        )
    }

    // ── NumPy ─────────────────────────────────────────────────────────────
    fn visit_numpy_docstring(&mut self, parsed: &Parsed, doc: &nn::NumPyDocstring<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            numpy_docstring,
            exit_numpy_docstring,
            self.wrap::<PyNumPyDocstring>(doc.syntax()),
            doc.syntax()
        )
    }

    fn visit_numpy_directive(&mut self, parsed: &Parsed, dir: &nn::NumPyDirective<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            numpy_directive,
            exit_numpy_directive,
            self.wrap::<PyNumPyDirective>(dir.syntax()),
            dir.syntax()
        )
    }

    fn visit_numpy_deprecation(&mut self, parsed: &Parsed, dep: &nn::NumPyDeprecation<'_>) -> Result<(), PyErr> {
        // Notification specialization: the generic directive hook already
        // descended the body, so fire the deprecation hooks WITHOUT walking
        // children again (no-children macro arm) — mirrors the core contract.
        visit_node!(
            self,
            parsed,
            numpy_deprecation,
            exit_numpy_deprecation,
            self.wrap::<PyNumPyDeprecation>(dep.syntax())
        )
    }

    fn visit_numpy_section(&mut self, parsed: &Parsed, sec: &nn::NumPySection<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            numpy_section,
            exit_numpy_section,
            self.wrap::<PyNumPySection>(sec.syntax()),
            sec.syntax()
        )
    }

    fn visit_numpy_parameter(&mut self, parsed: &Parsed, prm: &nn::NumPyParameter<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            numpy_parameter,
            exit_numpy_parameter,
            self.wrap::<PyNumPyParameter>(prm.syntax()),
            prm.syntax()
        )
    }

    fn visit_numpy_returns(&mut self, parsed: &Parsed, rtn: &nn::NumPyReturns<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            numpy_returns,
            exit_numpy_returns,
            self.wrap::<PyNumPyReturns>(rtn.syntax()),
            rtn.syntax()
        )
    }

    fn visit_numpy_yields(&mut self, parsed: &Parsed, yld: &nn::NumPyYields<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            numpy_yields,
            exit_numpy_yields,
            self.wrap::<PyNumPyYields>(yld.syntax()),
            yld.syntax()
        )
    }

    fn visit_numpy_exception(&mut self, parsed: &Parsed, exc: &nn::NumPyException<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            numpy_exception,
            exit_numpy_exception,
            self.wrap::<PyNumPyException>(exc.syntax()),
            exc.syntax()
        )
    }

    fn visit_numpy_warning(&mut self, parsed: &Parsed, wrn: &nn::NumPyWarning<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            numpy_warning,
            exit_numpy_warning,
            self.wrap::<PyNumPyWarning>(wrn.syntax()),
            wrn.syntax()
        )
    }

    fn visit_numpy_see_also_item(&mut self, parsed: &Parsed, sai: &nn::NumPySeeAlsoItem<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            numpy_see_also_item,
            exit_numpy_see_also_item,
            self.wrap::<PyNumPySeeAlsoItem>(sai.syntax()),
            sai.syntax()
        )
    }

    fn visit_numpy_reference(&mut self, parsed: &Parsed, r#ref: &nn::NumPyReference<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            numpy_reference,
            exit_numpy_reference,
            self.wrap::<PyNumPyReference>(r#ref.syntax()),
            r#ref.syntax()
        )
    }

    fn visit_numpy_attribute(&mut self, parsed: &Parsed, att: &nn::NumPyAttribute<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            numpy_attribute,
            exit_numpy_attribute,
            self.wrap::<PyNumPyAttribute>(att.syntax()),
            att.syntax()
        )
    }

    fn visit_numpy_method(&mut self, parsed: &Parsed, mtd: &nn::NumPyMethod<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            parsed,
            numpy_method,
            exit_numpy_method,
            self.wrap::<PyNumPyMethod>(mtd.syntax()),
            mtd.syntax()
        )
    }

    // ── Plain ─────────────────────────────────────────────────────────────
    fn visit_plain_docstring(&mut self, _parsed: &Parsed, doc: &pn::PlainDocstring<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            _parsed,
            plain_docstring,
            exit_plain_docstring,
            self.wrap::<PyPlainDocstring>(doc.syntax())
        )
    }
}

/// Walk any docstring depth-first, calling typed methods on ``visitor`` for each node.
///
/// Accepts a `GoogleDocstring`, `NumPyDocstring`, or `PlainDocstring`.
/// The visitor defines only the methods it needs; all others are silently skipped.
/// Returns ``visitor`` so results can be collected inline.
///
/// Every ``enter_*` / `exit_*`` method receives ``(node, ctx: WalkContext)`` where
/// ``ctx.line_col(offset)`` converts byte offsets to line/column positions.
///
/// ```python
/// class TypeAnnotationChecker(pydocstring.Visitor):
///     def enter_google_arg(self, arg, ctx): ...
///     def enter_numpy_parameter(self, param, ctx): ...
///
/// checker = TypeAnnotationChecker()
/// for docstring_text in all_docstrings:
///     doc = pydocstring.parse(docstring_text)  # auto-detects style
///     pydocstring.walk(doc, checker)           # returns the visitor
/// ```
///
/// Google `enter_*` / `exit_*` methods:
/// `enter_google_docstring`, `enter_google_section`, `enter_google_directive`,
/// `enter_google_deprecation`,
/// `enter_google_arg`, `enter_google_return`, `enter_google_yield`,
/// `enter_google_exception`, `enter_google_warning`,
/// `enter_google_see_also_item`, `enter_google_reference`,
/// `enter_google_attribute`, `enter_google_method`
///
/// NumPy `enter_*` / `exit_*` methods:
/// `enter_numpy_docstring`, `enter_numpy_section`, `enter_numpy_directive`,
/// `enter_numpy_deprecation`,
/// `enter_numpy_parameter`, `enter_numpy_returns`, `enter_numpy_yields`,
/// `enter_numpy_exception`, `enter_numpy_warning`, `enter_numpy_see_also_item`,
/// `enter_numpy_reference`, `enter_numpy_attribute`, `enter_numpy_method`
///
/// Plain `enter_*` / `exit_*` methods:
/// `enter_plain_docstring`
#[pyfunction]
fn walk(py: Python<'_>, doc: Py<PyAny>, visitor: Py<PyAny>) -> PyResult<Py<PyAny>> {
    let bound = doc.bind(py);
    let active = collect_active(py, &visitor)?;

    let arc = if let Ok(d) = bound.cast::<PyGoogleDocstring>() {
        d.borrow().nr.parsed.clone()
    } else if let Ok(d) = bound.cast::<PyNumPyDocstring>() {
        d.borrow().nr.parsed.clone()
    } else if let Ok(d) = bound.cast::<PyPlainDocstring>() {
        d.borrow().nr.parsed.clone()
    } else {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "expected GoogleDocstring, NumPyDocstring, or PlainDocstring",
        ));
    };

    let source = arc.source().to_string();
    let root = arc.root();
    let line_starts = build_line_starts(&source);
    let ctx = Py::new(
        py,
        PyWalkContext {
            source: source.clone(),
            line_starts,
        },
    )?;

    let mut dispatcher = PyDispatcher {
        py,
        arc: Arc::clone(&arc),
        visitor: visitor.clone_ref(py),
        active,
        ctx,
    };

    core_walk(&arc, root, &mut dispatcher)?;

    Ok(visitor)
}

// =============================================================================
// Module
// =============================================================================

/// `Visitor` is defined in `python/pydocstring/_visitor.py`.
/// `collect_active` reads `__pydocstring_active__` (a frozenset set by
/// `Visitor.__init_subclass__`) once via a single PyO3 `extract` call
/// and builds a pure-Rust `ActiveMethods` struct.

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
    m.add_class::<PyGoogleSectionKind>()?;
    m.add_class::<PyNumPySectionKind>()?;
    m.add_class::<PyTextRange>()?;
    m.add_class::<PyLineColumn>()?;
    m.add_class::<PyToken>()?;
    m.add_class::<PyTextBlock>()?;
    m.add_class::<PyWalkContext>()?;
    // Pattern matching & rewriting (#47)
    m.add_class::<PyMatch>()?;
    m.add_class::<PyCapture>()?;
    m.add("PatternError", m.py().get_type::<PatternError>())?;
    // Google CST wrappers
    m.add_class::<PyGoogleDocstring>()?;
    m.add_class::<PyGoogleSection>()?;
    m.add_class::<PyGoogleDeprecation>()?;
    m.add_class::<PyGoogleDirective>()?;
    m.add_class::<PyGoogleArg>()?;
    m.add_class::<PyGoogleReturn>()?;
    m.add_class::<PyGoogleYield>()?;
    m.add_class::<PyGoogleException>()?;
    m.add_class::<PyGoogleWarning>()?;
    m.add_class::<PyGoogleSeeAlsoItem>()?;
    m.add_class::<PyGoogleReference>()?;
    m.add_class::<PyGoogleAttribute>()?;
    m.add_class::<PyGoogleMethod>()?;
    // NumPy CST wrappers
    m.add_class::<PyNumPyDocstring>()?;
    m.add_class::<PyNumPySection>()?;
    m.add_class::<PyNumPyDeprecation>()?;
    m.add_class::<PyNumPyDirective>()?;
    m.add_class::<PyNumPyParameter>()?;
    m.add_class::<PyNumPyReturns>()?;
    m.add_class::<PyNumPyYields>()?;
    m.add_class::<PyNumPyException>()?;
    m.add_class::<PyNumPyWarning>()?;
    m.add_class::<PyNumPySeeAlsoItem>()?;
    m.add_class::<PyNumPyReference>()?;
    m.add_class::<PyNumPyAttribute>()?;
    m.add_class::<PyNumPyMethod>()?;
    // Plain CST wrapper
    m.add_class::<PyPlainDocstring>()?;
    // Model IR
    m.add_class::<PySectionKind>()?;
    m.add_class::<PyModelDocstring>()?;
    m.add_class::<PyModelSection>()?;
    m.add_class::<PyModelBlock>()?;
    m.add_class::<PyModelParameter>()?;
    m.add_class::<PyModelReturn>()?;
    m.add_class::<PyModelExceptionEntry>()?;
    m.add_class::<PyModelSeeAlsoEntry>()?;
    m.add_class::<PyModelReference>()?;
    m.add_class::<PyModelAttribute>()?;
    m.add_class::<PyModelMethod>()?;
    m.add_class::<PyModelDirective>()?;
    Ok(())
}
