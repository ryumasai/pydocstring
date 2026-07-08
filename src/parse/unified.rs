//! Style-independent typed views over the unified syntax tree.
//!
//! Every docstring style produces the same node vocabulary
//! ([`SyntaxKind::DOCUMENT`] / [`SyntaxKind::SECTION`] /
//! [`SyntaxKind::ENTRY`] / [`SyntaxKind::DIRECTIVE`] /
//! [`SyntaxKind::CITATION`]), so one set of zero-copy views — [`Document`],
//! [`Section`], [`Entry`], [`Directive`], [`Citation`], [`DefaultMarker`] — walks
//! any of them. This is the single code path over docstring structure: parse
//! with [`parse`](crate::parse::parse) (auto-detecting the style) and
//! traverse sections and entries uniformly, without per-style types.
//!
//! # Example
//!
//! The same traversal works for a Google-style and a NumPy-style docstring:
//!
//! ```rust
//! use pydocstring::model::SectionKind;
//! use pydocstring::parse::parse;
//! use pydocstring::parse::unified::Document;
//!
//! fn parameter_names(source: &str) -> Vec<String> {
//!     let parsed = parse(source);
//!     let doc = Document::new(&parsed);
//!     doc.sections()
//!         .filter(|s| s.kind(parsed.source()) == SectionKind::Parameters)
//!         .flat_map(|s| s.entries().collect::<Vec<_>>())
//!         .filter_map(|e| e.name().map(|n| n.text(parsed.source()).to_owned()))
//!         .collect()
//! }
//!
//! let google = "Summary.\n\nArgs:\n    x (int): The value.\n    y: Another.\n";
//! let numpy = "Summary.\n\nParameters\n----------\nx : int\n    The value.\ny\n    Another.\n";
//! assert_eq!(parameter_names(google), vec!["x", "y"]);
//! assert_eq!(parameter_names(numpy), vec!["x", "y"]);
//! ```
//!
//! Repeatable markers are exposed per occurrence — [`Entry::defaults`] yields
//! one [`Default`] per `default …` marker; [`Entry::default_value`] is the
//! first-occurrence shorthand that matches the model's normalization rule:
//!
//! ```rust
//! use pydocstring::parse::parse;
//! use pydocstring::parse::unified::Document;
//!
//! let parsed = parse("Summary.\n\nParameters\n----------\nx : int, default 1, default 2\n    Desc.\n");
//! let doc = Document::new(&parsed);
//! let entry = doc.sections().next().unwrap().entries().next().unwrap();
//! assert_eq!(entry.defaults().count(), 2);
//! assert_eq!(entry.default_value().unwrap().text(parsed.source()), "1");
//! ```

use crate::model::FreeSectionKind;
use crate::model::SectionKind;
use crate::parse::Style;
use crate::parse::google::kind::GoogleSectionKind;
use crate::parse::numpy::kind::NumPySectionKind;
use crate::parse::text_block::TextBlock;
use crate::parse::text_block::find_text_block;
use crate::syntax::Parsed;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::syntax::SyntaxToken;
use crate::text::TextRange;

// =============================================================================
// Document
// =============================================================================

/// Style-independent view of a parsed docstring root.
///
/// Construct with [`Document::new`] from any [`Parsed`] result, whatever the
/// source style.
#[derive(Debug, Clone, Copy)]
pub struct Document<'a> {
    root: &'a SyntaxNode,
    style: Style,
}

impl<'a> Document<'a> {
    /// View the root of `parsed` as a style-independent document.
    pub fn new(parsed: &'a Parsed) -> Self {
        debug_assert_eq!(parsed.root().kind(), SyntaxKind::DOCUMENT);
        Self {
            root: parsed.root(),
            style: parsed.style(),
        }
    }

    /// The style the docstring was parsed as.
    pub fn style(&self) -> Style {
        self.style
    }

    /// Access the underlying `SyntaxNode`.
    pub fn syntax(&self) -> &'a SyntaxNode {
        self.root
    }

    /// Brief summary block, if present.
    pub fn summary(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.root, SyntaxKind::SUMMARY)
    }

    /// Extended summary block, if present.
    pub fn extended_summary(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.root, SyntaxKind::EXTENDED_SUMMARY)
    }

    /// Iterate over document-level directives (e.g. `.. deprecated:: 1.0`).
    pub fn directives(&self) -> impl Iterator<Item = Directive<'a>> {
        self.root.nodes(SyntaxKind::DIRECTIVE).map(Directive)
    }

    /// Iterate over all sections, in source order.
    pub fn sections(&self) -> impl Iterator<Item = Section<'a>> {
        let style = self.style;
        self.root
            .nodes(SyntaxKind::SECTION)
            .map(move |node| Section { node, style })
    }

    /// Iterate over stray-prose paragraph blocks (`PARAGRAPH` nodes) between
    /// sections, in source order.
    pub fn paragraphs(&self) -> impl Iterator<Item = TextBlock<'a>> {
        self.root.nodes(SyntaxKind::PARAGRAPH).filter_map(TextBlock::cast)
    }
}

// =============================================================================
// Section
// =============================================================================

/// Style-independent view of a `SECTION` node.
#[derive(Debug, Clone, Copy)]
pub struct Section<'a> {
    node: &'a SyntaxNode,
    style: Style,
}

impl<'a> Section<'a> {
    /// Access the underlying `SyntaxNode`.
    pub fn syntax(&self) -> &'a SyntaxNode {
        self.node
    }

    /// The source range of this section.
    pub fn range(&self) -> &'a TextRange {
        self.node.range()
    }

    /// The header name text (e.g. `"Args"`, `"Parameters"`).
    pub fn header_name(&self, source: &'a str) -> &'a str {
        self.node
            .find_node(SyntaxKind::SECTION_HEADER)
            .expect("SECTION must have a SECTION_HEADER child")
            .required_token(SyntaxKind::NAME)
            .text(source)
    }

    /// The style-independent [`SectionKind`] of this section, resolved from
    /// the header name via the source style's section-name table.
    pub fn kind(&self, source: &str) -> SectionKind {
        let name = self.header_name(source);
        let lower = name.to_ascii_lowercase();
        match self.style {
            Style::Google => GoogleSectionKind::from_name(&lower).to_section_kind(name),
            Style::NumPy => NumPySectionKind::from_name(&lower).to_section_kind(name),
            // Plain docstrings produce no SECTION nodes; unreachable in
            // practice, but total: report an unknown free-text section.
            Style::Plain => SectionKind::FreeText(FreeSectionKind::Unknown(name.to_owned())),
        }
    }

    /// Iterate over the section's `ENTRY` nodes, in source order.
    pub fn entries(&self) -> impl Iterator<Item = Entry<'a>> {
        self.node.nodes(SyntaxKind::ENTRY).map(Entry)
    }

    /// Free-text body block (the `DESCRIPTION` child of a free-text
    /// section), if present.
    pub fn body(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.node, SyntaxKind::DESCRIPTION)
    }

    /// Iterate over the section's `CITATION` nodes (References sections).
    pub fn citations(&self) -> impl Iterator<Item = Citation<'a>> {
        self.node.nodes(SyntaxKind::CITATION).map(Citation)
    }
}

// =============================================================================
// Entry
// =============================================================================

/// Style-independent view of an `ENTRY` node (a parameter, return, yield,
/// exception, warning, attribute, method, or "See Also" item).
#[derive(Debug, Clone, Copy)]
pub struct Entry<'a>(&'a SyntaxNode);

impl<'a> Entry<'a> {
    /// Try to cast a `SyntaxNode` reference into this typed wrapper.
    pub fn cast(node: &'a SyntaxNode) -> Option<Self> {
        (node.kind() == SyntaxKind::ENTRY).then_some(Self(node))
    }

    /// Access the underlying `SyntaxNode`.
    pub fn syntax(&self) -> &'a SyntaxNode {
        self.0
    }

    /// The source range of this entry.
    pub fn range(&self) -> &'a TextRange {
        self.0.range()
    }

    /// All `NAME` tokens (entries can declare several comma-separated names).
    pub fn names(&self) -> impl Iterator<Item = &'a SyntaxToken> {
        self.0.tokens(SyntaxKind::NAME)
    }

    /// The first `NAME` token, if any (exception/warning entries carry a
    /// type instead of a name).
    pub fn name(&self) -> Option<&'a SyntaxToken> {
        self.names().next()
    }

    /// The type annotation token, if present: the `TYPE` of a parameter /
    /// attribute / exception entry, or the `RETURN_TYPE` of a return /
    /// yield entry.
    pub fn type_annotation(&self) -> Option<&'a SyntaxToken> {
        self.0
            .find_token(SyntaxKind::TYPE)
            .or_else(|| self.0.find_token(SyntaxKind::RETURN_TYPE))
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }

    /// Whether the entry carries at least one `optional` marker.
    pub fn is_optional(&self) -> bool {
        self.optionals().next().is_some()
    }

    /// All `optional` marker tokens, one per occurrence, in source order.
    pub fn optionals(&self) -> impl Iterator<Item = &'a SyntaxToken> {
        self.0.tokens(SyntaxKind::OPTIONAL)
    }

    /// All `default …` markers, one [`Default`] per occurrence, in source
    /// order.
    pub fn defaults(&self) -> impl Iterator<Item = DefaultMarker<'a>> {
        self.0.nodes(SyntaxKind::DEFAULT).map(DefaultMarker)
    }

    /// The first `default …` marker's value token, if present.
    ///
    /// First occurrence wins — the same normalization rule the model layer
    /// applies. Use [`Entry::defaults`] to see every occurrence.
    pub fn default_value(&self) -> Option<&'a SyntaxToken> {
        self.defaults().next().and_then(|d| d.value())
    }
}

// =============================================================================
// DefaultMarker
// =============================================================================

/// Typed view of one `DEFAULT` marker node (`default X` / `default=X` /
/// `default: X` inside a type annotation).
#[derive(Debug, Clone, Copy)]
pub struct DefaultMarker<'a>(pub(crate) &'a SyntaxNode);

impl<'a> DefaultMarker<'a> {
    /// Try to cast a `SyntaxNode` reference into this typed wrapper.
    pub fn cast(node: &'a SyntaxNode) -> Option<Self> {
        (node.kind() == SyntaxKind::DEFAULT).then_some(Self(node))
    }

    /// Access the underlying `SyntaxNode`.
    pub fn syntax(&self) -> &'a SyntaxNode {
        self.0
    }

    /// The source range of this marker.
    pub fn range(&self) -> &'a TextRange {
        self.0.range()
    }

    /// The `default` keyword token.
    pub fn keyword(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::DEFAULT_KEYWORD)
    }

    /// The `=` / `:` separator token, if present.
    pub fn separator(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DEFAULT_SEPARATOR)
    }

    /// The value token, if present (zero-length placeholders excluded).
    pub fn value(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DEFAULT_VALUE)
    }
}

// =============================================================================
// Directive
// =============================================================================

/// Style-independent view of a `DIRECTIVE` node
/// (e.g. `.. deprecated:: 1.6.0`).
#[derive(Debug, Clone, Copy)]
pub struct Directive<'a>(pub(crate) &'a SyntaxNode);

impl<'a> Directive<'a> {
    /// Try to cast a `SyntaxNode` reference into this typed wrapper.
    pub fn cast(node: &'a SyntaxNode) -> Option<Self> {
        (node.kind() == SyntaxKind::DIRECTIVE).then_some(Self(node))
    }

    /// Access the underlying `SyntaxNode`.
    pub fn syntax(&self) -> &'a SyntaxNode {
        self.0
    }

    /// The source range of this directive.
    pub fn range(&self) -> &'a TextRange {
        self.0.range()
    }

    /// The directive name token (e.g. `deprecated`).
    pub fn name(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::DIRECTIVE_NAME)
    }

    /// The directive argument token (e.g. the version of a
    /// `.. deprecated::`), if present.
    pub fn argument(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::ARGUMENT)
    }

    /// The directive body / description block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// Citation
// =============================================================================

/// Style-independent view of a `CITATION` node (`.. [label] content` in a
/// References section, or a plain-text reference line).
#[derive(Debug, Clone, Copy)]
pub struct Citation<'a>(pub(crate) &'a SyntaxNode);

impl<'a> Citation<'a> {
    /// Try to cast a `SyntaxNode` reference into this typed wrapper.
    pub fn cast(node: &'a SyntaxNode) -> Option<Self> {
        (node.kind() == SyntaxKind::CITATION).then_some(Self(node))
    }

    /// Access the underlying `SyntaxNode`.
    pub fn syntax(&self) -> &'a SyntaxNode {
        self.0
    }

    /// The source range of this citation.
    pub fn range(&self) -> &'a TextRange {
        self.0.range()
    }

    /// The citation label token (`1`, `CIT2002`, `#f1`, …), if present.
    pub fn label(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::LABEL)
    }

    /// The citation content block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}
