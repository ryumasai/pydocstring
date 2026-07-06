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
    assert_eq!(notes(&result).unwrap().text(result.source()), "This is a note.");
}

/// Multi-line free-text body preserves inner (relative) indentation verbatim.
#[test]
fn test_example_section() {
    let docstring = "Summary.\n\nExample:\n    >>> func(1)\n    1";
    let result = parse_google(docstring);
    assert_eq!(examples(&result).unwrap().text(result.source()), ">>> func(1)\n    1");
}
