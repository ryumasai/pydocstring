//! Integration tests for Google-style docstring parser.
//!
//! Division of labor: exhaustive input coverage lives in tests/corpus/google/
//! and tests/snapshots.rs (full CST and emit pinned per corpus file). The
//! modules here pin deliberate spec decisions and the typed-accessor contract.
//!
//! The per-style wrappers (`GoogleArg`, `GoogleSection`, …) are crate-private
//! (#119); these tests read the tree through the public lenses only: the
//! unified view ([`Document`] / [`Section`] / [`Entry`] / [`Citation`] /
//! [`Directive`]) and the raw CST reachable via `.syntax()`.

pub use pydocstring::model::FreeSectionKind;
pub use pydocstring::model::SectionKind;
pub use pydocstring::parse::TextBlock;
pub use pydocstring::parse::google::parse_google;
pub use pydocstring::parse::unified::Citation;
pub use pydocstring::parse::unified::Directive;
pub use pydocstring::parse::unified::Document;
pub use pydocstring::parse::unified::Entry;
pub use pydocstring::parse::unified::Section;
pub use pydocstring::syntax::Parsed;
pub use pydocstring::syntax::SyntaxKind;
pub use pydocstring::syntax::SyntaxNode;
pub use pydocstring::text::TextSize;

mod args;
mod edge_cases;
mod freetext;
mod raises;
mod returns;
mod sections;
mod structured;
mod summary;

// =============================================================================
// Shared helpers
// =============================================================================

/// Get the style-independent `Document` view of a `Parsed` result.
pub fn doc(result: &Parsed) -> Document<'_> {
    Document::new(result)
}

pub fn all_sections(result: &Parsed) -> Vec<Section<'_>> {
    doc(result).sections().collect()
}

/// The `SECTION_HEADER` node of a section (raw CST: the unified view exposes
/// only the header *name*).
pub fn header<'a>(section: &Section<'a>) -> &'a SyntaxNode {
    section
        .syntax()
        .find_node(SyntaxKind::SECTION_HEADER)
        .expect("SECTION must have a SECTION_HEADER child")
}

/// All entries of every section whose kind is `kind`, in source order.
pub fn entries_of(result: &Parsed, kind: SectionKind) -> Vec<Entry<'_>> {
    doc(result)
        .sections()
        .filter(|s| s.kind() == kind)
        .flat_map(|s| s.entries().collect::<Vec<_>>())
        .collect()
}

/// The body text of the first section whose kind is `kind`.
pub fn body_of(result: &Parsed, kind: SectionKind) -> Option<TextBlock<'_>> {
    doc(result).sections().find(|s| s.kind() == kind).and_then(|s| s.body())
}

pub fn args(result: &Parsed) -> Vec<Entry<'_>> {
    entries_of(result, SectionKind::Parameters)
}

pub fn returns(result: &Parsed) -> Option<Entry<'_>> {
    entries_of(result, SectionKind::Returns).into_iter().next()
}

pub fn yields(result: &Parsed) -> Option<Entry<'_>> {
    entries_of(result, SectionKind::Yields).into_iter().next()
}

pub fn raises(result: &Parsed) -> Vec<Entry<'_>> {
    entries_of(result, SectionKind::Raises)
}

pub fn attributes(result: &Parsed) -> Vec<Entry<'_>> {
    entries_of(result, SectionKind::Attributes)
}

pub fn keyword_args(result: &Parsed) -> Vec<Entry<'_>> {
    entries_of(result, SectionKind::KeywordParameters)
}

pub fn receives(result: &Parsed) -> Vec<Entry<'_>> {
    entries_of(result, SectionKind::Receives)
}

pub fn warns(result: &Parsed) -> Vec<Entry<'_>> {
    entries_of(result, SectionKind::Warns)
}

pub fn see_also(result: &Parsed) -> Vec<Entry<'_>> {
    entries_of(result, SectionKind::SeeAlso)
}

pub fn methods(result: &Parsed) -> Vec<Entry<'_>> {
    entries_of(result, SectionKind::Methods)
}

pub fn references(result: &Parsed) -> Vec<Citation<'_>> {
    doc(result)
        .sections()
        .filter(|s| s.kind() == SectionKind::References)
        .flat_map(|s| s.citations().collect::<Vec<_>>())
        .collect()
}

pub fn notes(result: &Parsed) -> Option<TextBlock<'_>> {
    body_of(result, SectionKind::FreeText(FreeSectionKind::Notes))
}

pub fn examples(result: &Parsed) -> Option<TextBlock<'_>> {
    body_of(result, SectionKind::FreeText(FreeSectionKind::Examples))
}
