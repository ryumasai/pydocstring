//! Spec pins for indentation rules, colon rules, missing tokens, and stray lines.
//! Exhaustive input coverage lives in tests/corpus/google/ + tests/snapshots.rs;
//! these tests pin deliberate parsing decisions and the accessor API.

use super::*;

// =============================================================================
// Indented docstrings
// =============================================================================

#[test]
fn test_indented_docstring() {
    let docstring = "    Summary.\n\n    Args:\n        x (int): Value.";
    let result = parse_google(docstring);
    assert_eq!(doc(&result).summary().unwrap().text(result.source()), "Summary.");
    let a = args(&result);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name().text(result.source()), "x");
    assert_eq!(a[0].r#type().unwrap().text(result.source()), "int");
}

/// Args entries at the same indent level as the section header (indent 0)
/// must be parsed as args, not silently dropped as stray lines.
/// Regression test: previously `x` and `y` became STRAY_LINE tokens.
#[test]
fn test_args_entries_same_indent_as_header() {
    let input = "Args:\nx (int): desc\ny (int): desc";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 2, "x and y must be parsed as args, not dropped as stray lines");
    assert_eq!(a[0].name().text(result.source()), "x");
    assert_eq!(a[0].r#type().unwrap().text(result.source()), "int");
    assert_eq!(a[0].description().unwrap().text(result.source()), "desc");
    assert_eq!(a[1].name().text(result.source()), "y");
    assert_eq!(a[1].r#type().unwrap().text(result.source()), "int");
    assert_eq!(a[1].description().unwrap().text(result.source()), "desc");
}

/// A stray line that appears AFTER properly-indented entries (at indent 4)
/// must still end the section even though the section header is at indent 0.
#[test]
fn test_stray_still_flushed_after_indented_entries() {
    let input = "Summary.\n\nArgs:\n    a (int): first.\nstray\n\nReturns:\n    int: result.";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 1, "stray must not become an arg entry");
    assert_eq!(a[0].name().text(result.source()), "a");
    assert!(returns(&result).is_some(), "Returns section must still be parsed");
}

/// Slightly mis-indented entries (3 spaces when first entry used 4) must be
/// parsed as arg entries, not dropped as STRAY_LINE.
/// The flush threshold is the section header's own indent, not body_min_indent.
#[test]
fn test_slightly_misindented_entry_not_stray() {
    let input = "Args:\n    x: desc\n   y: desc";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 2, "y must be an arg entry, not a STRAY_LINE");
    assert_eq!(a[0].name().text(result.source()), "x");
    assert_eq!(a[1].name().text(result.source()), "y");
}

/// Same check with an indented section header (header at 4, entries at 8, one at 7).
#[test]
fn test_slightly_misindented_entry_not_stray_nested() {
    let input = "    Args:\n        x: desc\n       y: desc";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 2, "y must be an arg entry, not a STRAY_LINE");
    assert_eq!(a[0].name().text(result.source()), "x");
    assert_eq!(a[1].name().text(result.source()), "y");
}

#[test]
fn test_indented_summary_span() {
    let docstring = "    Summary.";
    let result = parse_google(docstring);
    let s = doc(&result).summary().unwrap();
    assert_eq!(s.range().start(), TextSize::new(4));
    assert_eq!(s.range().end(), TextSize::new(12));
    assert_eq!(s.text(result.source()), "Summary.");
}

// =============================================================================
// Space-before-colon and colonless header tests
// =============================================================================

/// `Args :` (space before colon) should be dispatched as Args, not Unknown.
#[test]
fn test_section_header_space_before_colon() {
    let input = "Summary.\n\nArgs :\n    x (int): The value.";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 1, "expected 1 arg from 'Args :'");
    assert_eq!(a[0].name().text(result.source()), "x");

    assert_eq!(all_sections(&result)[0].header().name().text(result.source()), "Args");
    assert!(all_sections(&result)[0].header().colon().is_some());
}

/// Colonless `Args` should be parsed as Args section.
/// The section header should contain a missing COLON token.
#[test]
fn test_section_header_no_colon() {
    let input = "Summary.\n\nArgs\n    x (int): The value.";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 1, "expected 1 arg from colonless 'Args'");
    assert_eq!(a[0].name().text(result.source()), "x");

    let header = all_sections(&result)[0].header();
    assert_eq!(header.name().text(result.source()), "Args");
    assert!(header.colon().is_none(), "no COLON token for colonless header");
    let missing = header.syntax().find_missing(SyntaxKind::COLON);
    assert!(missing.is_some(), "colonless header should have a missing COLON");
    assert!(missing.unwrap().is_missing());
}

/// Unknown names without colon should NOT be treated as headers.
#[test]
fn test_unknown_name_without_colon_not_header() {
    let input = "Summary.\n\nSomeWord\n    x (int): value.";
    let result = parse_google(input);
    assert!(
        all_sections(&result).is_empty(),
        "unknown colonless name should not become a section"
    );
}

// =============================================================================
// Tab indentation tests
// =============================================================================

/// Args section with tab-indented entries.
#[test]
fn test_tab_indented_args() {
    let input = "Summary.\n\nArgs:\n\tx: The value.\n\ty: Another value.";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 2);
    assert_eq!(a[0].name().text(result.source()), "x");
    assert_eq!(a[0].description().unwrap().text(result.source()), "The value.");
    assert_eq!(a[1].name().text(result.source()), "y");
    assert_eq!(a[1].description().unwrap().text(result.source()), "Another value.");
}

/// Args entries with tab indent and descriptions with deeper tab+space indent.
#[test]
fn test_tab_args_with_continuation() {
    let input = "Summary.\n\nArgs:\n\tx: First line.\n\t    Continuation.\n\ty: Second.";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 2);
    assert_eq!(a[0].name().text(result.source()), "x");
    let desc = a[0].description().unwrap().text(result.source());
    assert!(desc.contains("First line."), "desc = {:?}", desc);
    assert!(desc.contains("Continuation."), "desc = {:?}", desc);
}

/// Section header detection with tab indentation matches.
#[test]
fn test_tab_indented_section_header() {
    let input = "\tSummary.\n\n\tArgs:\n\t\tx: The value.";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name().text(result.source()), "x");
}

// =============================================================================
// Missing token tests
// =============================================================================

/// `arg1 (int : desc.` — missing close bracket.
/// Parser should preserve type info with a missing CLOSE_BRACKET.
#[test]
fn test_missing_close_bracket() {
    let input = "Args:\n   arg1 (int : desc.";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name().text(result.source()), "arg1");
    assert!(a[0].open_bracket().is_some());
    assert_eq!(a[0].r#type().unwrap().text(result.source()), "int");
    assert!(
        a[0].close_bracket().is_none(),
        "no CLOSE_BRACKET when bracket is unmatched"
    );
    // Missing CLOSE_BRACKET token should be present.
    let missing = a[0].syntax().find_missing(SyntaxKind::CLOSE_BRACKET);
    assert!(missing.is_some(), "should have a missing CLOSE_BRACKET token");
    assert!(missing.unwrap().is_missing());
    assert_eq!(a[0].description().unwrap().text(result.source()), "desc.");
}

/// `arg1 (int) desc` — close bracket present but colon missing before description.
#[test]
fn test_missing_colon_after_bracket() {
    let input = "Args:\n    arg1 (int) description here.";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name().text(result.source()), "arg1");
    assert_eq!(a[0].r#type().unwrap().text(result.source()), "int");
    assert!(a[0].open_bracket().is_some());
    assert!(a[0].close_bracket().is_some());
    assert!(a[0].colon().is_none(), "no COLON token");
    // Missing COLON token.
    let missing = a[0].syntax().find_missing(SyntaxKind::COLON);
    assert!(missing.is_some(), "should have a missing COLON token");
    assert!(missing.unwrap().is_missing());
    assert_eq!(a[0].description().unwrap().text(result.source()), "description here.");
}

/// `arg1 (int` — missing close bracket and no colon/description.
#[test]
fn test_missing_close_bracket_no_colon() {
    let input = "Args:\n    arg1 (int";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name().text(result.source()), "arg1");
    assert_eq!(a[0].r#type().unwrap().text(result.source()), "int");
    assert!(a[0].open_bracket().is_some());
    assert!(a[0].close_bracket().is_none());
    assert!(a[0].colon().is_none());
    // Missing CLOSE_BRACKET but no missing COLON (no description).
    assert!(a[0].syntax().find_missing(SyntaxKind::CLOSE_BRACKET).is_some());
    assert!(a[0].syntax().find_missing(SyntaxKind::COLON).is_none());
}

/// `arg1 (int desc.` — no close bracket and no colon.
/// Entire content after `(` is TYPE; no colon/description.
#[test]
fn test_missing_bracket_no_colon_no_split() {
    let input = "Args:\n    arg1 (int desc.";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name().text(result.source()), "arg1");
    assert_eq!(a[0].r#type().unwrap().text(result.source()), "int desc.");
    assert!(a[0].close_bracket().is_none());
    assert!(a[0].colon().is_none());
    assert!(a[0].description().is_none());
}

/// `arg1 (int:desc.)` — colon inside brackets.
/// Entire bracket content is TYPE; colon inside brackets is not treated as separator.
#[test]
fn test_colon_inside_brackets() {
    let input = "Args:\n    arg1 (int:desc.)";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name().text(result.source()), "arg1");
    assert_eq!(a[0].r#type().unwrap().text(result.source()), "int:desc.");
    assert!(a[0].open_bracket().is_some());
    assert!(a[0].close_bracket().is_some());
    assert!(a[0].colon().is_none());
    assert!(a[0].description().is_none());
}

/// `arg1 (Dict[str:int])` — colon inside nested brackets should NOT split.
#[test]
fn test_colon_inside_nested_brackets_no_split() {
    let input = "Args:\n    arg1 (Dict[str:int])";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name().text(result.source()), "arg1");
    assert_eq!(a[0].r#type().unwrap().text(result.source()), "Dict[str:int]");
    assert!(a[0].description().is_none());
}

// =============================================================================
// Arg entry with no description — must not become a section header
// =============================================================================

/// `b :` (space before colon, no description) must be parsed as an arg entry,
/// not mistaken for a section header.  Regression test for the case where any
/// `word:` pattern inside a section body was mis-classified as a new section.
#[test]
fn test_arg_no_description_space_before_colon_not_header() {
    let input = "Args:\n    a (int): An integer parameter.\n    b :\n    c : A parameter.";
    let result = parse_google(input);

    // Only one section (Args), not three.
    let sections = all_sections(&result);
    assert_eq!(sections.len(), 1, "b : should not be a section header");

    let a = args(&result);
    assert_eq!(a.len(), 3, "expected 3 arg entries");

    assert_eq!(a[0].name().text(result.source()), "a");
    assert_eq!(a[0].r#type().unwrap().text(result.source()), "int");
    assert_eq!(
        a[0].description().unwrap().text(result.source()),
        "An integer parameter."
    );

    assert_eq!(a[1].name().text(result.source()), "b");
    assert!(a[1].r#type().is_none());
    assert!(a[1].description().is_none());

    assert_eq!(a[2].name().text(result.source()), "c");
    assert!(a[2].r#type().is_none());
    assert_eq!(a[2].description().unwrap().text(result.source()), "A parameter.");
}

// =============================================================================
// Stray lines between sections
// =============================================================================

/// A non-section, non-indented line that appears after a blank line following
/// a section's entries must NOT be absorbed into the previous section.
/// It should become a STRAY_LINE, and the next real section must be parsed
/// correctly.
#[test]
fn test_stray_line_between_args_and_returns() {
    let input = "Summary.\n\nArgs:\n    a: desc.\n\nstray line 1\n\nReturns:\n    desc\n\nstray line 2";
    let result = parse_google(input);

    // Args section should contain exactly one entry.
    let a = args(&result);
    assert_eq!(a.len(), 1, "stray line must not become an arg entry");
    assert_eq!(a[0].name().text(result.source()), "a");

    // Returns section should be present and its description should not include
    // the stray line.
    let r = returns(&result).unwrap();
    let desc = r.description().unwrap().text(result.source());
    assert!(
        !desc.contains("stray"),
        "stray line must not be part of Returns description"
    );
}

/// Same as above but WITHOUT blank lines before the stray lines.
#[test]
fn test_stray_line_between_args_and_returns_no_blank() {
    let input = "Summary.\n\nArgs:\n    a: desc.\nstray line 1\n\nReturns:\n    desc\nstray line 2\n";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 1, "stray line must not become an arg entry (no-blank case)");
    assert_eq!(a[0].name().text(result.source()), "a");
    let r = returns(&result).unwrap();
    let desc = r.description().unwrap().text(result.source());
    assert!(!desc.contains("stray"), "stray line must not be in Returns description");
}

/// A blank-line-separated entry at greater indent than the header must still
/// be absorbed into the same section (existing behaviour).
#[test]
fn test_blank_between_entries_within_section() {
    let input = "Summary.\n\nArgs:\n    x (int): Value.\n\n    y (str): Name.\n\nReturns:\n    bool: Success.";
    let result = parse_google(input);
    assert_eq!(args(&result).len(), 2, "both entries should belong to Args");
    assert!(returns(&result).is_some());
}

/// An arg description that has a blank line followed by a more-deeply-indented
/// continuation must keep both parts in the description.
#[test]
fn test_arg_description_blank_line_with_continuation() {
    // "        Second paragraph." is at 8 spaces — deeper than the entry (4).
    let input = "Summary.\n\nArgs:\n    a: First paragraph.\n\n        Second paragraph.\n\nReturns:\n    bool: ok.\n";
    let result = parse_google(input);
    let a = args(&result);
    assert_eq!(a.len(), 1, "should be exactly one arg");
    let desc = a[0].description().unwrap().text(result.source());
    assert!(desc.contains("First paragraph."), "desc = {:?}", desc);
    assert!(desc.contains("Second paragraph."), "desc = {:?}", desc);
    // Returns must still be parsed correctly.
    assert!(returns(&result).is_some());
}

/// A FreeText section (Notes) with a blank line between two paragraphs at the
/// same depth must keep both paragraphs in its body.
#[test]
fn test_freetext_description_blank_line_continuation() {
    let input = "Summary.\n\nNotes:\n    Paragraph one.\n\n    Paragraph two.\n\nArgs:\n    x: val.\n";
    let result = parse_google(input);
    let sections = all_sections(&result);
    // Notes section present
    let notes_sec = sections
        .iter()
        .find(|s| s.header().name().text(result.source()) == "Notes");
    assert!(notes_sec.is_some(), "Notes section should be present");
    let body = notes_sec.unwrap().syntax().find_token(SyntaxKind::BODY_TEXT).unwrap();
    let body_text = body.text(result.source());
    assert!(body_text.contains("Paragraph one."), "body = {:?}", body_text);
    assert!(body_text.contains("Paragraph two."), "body = {:?}", body_text);
    // Args must still be parsed
    assert_eq!(args(&result).len(), 1);
}

// =============================================================================
// RST-style :param lines inside Args section
// =============================================================================

/// RST-style `:param foo:` lines inside a Google `Args:` section must not
/// produce a GOOGLE_ARG with an empty NAME, which would panic when
/// `required_token(NAME)` is called.  They should be treated as bare-name
/// entries (fallback) so the line text is stored verbatim as the NAME token.
///
/// Regression test for: "required token NAME not found in GOOGLE_ARG".
#[test]
fn test_rst_style_param_in_args_no_panic() {
    let input = "Summary.\n\nArgs:\n    :param int seconds: The seconds.\n    :param int nanoseconds: The nanoseconds.";
    let result = parse_google(input);

    let a = args(&result);
    // Each RST-style line becomes a bare-name arg entry (no colon split).
    assert_eq!(a.len(), 2);
    // name() must not panic, and the full trimmed line is the name.
    assert_eq!(a[0].name().text(result.source()), ":param int seconds: The seconds.");
    assert_eq!(
        a[1].name().text(result.source()),
        ":param int nanoseconds: The nanoseconds."
    );
}
