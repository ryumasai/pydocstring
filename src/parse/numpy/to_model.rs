//! Convert a NumPy-style AST into the style-independent [`Docstring`] model.

use crate::model::{
    Attribute, Deprecation, Docstring, ExceptionEntry, FreeSectionKind, Method, Parameter, Reference, Return, Section,
    SeeAlsoEntry,
};
use crate::parse::ToModelOptions;
use crate::parse::numpy::kind::NumPySectionKind;
use crate::parse::numpy::nodes::{NumPyDocstring, NumPySection};
use crate::parse::utils::{blank_lines_before, convert_multiline_with_indentation};
use crate::syntax::Parsed;

/// Build a [`Docstring`] from a NumPy-style [`Parsed`] result.
///
/// Produces a normalized model (formatting trivia such as blank lines is
/// dropped). Use [`to_model_with_options`] to opt into preserving layout.
///
/// Returns `None` if the root node is not a `NUMPY_DOCSTRING`.
pub fn to_model(parsed: &Parsed) -> Option<Docstring> {
    to_model_with_options(parsed, ToModelOptions::default())
}

/// Build a [`Docstring`] from a NumPy-style [`Parsed`] result, honoring `options`.
///
/// Returns `None` if the root node is not a `NUMPY_DOCSTRING`.
pub fn to_model_with_options(parsed: &Parsed, options: ToModelOptions) -> Option<Docstring> {
    let source = parsed.source();
    let root = NumPyDocstring::cast(parsed.root())?;

    let summary = root.summary().map(|t| t.text(source).to_owned());
    let extended_summary = root.extended_summary().map(|t| t.text(source).to_owned());

    let deprecation = root.deprecation().map(|dep| Deprecation {
        version: dep.version().text(source).to_owned(),
        description: dep.description().map(|t| t.text(source).to_owned()),
    });

    let sections = root.sections().map(|s| convert_section(&s, source, options)).collect();

    Some(Docstring {
        summary,
        extended_summary,
        deprecation,
        sections,
    })
}

fn convert_section(section: &NumPySection<'_>, source: &str, options: ToModelOptions) -> Section {
    let kind = section.section_kind(source);
    let preserve = options.preserve_blank_lines;

    match kind {
        NumPySectionKind::Parameters | NumPySectionKind::Receives => {
            let mut prev_end = None;
            let entries = section
                .parameters()
                .map(|p| {
                    let mut e = convert_parameter(&p, source);
                    e.blank_lines_before = blank_lines_before(source, &mut prev_end, p.syntax().range(), preserve);
                    e
                })
                .collect();
            match kind {
                NumPySectionKind::Parameters => Section::Parameters(entries),
                NumPySectionKind::Receives => Section::Receives(entries),
                _ => unreachable!(),
            }
        }
        NumPySectionKind::OtherParameters => {
            let mut prev_end = None;
            Section::OtherParameters(
                section
                    .parameters()
                    .map(|p| {
                        let mut e = convert_parameter(&p, source);
                        e.blank_lines_before = blank_lines_before(source, &mut prev_end, p.syntax().range(), preserve);
                        e
                    })
                    .collect(),
            )
        }
        NumPySectionKind::Returns => {
            let mut prev_end = None;
            let entries: Vec<Return> = section
                .returns()
                .map(|r| Return {
                    blank_lines_before: blank_lines_before(source, &mut prev_end, r.syntax().range(), preserve),
                    name: r.name().map(|t| t.text(source).to_owned()),
                    type_annotation: r.return_type().map(|t| t.text(source).to_owned()),
                    description: r
                        .description()
                        .map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect();
            Section::Returns(entries)
        }
        NumPySectionKind::Yields => {
            let mut prev_end = None;
            let entries: Vec<Return> = section
                .yields()
                .map(|r| Return {
                    blank_lines_before: blank_lines_before(source, &mut prev_end, r.syntax().range(), preserve),
                    name: r.name().map(|t| t.text(source).to_owned()),
                    type_annotation: r.return_type().map(|t| t.text(source).to_owned()),
                    description: r
                        .description()
                        .map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect();
            Section::Yields(entries)
        }
        NumPySectionKind::Raises => {
            let mut prev_end = None;
            Section::Raises(
                section
                    .exceptions()
                    .map(|e| ExceptionEntry {
                        blank_lines_before: blank_lines_before(source, &mut prev_end, e.syntax().range(), preserve),
                        type_name: e.r#type().text(source).to_owned(),
                        description: e
                            .description()
                            .map(|t| convert_multiline_with_indentation(t.text(source))),
                    })
                    .collect(),
            )
        }
        NumPySectionKind::Warns => {
            let mut prev_end = None;
            Section::Warns(
                section
                    .warnings()
                    .map(|w| ExceptionEntry {
                        blank_lines_before: blank_lines_before(source, &mut prev_end, w.syntax().range(), preserve),
                        type_name: w.r#type().text(source).to_owned(),
                        description: w
                            .description()
                            .map(|t| convert_multiline_with_indentation(t.text(source))),
                    })
                    .collect(),
            )
        }
        NumPySectionKind::SeeAlso => {
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
        NumPySectionKind::References => {
            let mut prev_end = None;
            Section::References(
                section
                    .references()
                    .map(|r| Reference {
                        blank_lines_before: blank_lines_before(source, &mut prev_end, r.syntax().range(), preserve),
                        number: r.number().map(|t| t.text(source).to_owned()),
                        content: r.content().map(|t| convert_multiline_with_indentation(t.text(source))),
                    })
                    .collect(),
            )
        }
        NumPySectionKind::Attributes => {
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
        NumPySectionKind::Methods => {
            let mut prev_end = None;
            Section::Methods(
                section
                    .methods()
                    .map(|m| Method {
                        blank_lines_before: blank_lines_before(source, &mut prev_end, m.syntax().range(), preserve),
                        name: m.name().text(source).to_owned(),
                        type_annotation: None,
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
                .map(|t| t.text(source).to_owned())
                .unwrap_or_default();
            let free_kind = match kind {
                NumPySectionKind::Notes => FreeSectionKind::Notes,
                NumPySectionKind::Examples => FreeSectionKind::Examples,
                NumPySectionKind::Warnings => FreeSectionKind::Warnings,
                NumPySectionKind::Unknown => FreeSectionKind::Unknown(section.header().name().text(source).to_owned()),
                _ => unreachable!(),
            };
            Section::FreeText { kind: free_kind, body }
        }
    }
}

fn convert_parameter(param: &crate::parse::numpy::nodes::NumPyParameter<'_>, source: &str) -> Parameter {
    Parameter {
        names: param.names().map(|n| n.text(source).to_owned()).collect(),
        type_annotation: param.r#type().map(|t| t.text(source).to_owned()),
        description: param
            .description()
            .map(|t| convert_multiline_with_indentation(t.text(source))),
        is_optional: param.optional().is_some(),
        default_value: param.default_value().map(|t| t.text(source).to_owned()),
        blank_lines_before: 0,
    }
}
