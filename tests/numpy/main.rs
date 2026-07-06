//! Integration tests for the NumPy-style docstring parser.
//!
//! Exhaustive input coverage lives in tests/corpus/numpy/ + tests/snapshots.rs;
//! the tests here pin deliberate spec decisions and the typed-accessor contract.

pub use pydocstring::parse::numpy::{
    kind::NumPySectionKind,
    nodes::{
        NumPyAttribute, NumPyDeprecation, NumPyDocstring, NumPyException, NumPyMethod, NumPyParameter, NumPyReference,
        NumPyReturns, NumPySection, NumPySeeAlsoItem, NumPyWarning, NumPyYields,
    },
    parse_numpy,
};
pub use pydocstring::syntax::{Parsed, SyntaxToken};
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

/// Get the typed NumPyDocstring wrapper from a Parsed result.
pub fn doc(result: &Parsed) -> NumPyDocstring<'_> {
    NumPyDocstring::cast(result.root()).unwrap()
}

/// Extract all sections from a docstring.
pub fn all_sections<'a>(result: &'a Parsed) -> Vec<NumPySection<'a>> {
    doc(result).sections().collect()
}

pub fn parameters<'a>(result: &'a Parsed) -> Vec<NumPyParameter<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), NumPySectionKind::Parameters))
        .flat_map(|s| s.parameters().collect::<Vec<_>>())
        .collect()
}

pub fn returns<'a>(result: &'a Parsed) -> Vec<NumPyReturns<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), NumPySectionKind::Returns))
        .flat_map(|s| s.returns().collect::<Vec<_>>())
        .collect()
}

pub fn yields<'a>(result: &'a Parsed) -> Vec<NumPyYields<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), NumPySectionKind::Yields))
        .flat_map(|s| s.yields().collect::<Vec<_>>())
        .collect()
}

pub fn raises<'a>(result: &'a Parsed) -> Vec<NumPyException<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), NumPySectionKind::Raises))
        .flat_map(|s| s.exceptions().collect::<Vec<_>>())
        .collect()
}

pub fn warns<'a>(result: &'a Parsed) -> Vec<NumPyWarning<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), NumPySectionKind::Warns))
        .flat_map(|s| s.warnings().collect::<Vec<_>>())
        .collect()
}

pub fn see_also<'a>(result: &'a Parsed) -> Vec<NumPySeeAlsoItem<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), NumPySectionKind::SeeAlso))
        .flat_map(|s| s.see_also_items().collect::<Vec<_>>())
        .collect()
}

pub fn references<'a>(result: &'a Parsed) -> Vec<NumPyReference<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), NumPySectionKind::References))
        .flat_map(|s| s.references().collect::<Vec<_>>())
        .collect()
}

pub fn notes(result: &Parsed) -> Option<&SyntaxToken> {
    doc(result)
        .sections()
        .find(|s| matches!(s.section_kind(result.source()), NumPySectionKind::Notes))
        .and_then(|s| s.body_text())
}

pub fn receives<'a>(result: &'a Parsed) -> Vec<NumPyParameter<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), NumPySectionKind::Receives))
        .flat_map(|s| s.parameters().collect::<Vec<_>>())
        .collect()
}

pub fn attributes<'a>(result: &'a Parsed) -> Vec<NumPyAttribute<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), NumPySectionKind::Attributes))
        .flat_map(|s| s.attributes().collect::<Vec<_>>())
        .collect()
}

pub fn methods<'a>(result: &'a Parsed) -> Vec<NumPyMethod<'a>> {
    doc(result)
        .sections()
        .filter(|s| matches!(s.section_kind(result.source()), NumPySectionKind::Methods))
        .flat_map(|s| s.methods().collect::<Vec<_>>())
        .collect()
}
