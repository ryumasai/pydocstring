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
    assert_eq!(s.text(), "Brief description.");
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

    assert_eq!(doc(&result).summary().unwrap().text(), "Brief summary.");
    let desc = doc(&result).extended_summary().unwrap();
    assert_eq!(
        desc.text(),
        "Extended description that provides\nmore details about the function."
    );
}

/// A summary continues across lines until a blank line.
#[test]
fn test_multiline_summary() {
    let docstring = "This is a long summary\nthat spans two lines.\n\nExtended description.";
    let result = parse_google(docstring);
    assert_eq!(
        doc(&result).summary().unwrap().text(),
        "This is a long summary\nthat spans two lines."
    );
    let desc = doc(&result).extended_summary().unwrap();
    assert_eq!(desc.text(), "Extended description.");
}

/// A section header directly after summary lines (no blank line) terminates
/// the summary and starts the section.
#[test]
fn test_multiline_summary_then_section() {
    let docstring = "Summary line one\ncontinues here.\nArgs:\n    x (int): val";
    let result = parse_google(docstring);
    assert_eq!(
        doc(&result).summary().unwrap().text(),
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

/// SPEC: `.. deprecated:: <version>` directive between the summary and the
/// extended summary is recognized and does NOT become extended summary.
/// Also CONTRACT for the Directive accessors (name / argument / description).
#[test]
fn test_deprecation_directive() {
    let docstring = "Summary.\n\n.. deprecated:: 1.6.0\n    Use `new_func` instead.\n\nArgs:\n    x (int): The value.";
    let result = parse_google(docstring);

    let dep = doc(&result)
        .directives()
        .find(|d| d.name().text() == "deprecated")
        .expect("deprecation should be parsed");
    assert_eq!(dep.argument().unwrap().text(), "1.6.0");
    assert_eq!(dep.description().unwrap().text(), "Use `new_func` instead.");

    // The directive is not swallowed into the extended summary.
    assert!(doc(&result).extended_summary().is_none());
    assert_eq!(args(&result).len(), 1);
}

/// Leading blank lines are skipped before the summary.
#[test]
fn test_leading_blank_lines() {
    let docstring = "\n\n\nSummary.\n\nArgs:\n    x: Value.";
    let result = parse_google(docstring);
    assert_eq!(doc(&result).summary().unwrap().text(), "Summary.");
    assert_eq!(args(&result).len(), 1);
}
