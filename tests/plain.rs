use pydocstring::model::SectionKind;
use pydocstring::parse::parse_plain;
use pydocstring::parse::unified::Document;
use pydocstring::syntax::SyntaxKind;

#[test]
fn test_empty() {
    let result = parse_plain("");
    assert_eq!(result.root().kind(), SyntaxKind::DOCUMENT);
    let doc = Document::new(&result);
    assert!(doc.summary().is_none());
    assert!(doc.extended_summary().is_none());
}

#[test]
fn test_summary_only() {
    let result = parse_plain("Just a summary.");
    let doc = Document::new(&result);
    assert_eq!(doc.summary().unwrap().text(), "Just a summary.");
    assert!(doc.extended_summary().is_none());
}

#[test]
fn test_summary_and_extended() {
    let result = parse_plain("Summary.\n\nExtended description.\nMore lines.");
    let doc = Document::new(&result);
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
    let doc = Document::new(&result);
    assert_eq!(doc.summary().unwrap().text(), "Summary.");
}

#[test]
fn test_to_model_empty() {
    let result = parse_plain("");
    let model = result.to_model();
    assert!(model.summary.is_none());
    assert!(model.extended_summary.is_none());
    assert!(model.sections.is_empty());
}

#[test]
fn test_to_model_summary_only() {
    let result = parse_plain("Summary.");
    let model = result.to_model();
    assert_eq!(model.summary.as_deref(), Some("Summary."));
    assert!(model.extended_summary.is_none());
}

#[test]
fn test_to_model_dispatches_on_the_parsed_style() {
    // The same source, forced through two parsers. `to_model` follows the
    // style the tree was parsed with, so the plain reading keeps the section
    // as prose while the Google reading resolves it to a section.
    let src = "Summary.\n\nArgs:\n    x: Desc.";

    let plain = parse_plain(src).to_model();
    assert!(plain.sections.is_empty());

    let google = pydocstring::parse::parse_google(src).to_model();
    assert_eq!(google.sections.len(), 1);
    assert_eq!(google.sections[0].kind, SectionKind::Parameters);
}
