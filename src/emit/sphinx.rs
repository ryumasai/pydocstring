//! Emit a [`Docstring`] as a Sphinx-style (reStructuredText) docstring.

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

/// Emit a [`Docstring`] as a Sphinx-style (reStructuredText field list) string.
///
/// `base_indent` is the number of spaces prepended to every non-empty line
/// of output, so the result can be embedded at the correct indentation level
/// in a Python file.
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
/// use pydocstring::emit::sphinx::emit_sphinx;
///
/// let doc = Docstring {
///     summary: Some("Brief summary.".into()),
///     sections: vec![Section::Parameters(vec![Parameter {
///         names: vec!["x".into()],
///         type_annotation: Some("int".into()),
///         description: Some("The value.".into()),
///         is_optional: false,
///         default_value: None,
///     }])],
///     ..Default::default()
/// };
/// let text = emit_sphinx(&doc, 0);
/// assert!(text.contains(":param x: The value.\n"));
/// assert!(text.contains(":type x: int\n"));
/// ```
pub fn emit_sphinx(doc: &Docstring, base_indent: usize) -> String {
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

    // Deprecation
    if let Some(ref dep) = doc.deprecation {
        out.push('\n');
        emit_deprecation(&mut out, dep);
    }

    // Sections
    for section in &doc.sections {
        out.push('\n');
        emit_section(&mut out, section);
    }

    if base_indent == 0 {
        return out;
    }
    super::indent_lines(&out, base_indent)
}

fn emit_section(out: &mut String, section: &Section) {
    match section {
        Section::Parameters(params)
        | Section::KeywordParameters(params)
        | Section::OtherParameters(params)
        | Section::Receives(params) => {
            for p in params {
                emit_parameter(out, p);
            }
        }
        Section::Returns(returns) | Section::Yields(returns) => {
            for r in returns {
                emit_return(out, r);
            }
        }
        Section::Raises(entries) => {
            for e in entries {
                emit_exception(out, e);
            }
        }
        Section::Warns(entries) => {
            for e in entries {
                emit_warning(out, e);
            }
        }
        Section::Attributes(attrs) => {
            for a in attrs {
                emit_attribute(out, a);
            }
        }
        Section::Methods(methods) => {
            emit_methods(out, methods);
        }
        Section::SeeAlso(items) => {
            emit_see_also(out, items);
        }
        Section::References(refs) => {
            for r in refs {
                emit_reference(out, r);
            }
        }
        Section::FreeText { kind, body } => {
            emit_free_text(out, kind, body);
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
fn emit_attribute(out: &mut String, a: &Attribute) {
    out.push_str(":var ");
    out.push_str(&a.name);
    out.push(':');
    if let Some(ref desc) = a.description {
        out.push(' ');
        emit_multiline(out, desc, 4);
    }
    out.push('\n');
    if let Some(ref ty) = a.type_annotation {
        out.push_str(":vartype ");
        out.push_str(&a.name);
        out.push_str(": ");
        out.push_str(ty);
        out.push('\n');
    }
}

/// Sphinx: `.. rubric:: Methods` followed by a bullet list.
fn emit_methods(out: &mut String, methods: &[Method]) {
    out.push_str(".. rubric:: Methods\n\n");
    for m in methods {
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
}

/// Sphinx: `.. seealso::` directive.
fn emit_see_also(out: &mut String, items: &[SeeAlsoEntry]) {
    out.push_str(".. seealso::\n\n");
    for item in items {
        let mut body = item.names.join(", ");
        if let Some(ref desc) = item.description {
            body.push_str(": ");
            body.push_str(desc);
        }
        emit_indented_body(out, &body, 4);
    }
}

/// Sphinx: `.. [1] content` (reStructuredText citation syntax).
fn emit_reference(out: &mut String, r: &Reference) {
    if let Some(ref num) = r.number {
        out.push_str(".. [");
        out.push_str(num);
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

/// Sphinx: `.. deprecated:: version` directive.
fn emit_deprecation(out: &mut String, dep: &Deprecation) {
    out.push_str(".. deprecated:: ");
    out.push_str(&dep.version);
    out.push('\n');
    if let Some(ref desc) = dep.description {
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
    let mut lines = text.lines();
    if let Some(first) = lines.next() {
        out.push_str(first);
        for line in lines {
            out.push('\n');
            if !line.is_empty() {
                out.push_str(&" ".repeat(indent));
                out.push_str(line);
            }
        }
    }
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
