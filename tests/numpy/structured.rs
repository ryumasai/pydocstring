//! Spec + contract tests for Attributes, Methods, and Unknown sections.
//! Exhaustive input coverage lives in tests/corpus/numpy/ + tests/snapshots.rs;
//! these tests pin deliberate spec decisions and the typed-accessor contract.

use super::*;

// =============================================================================
// Attributes section
// =============================================================================

/// CONTRACT: NumPyAttribute accessors (name / type / colon / description).
#[test]
fn test_attributes_basic() {
    let docstring = "Summary.\n\nAttributes\n----------\nname : str\n    The name.\nage : int\n    The age.\n";
    let result = parse_numpy(docstring);
    let a = attributes(&result);
    assert_eq!(a.len(), 2);
    assert_eq!(a[0].name().text(result.source()), "name");
    assert_eq!(a[0].r#type().unwrap().text(result.source()), "str");
    assert!(a[0].colon().is_some());
    assert_eq!(a[0].description().unwrap().text(result.source()), "The name.");
    assert_eq!(a[1].name().text(result.source()), "age");
    assert_eq!(a[1].r#type().unwrap().text(result.source()), "int");
}

// =============================================================================
// Methods section
// =============================================================================

/// CONTRACT: NumPyMethod accessors (name / description); parens are part of
/// the method name.
#[test]
fn test_methods_basic() {
    let docstring =
        "Summary.\n\nMethods\n-------\nreset()\n    Reset the state.\nupdate(data)\n    Update with new data.\n";
    let result = parse_numpy(docstring);
    let m = methods(&result);
    assert_eq!(m.len(), 2);
    assert_eq!(m[0].name().text(result.source()), "reset()");
    assert_eq!(m[0].description().unwrap().text(result.source()), "Reset the state.");
    assert_eq!(m[1].name().text(result.source()), "update(data)");
    assert_eq!(
        m[1].description().unwrap().text(result.source()),
        "Update with new data."
    );
}

/// SPEC (issues #26/#31): `reset() : desc` colon split also applies in Methods.
#[test]
fn test_methods_with_colon() {
    let docstring = "Summary.\n\nMethods\n-------\nreset() : Reset the state.\n";
    let result = parse_numpy(docstring);
    let m = methods(&result);
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].name().text(result.source()), "reset()");
    assert!(m[0].colon().is_some());
    // Description may be inline or on next line depending on parser
    if let Some(desc) = m[0].description() {
        assert!(desc.text(result.source()).contains("Reset"));
    }
}

// =============================================================================
// Unknown section
// =============================================================================

/// SPEC: an unrecognized header with a valid underline still forms a section
/// (kind Unknown) instead of being swallowed into the previous section.
#[test]
fn test_unknown_section() {
    let docstring = "Summary.\n\nCustomSection\n-------------\nSome custom content.\n";
    let result = parse_numpy(docstring);
    let s = all_sections(&result);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].section_kind(result.source()), NumPySectionKind::Unknown);
    assert_eq!(s[0].header().name().text(result.source()), "CustomSection");
    let text = s[0].body_text();
    assert!(text.is_some());
    assert!(text.unwrap().text(result.source()).contains("Some custom content."));
}
