//! Convert a Google-style AST into the style-independent [`Docstring`] model.

use crate::model::{
    Attribute, Deprecation, Docstring, ExceptionEntry, FreeSectionKind, Method, Parameter, Return, Section,
    SeeAlsoEntry,
};
use crate::parse::google::kind::GoogleSectionKind;
use crate::parse::google::nodes::{GoogleDocstring, GoogleSection};
use crate::parse::utils::convert_multiline_with_indentation;
use crate::syntax::Parsed;

/// Build a [`Docstring`] from a Google-style [`Parsed`] result.
///
/// Returns `None` if the root node is not a `GOOGLE_DOCSTRING`.
pub fn to_model(parsed: &Parsed) -> Option<Docstring> {
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
            let entries = section.args().map(|a| convert_arg(&a, source)).collect();
            match kind {
                GoogleSectionKind::Args => Section::Parameters(entries),
                GoogleSectionKind::Receives => Section::Receives(entries),
                _ => unreachable!(),
            }
        }
        GoogleSectionKind::KeywordArgs => {
            Section::KeywordParameters(section.args().map(|a| convert_arg(&a, source)).collect())
        }
        GoogleSectionKind::OtherParameters => {
            Section::OtherParameters(section.args().map(|a| convert_arg(&a, source)).collect())
        }
        GoogleSectionKind::Returns => {
            let entries: Vec<Return> = section
                .returns()
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
                .yields()
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
        GoogleSectionKind::Raises => {
            Section::Raises(section.exceptions().map(|e| convert_exception(&e, source)).collect())
        }
        GoogleSectionKind::Warns => Section::Warns(
            section
                .warnings()
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
                .see_also_items()
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
        GoogleSectionKind::Methods => Section::Methods(
            section
                .methods()
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
