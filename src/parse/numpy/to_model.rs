//! Convert a NumPy-style AST into the style-independent [`Docstring`] model.

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
use crate::parse::numpy::nodes::NumPyDocstring;
use crate::parse::numpy::nodes::NumPyException;
use crate::parse::numpy::nodes::NumPyReference;
use crate::parse::numpy::nodes::NumPySection;
use crate::parse::numpy::nodes::NumPySeeAlsoItem;
use crate::parse::numpy::nodes::NumPyWarning;
use crate::parse::numpy::nodes::NumPyYields;
use crate::parse::text_block::TextBlock;
use crate::parse::utils::convert_multiline_with_indentation;
use crate::syntax::Parsed;
use crate::syntax::SyntaxElement;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;

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

/// Build a [`Section`] by walking the section's children in source order,
/// mapping each `ENTRY`/`CITATION`/`DESCRIPTION` node to the matching
/// [`Block`]. Typed entries are dispatched by the section's [`EntryRole`]
/// (the same routing the visitor uses), so a foreign entry never panics in a
/// typed accessor.
///
/// For Returns/Yields bodies this applies the **paragraph rule** (#104): the
/// CST keeps every base-indent line as an `ENTRY` (local, predictable — the
/// napoleon/numpydoc line grammar), and this model layer decides which bare
/// entries are prose. A maximal run of consecutive bare entries (no term
/// colon, no indented description, no blank line between them) becomes one
/// [`Block::Paragraph`] when the run is ≥2 lines or the body also contains a
/// genuine entry; a lone bare entry in an entry-less body stays a type-only
/// [`Block::Return`] (the #26 prefer_type decision — napoleon's `:rtype:` and
/// numpydoc agree). The rule is byte-neutral: a paragraph and a type-only
/// entry emit the same bare line, so the round trip holds either way.
fn convert_section(section: &NumPySection<'_>) -> Section {
    let np_kind = section.section_kind();
    let section_kind = np_kind.to_section_kind(section.header().name().text());
    let role = np_kind.entry_role();
    let parsed = section.parsed;

    // Plan the prose grouping over the ENTRY children (Returns/Yields only).
    let plans = if matches!(role, EntryRole::Return | EntryRole::Yield) {
        let entry_nodes: Vec<&SyntaxNode> = section
            .syntax()
            .children()
            .iter()
            .filter_map(|c| match c {
                SyntaxElement::Node(n) if n.kind() == SyntaxKind::ENTRY => Some(n),
                _ => None,
            })
            .collect();
        plan_paragraph_runs(parsed.source(), &entry_nodes)
    } else {
        Vec::new()
    };

    let mut blocks = Vec::new();
    let mut entry_index = 0;
    for child in section.syntax().children() {
        let SyntaxElement::Node(node) = child else {
            continue;
        };
        match node.kind() {
            // Free-text section bodies become paragraphs.
            SyntaxKind::DESCRIPTION => {
                if let Some(tb) = TextBlock::cast(parsed, node) {
                    blocks.push(Block::Paragraph(tb.text().to_owned()));
                }
            }
            SyntaxKind::CITATION => {
                let r = NumPyReference { parsed, node };
                blocks.push(Block::Reference(Reference {
                    label: r.label().map(|t| t.text().to_owned()),
                    content: r.content().map(|t| convert_multiline_with_indentation(t.text())),
                }));
            }
            SyntaxKind::ENTRY => {
                match plans.get(entry_index) {
                    Some(EntryPlan::Paragraph(text)) => blocks.push(Block::Paragraph(text.clone())),
                    Some(EntryPlan::Skip) => {}
                    Some(EntryPlan::Entry) | None => {
                        if let Some(block) = convert_entry(parsed, node, role) {
                            blocks.push(block);
                        }
                    }
                }
                entry_index += 1;
            }
            _ => {}
        }
    }

    Section {
        kind: section_kind,
        blocks,
    }
}

/// Per-entry outcome of the paragraph rule: convert as a typed entry, start a
/// prose paragraph covering this entry's run, or skip (interior of a run
/// already covered by a preceding `Paragraph`).
enum EntryPlan {
    Entry,
    Paragraph(String),
    Skip,
}

/// Apply the paragraph rule (see [`convert_section`]) to a section's `ENTRY`
/// children, in order: group maximal runs of consecutive bare entries and
/// decide entry-vs-prose per run.
fn plan_paragraph_runs(source: &str, entries: &[&SyntaxNode]) -> Vec<EntryPlan> {
    let n = entries.len();
    let genuine: Vec<bool> = entries
        .iter()
        .map(|e| crate::parse::utils::is_genuine_entry(e))
        .collect();
    let has_genuine = genuine.iter().any(|&g| g);

    let mut plans = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        if genuine[i] {
            plans.push(EntryPlan::Entry);
            i += 1;
            continue;
        }
        // Maximal run of consecutive bare entries with no blank line between.
        let mut j = i;
        while j + 1 < n
            && !genuine[j + 1]
            && !crate::parse::utils::blank_line_between(
                source,
                entries[j].range().end(),
                entries[j + 1].range().start(),
            )
        {
            j += 1;
        }
        if (j - i + 1) >= 2 || has_genuine {
            let span = crate::text::TextRange::new(entries[i].range().start(), entries[j].range().end());
            plans.push(EntryPlan::Paragraph(span.source_text(source).to_owned()));
            plans.extend((i..j).map(|_| EntryPlan::Skip));
        } else {
            plans.push(EntryPlan::Entry);
        }
        i = j + 1;
    }
    plans
}

/// Convert a single `ENTRY` node into a typed [`Block`], routing by the
/// enclosing section's [`EntryRole`]. Returns `None` for roles that own no
/// `ENTRY` children (References hold `CITATION`s; free-text sections hold none).
fn convert_entry(parsed: &Parsed, node: &SyntaxNode, role: EntryRole) -> Option<Block> {
    Some(match role {
        EntryRole::Parameter => Block::Parameter(convert_parameter(&crate::parse::numpy::nodes::NumPyParameter {
            parsed,
            node,
        })),
        EntryRole::Return => {
            let r = crate::parse::numpy::nodes::NumPyReturns { parsed, node };
            Block::Return(Return {
                name: r.name().map(|t| t.text().to_owned()),
                type_annotation: r.type_annotation().map(|t| t.text().to_owned()),
                description: r.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::Yield => {
            let r = NumPyYields { parsed, node };
            Block::Return(Return {
                name: r.name().map(|t| t.text().to_owned()),
                type_annotation: r.type_annotation().map(|t| t.text().to_owned()),
                description: r.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::Exception => {
            let e = NumPyException { parsed, node };
            Block::Exception(ExceptionEntry {
                type_name: e.type_annotation().text().to_owned(),
                description: e.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::Warning => {
            let w = NumPyWarning { parsed, node };
            Block::Exception(ExceptionEntry {
                type_name: w.type_annotation().text().to_owned(),
                description: w.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::Attribute => {
            let a = crate::parse::numpy::nodes::NumPyAttribute { parsed, node };
            Block::Attribute(Attribute {
                names: a.names().map(|n| n.text().to_owned()).collect(),
                type_annotation: a.type_annotation().map(|t| t.text().to_owned()),
                description: a.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::Method => {
            let m = crate::parse::numpy::nodes::NumPyMethod { parsed, node };
            Block::Method(Method {
                name: m.name().text().to_owned(),
                type_annotation: None,
                description: m.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::SeeAlsoItem => {
            let item = NumPySeeAlsoItem { parsed, node };
            Block::SeeAlso(SeeAlsoEntry {
                names: item.names().map(|n| n.text().to_owned()).collect(),
                description: item.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::Citation | EntryRole::FreeText => return None,
    })
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
