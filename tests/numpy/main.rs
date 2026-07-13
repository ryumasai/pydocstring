//! Integration tests for the NumPy-style docstring parser.
//!
//! Exhaustive input coverage lives in tests/corpus/numpy/ + tests/snapshots.rs;
//! the tests here pin deliberate spec decisions and the typed-accessor contract.
//!
//! The per-style CST wrappers are crate-private (#119); these tests read the
//! tree through the public lenses: the unified typed views
//! ([`Document`] / [`Section`] / [`Entry`] / [`Citation`] / [`Directive`]) and,
//! for anything the unified view does not name (punctuation, header
//! underlines, brackets), the raw [`SyntaxNode`] reachable via `.syntax()`.

pub use pydocstring::emit::EmitOptions;
pub use pydocstring::emit::numpy::emit_numpy;
pub use pydocstring::model::Docstring;
pub use pydocstring::model::FreeSectionKind;
pub use pydocstring::model::Section as ModelSection;
pub use pydocstring::model::SectionKind;
pub use pydocstring::parse::TextBlock;
pub use pydocstring::parse::parse_numpy;
pub use pydocstring::parse::unified::Citation;
pub use pydocstring::parse::unified::Directive;
pub use pydocstring::parse::unified::Document;
pub use pydocstring::parse::unified::Entry;
pub use pydocstring::parse::unified::Section;
pub use pydocstring::syntax::Parsed;
pub use pydocstring::syntax::SyntaxKind;
pub use pydocstring::syntax::SyntaxNode;
pub use pydocstring::syntax::SyntaxToken;
pub use pydocstring::text::TextSize;

mod edge_cases;
mod freetext;
mod parameters;
mod raises;
mod returns;
mod sections;
mod structured;
mod summary;

// =============================================================================
// Shared helpers
// =============================================================================

/// Get the unified `Document` view of a `Parsed` result.
pub fn doc(result: &Parsed) -> Document<'_> {
    Document::new(result)
}

/// Extract all sections from a docstring.
pub fn all_sections<'a>(result: &'a Parsed) -> Vec<Section<'a>> {
    doc(result).sections().collect()
}

/// All entries of every section of `kind`, in source order.
///
/// Every entry role (parameter, return, yield, exception, warning, attribute,
/// method, "See Also" item) is the same `ENTRY` node; the role is the parent
/// section's kind, so one helper covers them all.
pub fn entries_of<'a>(result: &'a Parsed, kind: SectionKind) -> Vec<Entry<'a>> {
    doc(result)
        .sections()
        .filter(|s| s.kind() == kind)
        .flat_map(|s| s.entries().collect::<Vec<_>>())
        .collect()
}

pub fn parameters<'a>(result: &'a Parsed) -> Vec<Entry<'a>> {
    entries_of(result, SectionKind::Parameters)
}

pub fn returns<'a>(result: &'a Parsed) -> Vec<Entry<'a>> {
    entries_of(result, SectionKind::Returns)
}

pub fn yields<'a>(result: &'a Parsed) -> Vec<Entry<'a>> {
    entries_of(result, SectionKind::Yields)
}

pub fn raises<'a>(result: &'a Parsed) -> Vec<Entry<'a>> {
    entries_of(result, SectionKind::Raises)
}

pub fn warns<'a>(result: &'a Parsed) -> Vec<Entry<'a>> {
    entries_of(result, SectionKind::Warns)
}

pub fn see_also<'a>(result: &'a Parsed) -> Vec<Entry<'a>> {
    entries_of(result, SectionKind::SeeAlso)
}

pub fn receives<'a>(result: &'a Parsed) -> Vec<Entry<'a>> {
    entries_of(result, SectionKind::Receives)
}

pub fn attributes<'a>(result: &'a Parsed) -> Vec<Entry<'a>> {
    entries_of(result, SectionKind::Attributes)
}

pub fn methods<'a>(result: &'a Parsed) -> Vec<Entry<'a>> {
    entries_of(result, SectionKind::Methods)
}

/// References sections carry `CITATION` nodes, never entries.
pub fn references<'a>(result: &'a Parsed) -> Vec<Citation<'a>> {
    doc(result)
        .sections()
        .filter(|s| s.kind() == SectionKind::References)
        .flat_map(|s| s.citations().collect::<Vec<_>>())
        .collect()
}

pub fn notes<'a>(result: &'a Parsed) -> Option<TextBlock<'a>> {
    doc(result)
        .sections()
        .find(|s| s.kind() == SectionKind::FreeText(FreeSectionKind::Notes))
        .and_then(|s| s.body())
}

// -----------------------------------------------------------------------------
// Raw-tree helpers for what the unified view does not name
// -----------------------------------------------------------------------------

/// The `SECTION_HEADER` child of a section.
pub fn header<'a>(section: &Section<'a>) -> &'a SyntaxNode {
    section
        .syntax()
        .find_node(SyntaxKind::SECTION_HEADER)
        .expect("SECTION must have a SECTION_HEADER child")
}

/// The header underline text (`----------`) of a NumPy section.
pub fn header_underline<'a>(result: &'a Parsed, section: &Section<'a>) -> &'a str {
    header(section)
        .find_token(SyntaxKind::UNDERLINE)
        .expect("NumPy SECTION_HEADER must have an UNDERLINE token")
        .text(result.source())
}

/// The `COLON` token of an entry, if the source wrote one.
pub fn colon<'a>(entry: &Entry<'a>) -> Option<&'a SyntaxToken> {
    entry.syntax().find_token(SyntaxKind::COLON)
}
