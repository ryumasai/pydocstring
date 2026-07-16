//! Spec + contract tests for Attributes, Methods, and Unknown sections.
//! Exhaustive input coverage lives in tests/corpus/numpy/ + tests/snapshots.rs;
//! these tests pin deliberate spec decisions and the typed-accessor contract.

use super::*;

// =============================================================================
// Attributes section
// =============================================================================

/// CONTRACT: attribute Entry accessors (name / type / colon / description).
#[test]
fn test_attributes_basic() {
    let docstring = "Summary.\n\nAttributes\n----------\nname : str\n    The name.\nage : int\n    The age.\n";
    let result = parse_numpy(docstring);
    let a = attributes(&result);
    assert_eq!(a.len(), 2);
    assert_eq!(a[0].name().unwrap().text(), "name");
    assert_eq!(a[0].type_annotation().unwrap().text(), "str");
    assert!(colon(&a[0]).is_some());
    assert_eq!(a[0].description().unwrap().text(), "The name.");
    assert_eq!(a[1].name().unwrap().text(), "age");
    assert_eq!(a[1].type_annotation().unwrap().text(), "int");
}

// =============================================================================
// Methods section
// =============================================================================

/// CONTRACT: method Entry accessors (name / description); parens are part of
/// the method name.
#[test]
fn test_methods_basic() {
    let docstring =
        "Summary.\n\nMethods\n-------\nreset()\n    Reset the state.\nupdate(data)\n    Update with new data.\n";
    let result = parse_numpy(docstring);
    let m = methods(&result);
    assert_eq!(m.len(), 2);
    assert_eq!(m[0].name().unwrap().text(), "reset()");
    assert_eq!(m[0].description().unwrap().text(), "Reset the state.");
    assert_eq!(m[1].name().unwrap().text(), "update(data)");
    assert_eq!(m[1].description().unwrap().text(), "Update with new data.");
}

/// SPEC (issues #26/#31): `reset() : desc` colon split also applies in Methods.
/// The inline text after the colon is the method's description (#39: it must
/// not be dropped from the tree).
#[test]
fn test_methods_with_colon() {
    let docstring = "Summary.\n\nMethods\n-------\nreset() : Reset the state.\n";
    let result = parse_numpy(docstring);
    let m = methods(&result);
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].name().unwrap().text(), "reset()");
    assert!(colon(&m[0]).is_some());
    assert_eq!(m[0].description().unwrap().text(), "Reset the state.");
}

// =============================================================================
// Unknown section
// =============================================================================

/// SPEC: a *registered* custom header with a valid underline forms a section
/// (kind Unknown) instead of being swallowed into the previous section; by
/// default the same lines are prose (napoleon's line, #147).
#[test]
fn test_unknown_section() {
    let docstring = "Summary.\n\nCustomSection\n-------------\nSome custom content.\n";
    assert_eq!(all_sections(&parse_numpy(docstring)).len(), 0);

    let opts = ParseOptions::new().with_custom_sections(["CustomSection"]);
    let result = parse_numpy_with(docstring, &opts);
    let s = all_sections(&result);
    assert_eq!(s.len(), 1);
    assert_eq!(
        s[0].kind(),
        SectionKind::FreeText(FreeSectionKind::Unknown("CustomSection".to_owned()))
    );
    assert_eq!(s[0].header_name(), "CustomSection");
    let text = s[0].body();
    assert!(text.is_some());
    assert!(text.unwrap().text().contains("Some custom content."));
}
