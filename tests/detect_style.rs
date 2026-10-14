use pydocstring::parse::Style;
use pydocstring::parse::detect_style;
use pydocstring::parse::parse;
use pydocstring::syntax::SyntaxKind;

#[test]
fn test_detect_numpy() {
    let input = "Summary.\n\nParameters\n----------\nx : int\n    Desc.";
    assert_eq!(detect_style(input), Style::NumPy);
}

#[test]
fn test_detect_google() {
    let input = "Summary.\n\nArgs:\n    x: Desc.";
    assert_eq!(detect_style(input), Style::Google);
}

#[test]
fn test_detect_plain_summary_only() {
    assert_eq!(detect_style("Just a summary."), Style::Plain);
}

#[test]
fn test_detect_plain_summary_and_extended() {
    assert_eq!(detect_style("Summary.\n\nMore detail here."), Style::Plain);
}

#[test]
fn test_detect_plain_empty() {
    assert_eq!(detect_style(""), Style::Plain);
}

#[test]
fn test_detect_plain_sphinx() {
    let input = "Summary.\n\n:param x: Description.\n:type x: int\n:rtype: int";
    assert_eq!(detect_style(input), Style::Plain);
}

#[test]
fn test_parse_dispatches_to_plain() {
    let result = parse("Just a summary.");
    assert_eq!(result.root().kind(), SyntaxKind::DOCUMENT);
    assert_eq!(result.style(), Style::Plain);
}

#[test]
fn test_parse_dispatches_to_google() {
    let result = parse("Summary.\n\nArgs:\n    x: Desc.");
    assert_eq!(result.root().kind(), SyntaxKind::DOCUMENT);
    assert_eq!(result.style(), Style::Google);
}

#[test]
fn test_parse_dispatches_to_numpy() {
    let result = parse("Summary.\n\nParameters\n----------\nx : int\n    Desc.");
    assert_eq!(result.root().kind(), SyntaxKind::DOCUMENT);
    assert_eq!(result.style(), Style::NumPy);
}

// =============================================================================
// SPEC: Parsed::style() — the root kind is style-neutral (DOCUMENT), so each
// parser records the style it parsed as.
// =============================================================================

#[test]
fn spec_parsed_style_reports_the_parser_style() {
    let google = pydocstring::parse::parse_google("Summary.\n\nArgs:\n    x: Desc.");
    assert_eq!(google.style(), Style::Google);
    assert_eq!(google.root().kind(), SyntaxKind::DOCUMENT);

    let numpy = pydocstring::parse::parse_numpy("Summary.\n\nParameters\n----------\nx : int\n    Desc.");
    assert_eq!(numpy.style(), Style::NumPy);
    assert_eq!(numpy.root().kind(), SyntaxKind::DOCUMENT);

    let plain = pydocstring::parse::parse_plain("Just a summary.");
    assert_eq!(plain.style(), Style::Plain);
    assert_eq!(plain.root().kind(), SyntaxKind::DOCUMENT);
}

/// SPEC: every style parses to the same unified `DOCUMENT` root, so the
/// style-independent view reads a Google-parsed tree with no per-style step.
#[test]
fn spec_unified_view_reads_a_google_parsed_document_root() {
    let parsed = pydocstring::parse::parse_google("Summary.\n\nArgs:\n    x: Desc.");
    assert_eq!(parsed.root().kind(), SyntaxKind::DOCUMENT);
    let doc = pydocstring::parse::unified::Document::new(&parsed);
    assert_eq!(doc.summary().unwrap().text(), "Summary.");
    assert_eq!(doc.sections().count(), 1);
}

// =============================================================================
// #142: detection must not disagree with the parsers it dispatches to
// =============================================================================

/// A reST transition / markdown rule in prose is not a NumPy underline: only
/// a *known* section name above the dashes flips detection.
#[test]
fn spec_dash_run_in_prose_is_not_numpy() {
    let input = "Summary.\n\nSome prose.\n---------\n\nArgs:\n    x: Desc.";
    assert_eq!(detect_style(input), Style::Google);

    let no_google_marker = "Summary.\n\nSome prose.\n---------\n\nMore prose.";
    assert_eq!(detect_style(no_google_marker), Style::Plain);
}

/// The Google parser trims the name (`Args :`) and accepts bare known names
/// (`Args`); detection accepts exactly the same spellings.
#[test]
fn spec_google_header_spellings_the_parser_accepts_detect_google() {
    assert_eq!(detect_style("Summary.\n\nArgs :\n    x: Desc."), Style::Google);
    assert_eq!(detect_style("Summary.\n\nArgs\n    x: Desc."), Style::Google);
}

/// A known name above the underline detects NumPy with the parser's own
/// underline rule — any run of dashes, not three-plus.
#[test]
fn spec_numpy_underline_rule_matches_the_parser() {
    assert_eq!(detect_style("Summary.\n\nReturns\n-\nint\n    Desc."), Style::NumPy);
}

/// A bare known name followed by an underline is NumPy, not Google: the
/// NumPy check wins on the same line.
#[test]
fn spec_underlined_known_name_prefers_numpy() {
    let input = "Summary.\n\nReturns\n-------\nint\n    Desc.";
    assert_eq!(detect_style(input), Style::NumPy);
}
