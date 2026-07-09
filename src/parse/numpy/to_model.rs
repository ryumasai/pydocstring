//! Convert a NumPy-style AST into the style-independent [`Docstring`] model.

use crate::model::Attribute;
use crate::model::Docstring;
use crate::model::ExceptionEntry;
use crate::model::FreeSectionKind;
use crate::model::Method;
use crate::model::Parameter;
use crate::model::Reference;
use crate::model::Return;
use crate::model::Section;
use crate::model::SectionKind;
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
    let root = NumPyDocstring::cast(parsed, parsed.root())?;

    let summary = root.summary().map(|t| t.text().to_owned());
    let extended_summary = root.extended_summary().map(|t| t.text().to_owned());

    let directives = crate::parse::utils::convert_directives(parsed);

    let sections = root.sections().map(|s| convert_section(&s)).collect();

    Some(Docstring {
        summary,
        extended_summary,
        directives,
        sections,
    })
}

fn convert_section(section: &NumPySection<'_>) -> Section {
    let kind = section.section_kind();

    match kind {
        NumPySectionKind::Parameters | NumPySectionKind::Receives => {
            let entries = section.parameters().map(|p| convert_parameter(&p)).collect();
            match kind {
                NumPySectionKind::Parameters => Section::Parameters(entries),
                NumPySectionKind::Receives => Section::Receives(entries),
                _ => unreachable!(),
            }
        }
        NumPySectionKind::OtherParameters => {
            Section::OtherParameters(section.parameters().map(|p| convert_parameter(&p)).collect())
        }
        NumPySectionKind::KeywordParameters => {
            Section::KeywordParameters(section.parameters().map(|p| convert_parameter(&p)).collect())
        }
        NumPySectionKind::Returns => {
            let entries: Vec<Return> = section
                .returns()
                .map(|r| Return {
                    name: r.name().map(|t| t.text().to_owned()),
                    type_annotation: r.type_annotation().map(|t| t.text().to_owned()),
                    description: r.description().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect();
            Section::Returns(entries)
        }
        NumPySectionKind::Yields => {
            let entries: Vec<Return> = section
                .yields()
                .map(|r| Return {
                    name: r.name().map(|t| t.text().to_owned()),
                    type_annotation: r.type_annotation().map(|t| t.text().to_owned()),
                    description: r.description().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect();
            Section::Yields(entries)
        }
        NumPySectionKind::Raises => Section::Raises(
            section
                .exceptions()
                .map(|e| ExceptionEntry {
                    type_name: e.type_annotation().text().to_owned(),
                    description: e.description().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect(),
        ),
        NumPySectionKind::Warns => Section::Warns(
            section
                .warnings()
                .map(|w| ExceptionEntry {
                    type_name: w.type_annotation().text().to_owned(),
                    description: w.description().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect(),
        ),
        NumPySectionKind::SeeAlso => Section::SeeAlso(
            section
                .see_also_items()
                .map(|item| SeeAlsoEntry {
                    names: item.names().map(|n| n.text().to_owned()).collect(),
                    description: item.description().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect(),
        ),
        NumPySectionKind::References => Section::References(
            section
                .references()
                .map(|r| Reference {
                    label: r.label().map(|t| t.text().to_owned()),
                    content: r.content().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect(),
        ),
        NumPySectionKind::Attributes => Section::Attributes(
            section
                .attributes()
                .map(|a| Attribute {
                    names: a.names().map(|n| n.text().to_owned()).collect(),
                    type_annotation: a.type_annotation().map(|t| t.text().to_owned()),
                    description: a.description().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect(),
        ),
        NumPySectionKind::Methods => Section::Methods(
            section
                .methods()
                .map(|m| Method {
                    name: m.name().text().to_owned(),
                    type_annotation: None,
                    description: m.description().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect(),
        ),
        // Free-text sections
        _ => {
            let body = section.body_text().map(|t| t.text().to_owned()).unwrap_or_default();
            // A structured kind reaching this arm would mean to_section_kind and
            // the structured arms above drifted apart; degrade gracefully.
            let free_kind = match kind.to_section_kind(section.header().name().text()) {
                SectionKind::FreeText(k) => k,
                _ => FreeSectionKind::Unknown(section.header().name().text().to_owned()),
            };
            Section::FreeText { kind: free_kind, body }
        }
    }
}

fn convert_parameter(param: &crate::parse::numpy::nodes::NumPyParameter<'_>) -> Parameter {
    Parameter {
        names: param.names().map(|n| n.text().to_owned()).collect(),
        type_annotation: param.type_annotation().map(|t| t.text().to_owned()),
        description: param
            .description()
            .map(|t| convert_multiline_with_indentation(t.text())),
        is_optional: param.is_optional(),
        default_value: param.default_value().map(|t| t.text().to_owned()),
    }
}
