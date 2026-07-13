//! Spec + contract tests for free-text sections (Notes) and item sections
//! (See Also, References).
//! Exhaustive input coverage lives in tests/corpus/numpy/ + tests/snapshots.rs;
//! these tests pin deliberate spec decisions and the typed-accessor contract.

use super::*;

// =============================================================================
// Notes section — free-text body contract
// =============================================================================

/// CONTRACT: free-text sections expose their content via `Section::body()`.
#[test]
fn test_with_notes_section() {
    let docstring = r#"Function with notes.

Notes
-----
This is an important note about the function.
"#;
    let result = parse_numpy(docstring);

    assert!(notes(&result).is_some());
    assert!(notes(&result).unwrap().text().contains("important note"));
}

// =============================================================================
// See Also section
// =============================================================================

/// CONTRACT: See Also entry accessors (names / description), including
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
    assert_eq!(names0[0].text(), "func_a");
    assert_eq!(items[0].description().map(|d| d.text()), Some("Does something."));
    assert_eq!(items[1].names().count(), 2);
    let names1: Vec<_> = items[1].names().collect();
    assert_eq!(names1[0].text(), "func_b");
    assert_eq!(names1[1].text(), "func_c");
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
    assert_eq!(names[0].text(), "func_a");
    assert!(sa[0].description().unwrap().text().contains("Description"));
}

// =============================================================================
// References section
// =============================================================================

/// CONTRACT: Citation accessors (label / description) plus the raw-tree
/// punctuation (directive marker / brackets) around them.
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
    assert_eq!(refs[0].label().unwrap().text(), "1");
    assert!(refs[0].description().unwrap().text().contains("Author A"));
    let cst0 = refs[0].syntax();
    assert_eq!(
        cst0.find_token(SyntaxKind::DIRECTIVE_MARKER)
            .map(|t| t.text(result.source())),
        Some("..")
    );
    assert!(cst0.find_token(SyntaxKind::OPEN_BRACKET).is_some());
    assert!(cst0.find_token(SyntaxKind::CLOSE_BRACKET).is_some());
    assert_eq!(refs[1].label().unwrap().text(), "2");
    assert!(refs[1].description().unwrap().text().contains("Author B"));
}
