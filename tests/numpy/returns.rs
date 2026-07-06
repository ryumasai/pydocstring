//! Spec + contract tests for Returns and Yields sections.
//! Exhaustive input coverage lives in tests/corpus/numpy/ + tests/snapshots.rs;
//! these tests pin deliberate spec decisions and the typed-accessor contract.

use super::*;

// =============================================================================
// Returns section
// =============================================================================

/// CONTRACT: NumPyReturns accessors (name / return_type / description).
#[test]
fn test_parse_named_returns() {
    let docstring = r#"Compute values.

Returns
-------
x : int
    The first value.
y : float
    The second value.
"#;
    let result = parse_numpy(docstring);
    assert_eq!(returns(&result).len(), 2);
    assert_eq!(returns(&result)[0].name().map(|n| n.text(result.source())), Some("x"));
    assert_eq!(
        returns(&result)[0].return_type().map(|t| t.text(result.source())),
        Some("int")
    );
    assert_eq!(
        returns(&result)[0].description().unwrap().text(result.source()),
        "The first value."
    );
    assert_eq!(returns(&result)[1].name().map(|n| n.text(result.source())), Some("y"));
}

/// SPEC (issues #26/#31): no spaces around colon: `result:int` splits name/type.
#[test]
fn test_returns_no_spaces_around_colon() {
    let docstring = "Summary.\n\nReturns\n-------\nresult:int\n    The result.\n";
    let result = parse_numpy(docstring);
    let r = returns(&result);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].name().unwrap().text(result.source()), "result");
    assert_eq!(r[0].return_type().unwrap().text(result.source()), "int");
}

/// SPEC (prefer_type): a bare un-indented line in Returns is the type, not a name.
#[test]
fn test_returns_type_only() {
    let docstring = "Summary.\n\nReturns\n-------\nint\n    The result.\n";
    let result = parse_numpy(docstring);
    let r = returns(&result);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].return_type().unwrap().text(result.source()), "int");
    assert_eq!(r[0].description().unwrap().text(result.source()), "The result.");
}

// =============================================================================
// Yields section
// =============================================================================

/// SPEC (prefer_type): a bare un-indented line in Yields is the type, not a name.
#[test]
fn test_yields_basic() {
    let docstring = "Summary.\n\nYields\n------\nint\n    The next value.\n";
    let result = parse_numpy(docstring);
    let y = yields(&result);
    assert_eq!(y.len(), 1);
    assert_eq!(y[0].return_type().unwrap().text(result.source()), "int");
    assert_eq!(y[0].description().unwrap().text(result.source()), "The next value.");
}

/// CONTRACT: NumPyYields accessors (name / return_type / description).
#[test]
fn test_yields_named() {
    let docstring = "Summary.\n\nYields\n------\nvalue : str\n    The generated string.\n";
    let result = parse_numpy(docstring);
    let y = yields(&result);
    assert_eq!(y.len(), 1);
    assert_eq!(y[0].name().unwrap().text(result.source()), "value");
    assert_eq!(y[0].return_type().unwrap().text(result.source()), "str");
    assert_eq!(
        y[0].description().unwrap().text(result.source()),
        "The generated string."
    );
}
