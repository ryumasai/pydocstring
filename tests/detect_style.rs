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
    let google = pydocstring::parse::google::parse_google("Summary.\n\nArgs:\n    x: Desc.");
    assert_eq!(google.style(), Style::Google);
    assert_eq!(google.root().kind(), SyntaxKind::DOCUMENT);

    let numpy = pydocstring::parse::numpy::parse_numpy("Summary.\n\nParameters\n----------\nx : int\n    Desc.");
    assert_eq!(numpy.style(), Style::NumPy);
    assert_eq!(numpy.root().kind(), SyntaxKind::DOCUMENT);

    let plain = pydocstring::parse::plain::parse_plain("Just a summary.");
    assert_eq!(plain.style(), Style::Plain);
    assert_eq!(plain.root().kind(), SyntaxKind::DOCUMENT);
}

/// SPEC: every style parses to the same unified `DOCUMENT` root, so the
/// style-independent view reads a Google-parsed tree with no per-style step.
#[test]
fn spec_unified_view_reads_a_google_parsed_document_root() {
    let parsed = pydocstring::parse::google::parse_google("Summary.\n\nArgs:\n    x: Desc.");
    assert_eq!(parsed.root().kind(), SyntaxKind::DOCUMENT);
    let doc = pydocstring::parse::unified::Document::new(&parsed);
    assert_eq!(doc.summary().unwrap().text(), "Summary.");
    assert_eq!(doc.sections().count(), 1);
}
