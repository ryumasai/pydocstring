//! Convert a Google-style AST into the style-independent [`Docstring`] model.

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
    let source = parsed.source();
    let root = GoogleDocstring::cast(parsed.root())?;

    let summary = root.summary().map(|t| t.text(source).to_owned());
    let extended_summary = root
        .extended_summary()
        .map(|t| convert_multiline_with_indentation(t.text(source)));

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

fn convert_section(section: &GoogleSection<'_>, source: &str) -> Section {
    let kind = section.section_kind(source);

    match kind {
        GoogleSectionKind::Args | GoogleSectionKind::Receives => {
            let entries = section.args(source).map(|a| convert_arg(&a, source)).collect();
            match kind {
                GoogleSectionKind::Args => Section::Parameters(entries),
                GoogleSectionKind::Receives => Section::Receives(entries),
                _ => unreachable!(),
            }
        }
        GoogleSectionKind::KeywordArgs => {
            Section::KeywordParameters(section.args(source).map(|a| convert_arg(&a, source)).collect())
        }
        GoogleSectionKind::OtherParameters => {
            Section::OtherParameters(section.args(source).map(|a| convert_arg(&a, source)).collect())
        }
        GoogleSectionKind::Returns => {
            let entries: Vec<Return> = section
                .returns(source)
                .into_iter()
                .map(|r| Return {
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
            let entries: Vec<Return> = section
                .yields(source)
                .into_iter()
                .map(|r| Return {
                    name: None,
                    type_annotation: r.return_type().map(|t| t.text(source).to_owned()),
                    description: r
                        .description()
                        .map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect();
            Section::Yields(entries)
        }
        GoogleSectionKind::Raises => Section::Raises(
            section
                .exceptions(source)
                .map(|e| convert_exception(&e, source))
                .collect(),
        ),
        GoogleSectionKind::Warns => Section::Warns(
            section
                .warnings(source)
                .map(|w| ExceptionEntry {
                    type_name: w.warning_type().text(source).to_owned(),
                    description: w
                        .description()
                        .map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect(),
        ),
        GoogleSectionKind::SeeAlso => Section::SeeAlso(
            section
                .see_also_items(source)
                .map(|item| SeeAlsoEntry {
                    names: item.names().map(|n| n.text(source).to_owned()).collect(),
                    description: item
                        .description()
                        .map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect(),
        ),
        GoogleSectionKind::Attributes => Section::Attributes(
            section
                .attributes(source)
                .map(|a| Attribute {
                    name: a.name().text(source).to_owned(),
                    type_annotation: a.r#type().map(|t| t.text(source).to_owned()),
                    description: a
                        .description()
                        .map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect(),
        ),
        GoogleSectionKind::References => Section::References(
            section
                .references()
                .map(|r| Reference {
                    number: r.number().map(|t| t.text(source).to_owned()),
                    content: r.content().map(|t| convert_multiline_with_indentation(t.text(source))),
                })
                .collect(),
        ),
        GoogleSectionKind::Methods => Section::Methods(
            section
                .methods(source)
                .map(|m| Method {
                    name: m.name().text(source).to_owned(),
                    type_annotation: m.r#type().map(|t| t.text(source).to_owned()),
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
                .map(|t| convert_multiline_with_indentation(t.text(source)))
                .unwrap_or_default();
            // A structured kind reaching this arm would mean to_section_kind and
            // the structured arms above drifted apart; degrade gracefully.
            let free_kind = match kind.to_section_kind(section.header().name().text(source)) {
                SectionKind::FreeText(k) => k,
                _ => FreeSectionKind::Unknown(section.header().name().text(source).to_owned()),
            };
            Section::FreeText { kind: free_kind, body }
        }
    }
}

fn convert_arg(arg: &crate::parse::google::nodes::GoogleArg<'_>, source: &str) -> Parameter {
    Parameter {
        names: arg.names().map(|n| n.text(source).to_owned()).collect(),
        type_annotation: arg.r#type().map(|t| t.text(source).to_owned()),
        description: arg
            .description()
            .map(|t| convert_multiline_with_indentation(t.text(source))),
        is_optional: arg.optional().is_some(),
        default_value: arg.default_value().map(|t| t.text(source).to_owned()),
    }
}

fn convert_exception(exc: &crate::parse::google::nodes::GoogleException<'_>, source: &str) -> ExceptionEntry {
    ExceptionEntry {
        type_name: exc.r#type().text(source).to_owned(),
        description: exc
            .description()
            .map(|t| convert_multiline_with_indentation(t.text(source))),
    }
}
