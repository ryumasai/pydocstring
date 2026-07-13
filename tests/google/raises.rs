//! Typed-accessor contract for Raises and Warns entries.
//! Exhaustive input coverage lives in tests/corpus/google/ + tests/snapshots.rs;
//! colon-separator rules are pinned once in args.rs, alias→kind mapping in sections.rs.

use super::*;

/// Raises entry accessor contract (the exception type is the entry TYPE).
#[test]
fn test_raises_single() {
    let docstring = "Summary.\n\nRaises:\n    ValueError: If the input is invalid.";
    let result = parse_google(docstring);
    let r = raises(&result);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].type_annotation().unwrap().text(), "ValueError");
    assert_eq!(r[0].description().unwrap().text(), "If the input is invalid.");
}

/// Warns entry accessor contract (the warning type is the entry TYPE).
#[test]
fn test_warns_basic() {
    let docstring = "Summary.\n\nWarns:\n    DeprecationWarning: If using old API.";
    let result = parse_google(docstring);
    let w = warns(&result);
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].type_annotation().unwrap().text(), "DeprecationWarning");
    assert_eq!(w[0].description().unwrap().text(), "If using old API.");
}
