//! Spec pins and typed-accessor contract for free-text sections (Notes, Examples, ...).
//! Exhaustive input coverage (Todo, References, admonitions, ...) lives in
//! tests/corpus/google/ + tests/snapshots.rs; header-alias→kind mapping is pinned
//! by the table test in sections.rs.

use super::*;

/// Free-text body accessor contract (single-line body).
#[test]
fn test_note_section() {
    let docstring = "Summary.\n\nNote:\n    This is a note.";
    let result = parse_google(docstring);
    assert_eq!(notes(&result).unwrap().text(), "This is a note.");
}

/// Multi-line free-text body preserves inner (relative) indentation verbatim.
#[test]
fn test_example_section() {
    let docstring = "Summary.\n\nExample:\n    >>> func(1)\n    1";
    let result = parse_google(docstring);
    assert_eq!(examples(&result).unwrap().text(), ">>> func(1)\n    1");
}

// =============================================================================
// References section
// =============================================================================

/// CONTRACT: GoogleReference accessors (number / content / directive_marker /
/// brackets) for rST-marker entries `.. [N] content`.
#[test]
fn test_references_rst_markers() {
    let docstring =
        "Summary.\n\nReferences:\n    .. [1] Author A, \"Title A\", 2020.\n    .. [2] Author B, \"Title B\", 2021.";
    let result = parse_google(docstring);
    let refs = references(&result);
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].label().unwrap().text(), "1");
    assert!(refs[0].content().unwrap().text().contains("Author A"));
    assert_eq!(refs[0].directive_marker().unwrap().text(), "..");
    assert!(refs[0].open_bracket().is_some());
    assert!(refs[0].close_bracket().is_some());
    assert_eq!(refs[1].label().unwrap().text(), "2");
    assert!(refs[1].content().unwrap().text().contains("Author B"));
}

/// SPEC: a plain (non-directive) reference line is a content-only entry.
#[test]
fn test_references_plain_line() {
    let docstring = "Summary.\n\nReferences:\n    Author, Title, 2024.";
    let result = parse_google(docstring);
    let refs = references(&result);
    assert_eq!(refs.len(), 1);
    assert!(refs[0].directive_marker().is_none());
    assert!(refs[0].label().is_none());
    assert_eq!(refs[0].content().unwrap().text(), "Author, Title, 2024.");
}

/// SPEC: a more-indented continuation line extends the previous entry's content.
#[test]
fn test_references_continuation_line() {
    let docstring =
        "Summary.\n\nReferences:\n    .. [1] Author B, \"Title B\", 2021,\n        with a continuation line.";
    let result = parse_google(docstring);
    let refs = references(&result);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].label().unwrap().text(), "1");
    assert_eq!(
        refs[0].content().unwrap().text(),
        "Author B, \"Title B\", 2021,\n        with a continuation line."
    );
}
