use pydocstring::parse::plain::nodes::PlainDocstring;
use pydocstring::parse::plain::parse_plain;
use pydocstring::parse::plain::to_model::to_model;
use pydocstring::syntax::SyntaxKind;

#[test]
fn test_empty() {
    let result = parse_plain("");
    assert_eq!(result.root().kind(), SyntaxKind::DOCUMENT);
    let doc = PlainDocstring::cast(&result, result.root()).unwrap();
    assert!(doc.summary().is_none());
    assert!(doc.extended_summary().is_none());
}

#[test]
fn test_summary_only() {
    let result = parse_plain("Just a summary.");
    let doc = PlainDocstring::cast(&result, result.root()).unwrap();
    assert_eq!(doc.summary().unwrap().text(), "Just a summary.");
    assert!(doc.extended_summary().is_none());
}

#[test]
fn test_summary_and_extended() {
    let result = parse_plain("Summary.\n\nExtended description.\nMore lines.");
    let doc = PlainDocstring::cast(&result, result.root()).unwrap();
    assert_eq!(doc.summary().unwrap().text(), "Summary.");
    assert_eq!(
        doc.extended_summary().unwrap().text(),
        "Extended description.\nMore lines."
    );
}

#[test]
fn test_sphinx_treated_as_plain() {
    let input = "Summary.\n\n:param x: Description.\n:rtype: int";
    let result = parse_plain(input);
    assert_eq!(result.root().kind(), SyntaxKind::DOCUMENT);
    let doc = PlainDocstring::cast(&result, result.root()).unwrap();
    assert_eq!(doc.summary().unwrap().text(), "Summary.");
}

#[test]
fn test_to_model_empty() {
    let result = parse_plain("");
    let model = to_model(&result).unwrap();
    assert!(model.summary.is_none());
    assert!(model.extended_summary.is_none());
    assert!(model.sections.is_empty());
}

#[test]
fn test_to_model_summary_only() {
    let result = parse_plain("Summary.");
    let model = to_model(&result).unwrap();
    assert_eq!(model.summary.as_deref(), Some("Summary."));
    assert!(model.extended_summary.is_none());
}

#[test]
fn test_to_model_returns_none_for_wrong_kind() {
    use pydocstring::parse::google::parse_google;
    let result = parse_google("Summary.\n\nArgs:\n    x: Desc.");
    // google root → plain to_model should return None
    assert!(to_model(&result).is_none());
}
