//! Convert a Google-style AST into the style-independent [`Docstring`] model.

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
use crate::parse::google::kind::GoogleSectionKind;
use crate::parse::google::nodes::GoogleDocstring;
use crate::parse::google::nodes::GoogleSection;
use crate::parse::utils::convert_multiline_with_indentation;
use crate::syntax::Parsed;

/// Build a [`Docstring`] from a Google-style [`Parsed`] result.
///
/// Returns `None` if the docstring was not parsed as
/// [`Style::Google`](crate::parse::Style::Google).
pub fn to_model(parsed: &Parsed) -> Option<Docstring> {
    if parsed.style() != crate::parse::Style::Google {
        return None;
    }
    let root = GoogleDocstring::cast(parsed, parsed.root())?;

    let summary = root.summary().map(|t| t.text().to_owned());
    let extended_summary = root
        .extended_summary()
        .map(|t| convert_multiline_with_indentation(t.text()));

    let directives = crate::parse::utils::convert_directives(parsed);

    let sections = root.sections().map(|s| convert_section(&s)).collect();

    Some(Docstring {
        summary,
        extended_summary,
        directives,
        sections,
    })
}

fn convert_section(section: &GoogleSection<'_>) -> Section {
    let kind = section.section_kind();

    match kind {
        GoogleSectionKind::Args | GoogleSectionKind::Receives => {
            let entries = section.args().map(|a| convert_arg(&a)).collect();
            match kind {
                GoogleSectionKind::Args => Section::Parameters(entries),
                GoogleSectionKind::Receives => Section::Receives(entries),
                _ => unreachable!(),
            }
        }
        GoogleSectionKind::KeywordArgs => Section::KeywordParameters(section.args().map(|a| convert_arg(&a)).collect()),
        GoogleSectionKind::OtherParameters => {
            Section::OtherParameters(section.args().map(|a| convert_arg(&a)).collect())
        }
        GoogleSectionKind::Returns => {
            let entries: Vec<Return> = section
                .returns()
                .into_iter()
                .map(|r| Return {
                    name: None,
                    type_annotation: r.type_annotation().map(|t| t.text().to_owned()),
                    description: r.description().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect();
            Section::Returns(entries)
        }
        GoogleSectionKind::Yields => {
            let entries: Vec<Return> = section
                .yields()
                .into_iter()
                .map(|r| Return {
                    name: None,
                    type_annotation: r.type_annotation().map(|t| t.text().to_owned()),
                    description: r.description().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect();
            Section::Yields(entries)
        }
        GoogleSectionKind::Raises => Section::Raises(section.exceptions().map(|e| convert_exception(&e)).collect()),
        GoogleSectionKind::Warns => Section::Warns(
            section
                .warnings()
                .map(|w| ExceptionEntry {
                    type_name: w.type_annotation().text().to_owned(),
                    description: w.description().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect(),
        ),
        GoogleSectionKind::SeeAlso => Section::SeeAlso(
            section
                .see_also_items()
                .map(|item| SeeAlsoEntry {
                    names: item.names().map(|n| n.text().to_owned()).collect(),
                    description: item.description().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect(),
        ),
        GoogleSectionKind::Attributes => Section::Attributes(
            section
                .attributes()
                .map(|a| Attribute {
                    names: a.names().map(|n| n.text().to_owned()).collect(),
                    type_annotation: a.type_annotation().map(|t| t.text().to_owned()),
                    description: a.description().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect(),
        ),
        GoogleSectionKind::References => Section::References(
            section
                .references()
                .map(|r| Reference {
                    label: r.label().map(|t| t.text().to_owned()),
                    content: r.content().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect(),
        ),
        GoogleSectionKind::Methods => Section::Methods(
            section
                .methods()
                .map(|m| Method {
                    name: m.name().text().to_owned(),
                    type_annotation: m.type_annotation().map(|t| t.text().to_owned()),
                    description: m.description().map(|t| convert_multiline_with_indentation(t.text())),
                })
                .collect(),
        ),
        // Free-text sections
        _ => {
            let body = section
                .body_text()
                .map(|t| convert_multiline_with_indentation(t.text()))
                .unwrap_or_default();
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

fn convert_arg(arg: &crate::parse::google::nodes::GoogleArg<'_>) -> Parameter {
    Parameter {
        names: arg.names().map(|n| n.text().to_owned()).collect(),
        type_annotation: arg.type_annotation().map(|t| t.text().to_owned()),
        description: arg.description().map(|t| convert_multiline_with_indentation(t.text())),
        is_optional: arg.is_optional(),
        default_value: arg.default_value().map(|t| t.text().to_owned()),
    }
}

fn convert_exception(exc: &crate::parse::google::nodes::GoogleException<'_>) -> ExceptionEntry {
    ExceptionEntry {
        type_name: exc.type_annotation().text().to_owned(),
        description: exc.description().map(|t| convert_multiline_with_indentation(t.text())),
    }
}
