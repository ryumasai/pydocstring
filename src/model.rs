//! Style-independent document model (IR) for docstrings.
//!
//! This module provides owned, editable data structures that represent the
//! semantic content of a docstring without being tied to any particular style
//! (Google, NumPy, etc.) or to source text positions.
//!
//! # Usage
//!
//! ```rust
//! use pydocstring::parse::google::{parse_google, to_model::to_model};
//!
//! let parsed = parse_google("Summary.\n\nArgs:\n    x (int): The value.\n");
//! let doc = to_model(&parsed).unwrap();
//! assert_eq!(doc.summary.as_deref(), Some("Summary."));
//! ```

// =============================================================================
// Docstring (root)
// =============================================================================

/// Style-independent representation of a parsed docstring.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Docstring {
    /// Brief one-line summary.
    pub summary: Option<String>,
    /// Extended description (may span multiple lines).
    pub extended_summary: Option<String>,
    /// Deprecation notice (NumPy `.. deprecated::` directive).
    pub deprecation: Option<Deprecation>,
    /// Ordered list of sections.
    pub sections: Vec<Section>,
}

// =============================================================================
// Section
// =============================================================================

/// A single section within a docstring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Section {
    /// `Args` / `Parameters` section.
    Parameters(Vec<Parameter>),
    /// `Keyword Args` / `Keyword Arguments` section (Google only).
    KeywordParameters(Vec<Parameter>),
    /// `Other Parameters` section.
    OtherParameters(Vec<Parameter>),
    /// `Receives` section.
    Receives(Vec<Parameter>),
    /// `Returns` section.
    Returns(Vec<Return>),
    /// `Yields` section.
    Yields(Vec<Return>),
    /// `Raises` section.
    Raises(Vec<ExceptionEntry>),
    /// `Warns` section.
    Warns(Vec<ExceptionEntry>),
    /// `Attributes` section.
    Attributes(Vec<Attribute>),
    /// `Methods` section.
    Methods(Vec<Method>),
    /// `See Also` section.
    SeeAlso(Vec<SeeAlsoEntry>),
    /// `References` section (NumPy structured references).
    References(Vec<Reference>),
    /// Free-text section (Notes, Examples, Warnings, etc.).
    FreeText {
        /// The kind of free-text section.
        kind: FreeSectionKind,
        /// The body text content.
        body: String,
    },
}

impl Section {
    /// Return the canonical section kind for this section.
    pub fn kind(&self) -> SectionKind {
        match self {
            Section::Parameters(_) => SectionKind::Parameters,
            Section::KeywordParameters(_) => SectionKind::KeywordParameters,
            Section::OtherParameters(_) => SectionKind::OtherParameters,
            Section::Receives(_) => SectionKind::Receives,
            Section::Returns(_) => SectionKind::Returns,
            Section::Yields(_) => SectionKind::Yields,
            Section::Raises(_) => SectionKind::Raises,
            Section::Warns(_) => SectionKind::Warns,
            Section::Attributes(_) => SectionKind::Attributes,
            Section::Methods(_) => SectionKind::Methods,
            Section::SeeAlso(_) => SectionKind::SeeAlso,
            Section::References(_) => SectionKind::References,
            Section::FreeText { kind, .. } => SectionKind::FreeText(kind.clone()),
        }
    }
}

// =============================================================================
// SectionKind
// =============================================================================

/// Unified section kind identifier (style-independent).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SectionKind {
    /// `Args` / `Parameters`
    Parameters,
    /// `Keyword Args` (Google only)
    KeywordParameters,
    /// `Other Parameters`
    OtherParameters,
    /// `Receives`
    Receives,
    /// `Returns`
    Returns,
    /// `Yields`
    Yields,
    /// `Raises`
    Raises,
    /// `Warns`
    Warns,
    /// `Attributes`
    Attributes,
    /// `Methods`
    Methods,
    /// `See Also`
    SeeAlso,
    /// `References`
    References,
    /// Free-text section
    FreeText(FreeSectionKind),
}

// =============================================================================
// FreeSectionKind
// =============================================================================

/// Kind of a free-text (non-structured) section.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FreeSectionKind {
    /// `Notes`
    Notes,
    /// `Examples`
    Examples,
    /// `Warnings`
    Warnings,
    /// `Todo` (Google only)
    Todo,
    /// `Attention` (Google only)
    Attention,
    /// `Caution` (Google only)
    Caution,
    /// `Danger` (Google only)
    Danger,
    /// `Error` (Google only)
    Error,
    /// `Hint` (Google only)
    Hint,
    /// `Important` (Google only)
    Important,
    /// `Tip` (Google only)
    Tip,
    /// Unrecognised section name.
    Unknown(String),
}

// =============================================================================
// Entry types
// =============================================================================

/// A parameter / argument entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// Parameter name(s). NumPy supports multiple names (`x, y`); Google always has one.
    pub names: Vec<String>,
    /// Type annotation (e.g. `int`, `Dict[str, Any]`).
    pub type_annotation: Option<String>,
    /// Description text (may be multi-line).
    pub description: Option<String>,
    /// Whether the parameter is marked `optional`.
    pub is_optional: bool,
    /// Default value text (NumPy `default: value` syntax).
    pub default_value: Option<String>,
}

/// A return / yield entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Return {
    /// Return name (NumPy supports named returns; Google does not).
    pub name: Option<String>,
    /// Type annotation.
    pub type_annotation: Option<String>,
    /// Description text.
    pub description: Option<String>,
}

/// An exception or warning entry (for Raises / Warns sections).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionEntry {
    /// Exception / warning type name (e.g. `ValueError`).
    pub type_name: String,
    /// Description text.
    pub description: Option<String>,
}

/// A see-also item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeeAlsoEntry {
    /// Referenced names (can be multiple, comma-separated).
    pub names: Vec<String>,
    /// Description text.
    pub description: Option<String>,
}

/// A reference entry (NumPy `.. [1] ...` style).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// Reference number (e.g. `"1"`).
    pub number: Option<String>,
    /// Reference content text.
    pub content: Option<String>,
}

/// An attribute entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// Attribute name.
    pub name: String,
    /// Type annotation.
    pub type_annotation: Option<String>,
    /// Description text.
    pub description: Option<String>,
}

/// A method entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    /// Method name.
    pub name: String,
    /// Type / signature info (Google puts this in brackets).
    pub type_annotation: Option<String>,
    /// Description text.
    pub description: Option<String>,
}

/// Deprecation notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deprecation {
    /// Version when deprecated (e.g. `"1.6.0"`).
    pub version: String,
    /// Description / reason for deprecation.
    pub description: Option<String>,
}
