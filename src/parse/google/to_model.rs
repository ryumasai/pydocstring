//! Convert a Google-style AST into the style-independent [`Docstring`] model.

use crate::model::Attribute;
use crate::model::Block;
use crate::model::Docstring;
use crate::model::ExceptionEntry;
use crate::model::Method;
use crate::model::Parameter;
use crate::model::Reference;
use crate::model::Return;
use crate::model::Section;
use crate::model::SeeAlsoEntry;
use crate::parse::EntryRole;
use crate::parse::google::nodes::GoogleAttribute;
use crate::parse::google::nodes::GoogleDocstring;
use crate::parse::google::nodes::GoogleException;
use crate::parse::google::nodes::GoogleMethod;
use crate::parse::google::nodes::GoogleReference;
use crate::parse::google::nodes::GoogleReturn;
use crate::parse::google::nodes::GoogleSection;
use crate::parse::google::nodes::GoogleSeeAlsoItem;
use crate::parse::google::nodes::GoogleWarning;
use crate::parse::google::nodes::GoogleYield;
use crate::parse::text_block::TextBlock;
use crate::parse::utils::convert_multiline_with_indentation;
use crate::syntax::Parsed;
use crate::syntax::SyntaxElement;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;

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

/// Build a [`Section`] by walking the section's children in source order,
/// mapping each `ENTRY`/`CITATION`/`DESCRIPTION` node to the matching
/// [`Block`], mirroring the NumPy converter (see its docs). Google needs no
/// paragraph rule: its Returns/Yields bodies collapse into a single node
/// (`ReturnsState`), so bare prose never parses as a run of type-only entries.
fn convert_section(section: &GoogleSection<'_>) -> Section {
    let g_kind = section.section_kind();
    let section_kind = g_kind.to_section_kind(section.header().name().text());
    let role = g_kind.entry_role();
    let parsed = section.parsed;

    let mut blocks = Vec::new();
    for child in section.syntax().children() {
        let SyntaxElement::Node(node) = child else {
            continue;
        };
        match node.kind() {
            SyntaxKind::DESCRIPTION => {
                if let Some(tb) = TextBlock::cast(parsed, node) {
                    blocks.push(Block::Paragraph(convert_multiline_with_indentation(tb.text())));
                }
            }
            SyntaxKind::CITATION => {
                let r = GoogleReference { parsed, node };
                blocks.push(Block::Reference(Reference {
                    label: r.label().map(|t| t.text().to_owned()),
                    content: r.content().map(|t| convert_multiline_with_indentation(t.text())),
                }));
            }
            SyntaxKind::ENTRY => {
                if let Some(block) = convert_entry(parsed, node, role) {
                    blocks.push(block);
                }
            }
            _ => {}
        }
    }

    Section {
        kind: section_kind,
        blocks,
    }
}

/// Convert a single `ENTRY` node into a typed [`Block`], routing by the
/// enclosing section's [`EntryRole`]. Returns `None` for roles that own no
/// `ENTRY` children.
fn convert_entry(parsed: &Parsed, node: &SyntaxNode, role: EntryRole) -> Option<Block> {
    Some(match role {
        EntryRole::Parameter => Block::Parameter(convert_arg(&crate::parse::google::nodes::GoogleArg { parsed, node })),
        EntryRole::Return => {
            let r = GoogleReturn { parsed, node };
            Block::Return(Return {
                name: None,
                type_annotation: r.type_annotation().map(|t| t.text().to_owned()),
                description: r.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::Yield => {
            let r = GoogleYield { parsed, node };
            Block::Return(Return {
                name: None,
                type_annotation: r.type_annotation().map(|t| t.text().to_owned()),
                description: r.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::Exception => Block::Exception(convert_exception(&GoogleException { parsed, node })),
        EntryRole::Warning => {
            let w = GoogleWarning { parsed, node };
            Block::Exception(ExceptionEntry {
                type_name: w.type_annotation().text().to_owned(),
                description: w.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::Attribute => {
            let a = GoogleAttribute { parsed, node };
            Block::Attribute(Attribute {
                names: a.names().map(|n| n.text().to_owned()).collect(),
                type_annotation: a.type_annotation().map(|t| t.text().to_owned()),
                description: a.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::Method => {
            let m = GoogleMethod { parsed, node };
            Block::Method(Method {
                name: m.name().text().to_owned(),
                type_annotation: m.type_annotation().map(|t| t.text().to_owned()),
                description: m.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::SeeAlsoItem => {
            let item = GoogleSeeAlsoItem { parsed, node };
            Block::SeeAlso(SeeAlsoEntry {
                names: item.names().map(|n| n.text().to_owned()).collect(),
                description: item.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::Citation | EntryRole::FreeText => return None,
    })
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
