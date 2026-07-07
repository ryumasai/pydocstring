//! Integration tests for Google-style docstring parser.
//!
//! Division of labor: exhaustive input coverage lives in tests/corpus/google/
//! and tests/snapshots.rs (full CST and emit pinned per corpus file). The
//! modules here pin deliberate spec decisions and the typed-accessor contract.

pub use pydocstring::parse::google::{
    GoogleArg, GoogleAttribute, GoogleDocstring, GoogleException, GoogleMethod, GoogleReference, GoogleReturn,
    GoogleSection, GoogleSectionKind, GoogleSeeAlsoItem, GoogleWarning, GoogleYield, parse_google,
};
pub use pydocstring::syntax::{Parsed, SyntaxKind, SyntaxToken};
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

/// Get the typed GoogleDocstring wrapper from a Parsed result.
pub fn doc(result: &Parsed) -> GoogleDocstring<'_> {
    GoogleDocstring::cast(result.root()).unwrap()
}

pub fn all_sections<'a>(result: &'a Parsed) -> Vec<GoogleSection<'a>> {
    doc(result).sections().collect()
}

pub fn args<'a>(result: &'a Parsed) -> Vec<GoogleArg<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), GoogleSectionKind::Args))
        .flat_map(|s| s.args().collect::<Vec<_>>())
        .collect()
}

pub fn returns<'a>(result: &'a Parsed) -> Option<GoogleReturn<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), GoogleSectionKind::Returns))
        .find_map(|s| s.returns())
}

pub fn yields<'a>(result: &'a Parsed) -> Option<GoogleYield<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), GoogleSectionKind::Yields))
        .find_map(|s| s.yields())
}

pub fn raises<'a>(result: &'a Parsed) -> Vec<GoogleException<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), GoogleSectionKind::Raises))
        .flat_map(|s| s.exceptions().collect::<Vec<_>>())
        .collect()
}

pub fn attributes<'a>(result: &'a Parsed) -> Vec<GoogleAttribute<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), GoogleSectionKind::Attributes))
        .flat_map(|s| s.attributes().collect::<Vec<_>>())
        .collect()
}

pub fn keyword_args<'a>(result: &'a Parsed) -> Vec<GoogleArg<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), GoogleSectionKind::KeywordArgs))
        .flat_map(|s| s.args().collect::<Vec<_>>())
        .collect()
}

pub fn receives<'a>(result: &'a Parsed) -> Vec<GoogleArg<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), GoogleSectionKind::Receives))
        .flat_map(|s| s.args().collect::<Vec<_>>())
        .collect()
}

pub fn warns<'a>(result: &'a Parsed) -> Vec<GoogleWarning<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), GoogleSectionKind::Warns))
        .flat_map(|s| s.warnings().collect::<Vec<_>>())
        .collect()
}

pub fn see_also<'a>(result: &'a Parsed) -> Vec<GoogleSeeAlsoItem<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), GoogleSectionKind::SeeAlso))
        .flat_map(|s| s.see_also_items().collect::<Vec<_>>())
        .collect()
}

pub fn methods<'a>(result: &'a Parsed) -> Vec<GoogleMethod<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), GoogleSectionKind::Methods))
        .flat_map(|s| s.methods().collect::<Vec<_>>())
        .collect()
}

pub fn references<'a>(result: &'a Parsed) -> Vec<GoogleReference<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), GoogleSectionKind::References))
        .flat_map(|s| s.references().collect::<Vec<_>>())
        .collect()
}

pub fn notes(result: &Parsed) -> Option<&SyntaxToken> {
    doc(result)
        .sections()
        .find(|s| matches!(s.section_kind(result.source()), GoogleSectionKind::Notes))
        .and_then(|s| s.body_text())
}

pub fn examples(result: &Parsed) -> Option<&SyntaxToken> {
    doc(result)
        .sections()
        .find(|s| matches!(s.section_kind(result.source()), GoogleSectionKind::Examples))
        .and_then(|s| s.body_text())
}
