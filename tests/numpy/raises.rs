//! Spec + contract tests for Raises and Warns sections.
//! Exhaustive input coverage lives in tests/corpus/numpy/ + tests/snapshots.rs;
//! these tests pin deliberate spec decisions and the typed-accessor contract.

use super::*;

// =============================================================================
// Raises — colon splitting spec
// =============================================================================

/// SPEC (issues #26/#31): `Type : description` on one line splits at the colon.
/// Also CONTRACT for exception Entry accessors (type / colon / description).
#[test]
fn test_raises_colon_split() {
    let docstring =
        "Summary.\n\nRaises\n------\nValueError : If the input is invalid.\nTypeError : If the type is wrong.";
    let result = parse_numpy(docstring);
    let exc = raises(&result);
    assert_eq!(exc.len(), 2);
    assert_eq!(exc[0].type_annotation().unwrap().text(), "ValueError");
    assert!(colon(&exc[0]).is_some());
    assert_eq!(exc[0].description().unwrap().text(), "If the input is invalid.");
    assert_eq!(exc[1].type_annotation().unwrap().text(), "TypeError");
    assert!(colon(&exc[1]).is_some());
    assert_eq!(exc[1].description().unwrap().text(), "If the type is wrong.");
}

/// SPEC: bare exception type with the description on the next indented line
/// (no colon token present).
#[test]
fn test_raises_no_colon() {
    let docstring = "Summary.\n\nRaises\n------\nValueError\n    If the input is invalid.";
    let result = parse_numpy(docstring);
    let exc = raises(&result);
    assert_eq!(exc.len(), 1);
    assert_eq!(exc[0].type_annotation().unwrap().text(), "ValueError");
    assert!(colon(&exc[0]).is_none());
    assert_eq!(exc[0].description().unwrap().text(), "If the input is invalid.");
}

/// SPEC: continuation lines after an inline `Type : desc` join the description.
#[test]
fn test_raises_colon_with_continuation() {
    let docstring = "Summary.\n\nRaises\n------\nValueError : If bad.\n    More detail here.";
    let result = parse_numpy(docstring);
    let exc = raises(&result);
    assert_eq!(exc.len(), 1);
    assert_eq!(exc[0].type_annotation().unwrap().text(), "ValueError");
    assert!(colon(&exc[0]).is_some());
    let desc = exc[0].description().unwrap().text();
    assert!(desc.contains("If bad."), "desc = {:?}", desc);
    assert!(desc.contains("More detail here."), "desc = {:?}", desc);
}

// =============================================================================
// Warns section
// =============================================================================

/// CONTRACT: warning Entry accessors (type / description).
#[test]
fn test_warns_basic() {
    let docstring = "Summary.\n\nWarns\n-----\nDeprecationWarning\n    If the old API is used.\n";
    let result = parse_numpy(docstring);
    let w = warns(&result);
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].type_annotation().unwrap().text(), "DeprecationWarning");
    assert_eq!(w[0].description().unwrap().text(), "If the old API is used.");
}

/// SPEC (issues #26/#31): `Type : description` colon split also applies in Warns.
#[test]
fn test_warns_colon_split() {
    let docstring = "Summary.\n\nWarns\n-----\nUserWarning : If input is unusual.\n";
    let result = parse_numpy(docstring);
    let w = warns(&result);
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].type_annotation().unwrap().text(), "UserWarning");
    assert!(colon(&w[0]).is_some());
    assert_eq!(w[0].description().unwrap().text(), "If input is unusual.");
}
