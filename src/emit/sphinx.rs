//! Emit a [`Docstring`] as a Sphinx-style (reStructuredText) docstring.

use super::EmitOptions;
use crate::model::Attribute;
use crate::model::Block;
use crate::model::Directive;
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

/// Emit a [`Docstring`] as a Sphinx-style (reStructuredText field list) string.
///
/// See [`EmitOptions`] for the knobs; `options.base_indent` indents every
/// non-empty output line so the result can be embedded at the correct
/// indentation level in a Python file.
///
/// Sphinx has no field role for every section that NumPy / Google support, so
/// some sections are rendered on a best-effort basis:
///
/// - `Yields` is emitted as `:return:` / `:rtype:` (matching Napoleon).
/// - `Attributes` use `:var:` / `:vartype:`.
/// - `Warns`, free-text sections (`Notes`, `Warnings`, …) and `See Also` are
///   emitted as reStructuredText directives (`.. warning::`, `.. note::`,
///   `.. seealso::`, …).
/// - `Methods` are emitted under a `.. rubric:: Methods`.
/// - `References` use the reStructuredText citation syntax (`.. [1] …`).
///
/// Multi-name parameters (a NumPy feature, e.g. `x, y`) are duplicated into one
/// `:param:` / `:type:` pair per name. Named returns (also NumPy-only) lose
/// their name, as Sphinx has no role for it.
///
/// # Example
///
/// ```rust
/// use pydocstring::model::{Docstring, Section, Parameter};
/// use pydocstring::emit::EmitOptions;
/// use pydocstring::emit::sphinx::emit_sphinx;
///
/// let doc = Docstring {
///     summary: Some("Brief summary.".into()),
///     sections: vec![Section::parameters(vec![Parameter {
///         names: vec!["x".into()],
///         type_annotation: Some("int".into()),
///         description: Some("The value.".into()),
///         is_optional: false,
///         default_value: None,
///     }])],
///     ..Default::default()
/// };
/// let text = emit_sphinx(&doc, &EmitOptions::default());
/// assert!(text.contains(":param x: The value.\n"));
/// assert!(text.contains(":type x: int\n"));
/// ```
pub fn emit_sphinx(doc: &Docstring, options: &EmitOptions) -> String {
    let mut out = String::new();

    // Summary
    if let Some(ref summary) = doc.summary {
        out.push_str(summary);
        out.push('\n');
    }

    // Extended summary
    if let Some(ref ext) = doc.extended_summary {
        out.push('\n');
        out.push_str(ext);
        out.push('\n');
    }

    // Directives (e.g. deprecation)
    for directive in &doc.directives {
        out.push('\n');
        emit_directive(&mut out, directive);
    }

    // Sections
    for section in &doc.sections {
        out.push('\n');
        emit_section(&mut out, section);
    }

    if options.base_indent == 0 {
        return out;
    }
    super::indent_lines(&out, options.base_indent)
}

fn emit_section(out: &mut String, section: &Section) {
    // Free-text sections render as one admonition/rubric wrapping the prose
    // body; paragraph boundaries are preserved as blank lines. Non-paragraph
    // blocks under a free-text kind are representable nonsense, but the model
    // contract is total emission — dispatch them after the admonition.
    if let SectionKind::FreeText(kind) = &section.kind {
        let body = section
            .blocks
            .iter()
            .filter_map(Block::as_paragraph)
            .collect::<Vec<_>>()
            .join("\n\n");
        emit_free_text(out, kind, &body);
        let entries: Vec<&Block> = section.blocks.iter().filter(|b| b.as_paragraph().is_none()).collect();
        if !entries.is_empty() {
            emit_blocks(out, &entries, &section.kind);
        }
        return;
    }

    // Structured sections: emit every block at its source-order position (the
    // model is permissive — any block can appear under any kind).
    let blocks: Vec<&Block> = section.blocks.iter().collect();
    emit_blocks(out, &blocks, &section.kind);
}

/// Emit blocks in source order, each with its kind-appropriate field/directive
/// form. Adjacent paragraphs are separated by a blank line. Methods and See
/// Also entries aggregate under a single rubric/directive, opened at the first
/// such block.
fn emit_blocks(out: &mut String, blocks: &[&Block], kind: &SectionKind) {
    let mut prev_paragraph = false;
    let mut methods_rubric_open = false;
    let mut seealso_open = false;
    for block in blocks {
        if let Block::Paragraph(text) = block {
            // Prose stays prose: emitted as a plain paragraph at its position
            // (best-effort — Sphinx field lists have no prose slot).
            if prev_paragraph {
                out.push('\n');
            }
            out.push_str(text);
            out.push('\n');
            prev_paragraph = true;
            continue;
        }
        prev_paragraph = false;
        match block {
            Block::Parameter(p) => emit_parameter(out, p),
            Block::Return(r) => emit_return(out, r),
            // Warns renders as `.. warning::` (no field role exists for
            // warning types); every other kind's exception as `:raises:`.
            Block::Exception(e) => {
                if *kind == SectionKind::Warns {
                    emit_warning(out, e);
                } else {
                    emit_exception(out, e);
                }
            }
            Block::Attribute(a) => emit_attribute(out, a),
            Block::Method(m) => {
                if !methods_rubric_open {
                    out.push_str(".. rubric:: Methods\n\n");
                    methods_rubric_open = true;
                }
                emit_method_item(out, m);
            }
            Block::SeeAlso(item) => {
                if !seealso_open {
                    out.push_str(".. seealso::\n\n");
                    seealso_open = true;
                }
                emit_see_also_item(out, item);
            }
            Block::Reference(r) => emit_reference(out, r),
            Block::Paragraph(_) => unreachable!("handled above"),
        }
    }
}

/// Sphinx: `:param name: description, defaults to value` + `:type name: type, optional`.
///
/// Multi-name parameters are duplicated: one `:param:` / `:type:` pair per name.
fn emit_parameter(out: &mut String, p: &Parameter) {
    let mut desc = p.description.clone().unwrap_or_default();
    if let Some(ref dv) = p.default_value {
        if desc.is_empty() {
            desc.push_str("defaults to ");
        } else {
            desc.push_str(", defaults to ");
        }
        desc.push_str(dv);
    }

    for name in &p.names {
        out.push_str(":param ");
        out.push_str(name);
        out.push(':');
        if !desc.is_empty() {
            out.push(' ');
            emit_multiline(out, &desc, 4);
        }
        out.push('\n');

        if p.type_annotation.is_some() || p.is_optional {
            out.push_str(":type ");
            out.push_str(name);
            out.push_str(": ");
            if let Some(ref ty) = p.type_annotation {
                out.push_str(ty);
            }
            if p.is_optional {
                if p.type_annotation.is_some() {
                    out.push_str(", ");
                }
                out.push_str("optional");
            }
            out.push('\n');
        }
    }
}

/// Sphinx: `:return: description` + `:rtype: type`. Named returns lose their name.
fn emit_return(out: &mut String, r: &Return) {
    out.push_str(":return:");
    if let Some(ref desc) = r.description {
        out.push(' ');
        emit_multiline(out, desc, 4);
    }
    out.push('\n');
    if let Some(ref ty) = r.type_annotation {
        out.push_str(":rtype: ");
        out.push_str(ty);
        out.push('\n');
    }
}

/// Sphinx: `:raises ValueError: description`.
fn emit_exception(out: &mut String, e: &ExceptionEntry) {
    out.push_str(":raises ");
    out.push_str(&e.type_name);
    out.push(':');
    if let Some(ref desc) = e.description {
        out.push(' ');
        emit_multiline(out, desc, 4);
    }
    out.push('\n');
}

/// Sphinx: `.. warning::` directive (no field role exists for warning types).
fn emit_warning(out: &mut String, e: &ExceptionEntry) {
    out.push_str(".. warning::\n\n");
    let mut body = e.type_name.clone();
    if let Some(ref desc) = e.description {
        body.push_str(": ");
        body.push_str(desc);
    }
    emit_indented_body(out, &body, 4);
}

/// Sphinx: `:var name: description` + `:vartype name: type`.
///
/// Multi-name attributes are duplicated: one `:var:` / `:vartype:` pair per
/// name, like [`emit_parameter`].
fn emit_attribute(out: &mut String, a: &Attribute) {
    for name in &a.names {
        out.push_str(":var ");
        out.push_str(name);
        out.push(':');
        if let Some(ref desc) = a.description {
            out.push(' ');
            emit_multiline(out, desc, 4);
        }
        out.push('\n');
        if let Some(ref ty) = a.type_annotation {
            out.push_str(":vartype ");
            out.push_str(name);
            out.push_str(": ");
            out.push_str(ty);
            out.push('\n');
        }
    }
}

/// Sphinx: one `* name(sig): desc` bullet under the `.. rubric:: Methods`
/// heading (the rubric itself is opened once by [`emit_blocks`]).
fn emit_method_item(out: &mut String, m: &Method) {
    out.push_str("* ");
    out.push_str(&m.name);
    if let Some(ref ty) = m.type_annotation {
        out.push('(');
        out.push_str(ty);
        out.push(')');
    }
    if let Some(ref desc) = m.description {
        out.push_str(": ");
        out.push_str(desc);
    }
    out.push('\n');
}

/// Sphinx: one indented `names: desc` line inside the `.. seealso::`
/// directive (the directive itself is opened once by [`emit_blocks`]).
fn emit_see_also_item(out: &mut String, item: &SeeAlsoEntry) {
    let mut body = item.names.join(", ");
    if let Some(ref desc) = item.description {
        body.push_str(": ");
        body.push_str(desc);
    }
    emit_indented_body(out, &body, 4);
}

/// Sphinx: `.. [1] content` (reStructuredText citation syntax).
fn emit_reference(out: &mut String, r: &Reference) {
    if let Some(ref label) = r.label {
        out.push_str(".. [");
        out.push_str(label);
        out.push(']');
        if let Some(ref content) = r.content {
            out.push(' ');
            out.push_str(content);
        }
    } else if let Some(ref content) = r.content {
        out.push_str(content);
    }
    out.push('\n');
}

/// Sphinx: `.. name:: argument` directive (e.g. `.. deprecated:: 1.6.0`).
fn emit_directive(out: &mut String, directive: &Directive) {
    out.push_str(".. ");
    out.push_str(&directive.name);
    out.push_str("::");
    if let Some(ref argument) = directive.argument {
        out.push(' ');
        out.push_str(argument);
    }
    out.push('\n');
    if let Some(ref desc) = directive.description {
        emit_indented_body(out, desc, 4);
    }
}

/// Free-text sections map to reStructuredText admonitions / rubrics.
fn emit_free_text(out: &mut String, kind: &FreeSectionKind, body: &str) {
    match kind {
        // Examples read as a rubric heading with the body as ordinary paragraphs.
        FreeSectionKind::Examples => {
            out.push_str(".. rubric:: Examples\n\n");
            emit_indented_body(out, body, 0);
        }
        // Unrecognised sections become a generic, titled admonition.
        FreeSectionKind::Unknown(name) => {
            out.push_str(".. admonition:: ");
            out.push_str(name);
            out.push_str("\n\n");
            emit_indented_body(out, body, 4);
        }
        _ => {
            out.push_str(".. ");
            out.push_str(admonition_name(kind));
            out.push_str("::\n\n");
            emit_indented_body(out, body, 4);
        }
    }
}

/// reStructuredText admonition directive name for a free-text section.
fn admonition_name(kind: &FreeSectionKind) -> &str {
    match kind {
        FreeSectionKind::Notes => "note",
        FreeSectionKind::Warnings => "warning",
        FreeSectionKind::Todo => "todo",
        FreeSectionKind::Attention => "attention",
        FreeSectionKind::Caution => "caution",
        FreeSectionKind::Danger => "danger",
        FreeSectionKind::Error => "error",
        FreeSectionKind::Hint => "hint",
        FreeSectionKind::Important => "important",
        FreeSectionKind::Tip => "tip",
        // Handled separately in `emit_free_text`.
        FreeSectionKind::Examples => "rubric",
        FreeSectionKind::Unknown(_) => "admonition",
    }
}

/// Emit `text`, indenting every continuation line (after the first) by `indent`.
fn emit_multiline(out: &mut String, text: &str, indent: usize) {
    super::emit_multiline_with_indentation(out, text, indent);
}

/// Emit `body` with every non-empty line indented by `indent` spaces.
fn emit_indented_body(out: &mut String, body: &str, indent: usize) {
    let prefix = " ".repeat(indent);
    for line in body.lines() {
        if !line.is_empty() {
            out.push_str(&prefix);
            out.push_str(line);
        }
        out.push('\n');
    }
}
