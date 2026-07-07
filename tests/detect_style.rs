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
    assert_eq!(result.root().kind(), SyntaxKind::PLAIN_DOCSTRING);
}

#[test]
fn test_parse_dispatches_to_google() {
    let result = parse("Summary.\n\nArgs:\n    x: Desc.");
    assert_eq!(result.root().kind(), SyntaxKind::GOOGLE_DOCSTRING);
}

#[test]
fn test_parse_dispatches_to_numpy() {
    let result = parse("Summary.\n\nParameters\n----------\nx : int\n    Desc.");
    assert_eq!(result.root().kind(), SyntaxKind::NUMPY_DOCSTRING);
}
