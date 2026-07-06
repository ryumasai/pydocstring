//! Spec + contract tests for free-text sections (Notes) and item sections
//! (See Also, References).
//! Exhaustive input coverage lives in tests/corpus/numpy/ + tests/snapshots.rs;
//! these tests pin deliberate spec decisions and the typed-accessor contract.

use super::*;

// =============================================================================
// Notes section — free-text body contract
// =============================================================================

/// CONTRACT: free-text sections expose their content via `body_text()`.
#[test]
fn test_with_notes_section() {
    let docstring = r#"Function with notes.

Notes
-----
This is an important note about the function.
"#;
    let result = parse_numpy(docstring);

    assert!(notes(&result).is_some());
    assert!(notes(&result).unwrap().text(result.source()).contains("important note"));
}

// =============================================================================
// See Also section
// =============================================================================

/// CONTRACT: NumPySeeAlsoItem accessors (names / description), including
/// comma-separated multi-name items.
#[test]
fn test_see_also_parsing() {
    let docstring = r#"Summary.

See Also
--------
func_a : Does something.
func_b, func_c
"#;
    let result = parse_numpy(docstring);
    let items = see_also(&result);
    assert_eq!(items.len(), 2);
    let names0: Vec<_> = items[0].names().collect();
    assert_eq!(names0[0].text(result.source()), "func_a");
    assert_eq!(
        items[0].description().map(|d| d.text(result.source())),
        Some("Does something.")
    );
    assert_eq!(items[1].names().count(), 2);
    let names1: Vec<_> = items[1].names().collect();
    assert_eq!(names1[0].text(result.source()), "func_b");
    assert_eq!(names1[1].text(result.source()), "func_c");
}

/// SPEC (issues #26/#31): `func_a: desc` with no space before the colon still
/// splits name/description in See Also.
#[test]
fn test_see_also_no_space_before_colon() {
    let docstring = "Summary.\n\nSee Also\n--------\nfunc_a: Description of func_a.\n";
    let result = parse_numpy(docstring);
    let sa = see_also(&result);
    assert_eq!(sa.len(), 1);
    let names: Vec<_> = sa[0].names().collect();
    assert_eq!(names[0].text(result.source()), "func_a");
    assert!(
        sa[0]
            .description()
            .unwrap()
            .text(result.source())
            .contains("Description")
    );
}

// =============================================================================
// References section
// =============================================================================

/// CONTRACT: NumPyReference accessors (number / content / directive_marker /
/// brackets).
#[test]
fn test_references_parsing() {
    let docstring = r#"Summary.

References
----------
.. [1] Author A, "Title A", 2020.
.. [2] Author B, "Title B", 2021.
"#;
    let result = parse_numpy(docstring);
    let refs = references(&result);
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].number().unwrap().text(result.source()), "1");
    assert!(refs[0].content().unwrap().text(result.source()).contains("Author A"));
    assert_eq!(refs[0].directive_marker().unwrap().text(result.source()), "..");
    assert!(refs[0].open_bracket().is_some());
    assert!(refs[0].close_bracket().is_some());
    assert_eq!(refs[1].number().unwrap().text(result.source()), "2");
    assert!(refs[1].content().unwrap().text(result.source()).contains("Author B"));
}
