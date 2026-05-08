//! Convert a Google-style AST into the style-independent [`Docstring`] model.

use crate::model::{
    Attribute, Docstring, ExceptionEntry, FreeSectionKind, Method, Parameter, Return, Section, SeeAlsoEntry,
};
use crate::parse::google::kind::GoogleSectionKind;
use crate::parse::google::nodes::{GoogleDocstring, GoogleSection};
use crate::syntax::Parsed;

/// Build a [`Docstring`] from a Google-style [`Parsed`] result.
///
/// Returns `None` if the root node is not a `GOOGLE_DOCSTRING`.
pub fn to_model(parsed: &Parsed) -> Option<Docstring> {
    let source = parsed.source();
    let root = GoogleDocstring::cast(parsed.root())?;

    let summary = root.summary().map(|t| t.text(source).to_owned());
    let extended_summary = root.extended_summary().map(|t| t.text(source).to_owned());

    let sections = root.sections().map(|s| convert_section(&s, source)).collect();

    Some(Docstring {
        summary,
        extended_summary,
        deprecation: None,
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
                    description: r.description().map(|t| t.text(source).to_owned()),
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
                    description: r.description().map(|t| t.text(source).to_owned()),
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
                    description: w.description().map(|t| t.text(source).to_owned()),
                })
                .collect(),
        ),
        GoogleSectionKind::SeeAlso => Section::SeeAlso(
            section
                .see_also_items()
                .map(|item| SeeAlsoEntry {
                    names: item.names().map(|n| n.text(source).to_owned()).collect(),
                    description: item.description().map(|t| t.text(source).to_owned()),
                })
                .collect(),
        ),
        GoogleSectionKind::Attributes => Section::Attributes(
            section
                .attributes()
                .map(|a| Attribute {
                    name: a.name().text(source).to_owned(),
                    type_annotation: a.r#type().map(|t| t.text(source).to_owned()),
                    description: a.description().map(|t| t.text(source).to_owned()),
                })
                .collect(),
        ),
        GoogleSectionKind::Methods => Section::Methods(
            section
                .methods()
                .map(|m| Method {
                    name: m.name().text(source).to_owned(),
                    type_annotation: m.r#type().map(|t| t.text(source).to_owned()),
                    description: m.description().map(|t| t.text(source).to_owned()),
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
        description: arg.description().map(|t| {
            let text = t.text(source);
            let description_indent = text.lines().skip(1).filter_map(|line| {
                let trimmed_len = line.trim_start().len();
                if trimmed_len == 0 {
                    None
                } else {
                    Some(line.len() - trimmed_len)
                }
            }).min().unwrap_or(0);
            let mut lines = text.lines();
            let first_line = lines.next().unwrap().trim_end(); // at least one description line => we can safely unwrap
            lines.map(|line| {
                if description_indent >= line.len() { // empty line
                    &line[0..0]
                } else {
                    line[description_indent..].trim_end()
                }
            }).fold(first_line.to_owned(), |a, b| a + "\n" + b)
        }),
        is_optional: arg.optional().is_some(),
        default_value: None,
    }
}

fn convert_exception(exc: &crate::parse::google::nodes::GoogleException<'_>, source: &str) -> ExceptionEntry {
    ExceptionEntry {
        type_name: exc.r#type().text(source).to_owned(),
        description: exc.description().map(|t| t.text(source).to_owned()),
    }
}
