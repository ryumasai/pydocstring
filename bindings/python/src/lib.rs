use pyo3::PyClass;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyString};

use pydocstring_core::model;
use pydocstring_core::parse::google;
use pydocstring_core::parse::google::kind::GoogleSectionKind;
use pydocstring_core::parse::google::nodes as gn;
use pydocstring_core::parse::numpy::kind::NumPySectionKind;
use pydocstring_core::parse::numpy::nodes as nn;
use pydocstring_core::parse::plain::nodes as pn;
use pydocstring_core::parse::visitor::{DocstringVisitor, walk as core_walk};
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

// ─── Token ──────────────────────────────────────────────────────────────────

/// A typed token: a text fragment plus its byte range in the source.
///
/// The field name on the parent object (e.g. `.name`, `.description`) implies
/// the semantic kind; no redundant `kind` field is exposed.
#[pyclass(frozen, skip_from_py_object, name = "Token")]
struct PyToken {
    text: String,
    range: TextRange,
}

#[pymethods]
impl PyToken {
    #[getter]
    fn text(&self) -> &str {
        &self.text
    }
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    /// Whether this token is a zero-length placeholder inserted by the parser
    /// to represent a syntactically missing element.
    ///
    /// For example, ``arg (int)`` without a closing ``)`` produces a missing
    /// CLOSE_BRACKET token; ``arg ():`` produces a missing TYPE token.
    /// Equivalent to ``token.range.is_empty()``.
    fn is_missing(&self) -> bool {
        self.range.is_empty()
    }
    fn __eq__(&self, other: &PyToken) -> bool {
        self.text == other.text && self.range == other.range
    }
    fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.text.hash(&mut hasher);
        self.range.start().raw().hash(&mut hasher);
        self.range.end().raw().hash(&mut hasher);
        hasher.finish()
    }
    fn __repr__(&self) -> String {
        format!("Token({:?})", self.text)
    }
}

// ─── Token helpers ───────────────────────────────────────────────────────────

fn mk_token(py: Python<'_>, token: &SyntaxToken, source: &str) -> PyResult<Py<PyToken>> {
    Py::new(
        py,
        PyToken {
            text: token.text(source).to_string(),
            range: *token.range(),
        },
    )
}

fn mk_token_opt(py: Python<'_>, token: Option<&SyntaxToken>, source: &str) -> PyResult<Option<Py<PyToken>>> {
    token.map(|t| mk_token(py, t, source)).transpose()
}

fn mk_token_or_missing(
    py: Python<'_>,
    present: Option<&SyntaxToken>,
    node: &SyntaxNode,
    kind: SyntaxKind,
    source: &str,
) -> PyResult<Option<Py<PyToken>>> {
    match present {
        Some(t) => Ok(Some(mk_token(py, t, source)?)),
        None => mk_token_opt(py, node.find_missing(kind), source),
    }
}

fn mk_tokens<'a>(
    py: Python<'_>,
    tokens: impl Iterator<Item = &'a SyntaxToken>,
    source: &str,
) -> PyResult<Vec<Py<PyToken>>> {
    tokens.map(|t| mk_token(py, t, source)).collect()
}

// ─── Style ──────────────────────────────────────────────────────────────────

#[pyclass(eq, eq_int, frozen, skip_from_py_object, name = "Style")]
#[derive(Clone, PartialEq)]
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
                Self::Raises => "RAISES",
                Self::Warns => "WARNS",
                Self::Warnings => "WARNINGS",
                Self::SeeAlso => "SEE_ALSO",
                Self::Notes => "NOTES",
                Self::References => "REFERENCES",
                Self::Examples => "EXAMPLES",
                Self::Attributes => "ATTRIBUTES",
                Self::Methods => "METHODS",
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
        NumPySectionKind::Raises => PyNumPySectionKind::Raises,
        NumPySectionKind::Warns => PyNumPySectionKind::Warns,
        NumPySectionKind::Warnings => PyNumPySectionKind::Warnings,
        NumPySectionKind::SeeAlso => PyNumPySectionKind::SeeAlso,
        NumPySectionKind::Notes => PyNumPySectionKind::Notes,
        NumPySectionKind::References => PyNumPySectionKind::References,
        NumPySectionKind::Examples => PyNumPySectionKind::Examples,
        NumPySectionKind::Attributes => PyNumPySectionKind::Attributes,
        NumPySectionKind::Methods => PyNumPySectionKind::Methods,
        NumPySectionKind::Unknown => PyNumPySectionKind::Unknown,
    }
}

// =============================================================================
// Google typed wrappers
// =============================================================================

// ─── GoogleArg ───────────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "GoogleArg")]
struct PyGoogleArg {
    range: TextRange,
    name: Py<PyToken>,
    open_bracket: Option<Py<PyToken>>,
    r#type: Option<Py<PyToken>>,
    close_bracket: Option<Py<PyToken>>,
    colon: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
    optional: Option<Py<PyToken>>,
}

#[pymethods]
impl PyGoogleArg {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn name(&self, py: Python<'_>) -> Py<PyToken> {
        self.name.clone_ref(py)
    }
    #[getter]
    fn open_bracket(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.open_bracket.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.r#type.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn close_bracket(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.close_bracket.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn optional(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.optional.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        format!("GoogleArg({:?})", self.name.borrow(py).text)
    }
}

fn build_google_arg(py: Python<'_>, arg: &gn::GoogleArg<'_>, source: &str) -> PyResult<Py<PyGoogleArg>> {
    Py::new(
        py,
        PyGoogleArg {
            range: *arg.syntax().range(),
            name: mk_token(py, arg.name(), source)?,
            open_bracket: mk_token_opt(py, arg.open_bracket(), source)?,
            r#type: mk_token_or_missing(py, arg.r#type(), arg.syntax(), SyntaxKind::TYPE, source)?,
            close_bracket: mk_token_or_missing(
                py,
                arg.close_bracket(),
                arg.syntax(),
                SyntaxKind::CLOSE_BRACKET,
                source,
            )?,
            colon: mk_token_or_missing(py, arg.colon(), arg.syntax(), SyntaxKind::COLON, source)?,
            description: mk_token_or_missing(py, arg.description(), arg.syntax(), SyntaxKind::DESCRIPTION, source)?,
            optional: mk_token_opt(py, arg.optional(), source)?,
        },
    )
}

// ─── GoogleReturn ────────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "GoogleReturn")]
struct PyGoogleReturn {
    range: TextRange,
    return_type: Option<Py<PyToken>>,
    colon: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyGoogleReturn {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn return_type(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.return_type.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self) -> &'static str {
        "GoogleReturn(...)"
    }
}

fn build_google_return(py: Python<'_>, rtn: &gn::GoogleReturn<'_>, source: &str) -> PyResult<Py<PyGoogleReturn>> {
    Py::new(
        py,
        PyGoogleReturn {
            range: *rtn.syntax().range(),
            return_type: mk_token_opt(py, rtn.return_type(), source)?,
            colon: mk_token_opt(py, rtn.colon(), source)?,
            description: mk_token_opt(py, rtn.description(), source)?,
        },
    )
}

// ─── GoogleYield ─────────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "GoogleYield")]
struct PyGoogleYield {
    range: TextRange,
    return_type: Option<Py<PyToken>>,
    colon: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyGoogleYield {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn return_type(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.return_type.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self) -> &'static str {
        "GoogleYield(...)"
    }
}

fn build_google_yield(py: Python<'_>, yld: &gn::GoogleYield<'_>, source: &str) -> PyResult<Py<PyGoogleYield>> {
    Py::new(
        py,
        PyGoogleYield {
            range: *yld.syntax().range(),
            return_type: mk_token_opt(py, yld.return_type(), source)?,
            colon: mk_token_opt(py, yld.colon(), source)?,
            description: mk_token_opt(py, yld.description(), source)?,
        },
    )
}

// ─── GoogleException ─────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "GoogleException")]
struct PyGoogleException {
    range: TextRange,
    r#type: Py<PyToken>,
    colon: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyGoogleException {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> Py<PyToken> {
        self.r#type.clone_ref(py)
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        format!("GoogleException({:?})", self.r#type.borrow(py).text)
    }
}

fn build_google_exception(
    py: Python<'_>,
    exc: &gn::GoogleException<'_>,
    source: &str,
) -> PyResult<Py<PyGoogleException>> {
    Py::new(
        py,
        PyGoogleException {
            range: *exc.syntax().range(),
            r#type: mk_token(py, exc.r#type(), source)?,
            colon: mk_token_opt(py, exc.colon(), source)?,
            description: mk_token_or_missing(py, exc.description(), exc.syntax(), SyntaxKind::DESCRIPTION, source)?,
        },
    )
}

// ─── GoogleWarning ───────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "GoogleWarning")]
struct PyGoogleWarning {
    range: TextRange,
    warning_type: Py<PyToken>,
    colon: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyGoogleWarning {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn warning_type(&self, py: Python<'_>) -> Py<PyToken> {
        self.warning_type.clone_ref(py)
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        format!("GoogleWarning({:?})", self.warning_type.borrow(py).text)
    }
}

fn build_google_warning(py: Python<'_>, wrn: &gn::GoogleWarning<'_>, source: &str) -> PyResult<Py<PyGoogleWarning>> {
    Py::new(
        py,
        PyGoogleWarning {
            range: *wrn.syntax().range(),
            warning_type: mk_token(py, wrn.warning_type(), source)?,
            colon: mk_token_opt(py, wrn.colon(), source)?,
            description: mk_token_or_missing(py, wrn.description(), wrn.syntax(), SyntaxKind::DESCRIPTION, source)?,
        },
    )
}

// ─── GoogleSeeAlsoItem ───────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "GoogleSeeAlsoItem")]
struct PyGoogleSeeAlsoItem {
    range: TextRange,
    names: Vec<Py<PyToken>>,
    colon: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyGoogleSeeAlsoItem {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn names(&self, py: Python<'_>) -> Vec<Py<PyToken>> {
        self.names.iter().map(|n| n.clone_ref(py)).collect()
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self) -> &'static str {
        "GoogleSeeAlsoItem(...)"
    }
}

fn build_google_see_also_item(
    py: Python<'_>,
    sai: &gn::GoogleSeeAlsoItem<'_>,
    source: &str,
) -> PyResult<Py<PyGoogleSeeAlsoItem>> {
    Py::new(
        py,
        PyGoogleSeeAlsoItem {
            range: *sai.syntax().range(),
            names: mk_tokens(py, sai.names(), source)?,
            colon: mk_token_opt(py, sai.colon(), source)?,
            description: mk_token_or_missing(py, sai.description(), sai.syntax(), SyntaxKind::DESCRIPTION, source)?,
        },
    )
}

// ─── GoogleAttribute ─────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "GoogleAttribute")]
struct PyGoogleAttribute {
    range: TextRange,
    name: Py<PyToken>,
    open_bracket: Option<Py<PyToken>>,
    r#type: Option<Py<PyToken>>,
    close_bracket: Option<Py<PyToken>>,
    colon: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyGoogleAttribute {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn name(&self, py: Python<'_>) -> Py<PyToken> {
        self.name.clone_ref(py)
    }
    #[getter]
    fn open_bracket(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.open_bracket.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.r#type.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn close_bracket(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.close_bracket.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        format!("GoogleAttribute({:?})", self.name.borrow(py).text)
    }
}

fn build_google_attribute(
    py: Python<'_>,
    att: &gn::GoogleAttribute<'_>,
    source: &str,
) -> PyResult<Py<PyGoogleAttribute>> {
    Py::new(
        py,
        PyGoogleAttribute {
            range: *att.syntax().range(),
            name: mk_token(py, att.name(), source)?,
            open_bracket: mk_token_opt(py, att.open_bracket(), source)?,
            r#type: mk_token_or_missing(py, att.r#type(), att.syntax(), SyntaxKind::TYPE, source)?,
            close_bracket: mk_token_or_missing(
                py,
                att.close_bracket(),
                att.syntax(),
                SyntaxKind::CLOSE_BRACKET,
                source,
            )?,
            colon: mk_token_or_missing(py, att.colon(), att.syntax(), SyntaxKind::COLON, source)?,
            description: mk_token_or_missing(py, att.description(), att.syntax(), SyntaxKind::DESCRIPTION, source)?,
        },
    )
}

// ─── GoogleMethod ────────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "GoogleMethod")]
struct PyGoogleMethod {
    range: TextRange,
    name: Py<PyToken>,
    open_bracket: Option<Py<PyToken>>,
    r#type: Option<Py<PyToken>>,
    close_bracket: Option<Py<PyToken>>,
    colon: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyGoogleMethod {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn name(&self, py: Python<'_>) -> Py<PyToken> {
        self.name.clone_ref(py)
    }
    #[getter]
    fn open_bracket(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.open_bracket.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.r#type.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn close_bracket(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.close_bracket.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        format!("GoogleMethod({:?})", self.name.borrow(py).text)
    }
}

fn build_google_method(py: Python<'_>, mtd: &gn::GoogleMethod<'_>, source: &str) -> PyResult<Py<PyGoogleMethod>> {
    Py::new(
        py,
        PyGoogleMethod {
            range: *mtd.syntax().range(),
            name: mk_token(py, mtd.name(), source)?,
            open_bracket: mk_token_opt(py, mtd.open_bracket(), source)?,
            r#type: mk_token_or_missing(py, mtd.r#type(), mtd.syntax(), SyntaxKind::TYPE, source)?,
            close_bracket: mk_token_or_missing(
                py,
                mtd.close_bracket(),
                mtd.syntax(),
                SyntaxKind::CLOSE_BRACKET,
                source,
            )?,
            colon: mk_token_or_missing(py, mtd.colon(), mtd.syntax(), SyntaxKind::COLON, source)?,
            description: mk_token_or_missing(py, mtd.description(), mtd.syntax(), SyntaxKind::DESCRIPTION, source)?,
        },
    )
}

// ─── GoogleSection ───────────────────────────────────────────────────────────

/// A thin wrapper for a Google section node (no eager child allocation).
#[pyclass(frozen, skip_from_py_object, name = "GoogleSection")]
struct PyGoogleSection {
    range: TextRange,
    section_kind: PyGoogleSectionKind,
    header_name: Py<PyToken>,
}

#[pymethods]
impl PyGoogleSection {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn section_kind(&self) -> PyGoogleSectionKind {
        self.section_kind
    }
    #[getter]
    fn header_name(&self, py: Python<'_>) -> Py<PyToken> {
        self.header_name.clone_ref(py)
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        format!("GoogleSection({:?})", self.header_name.borrow(py).text)
    }
}

fn build_google_section(py: Python<'_>, sec: &gn::GoogleSection<'_>, source: &str) -> PyResult<Py<PyGoogleSection>> {
    Py::new(
        py,
        PyGoogleSection {
            range: *sec.syntax().range(),
            section_kind: google_section_kind_to_py(sec.section_kind(source)),
            header_name: mk_token(py, sec.header().name(), source)?,
        },
    )
}

// ─── GoogleDocstring ─────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "GoogleDocstring")]
struct PyGoogleDocstring {
    range: TextRange,
    summary: Option<Py<PyToken>>,
    extended_summary: Option<Py<PyToken>>,
    stray_lines: Vec<Py<PyToken>>,
    sections: Vec<Py<PyGoogleSection>>,
    source: String,
    /// Cached CST — avoids re-parsing when `walk()` is called.
    parsed: Arc<Parsed>,
}

#[pymethods]
impl PyGoogleDocstring {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn summary(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.summary.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn extended_summary(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.extended_summary.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn stray_lines(&self, py: Python<'_>) -> Vec<Py<PyToken>> {
        self.stray_lines.iter().map(|t| t.clone_ref(py)).collect()
    }
    #[getter]
    fn sections(&self, py: Python<'_>) -> Vec<Py<PyGoogleSection>> {
        self.sections.iter().map(|s| s.clone_ref(py)).collect()
    }
    #[getter]
    fn source(&self) -> &str {
        &self.source
    }
    #[getter]
    fn style(&self) -> PyStyle {
        PyStyle::Google
    }
    fn pretty_print(&self) -> String {
        self.parsed.pretty_print()
    }
    fn to_model(&self) -> PyResult<PyModelDocstring> {
        pydocstring_core::parse::google::to_model::to_model(&self.parsed)
            .map(|doc| PyModelDocstring::try_from(&doc))
            .transpose()?
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("failed to convert to model"))
    }
    fn __repr__(&self) -> &'static str {
        "GoogleDocstring(...)"
    }
}

fn build_google_docstring_node(
    py: Python<'_>,
    doc: &gn::GoogleDocstring<'_>,
    source: &str,
    parsed: Arc<Parsed>,
) -> PyResult<Py<PyGoogleDocstring>> {
    let summary = mk_token_opt(py, doc.summary(), source)?;
    let extended_summary = mk_token_opt(py, doc.extended_summary(), source)?;
    let stray_lines = mk_tokens(py, doc.stray_lines(), source)?;
    let sections = doc
        .sections()
        .map(|sec| build_google_section(py, &sec, source))
        .collect::<PyResult<_>>()?;
    Py::new(
        py,
        PyGoogleDocstring {
            range: *doc.syntax().range(),
            summary,
            extended_summary,
            stray_lines,
            sections,
            source: source.to_string(),
            parsed,
        },
    )
}

fn build_google_docstring(py: Python<'_>, parsed: Parsed) -> PyResult<Py<PyGoogleDocstring>> {
    let arc = Arc::new(parsed);
    let arc2 = Arc::clone(&arc);
    let doc = gn::GoogleDocstring::cast(arc.root())
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("root is not GOOGLE_DOCSTRING"))?;
    build_google_docstring_node(py, &doc, arc.source(), arc2)
}

// =============================================================================
// NumPy typed wrappers
// =============================================================================

// ─── NumPyDeprecation ────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "NumPyDeprecation")]
struct PyNumPyDeprecation {
    range: TextRange,
    directive_marker: Option<Py<PyToken>>,
    keyword: Option<Py<PyToken>>,
    double_colon: Option<Py<PyToken>>,
    version: Py<PyToken>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyNumPyDeprecation {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn directive_marker(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.directive_marker.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn keyword(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.keyword.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn double_colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.double_colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn version(&self, py: Python<'_>) -> Py<PyToken> {
        self.version.clone_ref(py)
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        format!("NumPyDeprecation({:?})", self.version.borrow(py).text)
    }
}

fn build_numpy_deprecation(
    py: Python<'_>,
    dep: &nn::NumPyDeprecation<'_>,
    source: &str,
) -> PyResult<Py<PyNumPyDeprecation>> {
    Py::new(
        py,
        PyNumPyDeprecation {
            range: *dep.syntax().range(),
            directive_marker: mk_token_opt(py, dep.directive_marker(), source)?,
            keyword: mk_token_opt(py, dep.keyword(), source)?,
            double_colon: mk_token_opt(py, dep.double_colon(), source)?,
            version: mk_token(py, dep.version(), source)?,
            description: mk_token_opt(py, dep.description(), source)?,
        },
    )
}

// ─── NumPyParameter ──────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "NumPyParameter")]
struct PyNumPyParameter {
    range: TextRange,
    names: Vec<Py<PyToken>>,
    colon: Option<Py<PyToken>>,
    r#type: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
    optional: Option<Py<PyToken>>,
    default_keyword: Option<Py<PyToken>>,
    default_separator: Option<Py<PyToken>>,
    default_value: Option<Py<PyToken>>,
}

#[pymethods]
impl PyNumPyParameter {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn names(&self, py: Python<'_>) -> Vec<Py<PyToken>> {
        self.names.iter().map(|n| n.clone_ref(py)).collect()
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.r#type.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn optional(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.optional.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn default_keyword(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.default_keyword.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn default_separator(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.default_separator.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn default_value(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.default_value.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        let first = self
            .names
            .first()
            .map(|n| n.borrow(py).text.clone())
            .unwrap_or_default();
        format!("NumPyParameter({:?})", first)
    }
}

fn build_numpy_parameter(py: Python<'_>, prm: &nn::NumPyParameter<'_>, source: &str) -> PyResult<Py<PyNumPyParameter>> {
    Py::new(
        py,
        PyNumPyParameter {
            range: *prm.syntax().range(),
            names: mk_tokens(py, prm.names(), source)?,
            colon: mk_token_opt(py, prm.colon(), source)?,
            r#type: mk_token_or_missing(py, prm.r#type(), prm.syntax(), SyntaxKind::TYPE, source)?,
            description: mk_token_opt(py, prm.description(), source)?,
            optional: mk_token_opt(py, prm.optional(), source)?,
            default_keyword: mk_token_opt(py, prm.default_keyword(), source)?,
            default_separator: mk_token_opt(py, prm.default_separator(), source)?,
            default_value: mk_token_or_missing(
                py,
                prm.default_value(),
                prm.syntax(),
                SyntaxKind::DEFAULT_VALUE,
                source,
            )?,
        },
    )
}

// ─── NumPyReturns ────────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "NumPyReturns")]
struct PyNumPyReturns {
    range: TextRange,
    name: Option<Py<PyToken>>,
    colon: Option<Py<PyToken>>,
    return_type: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyNumPyReturns {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn name(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.name.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn return_type(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.return_type.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self) -> &'static str {
        "NumPyReturns(...)"
    }
}

fn build_numpy_returns(py: Python<'_>, rtn: &nn::NumPyReturns<'_>, source: &str) -> PyResult<Py<PyNumPyReturns>> {
    Py::new(
        py,
        PyNumPyReturns {
            range: *rtn.syntax().range(),
            name: mk_token_opt(py, rtn.name(), source)?,
            colon: mk_token_opt(py, rtn.colon(), source)?,
            return_type: mk_token_opt(py, rtn.return_type(), source)?,
            description: mk_token_opt(py, rtn.description(), source)?,
        },
    )
}

// ─── NumPyYields ─────────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "NumPyYields")]
struct PyNumPyYields {
    range: TextRange,
    name: Option<Py<PyToken>>,
    colon: Option<Py<PyToken>>,
    return_type: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyNumPyYields {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn name(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.name.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn return_type(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.return_type.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self) -> &'static str {
        "NumPyYields(...)"
    }
}

fn build_numpy_yields(py: Python<'_>, yld: &nn::NumPyYields<'_>, source: &str) -> PyResult<Py<PyNumPyYields>> {
    Py::new(
        py,
        PyNumPyYields {
            range: *yld.syntax().range(),
            name: mk_token_opt(py, yld.name(), source)?,
            colon: mk_token_opt(py, yld.colon(), source)?,
            return_type: mk_token_opt(py, yld.return_type(), source)?,
            description: mk_token_opt(py, yld.description(), source)?,
        },
    )
}

// ─── NumPyException ──────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "NumPyException")]
struct PyNumPyException {
    range: TextRange,
    r#type: Py<PyToken>,
    colon: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyNumPyException {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> Py<PyToken> {
        self.r#type.clone_ref(py)
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        format!("NumPyException({:?})", self.r#type.borrow(py).text)
    }
}

fn build_numpy_exception(py: Python<'_>, exc: &nn::NumPyException<'_>, source: &str) -> PyResult<Py<PyNumPyException>> {
    Py::new(
        py,
        PyNumPyException {
            range: *exc.syntax().range(),
            r#type: mk_token(py, exc.r#type(), source)?,
            colon: mk_token_opt(py, exc.colon(), source)?,
            description: mk_token_opt(py, exc.description(), source)?,
        },
    )
}

// ─── NumPyWarning ────────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "NumPyWarning")]
struct PyNumPyWarning {
    range: TextRange,
    r#type: Py<PyToken>,
    colon: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyNumPyWarning {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> Py<PyToken> {
        self.r#type.clone_ref(py)
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        format!("NumPyWarning({:?})", self.r#type.borrow(py).text)
    }
}

fn build_numpy_warning(py: Python<'_>, wrn: &nn::NumPyWarning<'_>, source: &str) -> PyResult<Py<PyNumPyWarning>> {
    Py::new(
        py,
        PyNumPyWarning {
            range: *wrn.syntax().range(),
            r#type: mk_token(py, wrn.r#type(), source)?,
            colon: mk_token_opt(py, wrn.colon(), source)?,
            description: mk_token_opt(py, wrn.description(), source)?,
        },
    )
}

// ─── NumPySeeAlsoItem ────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "NumPySeeAlsoItem")]
struct PyNumPySeeAlsoItem {
    range: TextRange,
    names: Vec<Py<PyToken>>,
    colon: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyNumPySeeAlsoItem {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn names(&self, py: Python<'_>) -> Vec<Py<PyToken>> {
        self.names.iter().map(|n| n.clone_ref(py)).collect()
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self) -> &'static str {
        "NumPySeeAlsoItem(...)"
    }
}

fn build_numpy_see_also_item(
    py: Python<'_>,
    sai: &nn::NumPySeeAlsoItem<'_>,
    source: &str,
) -> PyResult<Py<PyNumPySeeAlsoItem>> {
    Py::new(
        py,
        PyNumPySeeAlsoItem {
            range: *sai.syntax().range(),
            names: mk_tokens(py, sai.names(), source)?,
            colon: mk_token_opt(py, sai.colon(), source)?,
            description: mk_token_opt(py, sai.description(), source)?,
        },
    )
}

// ─── NumPyReference ──────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "NumPyReference")]
struct PyNumPyReference {
    range: TextRange,
    directive_marker: Option<Py<PyToken>>,
    open_bracket: Option<Py<PyToken>>,
    number: Option<Py<PyToken>>,
    close_bracket: Option<Py<PyToken>>,
    content: Option<Py<PyToken>>,
}

#[pymethods]
impl PyNumPyReference {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn directive_marker(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.directive_marker.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn open_bracket(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.open_bracket.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn number(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.number.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn close_bracket(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.close_bracket.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn content(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.content.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self) -> &'static str {
        "NumPyReference(...)"
    }
}

fn build_numpy_reference(py: Python<'_>, r: &nn::NumPyReference<'_>, source: &str) -> PyResult<Py<PyNumPyReference>> {
    Py::new(
        py,
        PyNumPyReference {
            range: *r.syntax().range(),
            directive_marker: mk_token_opt(py, r.directive_marker(), source)?,
            open_bracket: mk_token_opt(py, r.open_bracket(), source)?,
            number: mk_token_opt(py, r.number(), source)?,
            close_bracket: mk_token_opt(py, r.close_bracket(), source)?,
            content: mk_token_opt(py, r.content(), source)?,
        },
    )
}

// ─── NumPyAttribute ──────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "NumPyAttribute")]
struct PyNumPyAttribute {
    range: TextRange,
    name: Py<PyToken>,
    colon: Option<Py<PyToken>>,
    r#type: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyNumPyAttribute {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn name(&self, py: Python<'_>) -> Py<PyToken> {
        self.name.clone_ref(py)
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn r#type(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.r#type.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        format!("NumPyAttribute({:?})", self.name.borrow(py).text)
    }
}

fn build_numpy_attribute(py: Python<'_>, att: &nn::NumPyAttribute<'_>, source: &str) -> PyResult<Py<PyNumPyAttribute>> {
    Py::new(
        py,
        PyNumPyAttribute {
            range: *att.syntax().range(),
            name: mk_token(py, att.name(), source)?,
            colon: mk_token_opt(py, att.colon(), source)?,
            r#type: mk_token_opt(py, att.r#type(), source)?,
            description: mk_token_opt(py, att.description(), source)?,
        },
    )
}

// ─── NumPyMethod ─────────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "NumPyMethod")]
struct PyNumPyMethod {
    range: TextRange,
    name: Py<PyToken>,
    colon: Option<Py<PyToken>>,
    description: Option<Py<PyToken>>,
}

#[pymethods]
impl PyNumPyMethod {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn name(&self, py: Python<'_>) -> Py<PyToken> {
        self.name.clone_ref(py)
    }
    #[getter]
    fn colon(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.colon.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn description(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.description.as_ref().map(|t| t.clone_ref(py))
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        format!("NumPyMethod({:?})", self.name.borrow(py).text)
    }
}

fn build_numpy_method(py: Python<'_>, mtd: &nn::NumPyMethod<'_>, source: &str) -> PyResult<Py<PyNumPyMethod>> {
    Py::new(
        py,
        PyNumPyMethod {
            range: *mtd.syntax().range(),
            name: mk_token(py, mtd.name(), source)?,
            colon: mk_token_opt(py, mtd.colon(), source)?,
            description: mk_token_opt(py, mtd.description(), source)?,
        },
    )
}

// ─── NumPySection ────────────────────────────────────────────────────────────

/// A thin wrapper for a NumPy section node (no eager child allocation).
#[pyclass(frozen, skip_from_py_object, name = "NumPySection")]
struct PyNumPySection {
    range: TextRange,
    section_kind: PyNumPySectionKind,
    header_name: Py<PyToken>,
}

#[pymethods]
impl PyNumPySection {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn section_kind(&self) -> PyNumPySectionKind {
        self.section_kind
    }
    #[getter]
    fn header_name(&self, py: Python<'_>) -> Py<PyToken> {
        self.header_name.clone_ref(py)
    }
    fn __repr__(&self, py: Python<'_>) -> String {
        format!("NumPySection({:?})", self.header_name.borrow(py).text)
    }
}

fn build_numpy_section(py: Python<'_>, sec: &nn::NumPySection<'_>, source: &str) -> PyResult<Py<PyNumPySection>> {
    Py::new(
        py,
        PyNumPySection {
            range: *sec.syntax().range(),
            section_kind: numpy_section_kind_to_py(sec.section_kind(source)),
            header_name: mk_token(py, sec.header().name(), source)?,
        },
    )
}

// ─── NumPyDocstring ──────────────────────────────────────────────────────────

#[pyclass(frozen, skip_from_py_object, name = "NumPyDocstring")]
struct PyNumPyDocstring {
    range: TextRange,
    summary: Option<Py<PyToken>>,
    extended_summary: Option<Py<PyToken>>,
    deprecation: Option<Py<PyNumPyDeprecation>>,
    sections: Vec<Py<PyNumPySection>>,
    source: String,
    /// Cached CST — avoids re-parsing when `walk()` is called.
    parsed: Arc<Parsed>,
}

#[pymethods]
impl PyNumPyDocstring {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn summary(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.summary.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn extended_summary(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.extended_summary.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn deprecation(&self, py: Python<'_>) -> Option<Py<PyNumPyDeprecation>> {
        self.deprecation.as_ref().map(|d| d.clone_ref(py))
    }
    #[getter]
    fn sections(&self, py: Python<'_>) -> Vec<Py<PyNumPySection>> {
        self.sections.iter().map(|s| s.clone_ref(py)).collect()
    }
    #[getter]
    fn source(&self) -> &str {
        &self.source
    }
    #[getter]
    fn style(&self) -> PyStyle {
        PyStyle::NumPy
    }
    fn pretty_print(&self) -> String {
        self.parsed.pretty_print()
    }
    fn to_model(&self) -> PyResult<PyModelDocstring> {
        pydocstring_core::parse::numpy::to_model::to_model(&self.parsed)
            .map(|doc| PyModelDocstring::try_from(&doc))
            .transpose()?
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("failed to convert to model"))
    }
    fn __repr__(&self) -> &'static str {
        "NumPyDocstring(...)"
    }
}

fn build_numpy_docstring_node(
    py: Python<'_>,
    doc: &nn::NumPyDocstring<'_>,
    source: &str,
    parsed: Arc<Parsed>,
) -> PyResult<Py<PyNumPyDocstring>> {
    let summary = mk_token_opt(py, doc.summary(), source)?;
    let extended_summary = mk_token_opt(py, doc.extended_summary(), source)?;
    let deprecation = doc
        .deprecation()
        .map(|dep| build_numpy_deprecation(py, &dep, source))
        .transpose()?;
    let sections = doc
        .sections()
        .map(|sec| build_numpy_section(py, &sec, source))
        .collect::<PyResult<_>>()?;
    Py::new(
        py,
        PyNumPyDocstring {
            range: *doc.syntax().range(),
            summary,
            extended_summary,
            deprecation,
            sections,
            source: source.to_string(),
            parsed,
        },
    )
}

fn build_numpy_docstring(py: Python<'_>, parsed: Parsed) -> PyResult<Py<PyNumPyDocstring>> {
    let arc = Arc::new(parsed);
    let arc2 = Arc::clone(&arc);
    let doc = nn::NumPyDocstring::cast(arc.root())
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("root is not NUMPY_DOCSTRING"))?;
    build_numpy_docstring_node(py, &doc, arc.source(), arc2)
}

// =============================================================================
// Plain docstring
// =============================================================================

#[pyclass(frozen, skip_from_py_object, name = "PlainDocstring")]
struct PyPlainDocstring {
    range: TextRange,
    summary: Option<Py<PyToken>>,
    extended_summary: Option<Py<PyToken>>,
    source: String,
    /// Cached CST — avoids re-parsing when `walk()` is called.
    parsed: Arc<Parsed>,
}

#[pymethods]
impl PyPlainDocstring {
    #[getter]
    fn range(&self, py: Python<'_>) -> PyResult<Py<PyTextRange>> {
        Py::new(py, PyTextRange::from(self.range))
    }
    #[getter]
    fn summary(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.summary.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn extended_summary(&self, py: Python<'_>) -> Option<Py<PyToken>> {
        self.extended_summary.as_ref().map(|t| t.clone_ref(py))
    }
    #[getter]
    fn source(&self) -> &str {
        &self.source
    }
    #[getter]
    fn style(&self) -> PyStyle {
        PyStyle::Plain
    }
    fn pretty_print(&self) -> String {
        self.parsed.pretty_print()
    }
    fn to_model(&self) -> PyResult<PyModelDocstring> {
        pydocstring_core::parse::plain::to_model::to_model(&self.parsed)
            .map(|doc| PyModelDocstring::try_from(&doc))
            .transpose()?
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("failed to convert to model"))
    }
    fn __repr__(&self) -> &'static str {
        "PlainDocstring(...)"
    }
}

fn build_plain_docstring(py: Python<'_>, parsed: Parsed) -> PyResult<Py<PyPlainDocstring>> {
    let arc = Arc::new(parsed);
    let source = arc.source();
    let doc = pn::PlainDocstring::cast(arc.root())
        .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("root is not PLAIN_DOCSTRING"))?;
    let summary = mk_token_opt(py, doc.summary(), source)?;
    let extended_summary = mk_token_opt(py, doc.extended_summary(), source)?;
    Py::new(
        py,
        PyPlainDocstring {
            range: *arc.root().range(),
            summary,
            extended_summary,
            source: source.to_string(),
            parsed: arc,
        },
    )
}

fn build_plain_docstring_node(
    py: Python<'_>,
    doc: &pn::PlainDocstring<'_>,
    source: &str,
    parsed: Arc<Parsed>,
) -> PyResult<Py<PyPlainDocstring>> {
    let summary = mk_token_opt(py, doc.summary(), source)?;
    let extended_summary = mk_token_opt(py, doc.extended_summary(), source)?;
    Py::new(
        py,
        PyPlainDocstring {
            range: *doc.syntax().range(),
            summary,
            extended_summary,
            source: source.to_string(),
            parsed,
        },
    )
}

// =============================================================================
// Model IR types
// =============================================================================

#[pyclass(name = "Deprecation")]
struct PyModelDeprecation {
    #[pyo3(get, set)]
    version: Py<PyString>,
    #[pyo3(get, set)]
    description: Option<Py<PyString>>,
}

#[pymethods]
impl PyModelDeprecation {
    #[new]
    #[pyo3(signature = (version, *, description=None))]
    fn new(version: Py<PyString>, description: Option<Py<PyString>>) -> Self {
        Self { version, description }
    }
    fn __repr__(&self) -> String {
        format!("Deprecation(version={:?})", self.version)
    }
}

impl TryFrom<&model::Deprecation> for PyModelDeprecation {
    type Error = PyErr;

    fn try_from(dep: &model::Deprecation) -> Result<Self, Self::Error> {
        Python::attach(|py| {
            Ok(Self {
                version: (&dep.version).into_pyobject(py)?.unbind(),
                description: dep
                    .description
                    .as_ref()
                    .map(|d| -> PyResult<_> { Ok(d.into_pyobject(py)?.unbind()) })
                    .transpose()?,
            })
        })
    }
}

impl TryInto<model::Deprecation> for &PyModelDeprecation {
    type Error = PyErr;

    fn try_into(self) -> Result<model::Deprecation, Self::Error> {
        Python::attach(|py| {
            Ok(model::Deprecation {
                version: self.version.extract(py)?,
                description: self.description.as_ref().map(|d| d.extract(py)).transpose()?,
            })
        })
    }
}

#[pyclass(name = "Parameter")]
struct PyModelParameter {
    #[pyo3(get, set)]
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
        if names.bind(py).into_iter().any(|n| !n.is_instance_of::<PyString>()) {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "Parameter names must be strings.",
            ));
        }
        Ok(Self {
            names,
            type_annotation,
            description,
            is_optional,
            default_value,
        })
    }
    fn __repr__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        ", ".into_pyobject(py)?
            .call_method("join", (self.names.bind(py),), None)
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
    #[pyo3(get, set)]
    names: Py<PyList>,
    #[pyo3(get, set)]
    description: Option<Py<PyString>>,
}

#[pymethods]
impl PyModelSeeAlsoEntry {
    #[new]
    #[pyo3(signature = (names, *, description=None))]
    fn new(py: Python<'_>, names: Py<PyList>, description: Option<Py<PyString>>) -> PyResult<Self> {
        if names.bind(py).into_iter().any(|n| !n.is_instance_of::<PyString>()) {
            return Err(pyo3::exceptions::PyTypeError::new_err("Names must be strings."));
        }
        Ok(Self { names, description })
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
    number: Option<Py<PyString>>,
    #[pyo3(get, set)]
    content: Option<Py<PyString>>,
}

#[pymethods]
impl PyModelReference {
    #[new]
    #[pyo3(signature = (*, number=None, content=None))]
    fn new(number: Option<Py<PyString>>, content: Option<Py<PyString>>) -> Self {
        Self { number, content }
    }
    fn __repr__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.number.as_ref().map_or_else(
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
                number: reference
                    .number
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
                number: self
                    .number
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
    #[pyo3(get, set)]
    name: Py<PyString>,
    #[pyo3(get, set)]
    type_annotation: Option<Py<PyString>>,
    #[pyo3(get, set)]
    description: Option<Py<PyString>>,
}

#[pymethods]
impl PyModelAttribute {
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
        "Attribute({})"
            .into_pyobject(py)?
            .call_method("format", (&self.name,), None)
    }
}

impl TryFrom<&model::Attribute> for PyModelAttribute {
    type Error = PyErr;

    fn try_from(attribute: &model::Attribute) -> Result<Self, Self::Error> {
        Python::attach(|py| {
            Ok(Self {
                name: (&attribute.name).into_pyobject(py)?.unbind(),
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

fn section_to_py_kind(section: &PyModelSection) -> PySectionKind {
    match section {
        PyModelSection::Parameters(_) => PySectionKind::Parameters,
        PyModelSection::KeywordParameters(_) => PySectionKind::KeywordParameters,
        PyModelSection::OtherParameters(_) => PySectionKind::OtherParameters,
        PyModelSection::Receives(_) => PySectionKind::Receives,
        PyModelSection::Returns(_) => PySectionKind::Returns,
        PyModelSection::Yields(_) => PySectionKind::Yields,
        PyModelSection::Raises(_) => PySectionKind::Raises,
        PyModelSection::Warns(_) => PySectionKind::Warns,
        PyModelSection::Attributes(_) => PySectionKind::Attributes,
        PyModelSection::Methods(_) => PySectionKind::Methods,
        PyModelSection::SeeAlso(_) => PySectionKind::SeeAlso,
        PyModelSection::References(_) => PySectionKind::References,
        PyModelSection::FreeText { kind, .. } => *kind,
    }
}

// ─── Model Section ───────────────────────────────────────────────────────────

#[pyclass(name = "Section")]
enum PyModelSection {
    Parameters(Py<PyList>),
    KeywordParameters(Py<PyList>),
    OtherParameters(Py<PyList>),
    Receives(Py<PyList>),
    Returns(Py<PyList>),
    Yields(Py<PyList>),
    Raises(Py<PyList>),
    Warns(Py<PyList>),
    Attributes(Py<PyList>),
    Methods(Py<PyList>),
    SeeAlso(Py<PyList>),
    References(Py<PyList>),
    FreeText {
        kind: PySectionKind,
        body: Py<PyString>,
        name: Option<Py<PyString>>,
    },
}

#[pymethods]
impl PyModelSection {
    #[new]
    #[pyo3(signature = (kind, *, unknown_name=None, parameters=None, returns=None, exceptions=None, attributes=None, methods=None, see_also_entries=None, references=None, body=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        kind: PySectionKind,
        unknown_name: Option<Py<PyString>>,
        parameters: Option<Py<PyList>>,
        returns: Option<Py<PyList>>,
        exceptions: Option<Py<PyList>>,
        attributes: Option<Py<PyList>>,
        methods: Option<Py<PyList>>,
        see_also_entries: Option<Py<PyList>>,
        references: Option<Py<PyList>>,
        body: Option<Py<PyString>>,
    ) -> PyResult<Self> {
        // Validate that only the relevant kwargs for this kind are supplied.
        let kind_name = py_section_kind_name(kind);

        macro_rules! reject {
            ($uses:expr, $arg:ident $(, $type:ty)?) => {
                if let Some(ref _list) = $arg {
                    if !$uses {
                        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                            "Section(SectionKind.{}) does not accept '{}'",
                            kind_name, stringify!($arg)
                        )));
                    } $(else if _list.bind(py).into_iter().any(|i| {!i.is_instance_of::<$type>()}) {
                        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                            "Section(SectionKind.{}) only accepts '{}'",
                            kind_name, <$type>::NAME
                        )));
                    })?
                }
            };
        }

        let uses_parameters = matches!(
            kind,
            PySectionKind::Parameters
                | PySectionKind::KeywordParameters
                | PySectionKind::OtherParameters
                | PySectionKind::Receives
        );
        let uses_returns = matches!(kind, PySectionKind::Returns | PySectionKind::Yields);
        let uses_exceptions = matches!(kind, PySectionKind::Raises | PySectionKind::Warns);
        let uses_attributes = matches!(kind, PySectionKind::Attributes);
        let uses_methods = matches!(kind, PySectionKind::Methods);
        let uses_see_also = matches!(kind, PySectionKind::SeeAlso);
        let uses_references = matches!(kind, PySectionKind::References);
        let is_freetext = !uses_parameters
            && !uses_returns
            && !uses_exceptions
            && !uses_attributes
            && !uses_methods
            && !uses_see_also
            && !uses_references;

        reject!(uses_parameters, parameters, PyModelParameter);
        reject!(uses_returns, returns, PyModelReturn);
        reject!(uses_exceptions, exceptions, PyModelExceptionEntry);
        reject!(uses_attributes, attributes, PyModelAttribute);
        reject!(uses_methods, methods, PyModelMethod);
        reject!(uses_see_also, see_also_entries, PyModelSeeAlsoEntry);
        reject!(uses_references, references, PyModelReference);
        reject!(is_freetext, body);
        reject!(matches!(kind, PySectionKind::Unknown), unknown_name);

        let ret = match kind {
            PySectionKind::Parameters => {
                PyModelSection::Parameters(parameters.unwrap_or_else(|| PyList::empty(py).unbind()))
            }
            PySectionKind::KeywordParameters => {
                PyModelSection::KeywordParameters(parameters.unwrap_or_else(|| PyList::empty(py).unbind()))
            }
            PySectionKind::OtherParameters => {
                PyModelSection::OtherParameters(parameters.unwrap_or_else(|| PyList::empty(py).unbind()))
            }
            PySectionKind::Receives => {
                PyModelSection::Receives(parameters.unwrap_or_else(|| PyList::empty(py).unbind()))
            }
            PySectionKind::Returns => PyModelSection::Returns(returns.unwrap_or_else(|| PyList::empty(py).unbind())),
            PySectionKind::Yields => PyModelSection::Yields(returns.unwrap_or_else(|| PyList::empty(py).unbind())),
            PySectionKind::Raises => PyModelSection::Raises(exceptions.unwrap_or_else(|| PyList::empty(py).unbind())),
            PySectionKind::Warns => PyModelSection::Warns(exceptions.unwrap_or_else(|| PyList::empty(py).unbind())),
            PySectionKind::Attributes => {
                PyModelSection::Attributes(attributes.unwrap_or_else(|| PyList::empty(py).unbind()))
            }
            PySectionKind::Methods => PyModelSection::Methods(methods.unwrap_or_else(|| PyList::empty(py).unbind())),
            PySectionKind::SeeAlso => {
                PyModelSection::SeeAlso(see_also_entries.unwrap_or_else(|| PyList::empty(py).unbind()))
            }
            PySectionKind::References => {
                PyModelSection::References(references.unwrap_or_else(|| PyList::empty(py).unbind()))
            }
            PySectionKind::Notes => PyModelSection::FreeText {
                kind,
                body: body.unwrap_or_else(|| PyString::new(py, "").unbind()),
                name: None,
            },
            PySectionKind::Examples => PyModelSection::FreeText {
                kind,
                body: body.unwrap_or_else(|| PyString::new(py, "").unbind()),
                name: None,
            },
            PySectionKind::Warnings => PyModelSection::FreeText {
                kind,
                body: body.unwrap_or_else(|| PyString::new(py, "").unbind()),
                name: None,
            },
            PySectionKind::Todo => PyModelSection::FreeText {
                kind,
                body: body.unwrap_or_else(|| PyString::new(py, "").unbind()),
                name: None,
            },
            PySectionKind::Attention => PyModelSection::FreeText {
                kind,
                body: body.unwrap_or_else(|| PyString::new(py, "").unbind()),
                name: None,
            },
            PySectionKind::Caution => PyModelSection::FreeText {
                kind,
                body: body.unwrap_or_else(|| PyString::new(py, "").unbind()),
                name: None,
            },
            PySectionKind::Danger => PyModelSection::FreeText {
                kind,
                body: body.unwrap_or_else(|| PyString::new(py, "").unbind()),
                name: None,
            },
            PySectionKind::Error => PyModelSection::FreeText {
                kind,
                body: body.unwrap_or_else(|| PyString::new(py, "").unbind()),
                name: None,
            },
            PySectionKind::Hint => PyModelSection::FreeText {
                kind,
                body: body.unwrap_or_else(|| PyString::new(py, "").unbind()),
                name: None,
            },
            PySectionKind::Important => PyModelSection::FreeText {
                kind,
                body: body.unwrap_or_else(|| PyString::new(py, "").unbind()),
                name: None,
            },
            PySectionKind::Tip => PyModelSection::FreeText {
                kind,
                body: body.unwrap_or_else(|| PyString::new(py, "").unbind()),
                name: None,
            },
            PySectionKind::Unknown => PyModelSection::FreeText {
                kind,
                body: body.unwrap_or_else(|| PyString::new(py, "").unbind()),
                name: unknown_name,
            },
        };
        Ok(ret)
    }

    #[getter]
    fn kind(&self) -> PySectionKind {
        section_to_py_kind(self)
    }

    #[getter]
    fn unknown_name<'py>(&self, py: Python<'py>) -> Option<&Bound<'py, PyString>> {
        match &self {
            PyModelSection::FreeText {
                kind: PySectionKind::Unknown,
                body: _,
                name,
            } if name.is_some() => Some(name.as_ref().unwrap().bind(py)),
            _ => None,
        }
    }

    #[getter]
    fn parameters<'py>(&self, py: Python<'py>) -> Option<&Bound<'py, PyList>> {
        match &self {
            PyModelSection::Parameters(ps)
            | PyModelSection::KeywordParameters(ps)
            | PyModelSection::OtherParameters(ps)
            | PyModelSection::Receives(ps) => Some(ps.bind(py)),
            _ => None,
        }
    }

    #[getter]
    fn returns<'py>(&self, py: Python<'py>) -> Option<&Bound<'py, PyList>> {
        match &self {
            PyModelSection::Returns(rs) | PyModelSection::Yields(rs) => Some(rs.bind(py)),
            _ => None,
        }
    }

    #[getter]
    fn exceptions<'py>(&self, py: Python<'py>) -> Option<&Bound<'py, PyList>> {
        match &self {
            PyModelSection::Raises(es) | PyModelSection::Warns(es) => Some(es.bind(py)),
            _ => None,
        }
    }

    #[getter]
    fn attributes<'py>(&self, py: Python<'py>) -> Option<&Bound<'py, PyList>> {
        match &self {
            PyModelSection::Attributes(attrs) => Some(attrs.bind(py)),
            _ => None,
        }
    }

    #[getter]
    fn methods<'py>(&self, py: Python<'py>) -> Option<&Bound<'py, PyList>> {
        match &self {
            PyModelSection::Methods(ms) => Some(ms.bind(py)),
            _ => None,
        }
    }

    #[getter]
    fn see_also_entries<'py>(&self, py: Python<'py>) -> Option<&Bound<'py, PyList>> {
        match &self {
            PyModelSection::SeeAlso(items) => Some(items.bind(py)),
            _ => None,
        }
    }

    #[getter]
    fn references<'py>(&self, py: Python<'py>) -> Option<&Bound<'py, PyList>> {
        match &self {
            PyModelSection::References(refs) => Some(refs.bind(py)),
            _ => None,
        }
    }

    #[getter]
    fn body<'py>(&self, py: Python<'py>) -> Option<&Bound<'py, PyString>> {
        match &self {
            Self::FreeText { kind: _, body, .. } => Some(body.bind(py)),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Section(SectionKind.{})",
            py_section_kind_name(section_to_py_kind(self))
        )
    }
}

impl TryFrom<&model::Section> for PyModelSection {
    type Error = PyErr;

    fn try_from(section: &model::Section) -> Result<Self, Self::Error> {
        Python::attach(|py| -> Result<Self, Self::Error> {
            match section {
                model::Section::Parameters(params) => Ok(PyModelSection::Parameters(
                    PyList::new(
                        py,
                        params
                            .iter()
                            .map(PyModelParameter::try_from)
                            .collect::<Result<Vec<PyModelParameter>, _>>()?,
                    )?
                    .unbind(),
                )),
                model::Section::KeywordParameters(params) => Ok(PyModelSection::KeywordParameters(
                    PyList::new(
                        py,
                        params
                            .iter()
                            .map(PyModelParameter::try_from)
                            .collect::<Result<Vec<PyModelParameter>, _>>()?,
                    )?
                    .unbind(),
                )),
                model::Section::OtherParameters(params) => Ok(PyModelSection::OtherParameters(
                    PyList::new(
                        py,
                        params
                            .iter()
                            .map(PyModelParameter::try_from)
                            .collect::<Result<Vec<PyModelParameter>, _>>()?,
                    )?
                    .unbind(),
                )),
                model::Section::Receives(params) => Ok(PyModelSection::Receives(
                    PyList::new(
                        py,
                        params
                            .iter()
                            .map(PyModelParameter::try_from)
                            .collect::<Result<Vec<PyModelParameter>, _>>()?,
                    )?
                    .unbind(),
                )),
                model::Section::Returns(returns) => Ok(PyModelSection::Returns(
                    PyList::new(
                        py,
                        returns
                            .iter()
                            .map(PyModelReturn::try_from)
                            .collect::<Result<Vec<PyModelReturn>, _>>()?,
                    )?
                    .unbind(),
                )),
                model::Section::Yields(returns) => Ok(PyModelSection::Returns(
                    PyList::new(
                        py,
                        returns
                            .iter()
                            .map(PyModelReturn::try_from)
                            .collect::<Result<Vec<PyModelReturn>, _>>()?,
                    )?
                    .unbind(),
                )),
                model::Section::Raises(exceptions) => Ok(PyModelSection::Raises(
                    PyList::new(
                        py,
                        exceptions
                            .iter()
                            .map(PyModelExceptionEntry::try_from)
                            .collect::<Result<Vec<PyModelExceptionEntry>, _>>()?,
                    )?
                    .unbind(),
                )),
                model::Section::Warns(exceptions) => Ok(PyModelSection::Raises(
                    PyList::new(
                        py,
                        exceptions
                            .iter()
                            .map(PyModelExceptionEntry::try_from)
                            .collect::<Result<Vec<PyModelExceptionEntry>, _>>()?,
                    )?
                    .unbind(),
                )),
                model::Section::Attributes(attributes) => Ok(PyModelSection::Attributes(
                    PyList::new(
                        py,
                        attributes
                            .iter()
                            .map(PyModelAttribute::try_from)
                            .collect::<Result<Vec<PyModelAttribute>, _>>()?,
                    )?
                    .unbind(),
                )),
                model::Section::Methods(methods) => Ok(PyModelSection::Methods(
                    PyList::new(
                        py,
                        methods
                            .iter()
                            .map(PyModelMethod::try_from)
                            .collect::<Result<Vec<PyModelMethod>, _>>()?,
                    )?
                    .unbind(),
                )),
                model::Section::SeeAlso(seealso) => Ok(PyModelSection::SeeAlso(
                    PyList::new(
                        py,
                        seealso
                            .iter()
                            .map(PyModelSeeAlsoEntry::try_from)
                            .collect::<Result<Vec<PyModelSeeAlsoEntry>, _>>()?,
                    )?
                    .unbind(),
                )),
                model::Section::References(refs) => Ok(PyModelSection::References(
                    PyList::new(
                        py,
                        refs.iter()
                            .map(PyModelReference::try_from)
                            .collect::<Result<Vec<PyModelReference>, _>>()?,
                    )?
                    .unbind(),
                )),
                model::Section::FreeText { kind, body } => Ok(match kind {
                    model::FreeSectionKind::Notes => PyModelSection::FreeText {
                        kind: PySectionKind::Notes,
                        body: body.into_pyobject(py)?.unbind(),
                        name: None,
                    },
                    model::FreeSectionKind::Examples => PyModelSection::FreeText {
                        kind: PySectionKind::Examples,
                        body: body.into_pyobject(py)?.unbind(),
                        name: None,
                    },
                    model::FreeSectionKind::Warnings => PyModelSection::FreeText {
                        kind: PySectionKind::Warnings,
                        body: body.into_pyobject(py)?.unbind(),
                        name: None,
                    },
                    model::FreeSectionKind::Todo => PyModelSection::FreeText {
                        kind: PySectionKind::Todo,
                        body: body.into_pyobject(py)?.unbind(),
                        name: None,
                    },
                    model::FreeSectionKind::Attention => PyModelSection::FreeText {
                        kind: PySectionKind::Attention,
                        body: body.into_pyobject(py)?.unbind(),
                        name: None,
                    },
                    model::FreeSectionKind::Caution => PyModelSection::FreeText {
                        kind: PySectionKind::Caution,
                        body: body.into_pyobject(py)?.unbind(),
                        name: None,
                    },
                    model::FreeSectionKind::Danger => PyModelSection::FreeText {
                        kind: PySectionKind::Danger,
                        body: body.into_pyobject(py)?.unbind(),
                        name: None,
                    },
                    model::FreeSectionKind::Error => PyModelSection::FreeText {
                        kind: PySectionKind::Error,
                        body: body.into_pyobject(py)?.unbind(),
                        name: None,
                    },
                    model::FreeSectionKind::Hint => PyModelSection::FreeText {
                        kind: PySectionKind::Hint,
                        body: body.into_pyobject(py)?.unbind(),
                        name: None,
                    },
                    model::FreeSectionKind::Important => PyModelSection::FreeText {
                        kind: PySectionKind::Important,
                        body: body.into_pyobject(py)?.unbind(),
                        name: None,
                    },
                    model::FreeSectionKind::Tip => PyModelSection::FreeText {
                        kind: PySectionKind::Tip,
                        body: body.into_pyobject(py)?.unbind(),
                        name: None,
                    },
                    model::FreeSectionKind::Unknown(name) => PyModelSection::FreeText {
                        kind: PySectionKind::Unknown,
                        body: body.into_pyobject(py)?.unbind(),
                        name: Some(name.into_pyobject(py)?.unbind()),
                    },
                }),
            }
        })
    }
}

impl TryInto<model::Section> for &PyModelSection {
    type Error = PyErr;

    fn try_into(self) -> Result<model::Section, Self::Error> {
        Python::attach(|py| -> Result<model::Section, Self::Error> {
            match self {
                PyModelSection::Parameters(params) => Ok(model::Section::Parameters(
                    params
                        .bind(py)
                        .iter()
                        .map(|param| param.cast::<PyModelParameter>()?.borrow().deref().try_into())
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                PyModelSection::KeywordParameters(params) => Ok(model::Section::KeywordParameters(
                    params
                        .bind(py)
                        .iter()
                        .map(|param| param.cast::<PyModelParameter>()?.borrow().deref().try_into())
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                PyModelSection::OtherParameters(params) => Ok(model::Section::OtherParameters(
                    params
                        .bind(py)
                        .iter()
                        .map(|param| param.cast::<PyModelParameter>()?.borrow().deref().try_into())
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                PyModelSection::Receives(params) => Ok(model::Section::Receives(
                    params
                        .bind(py)
                        .iter()
                        .map(|param| param.cast::<PyModelParameter>()?.borrow().deref().try_into())
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                PyModelSection::Returns(returns) => Ok(model::Section::Returns(
                    returns
                        .bind(py)
                        .iter()
                        .map(|ret| ret.cast::<PyModelReturn>()?.borrow().deref().try_into())
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                PyModelSection::Yields(returns) => Ok(model::Section::Yields(
                    returns
                        .bind(py)
                        .iter()
                        .map(|ret| ret.cast::<PyModelReturn>()?.borrow().deref().try_into())
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                PyModelSection::Raises(exceptions) => Ok(model::Section::Raises(
                    exceptions
                        .bind(py)
                        .iter()
                        .map(|ret| ret.cast::<PyModelExceptionEntry>()?.borrow().deref().try_into())
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                PyModelSection::Warns(exceptions) => Ok(model::Section::Warns(
                    exceptions
                        .bind(py)
                        .iter()
                        .map(|ret| ret.cast::<PyModelExceptionEntry>()?.borrow().deref().try_into())
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                PyModelSection::Attributes(attrs) => Ok(model::Section::Attributes(
                    attrs
                        .bind(py)
                        .iter()
                        .map(|ret| ret.cast::<PyModelAttribute>()?.borrow().deref().try_into())
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                PyModelSection::Methods(methods) => Ok(model::Section::Methods(
                    methods
                        .bind(py)
                        .iter()
                        .map(|ret| ret.cast::<PyModelMethod>()?.borrow().deref().try_into())
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                PyModelSection::SeeAlso(seealso) => Ok(model::Section::SeeAlso(
                    seealso
                        .bind(py)
                        .iter()
                        .map(|ret| ret.cast::<PyModelSeeAlsoEntry>()?.borrow().deref().try_into())
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                PyModelSection::References(refs) => Ok(model::Section::References(
                    refs.bind(py)
                        .iter()
                        .map(|ret| ret.cast::<PyModelReference>()?.borrow().deref().try_into())
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                PyModelSection::FreeText { kind, body, name } => Ok(match kind {
                    PySectionKind::Notes => model::Section::FreeText {
                        kind: model::FreeSectionKind::Notes,
                        body: body.extract(py)?,
                    },
                    PySectionKind::Examples => model::Section::FreeText {
                        kind: model::FreeSectionKind::Examples,
                        body: body.extract(py)?,
                    },
                    PySectionKind::Warnings => model::Section::FreeText {
                        kind: model::FreeSectionKind::Warnings,
                        body: body.extract(py)?,
                    },
                    PySectionKind::Todo => model::Section::FreeText {
                        kind: model::FreeSectionKind::Todo,
                        body: body.extract(py)?,
                    },
                    PySectionKind::Attention => model::Section::FreeText {
                        kind: model::FreeSectionKind::Attention,
                        body: body.extract(py)?,
                    },
                    PySectionKind::Caution => model::Section::FreeText {
                        kind: model::FreeSectionKind::Caution,
                        body: body.extract(py)?,
                    },
                    PySectionKind::Danger => model::Section::FreeText {
                        kind: model::FreeSectionKind::Danger,
                        body: body.extract(py)?,
                    },
                    PySectionKind::Error => model::Section::FreeText {
                        kind: model::FreeSectionKind::Error,
                        body: body.extract(py)?,
                    },
                    PySectionKind::Hint => model::Section::FreeText {
                        kind: model::FreeSectionKind::Hint,
                        body: body.extract(py)?,
                    },
                    PySectionKind::Important => model::Section::FreeText {
                        kind: model::FreeSectionKind::Important,
                        body: body.extract(py)?,
                    },
                    PySectionKind::Tip => model::Section::FreeText {
                        kind: model::FreeSectionKind::Tip,
                        body: body.extract(py)?,
                    },
                    PySectionKind::Unknown => model::Section::FreeText {
                        kind: model::FreeSectionKind::Unknown(
                            name.as_ref()
                                .ok_or(pyo3::exceptions::PyValueError::new_err(
                                    "Section(SectionKind.Unknown) requres a name.",
                                ))?
                                .extract(py)?,
                        ),
                        body: body.extract(py)?,
                    },
                    _ => unreachable!(),
                }),
            }
        })
    }
}

// ─── Model Docstring ─────────────────────────────────────────────────────────

#[pyclass(name = "Docstring")]
struct PyModelDocstring {
    summary: Option<Py<PyString>>,
    extended_summary: Option<Py<PyString>>,
    deprecation: Option<Py<PyModelDeprecation>>,
    sections: Py<PyList>,
}

impl PyModelDocstring {
    fn verify_sections(py: Python<'_>, sections: &Py<PyList>) -> PyResult<()> {
        if sections
            .bind(py)
            .into_iter()
            .any(|s| !s.is_instance_of::<PyModelSection>())
        {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "Docstring only accepts Sections in the 'sections' argument.".to_string(),
            ));
        }
        Ok(())
    }
}

#[pymethods]
impl PyModelDocstring {
    #[new]
    #[pyo3(signature = (*, summary=None, extended_summary=None, deprecation=None, sections=None))]
    fn new(
        py: Python<'_>,
        summary: Option<Py<PyString>>,
        extended_summary: Option<Py<PyString>>,
        deprecation: Option<Py<PyModelDeprecation>>,
        sections: Option<Py<PyList>>,
    ) -> PyResult<Self> {
        let sections = if let Some(sec) = sections {
            Self::verify_sections(py, &sec)?;
            sec
        } else {
            PyList::empty(py).unbind()
        };
        Ok(Self {
            summary,
            extended_summary,
            deprecation,
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

    #[getter]
    fn deprecation<'py>(&self, py: Python<'py>) -> Option<&Bound<'py, PyModelDeprecation>> {
        match &self.deprecation {
            Some(deprecation) => Some(deprecation.bind(py)),
            _ => None,
        }
    }
    #[setter]
    fn set_deprecation(&mut self, dep: Option<Py<PyModelDeprecation>>) {
        self.deprecation = dep;
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
    fn __repr__(&self) -> String {
        format!("Docstring(summary={:?})", self.summary)
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
                deprecation: docstr
                    .deprecation
                    .as_ref()
                    .map(|x| -> PyResult<_> { Py::new(py, PyModelDeprecation::try_from(x)?) })
                    .transpose()?,
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
                deprecation: self
                    .deprecation
                    .as_ref()
                    .map(|x| x.bind(py).borrow().deref().try_into())
                    .transpose()?,
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
    use pydocstring_core::syntax::SyntaxKind;
    let parsed = pydocstring_core::parse::parse(input);
    let kind = parsed.root().kind();
    match kind {
        SyntaxKind::GOOGLE_DOCSTRING => Ok(ParsedDocstring::Google(build_google_docstring(py, parsed)?)),
        SyntaxKind::NUMPY_DOCSTRING => Ok(ParsedDocstring::NumPy(build_numpy_docstring(py, parsed)?)),
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
    }
}

/// Emit a model `Docstring` as Google-style text.
#[pyfunction]
#[pyo3(name = "emit_google", signature = (doc, base_indent=0))]
fn py_emit_google(py: Python<'_>, doc: Py<PyModelDocstring>, base_indent: usize) -> PyResult<String> {
    Ok(pydocstring_core::emit::google::emit_google(
        &doc.borrow(py).deref().try_into()?,
        base_indent,
    ))
}

/// Emit a model `Docstring` as NumPy-style text.
#[pyfunction]
#[pyo3(name = "emit_numpy", signature = (doc, base_indent=0))]
fn py_emit_numpy(py: Python<'_>, doc: Py<PyModelDocstring>, base_indent: usize) -> PyResult<String> {
    Ok(pydocstring_core::emit::numpy::emit_numpy(
        &doc.borrow(py).deref().try_into()?,
        base_indent,
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
    google_section: bool,
    google_arg: bool,
    google_return: bool,
    google_yield: bool,
    google_exception: bool,
    google_warning: bool,
    google_see_also_item: bool,
    google_attribute: bool,
    google_method: bool,
    // Google (exit)
    exit_google_docstring: bool,
    exit_google_section: bool,
    exit_google_arg: bool,
    exit_google_return: bool,
    exit_google_yield: bool,
    exit_google_exception: bool,
    exit_google_warning: bool,
    exit_google_see_also_item: bool,
    exit_google_attribute: bool,
    exit_google_method: bool,
    // NumPy (enter)
    numpy_docstring: bool,
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
        google_section: has("enter_google_section"),
        google_arg: has("enter_google_arg"),
        google_return: has("enter_google_return"),
        google_yield: has("enter_google_yield"),
        google_exception: has("enter_google_exception"),
        google_warning: has("enter_google_warning"),
        google_see_also_item: has("enter_google_see_also_item"),
        google_attribute: has("enter_google_attribute"),
        google_method: has("enter_google_method"),
        // Google (exit)
        exit_google_docstring: has("exit_google_docstring"),
        exit_google_section: has("exit_google_section"),
        exit_google_arg: has("exit_google_arg"),
        exit_google_return: has("exit_google_return"),
        exit_google_yield: has("exit_google_yield"),
        exit_google_exception: has("exit_google_exception"),
        exit_google_warning: has("exit_google_warning"),
        exit_google_see_also_item: has("exit_google_see_also_item"),
        exit_google_attribute: has("exit_google_attribute"),
        exit_google_method: has("exit_google_method"),
        // NumPy (enter)
        numpy_docstring: has("enter_numpy_docstring"),
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

/// Walk the typed children of a Google section, dispatching visitor methods.
///
/// Each child collection is built at most once and shared between
/// `enter_google_section` and per-child `enter_google_*` calls via `clone_ref`.
/// Walk the children of a section node, dispatching visitor methods.
///
/// Accepts either a `GOOGLE_SECTION` or `NUMPY_SECTION` node.  The section
/// kind is read from `node.kind()` — no per-style function needed.
/// Each child collection is built at most once and shared between the
/// section object and per-child dispatches via `clone_ref`.

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

/// Iterate the children of `node` and dispatch each child via [`core_walk`].
/// Used by every `enter_*` / `exit_*` override in [`PyDispatcher`] to continue descent.
#[inline]
fn walk_children(source: &str, node: &SyntaxNode, dispatcher: &mut PyDispatcher<'_>) -> PyResult<()> {
    for child in node.children() {
        if let SyntaxElement::Node(n) = child {
            core_walk(source, n, dispatcher)?;
        }
    }
    Ok(())
}

/// Generates a `DocstringVisitor` method body for `PyDispatcher`.
///
/// Variant with children:
///   `visit_node!(self, source, ENTER_FIELD, EXIT_FIELD, build_expr, syntax_expr)`
///
/// Variant without children (Plain):
///   `visit_node!(self, source, ENTER_FIELD, EXIT_FIELD, build_expr)`
///
/// The method name strings are derived automatically via `concat!` / `stringify!`.
macro_rules! visit_node {
    // ── with children ────────────────────────────────────────────────────
    ($self:ident, $source:expr, $enter:ident, $exit:ident, $build:expr, $syntax:expr) => {{
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
        walk_children($source, $syntax, $self)?;
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
    ($self:ident, $source:expr, $enter:ident, $exit:ident, $build:expr) => {{
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

impl<'py> DocstringVisitor for PyDispatcher<'py> {
    type Error = PyErr;

    // ── Google ────────────────────────────────────────────────────────────
    fn visit_google_docstring(&mut self, source: &str, doc: &gn::GoogleDocstring<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            google_docstring,
            exit_google_docstring,
            build_google_docstring_node(self.py, doc, source, Arc::clone(&self.arc)),
            doc.syntax()
        )
    }

    fn visit_google_section(&mut self, source: &str, sec: &gn::GoogleSection<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            google_section,
            exit_google_section,
            build_google_section(self.py, sec, source),
            sec.syntax()
        )
    }

    fn visit_google_arg(&mut self, source: &str, arg: &gn::GoogleArg<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            google_arg,
            exit_google_arg,
            build_google_arg(self.py, arg, source),
            arg.syntax()
        )
    }

    fn visit_google_return(&mut self, source: &str, rtn: &gn::GoogleReturn<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            google_return,
            exit_google_return,
            build_google_return(self.py, rtn, source),
            rtn.syntax()
        )
    }

    fn visit_google_yield(&mut self, source: &str, yld: &gn::GoogleYield<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            google_yield,
            exit_google_yield,
            build_google_yield(self.py, yld, source),
            yld.syntax()
        )
    }

    fn visit_google_exception(&mut self, source: &str, exc: &gn::GoogleException<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            google_exception,
            exit_google_exception,
            build_google_exception(self.py, exc, source),
            exc.syntax()
        )
    }

    fn visit_google_warning(&mut self, source: &str, wrn: &gn::GoogleWarning<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            google_warning,
            exit_google_warning,
            build_google_warning(self.py, wrn, source),
            wrn.syntax()
        )
    }

    fn visit_google_see_also_item(&mut self, source: &str, sai: &gn::GoogleSeeAlsoItem<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            google_see_also_item,
            exit_google_see_also_item,
            build_google_see_also_item(self.py, sai, source),
            sai.syntax()
        )
    }

    fn visit_google_attribute(&mut self, source: &str, att: &gn::GoogleAttribute<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            google_attribute,
            exit_google_attribute,
            build_google_attribute(self.py, att, source),
            att.syntax()
        )
    }

    fn visit_google_method(&mut self, source: &str, mtd: &gn::GoogleMethod<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            google_method,
            exit_google_method,
            build_google_method(self.py, mtd, source),
            mtd.syntax()
        )
    }

    // ── NumPy ─────────────────────────────────────────────────────────────
    fn visit_numpy_docstring(&mut self, source: &str, doc: &nn::NumPyDocstring<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            numpy_docstring,
            exit_numpy_docstring,
            build_numpy_docstring_node(self.py, doc, source, Arc::clone(&self.arc)),
            doc.syntax()
        )
    }

    fn visit_numpy_deprecation(&mut self, source: &str, dep: &nn::NumPyDeprecation<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            numpy_deprecation,
            exit_numpy_deprecation,
            build_numpy_deprecation(self.py, dep, source),
            dep.syntax()
        )
    }

    fn visit_numpy_section(&mut self, source: &str, sec: &nn::NumPySection<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            numpy_section,
            exit_numpy_section,
            build_numpy_section(self.py, sec, source),
            sec.syntax()
        )
    }

    fn visit_numpy_parameter(&mut self, source: &str, prm: &nn::NumPyParameter<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            numpy_parameter,
            exit_numpy_parameter,
            build_numpy_parameter(self.py, prm, source),
            prm.syntax()
        )
    }

    fn visit_numpy_returns(&mut self, source: &str, rtn: &nn::NumPyReturns<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            numpy_returns,
            exit_numpy_returns,
            build_numpy_returns(self.py, rtn, source),
            rtn.syntax()
        )
    }

    fn visit_numpy_yields(&mut self, source: &str, yld: &nn::NumPyYields<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            numpy_yields,
            exit_numpy_yields,
            build_numpy_yields(self.py, yld, source),
            yld.syntax()
        )
    }

    fn visit_numpy_exception(&mut self, source: &str, exc: &nn::NumPyException<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            numpy_exception,
            exit_numpy_exception,
            build_numpy_exception(self.py, exc, source),
            exc.syntax()
        )
    }

    fn visit_numpy_warning(&mut self, source: &str, wrn: &nn::NumPyWarning<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            numpy_warning,
            exit_numpy_warning,
            build_numpy_warning(self.py, wrn, source),
            wrn.syntax()
        )
    }

    fn visit_numpy_see_also_item(&mut self, source: &str, sai: &nn::NumPySeeAlsoItem<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            numpy_see_also_item,
            exit_numpy_see_also_item,
            build_numpy_see_also_item(self.py, sai, source),
            sai.syntax()
        )
    }

    fn visit_numpy_reference(&mut self, source: &str, r#ref: &nn::NumPyReference<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            numpy_reference,
            exit_numpy_reference,
            build_numpy_reference(self.py, r#ref, source),
            r#ref.syntax()
        )
    }

    fn visit_numpy_attribute(&mut self, source: &str, att: &nn::NumPyAttribute<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            numpy_attribute,
            exit_numpy_attribute,
            build_numpy_attribute(self.py, att, source),
            att.syntax()
        )
    }

    fn visit_numpy_method(&mut self, source: &str, mtd: &nn::NumPyMethod<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            numpy_method,
            exit_numpy_method,
            build_numpy_method(self.py, mtd, source),
            mtd.syntax()
        )
    }

    // ── Plain ─────────────────────────────────────────────────────────────
    fn visit_plain_docstring(&mut self, source: &str, doc: &pn::PlainDocstring<'_>) -> Result<(), PyErr> {
        visit_node!(
            self,
            source,
            plain_docstring,
            exit_plain_docstring,
            build_plain_docstring_node(self.py, doc, source, Arc::clone(&self.arc))
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
/// class TypeAnnotationChecker:
///     def enter_google_arg(self, arg, ctx): ...
///     def enter_numpy_parameter(self, param, ctx): ...
///
/// for docstring_text in all_docstrings:
///     doc = pydocstring.parse(docstring_text)   # auto-detects style
///     checker = pydocstring.walk(doc, checker)  # returns visitor
/// ```
///
/// Google `enter_*` / `exit_*` methods:
/// `enter_google_docstring`, `enter_google_section`, `enter_google_arg`,
/// `enter_google_return`, `enter_google_yield`, `enter_google_exception`,
/// `enter_google_warning`, `enter_google_see_also_item`,
/// `enter_google_attribute`, `enter_google_method`
///
/// NumPy `enter_*` / `exit_*` methods:
/// `enter_numpy_docstring`, `enter_numpy_section`, `enter_numpy_deprecation`,
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
        d.borrow().parsed.clone()
    } else if let Ok(d) = bound.cast::<PyNumPyDocstring>() {
        d.borrow().parsed.clone()
    } else if let Ok(d) = bound.cast::<PyPlainDocstring>() {
        d.borrow().parsed.clone()
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

    core_walk(&source, root, &mut dispatcher)?;

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
    m.add_function(wrap_pyfunction!(walk, m)?)?;
    // Core types
    m.add_class::<PyStyle>()?;
    m.add_class::<PyGoogleSectionKind>()?;
    m.add_class::<PyNumPySectionKind>()?;
    m.add_class::<PyTextRange>()?;
    m.add_class::<PyLineColumn>()?;
    m.add_class::<PyToken>()?;
    m.add_class::<PyWalkContext>()?;
    // Google CST wrappers
    m.add_class::<PyGoogleDocstring>()?;
    m.add_class::<PyGoogleSection>()?;
    m.add_class::<PyGoogleArg>()?;
    m.add_class::<PyGoogleReturn>()?;
    m.add_class::<PyGoogleYield>()?;
    m.add_class::<PyGoogleException>()?;
    m.add_class::<PyGoogleWarning>()?;
    m.add_class::<PyGoogleSeeAlsoItem>()?;
    m.add_class::<PyGoogleAttribute>()?;
    m.add_class::<PyGoogleMethod>()?;
    // NumPy CST wrappers
    m.add_class::<PyNumPyDocstring>()?;
    m.add_class::<PyNumPySection>()?;
    m.add_class::<PyNumPyDeprecation>()?;
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
    m.add_class::<PyModelParameter>()?;
    m.add_class::<PyModelReturn>()?;
    m.add_class::<PyModelExceptionEntry>()?;
    m.add_class::<PyModelSeeAlsoEntry>()?;
    m.add_class::<PyModelReference>()?;
    m.add_class::<PyModelAttribute>()?;
    m.add_class::<PyModelMethod>()?;
    m.add_class::<PyModelDeprecation>()?;
    Ok(())
}
