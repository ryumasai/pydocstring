//! Convert a NumPy-style AST into the style-independent [`Docstring`] model.

use crate::model::Attribute;
use crate::model::Deprecation;
use crate::model::Docstring;
use crate::model::ExceptionEntry;
use crate::model::FreeSectionKind;
use crate::model::Method;
use crate::model::Parameter;
use crate::model::Reference;
use crate::model::Return;
use crate::model::Section;
use crate::model::SeeAlsoEntry;
use crate::parse::numpy::kind::NumPySectionKind;
use crate::parse::numpy::nodes::NumPyDocstring;
use crate::parse::numpy::nodes::NumPySection;
use crate::parse::utils::convert_multiline_with_indentation;
use crate::syntax::Parsed;

/// Build a [`Docstring`] from a NumPy-style [`Parsed`] result.
///
/// Returns `None` if the docstring was not parsed as
/// [`Style::NumPy`](crate::parse::Style::NumPy).
pub fn to_model(parsed: &Parsed) -> Option<Docstring> {
    if parsed.style() != crate::parse::Style::NumPy {
        return None;
    }
    let source = parsed.source();
    let root = NumPyDocstring::cast(parsed.root())?;

    let summary = root.summary().map(|t| t.text(source).to_owned());
    let extended_summary = root.extended_summary().map(|t| t.text(source).to_owned());

    let deprecation = root.deprecation().map(|dep| Deprecation {
        version: dep.version().text(source).to_owned(),
        description: dep.description().map(|t| t.text(source).to_owned()),
    });

    let sections = root.sections().map(|s| convert_section(&s, source)).collect();

    Some(Docstring {
        summary,
        extended_summary,
        deprecation,
        sections,
    })
}

fn convert_section(section: &NumPySection<'_>, source: &str) -> Section {
    let kind = section.section_kind(source);

    match kind {
        NumPySectionKind::Parameters | NumPySectionKind::Receives => {
            let entries = section.parameters().map(|p| convert_parameter(&p, source)).collect();
            match kind {
                NumPySectionKind::Parameters => Section::Parameters(entries),
                NumPySectionKind::Receives => Section::Receives(entries),
                _ => unreachable!(),
            }
        }
        NumPySectionKind::OtherParameters => {
            Section::OtherParameters(section.parameters().map(|p| convert_parameter(&p, source)).collect())
        }
        NumPySectionKind::KeywordParameters => {
            Section::KeywordParameters(section.parameters().map(|p| convert_parameter(&p, source)).collect())
        }
        NumPySectionKind::Returns => {
            let entries: Vec<Return> = section
                .returns()
                .map(|r| Return {
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
            let entries: Vec<Return> = section
                .yields()
                .map(|r| Return {
                    name: r.name().map(|t| t.text(source).to_owned()),
                    type_annotation: r.return_type().map(|t| t.text(source).to_owned()),
                    description: r
                        .description()
                        .map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect();
            Section::Yields(entries)
        }
        NumPySectionKind::Raises => Section::Raises(
            section
                .exceptions()
                .map(|e| ExceptionEntry {
                    type_name: e.r#type().text(source).to_owned(),
                    description: e
                        .description()
                        .map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect(),
        ),
        NumPySectionKind::Warns => Section::Warns(
            section
                .warnings()
                .map(|w| ExceptionEntry {
                    type_name: w.r#type().text(source).to_owned(),
                    description: w
                        .description()
                        .map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect(),
        ),
        NumPySectionKind::SeeAlso => Section::SeeAlso(
            section
                .see_also_items()
                .map(|item| SeeAlsoEntry {
                    names: item.names().map(|n| n.text(source).to_owned()).collect(),
                    description: item
                        .description()
                        .map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect(),
        ),
        NumPySectionKind::References => Section::References(
            section
                .references()
                .map(|r| Reference {
                    number: r.number().map(|t| t.text(source).to_owned()),
                    content: r.content().map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect(),
        ),
        NumPySectionKind::Attributes => Section::Attributes(
            section
                .attributes()
                .map(|a| Attribute {
                    name: a.name().text(source).to_owned(),
                    type_annotation: a.r#type().map(|t| t.text(source).to_owned()),
                    description: a
                        .description()
                        .map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect(),
        ),
        NumPySectionKind::Methods => Section::Methods(
            section
                .methods()
                .map(|m| Method {
                    name: m.name().text(source).to_owned(),
                    type_annotation: None,
                    description: m
                        .description()
                        .map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect(),
        ),
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
                NumPySectionKind::Todo => FreeSectionKind::Todo,
                NumPySectionKind::Attention => FreeSectionKind::Attention,
                NumPySectionKind::Caution => FreeSectionKind::Caution,
                NumPySectionKind::Danger => FreeSectionKind::Danger,
                NumPySectionKind::Error => FreeSectionKind::Error,
                NumPySectionKind::Hint => FreeSectionKind::Hint,
                NumPySectionKind::Important => FreeSectionKind::Important,
                NumPySectionKind::Tip => FreeSectionKind::Tip,
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
    }
}
