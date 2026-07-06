//! Spec tests for indentation handling (indented docstrings, tabs, mixed).
//! Exhaustive input coverage lives in tests/corpus/numpy/ + tests/snapshots.rs;
//! these tests pin deliberate spec decisions and the typed-accessor contract.

use super::*;

// =============================================================================
// Indented docstrings (class/method bodies)
// =============================================================================

/// SPEC: a uniformly indented docstring (method body style) parses the same
/// as an unindented one.
#[test]
fn test_indented_docstring() {
    let docstring = "    Summary line.\n\n    Parameters\n    ----------\n    x : int\n        Description of x.\n    y : str, optional\n        Description of y.\n\n    Returns\n    -------\n    bool\n        The result.\n";
    let result = parse_numpy(docstring);

    assert_eq!(doc(&result).summary().unwrap().text(result.source()), "Summary line.");
    assert_eq!(parameters(&result).len(), 2);
    let names0: Vec<_> = parameters(&result)[0].names().collect();
    assert_eq!(names0[0].text(result.source()), "x");
    assert_eq!(
        parameters(&result)[0].r#type().map(|t| t.text(result.source())),
        Some("int")
    );
    let names1: Vec<_> = parameters(&result)[1].names().collect();
    assert_eq!(names1[0].text(result.source()), "y");
    assert!(parameters(&result)[1].optional().is_some());
    assert_eq!(returns(&result).len(), 1);
    assert_eq!(
        returns(&result)[0].return_type().map(|t| t.text(result.source())),
        Some("bool")
    );
}

/// SPEC: an unindented first line followed by indented body (common docstring
/// layout) still finds sections at the body indent.
#[test]
fn test_mixed_indent_first_line() {
    let docstring = "Summary.\n\n    Parameters\n    ----------\n    x : int\n        Description.\n";
    let result = parse_numpy(docstring);

    assert_eq!(doc(&result).summary().unwrap().text(result.source()), "Summary.");
    assert_eq!(parameters(&result).len(), 1);
    let names: Vec<_> = parameters(&result)[0].names().collect();
    assert_eq!(names[0].text(result.source()), "x");
    assert_eq!(
        parameters(&result)[0].description().unwrap().text(result.source()),
        "Description."
    );
}

// =============================================================================
// Tab indentation
// =============================================================================

/// SPEC: tab-indented description lines count as indented continuation.
#[test]
fn test_tab_indented_parameters() {
    let docstring = "Summary.\n\nParameters\n----------\nx : int\n\tDescription of x.\ny : str\n\tDescription of y.";
    let result = parse_numpy(docstring);
    let params = parameters(&result);
    assert_eq!(params.len(), 2);
    let names0: Vec<_> = params[0].names().collect();
    assert_eq!(names0[0].text(result.source()), "x");
    assert_eq!(
        params[0].description().unwrap().text(result.source()),
        "Description of x."
    );
    let names1: Vec<_> = params[1].names().collect();
    assert_eq!(names1[0].text(result.source()), "y");
    assert_eq!(
        params[1].description().unwrap().text(result.source()),
        "Description of y."
    );
}

/// SPEC: mixed tabs and spaces in continuation lines stay within one entry.
#[test]
fn test_mixed_tab_space_parameters() {
    let docstring = "Summary.\n\nParameters\n----------\nx : int\n\tThe value.\n\t  More detail.";
    let result = parse_numpy(docstring);
    let params = parameters(&result);
    assert_eq!(params.len(), 1);
    let names: Vec<_> = params[0].names().collect();
    assert_eq!(names[0].text(result.source()), "x");
    let desc = params[0].description().unwrap().text(result.source());
    assert!(desc.contains("The value."), "desc = {:?}", desc);
}
