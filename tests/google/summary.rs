//! Spec pins for summary/extended-summary boundaries and empty-input behaviour.
//! Exhaustive input coverage lives in tests/corpus/google/ + tests/snapshots.rs.

use super::*;

/// Summary accessor + span contract.
#[test]
fn test_summary_span() {
    let docstring = "Brief description.";
    let result = parse_google(docstring);
    let s = doc(&result).summary().unwrap();
    assert_eq!(s.range().start(), TextSize::new(0));
    assert_eq!(s.range().end(), TextSize::new(18));
    assert_eq!(s.text(result.source()), "Brief description.");
}

#[test]
fn test_empty_docstring() {
    let result = parse_google("");
    assert!(doc(&result).summary().is_none());
}

#[test]
fn test_whitespace_only_docstring() {
    let result = parse_google("   \n   \n");
    assert!(doc(&result).summary().is_none());
}

/// Extended-summary accessor contract: blank line separates summary from
/// extended description.
#[test]
fn test_summary_with_description() {
    let docstring = "Brief summary.\n\nExtended description that provides\nmore details about the function.";
    let result = parse_google(docstring);

    assert_eq!(doc(&result).summary().unwrap().text(result.source()), "Brief summary.");
    let desc = doc(&result).extended_summary().unwrap();
    assert_eq!(
        desc.text(result.source()),
        "Extended description that provides\nmore details about the function."
    );
}

/// A summary continues across lines until a blank line.
#[test]
fn test_multiline_summary() {
    let docstring = "This is a long summary\nthat spans two lines.\n\nExtended description.";
    let result = parse_google(docstring);
    assert_eq!(
        doc(&result).summary().unwrap().text(result.source()),
        "This is a long summary\nthat spans two lines."
    );
    let desc = doc(&result).extended_summary().unwrap();
    assert_eq!(desc.text(result.source()), "Extended description.");
}

/// A section header directly after summary lines (no blank line) terminates
/// the summary and starts the section.
#[test]
fn test_multiline_summary_then_section() {
    let docstring = "Summary line one\ncontinues here.\nArgs:\n    x (int): val";
    let result = parse_google(docstring);
    assert_eq!(
        doc(&result).summary().unwrap().text(result.source()),
        "Summary line one\ncontinues here."
    );
    assert!(doc(&result).extended_summary().is_none());
    assert_eq!(doc(&result).sections().count(), 1);
}

/// A docstring may start directly with a section — no summary required.
#[test]
fn test_section_only_no_summary() {
    let docstring = "Args:\n    x (int): Value.";
    let result = parse_google(docstring);
    assert_eq!(args(&result).len(), 1);
}

/// Leading blank lines are skipped before the summary.
#[test]
fn test_leading_blank_lines() {
    let docstring = "\n\n\nSummary.\n\nArgs:\n    x: Value.";
    let result = parse_google(docstring);
    assert_eq!(doc(&result).summary().unwrap().text(result.source()), "Summary.");
    assert_eq!(args(&result).len(), 1);
}
