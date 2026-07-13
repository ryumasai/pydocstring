//! Style-independent document model (IR) for docstrings.
//!
//! This module provides owned, editable data structures that represent the
//! semantic content of a docstring without being tied to any particular style
//! (Google, NumPy, etc.) or to source text positions.
//!
//! # Usage
//!
//! ```rust
//! use pydocstring::parse::parse;
//!
//! let doc = parse("Summary.\n\nArgs:\n    x (int): The value.\n").to_model();
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
    /// Document-level rST directives (`.. name:: argument`), in source
    /// order. The only directive the parsers produce today is
    /// `.. deprecated::`; see [`Docstring::deprecation`] for the shorthand.
    pub directives: Vec<Directive>,
    /// Ordered list of sections.
    pub sections: Vec<Section>,
}

impl Docstring {
    /// The deprecation notice, if present: the first directive named
    /// `deprecated` (its [`Directive::argument`] is the version).
    pub fn deprecation(&self) -> Option<&Directive> {
        self.directives.iter().find(|d| d.name == "deprecated")
    }
}

// =============================================================================
// Section
// =============================================================================

/// A single section within a docstring.
///
/// A section is a [`SectionKind`] paired with a flat sequence of [`Block`]s in
/// source order. Replacing the pre-0.4 role-keyed `enum Section`
/// (`Parameters(Vec<Parameter>)`, …), this completes the style-independent
/// unification: `kind` identifies the section and `blocks` carries its body
/// without baking the role into the shape.
///
/// The representation is permissive — nothing statically prevents a
/// [`Block::Parameter`] under a `Raises` section — consistent with the CST's
/// "permissive structure + documented interpretation" line; emitters render
/// every block totally.
///
/// `#[non_exhaustive]`: build one with [`Section::new`] or one of the
/// role-named constructors, not a struct literal, so that a future field is
/// not a breaking change. The rest of the model IR is deliberately *not*
/// `#[non_exhaustive]` — [`Docstring`], [`Parameter`] and the other entry
/// types are values you construct to feed [`emit`](crate::emit), and sealing
/// them would take struct-literal construction away with nothing to replace
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Section {
    /// The style-independent kind of this section.
    pub kind: SectionKind,
    /// The section body: a flat sequence of blocks in source order.
    pub blocks: Vec<Block>,
}

impl Section {
    /// Construct a section from its kind and block sequence.
    pub fn new(kind: SectionKind, blocks: Vec<Block>) -> Self {
        Self { kind, blocks }
    }

    /// A `Parameters` section wrapping each parameter in a [`Block::Parameter`].
    pub fn parameters(params: Vec<Parameter>) -> Self {
        Self::new(
            SectionKind::Parameters,
            params.into_iter().map(Block::Parameter).collect(),
        )
    }

    /// A `Keyword Parameters` section (Google `Keyword Args`).
    pub fn keyword_parameters(params: Vec<Parameter>) -> Self {
        Self::new(
            SectionKind::KeywordParameters,
            params.into_iter().map(Block::Parameter).collect(),
        )
    }

    /// An `Other Parameters` section.
    pub fn other_parameters(params: Vec<Parameter>) -> Self {
        Self::new(
            SectionKind::OtherParameters,
            params.into_iter().map(Block::Parameter).collect(),
        )
    }

    /// A `Receives` section.
    pub fn receives(params: Vec<Parameter>) -> Self {
        Self::new(
            SectionKind::Receives,
            params.into_iter().map(Block::Parameter).collect(),
        )
    }

    /// A `Returns` section wrapping each entry in a [`Block::Return`].
    pub fn returns(returns: Vec<Return>) -> Self {
        Self::new(SectionKind::Returns, returns.into_iter().map(Block::Return).collect())
    }

    /// A `Yields` section.
    pub fn yields(returns: Vec<Return>) -> Self {
        Self::new(SectionKind::Yields, returns.into_iter().map(Block::Return).collect())
    }

    /// A `Raises` section wrapping each entry in a [`Block::Exception`].
    pub fn raises(entries: Vec<ExceptionEntry>) -> Self {
        Self::new(SectionKind::Raises, entries.into_iter().map(Block::Exception).collect())
    }

    /// A `Warns` section.
    pub fn warns(entries: Vec<ExceptionEntry>) -> Self {
        Self::new(SectionKind::Warns, entries.into_iter().map(Block::Exception).collect())
    }

    /// An `Attributes` section wrapping each entry in a [`Block::Attribute`].
    pub fn attributes(attrs: Vec<Attribute>) -> Self {
        Self::new(
            SectionKind::Attributes,
            attrs.into_iter().map(Block::Attribute).collect(),
        )
    }

    /// A `Methods` section wrapping each entry in a [`Block::Method`].
    pub fn methods(methods: Vec<Method>) -> Self {
        Self::new(SectionKind::Methods, methods.into_iter().map(Block::Method).collect())
    }

    /// A `See Also` section wrapping each entry in a [`Block::SeeAlso`].
    pub fn see_also(items: Vec<SeeAlsoEntry>) -> Self {
        Self::new(SectionKind::SeeAlso, items.into_iter().map(Block::SeeAlso).collect())
    }

    /// A `References` section wrapping each entry in a [`Block::Reference`].
    pub fn references(refs: Vec<Reference>) -> Self {
        Self::new(
            SectionKind::References,
            refs.into_iter().map(Block::Reference).collect(),
        )
    }

    /// A free-text section (Notes, Examples, …) whose body is a single
    /// [`Block::Paragraph`]. (The full dissolve of free-text bodies into
    /// multiple blocks is deferred to Phase 4 increment E.)
    pub fn free_text(kind: FreeSectionKind, body: String) -> Self {
        Self::new(SectionKind::FreeText(kind), vec![Block::Paragraph(body)])
    }
}

/// A single body block within a [`Section`], in source order.
///
/// A structured section body is a flat run of blocks: prose [`Block::Paragraph`]s
/// interleaved with typed entries. Prose is a *model-layer* notion (#104): the
/// CST keeps every base-indent body line as an `ENTRY` (the predictable
/// napoleon/numpydoc line grammar), and `to_model` decides which bare entries
/// read as paragraphs.
///
/// `#[non_exhaustive]`: new block shapes (e.g. reST fields, literal/doctest
/// blocks) may be added in minor releases, so downstream `match`es need a
/// wildcard arm.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Block {
    /// A prose paragraph (a section intro, a between-entries note, or a
    /// free-text section body).
    Paragraph(String),
    /// A parameter / argument entry.
    Parameter(Parameter),
    /// A return / yield entry.
    Return(Return),
    /// An exception / warning entry.
    Exception(ExceptionEntry),
    /// An attribute entry.
    Attribute(Attribute),
    /// A method entry.
    Method(Method),
    /// A see-also entry.
    SeeAlso(SeeAlsoEntry),
    /// A reference / citation entry.
    Reference(Reference),
}

impl Block {
    /// The [`Parameter`] if this is a [`Block::Parameter`].
    pub fn as_parameter(&self) -> Option<&Parameter> {
        match self {
            Block::Parameter(p) => Some(p),
            _ => None,
        }
    }

    /// The [`Return`] if this is a [`Block::Return`].
    pub fn as_return(&self) -> Option<&Return> {
        match self {
            Block::Return(r) => Some(r),
            _ => None,
        }
    }

    /// The [`ExceptionEntry`] if this is a [`Block::Exception`].
    pub fn as_exception(&self) -> Option<&ExceptionEntry> {
        match self {
            Block::Exception(e) => Some(e),
            _ => None,
        }
    }

    /// The [`Attribute`] if this is a [`Block::Attribute`].
    pub fn as_attribute(&self) -> Option<&Attribute> {
        match self {
            Block::Attribute(a) => Some(a),
            _ => None,
        }
    }

    /// The [`Method`] if this is a [`Block::Method`].
    pub fn as_method(&self) -> Option<&Method> {
        match self {
            Block::Method(m) => Some(m),
            _ => None,
        }
    }

    /// The [`SeeAlsoEntry`] if this is a [`Block::SeeAlso`].
    pub fn as_see_also(&self) -> Option<&SeeAlsoEntry> {
        match self {
            Block::SeeAlso(s) => Some(s),
            _ => None,
        }
    }

    /// The [`Reference`] if this is a [`Block::Reference`].
    pub fn as_reference(&self) -> Option<&Reference> {
        match self {
            Block::Reference(r) => Some(r),
            _ => None,
        }
    }

    /// The prose text if this is a [`Block::Paragraph`].
    pub fn as_paragraph(&self) -> Option<&str> {
        match self {
            Block::Paragraph(p) => Some(p),
            _ => None,
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
    /// Citation label (`1`, `CIT2002`, `#f1`, … — the text inside
    /// `.. [label]`). Renamed from `number` in 0.3.0: labels are not always
    /// numeric.
    pub label: Option<String>,
    /// Reference content text.
    pub content: Option<String>,
}

/// An attribute entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// Attribute name(s). NumPy supports multiple names (`jac, hess`), like
    /// [`Parameter::names`]; Google always has one. Renamed from `name` in
    /// 0.3.0: keeping only the first name dropped the rest (#89).
    pub names: Vec<String>,
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

/// A document-level rST directive (`.. name:: argument` + indented body).
///
/// Generalizes the pre-0.3.0 `Deprecation` struct (which this replaces): a
/// deprecation notice is a `Directive` with `name == "deprecated"` whose
/// `argument` is the version. See [`Docstring::deprecation`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Directive {
    /// Directive name (e.g. `"deprecated"`).
    pub name: String,
    /// Directive argument (e.g. the version of a `.. deprecated::`).
    pub argument: Option<String>,
    /// Directive body / description text.
    pub description: Option<String>,
}
