//! Convert a Google-style AST into the style-independent [`Docstring`] model.

use crate::model::{
    Attribute, Docstring, ExceptionEntry, FreeSectionKind, Method, Parameter, Return, Section, SeeAlsoEntry,
};
use crate::parse::ToModelOptions;
use crate::parse::google::kind::GoogleSectionKind;
use crate::parse::google::nodes::{GoogleDocstring, GoogleSection};
use crate::parse::utils::{blank_lines_before, convert_multiline_with_indentation};
use crate::syntax::Parsed;

/// Build a [`Docstring`] from a Google-style [`Parsed`] result.
///
/// Produces a normalized model (formatting trivia such as blank lines is
/// dropped). Use [`to_model_with_options`] to opt into preserving layout.
///
/// Returns `None` if the root node is not a `GOOGLE_DOCSTRING`.
pub fn to_model(parsed: &Parsed) -> Option<Docstring> {
    to_model_with_options(parsed, ToModelOptions::default())
}

/// Build a [`Docstring`] from a Google-style [`Parsed`] result, honoring `options`.
///
/// Returns `None` if the root node is not a `GOOGLE_DOCSTRING`.
pub fn to_model_with_options(parsed: &Parsed, options: ToModelOptions) -> Option<Docstring> {
    let source = parsed.source();
    let root = GoogleDocstring::cast(parsed.root())?;

    let summary = root.summary().map(|t| t.text(source).to_owned());
    let extended_summary = root
        .extended_summary()
        .map(|t| convert_multiline_with_indentation(t.text(source)));

    let sections = root.sections().map(|s| convert_section(&s, source, options)).collect();

    Some(Docstring {
        summary,
        extended_summary,
        deprecation: None,
        sections,
    })
}

fn convert_section(section: &GoogleSection<'_>, source: &str, options: ToModelOptions) -> Section {
    let kind = section.section_kind(source);
    let preserve = options.preserve_blank_lines;

    match kind {
        GoogleSectionKind::Args
        | GoogleSectionKind::Receives
        | GoogleSectionKind::KeywordArgs
        | GoogleSectionKind::OtherParameters => {
            let mut prev_end = None;
            let entries: Vec<Parameter> = section
                .args()
                .map(|a| {
                    let mut e = convert_arg(&a, source);
                    e.blank_lines_before = blank_lines_before(source, &mut prev_end, a.syntax().range(), preserve);
                    e
                })
                .collect();
            match kind {
                GoogleSectionKind::Args => Section::Parameters(entries),
                GoogleSectionKind::Receives => Section::Receives(entries),
                GoogleSectionKind::KeywordArgs => Section::KeywordParameters(entries),
                GoogleSectionKind::OtherParameters => Section::OtherParameters(entries),
                _ => unreachable!(),
            }
        }
        GoogleSectionKind::Returns => {
            let mut prev_end = None;
            let entries: Vec<Return> = section
                .returns()
                .into_iter()
                .map(|r| Return {
                    blank_lines_before: blank_lines_before(source, &mut prev_end, r.syntax().range(), preserve),
                    name: None,
                    type_annotation: r.return_type().map(|t| t.text(source).to_owned()),
                    description: r
                        .description()
                        .map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect();
            Section::Returns(entries)
        }
        GoogleSectionKind::Yields => {
            let mut prev_end = None;
            let entries: Vec<Return> = section
                .yields()
                .into_iter()
                .map(|r| Return {
                    blank_lines_before: blank_lines_before(source, &mut prev_end, r.syntax().range(), preserve),
                    name: None,
                    type_annotation: r.return_type().map(|t| t.text(source).to_owned()),
                    description: r
                        .description()
                        .map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect();
            Section::Yields(entries)
        }
        GoogleSectionKind::Raises => {
            let mut prev_end = None;
            Section::Raises(
                section
                    .exceptions()
                    .map(|e| {
                        let mut entry = convert_exception(&e, source);
                        entry.blank_lines_before =
                            blank_lines_before(source, &mut prev_end, e.syntax().range(), preserve);
                        entry
                    })
                    .collect(),
            )
        }
        GoogleSectionKind::Warns => {
            let mut prev_end = None;
            Section::Warns(
                section
                    .warnings()
                    .map(|w| ExceptionEntry {
                        blank_lines_before: blank_lines_before(source, &mut prev_end, w.syntax().range(), preserve),
                        type_name: w.warning_type().text(source).to_owned(),
                        description: w
                            .description()
                            .map(|t| convert_multiline_with_indentation(t.text(source))),
                    })
                    .collect(),
            )
        }
        GoogleSectionKind::SeeAlso => {
            let mut prev_end = None;
            Section::SeeAlso(
                section
                    .see_also_items()
                    .map(|item| SeeAlsoEntry {
                        blank_lines_before: blank_lines_before(source, &mut prev_end, item.syntax().range(), preserve),
                        names: item.names().map(|n| n.text(source).to_owned()).collect(),
                        description: item
                            .description()
                            .map(|t| convert_multiline_with_indentation(t.text(source))),
                    })
                    .collect(),
            )
        }
        GoogleSectionKind::Attributes => {
            let mut prev_end = None;
            Section::Attributes(
                section
                    .attributes()
                    .map(|a| Attribute {
                        blank_lines_before: blank_lines_before(source, &mut prev_end, a.syntax().range(), preserve),
                        name: a.name().text(source).to_owned(),
                        type_annotation: a.r#type().map(|t| t.text(source).to_owned()),
                        description: a
                            .description()
                            .map(|t| convert_multiline_with_indentation(t.text(source))),
                    })
                    .collect(),
            )
        }
        GoogleSectionKind::Methods => {
            let mut prev_end = None;
            Section::Methods(
                section
                    .methods()
                    .map(|m| Method {
                        blank_lines_before: blank_lines_before(source, &mut prev_end, m.syntax().range(), preserve),
                        name: m.name().text(source).to_owned(),
                        type_annotation: m.r#type().map(|t| t.text(source).to_owned()),
                        description: m
                            .description()
                            .map(|t| convert_multiline_with_indentation(t.text(source))),
                    })
                    .collect(),
            )
        }
        // Free-text sections
        _ => {
            let body = section
                .body_text()
                .map(|t| convert_multiline_with_indentation(t.text(source)))
                .unwrap_or_default();
            let free_kind = match kind {
                GoogleSectionKind::Notes => FreeSectionKind::Notes,
                GoogleSectionKind::Examples => FreeSectionKind::Examples,
                GoogleSectionKind::Todo => FreeSectionKind::Todo,
                GoogleSectionKind::References => FreeSectionKind::Unknown("References".into()),
                GoogleSectionKind::Warnings => FreeSectionKind::Warnings,
                GoogleSectionKind::Attention => FreeSectionKind::Attention,
                GoogleSectionKind::Caution => FreeSectionKind::Caution,
                GoogleSectionKind::Danger => FreeSectionKind::Danger,
                GoogleSectionKind::Error => FreeSectionKind::Error,
                GoogleSectionKind::Hint => FreeSectionKind::Hint,
                GoogleSectionKind::Important => FreeSectionKind::Important,
                GoogleSectionKind::Tip => FreeSectionKind::Tip,
                GoogleSectionKind::Unknown => FreeSectionKind::Unknown(section.header().name().text(source).to_owned()),
                _ => unreachable!(),
            };
            Section::FreeText { kind: free_kind, body }
        }
    }
}

fn convert_arg(arg: &crate::parse::google::nodes::GoogleArg<'_>, source: &str) -> Parameter {
    Parameter {
        names: vec![arg.name().text(source).to_owned()],
        type_annotation: arg.r#type().map(|t| t.text(source).to_owned()),
        description: arg
            .description()
            .map(|t| convert_multiline_with_indentation(t.text(source))),
        is_optional: arg.optional().is_some(),
        default_value: None,
        blank_lines_before: 0,
    }
}

fn convert_exception(exc: &crate::parse::google::nodes::GoogleException<'_>, source: &str) -> ExceptionEntry {
    ExceptionEntry {
        type_name: exc.r#type().text(source).to_owned(),
        description: exc
            .description()
            .map(|t| convert_multiline_with_indentation(t.text(source))),
        blank_lines_before: 0,
    }
}
