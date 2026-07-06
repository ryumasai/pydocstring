//! Spec + contract tests for summary / extended summary / empty input /
//! signature-line handling.
//! Exhaustive input coverage lives in tests/corpus/numpy/ + tests/snapshots.rs;
//! these tests pin deliberate spec decisions and the typed-accessor contract.

use super::*;

// =============================================================================
// Summary spans and accessors
// =============================================================================

/// CONTRACT: summary token text and byte range.
#[test]
fn test_parse_simple_span() {
    let docstring = "Brief description.";
    let result = parse_numpy(docstring);
    assert_eq!(
        doc(&result).summary().unwrap().text(result.source()),
        "Brief description."
    );
    assert_eq!(doc(&result).summary().unwrap().range().start(), TextSize::new(0));
    assert_eq!(doc(&result).summary().unwrap().range().end(), TextSize::new(18));
}

/// SPEC: the summary runs to the first blank line (may span multiple lines);
/// text after the blank line is the extended summary.
#[test]
fn test_multiline_summary() {
    let docstring = "This is a long summary\nthat spans two lines.\n\nExtended description.";
    let result = parse_numpy(docstring);
    assert_eq!(
        doc(&result).summary().unwrap().text(result.source()),
        "This is a long summary\nthat spans two lines."
    );
    let desc = doc(&result).extended_summary().unwrap();
    assert_eq!(desc.text(result.source()), "Extended description.");
}

// =============================================================================
// Empty input
// =============================================================================

/// SPEC: empty input produces no summary.
#[test]
fn test_empty_docstring() {
    let result = parse_numpy("");
    assert!(doc(&result).summary().is_none());
}

/// SPEC: whitespace-only input produces no summary.
#[test]
fn test_whitespace_only_docstring() {
    let result = parse_numpy("   \n\n   ");
    assert!(doc(&result).summary().is_none());
}

/// CONTRACT: the root node's range covers the entire input.
#[test]
fn test_docstring_span_covers_entire_input() {
    let docstring = "First line.\n\nSecond line.";
    let result = parse_numpy(docstring);
    assert_eq!(doc(&result).syntax().range().start(), TextSize::new(0));
    assert_eq!(doc(&result).syntax().range().end().raw() as usize, docstring.len());
}

// =============================================================================
// Signature-like line is treated as summary
// =============================================================================

/// SPEC: a signature-like first line (`add(a, b)`) is treated as the summary,
/// not stripped or parsed specially.
#[test]
fn test_parse_with_signature_line() {
    let docstring = r#"add(a, b)

The sum of two numbers.

Parameters
----------
a : int
    First number.
b : int
    Second number.
"#;
    let result = parse_numpy(docstring);
    assert_eq!(doc(&result).summary().unwrap().text(result.source()), "add(a, b)");
    assert_eq!(parameters(&result).len(), 2);
}
