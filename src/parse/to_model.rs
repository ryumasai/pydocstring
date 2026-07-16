//! Convert a parsed docstring of any style into the style-independent
//! [`Docstring`] model (#148; replaces the three per-style converters).
//!
//! The CST is style-neutral, so one walk converts every style. Exactly one
//! behavioral switch remains keyed on [`Parsed::style`]: Google dedents
//! prose blocks (extended summary, free-text section bodies) while NumPy and
//! Plain store them raw — a pre-existing asymmetry preserved verbatim here
//! (the snapshots pin it); whether it should survive is its own question.
//!
//! For Returns/Yields bodies this applies the **paragraph rule** (#104): the
//! CST keeps every base-indent line as an `ENTRY` (local, predictable — the
//! napoleon/numpydoc line grammar), and this model layer decides which bare
//! entries are prose. A maximal run of consecutive bare entries (no term
//! colon, no indented description, no blank line between them) becomes one
//! [`Block::Paragraph`] when the run is ≥2 lines or the body also contains a
//! genuine entry; a lone bare entry in an entry-less body stays a type-only
//! [`Block::Return`] (the #26 prefer_type decision — napoleon's `:rtype:`
//! and numpydoc agree). The rule is byte-neutral: a paragraph and a
//! type-only entry emit the same bare line, so the round trip holds either
//! way. Google's Returns bodies collapse into a single genuine entry at
//! parse time (`ReturnsState`), so the rule passes them through unchanged.

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
use crate::parse::Style;
use crate::parse::nodes::DocNode;
use crate::parse::nodes::ExceptionNode;
use crate::parse::nodes::MethodNode;
use crate::parse::nodes::ParameterNode;
use crate::parse::nodes::ReferenceNode;
use crate::parse::nodes::ReturnNode;
use crate::parse::nodes::SectionNode;
use crate::parse::text_block::TextBlock;
use crate::parse::utils::convert_multiline_with_indentation;
use crate::syntax::Parsed;
use crate::syntax::SyntaxElement;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;

/// Build a [`Docstring`] from a [`Parsed`] result of any style.
pub(crate) fn to_model(parsed: &Parsed) -> Docstring {
    let root = DocNode::root(parsed);

    Docstring {
        summary: root.summary().map(|t| t.text().to_owned()),
        extended_summary: root.extended_summary().map(|t| prose_text(parsed, &t)),
        directives: crate::parse::utils::convert_directives(parsed),
        sections: root.sections().map(|s| convert_section(&s)).collect(),
    }
}

/// A prose block's model text. Google dedents; NumPy and Plain store the
/// text raw — the pre-existing per-style asymmetry, preserved verbatim
/// (see the module docs).
fn prose_text(parsed: &Parsed, tb: &TextBlock<'_>) -> String {
    if parsed.style() == Style::Google {
        convert_multiline_with_indentation(tb.text())
    } else {
        tb.text().to_owned()
    }
}

/// Build a [`Section`] by walking the section's children in source order,
/// mapping each `ENTRY`/`CITATION`/`DESCRIPTION` node to the matching
/// [`Block`]. Typed entries are dispatched by the section's [`EntryRole`]
/// (the same routing the visitor uses), so a foreign entry never panics in
/// a typed accessor.
fn convert_section(section: &SectionNode<'_>) -> Section {
    let name = section.section_kind();
    let section_kind = name.to_section_kind(section.header_name());
    let role = name.entry_role();
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
                    blocks.push(Block::Paragraph(prose_text(parsed, &tb)));
                }
            }
            SyntaxKind::CITATION => {
                let r = ReferenceNode { parsed, node };
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

/// Per-entry outcome of the paragraph rule: convert as a typed entry, start
/// a prose paragraph covering this entry's run, or skip (interior of a run
/// already covered by a preceding `Paragraph`).
enum EntryPlan {
    Entry,
    Paragraph(String),
    Skip,
}

/// Apply the paragraph rule (see the module docs) to a section's `ENTRY`
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
/// `ENTRY` children (References hold `CITATION`s; free-text sections hold
/// none).
fn convert_entry(parsed: &Parsed, node: &SyntaxNode, role: EntryRole) -> Option<Block> {
    Some(match role {
        EntryRole::Parameter | EntryRole::Attribute => {
            let p = ParameterNode { parsed, node };
            let names = p.names().map(|n| n.text().to_owned()).collect();
            let type_annotation = p.type_annotation().map(|t| t.text().to_owned());
            let description = p.description().map(|t| convert_multiline_with_indentation(t.text()));
            if role == EntryRole::Attribute {
                Block::Attribute(Attribute {
                    names,
                    type_annotation,
                    description,
                })
            } else {
                Block::Parameter(Parameter {
                    names,
                    type_annotation,
                    description,
                    is_optional: p.is_optional(),
                    default_value: p.default_value().map(|t| t.text().to_owned()),
                })
            }
        }
        EntryRole::Return | EntryRole::Yield => {
            let r = ReturnNode { parsed, node };
            Block::Return(Return {
                name: r.name().map(|t| t.text().to_owned()),
                type_annotation: r.type_annotation().map(|t| t.text().to_owned()),
                description: r.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::Exception | EntryRole::Warning => {
            let e = ExceptionNode { parsed, node };
            Block::Exception(ExceptionEntry {
                type_name: e.type_annotation().text().to_owned(),
                description: e.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::Method => {
            let m = MethodNode { parsed, node };
            Block::Method(Method {
                name: m.name().text().to_owned(),
                type_annotation: m.type_annotation().map(|t| t.text().to_owned()),
                description: m.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::SeeAlsoItem => {
            let item = crate::parse::nodes::SeeAlsoNode { parsed, node };
            Block::SeeAlso(SeeAlsoEntry {
                names: item.names().map(|n| n.text().to_owned()).collect(),
                description: item.description().map(|t| convert_multiline_with_indentation(t.text())),
            })
        }
        EntryRole::Citation | EntryRole::FreeText => return None,
    })
}
