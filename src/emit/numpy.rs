//! Emit a [`Docstring`] as a NumPy-style docstring.

use super::EmitOptions;
use crate::model::Attribute;
use crate::model::Docstring;
use crate::model::ExceptionEntry;
use crate::model::FreeSectionKind;
use crate::model::Method;
use crate::model::Parameter;
use crate::model::Reference;
use crate::model::Return;
use crate::model::Section;
use crate::model::SeeAlsoEntry;

/// Emit a [`Docstring`] as a NumPy-style docstring string.
///
/// See [`EmitOptions`] for the knobs; `options.base_indent` indents every
/// non-empty output line so the result can be embedded at the correct
/// indentation level in a Python file.
///
/// # Example
///
/// ```rust
/// use pydocstring::model::{Docstring, Section, Parameter};
/// use pydocstring::emit::EmitOptions;
/// use pydocstring::emit::numpy::emit_numpy;
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
/// let text = emit_numpy(&doc, &EmitOptions::default());
/// assert!(text.contains("Parameters\n----------"));
/// ```
pub fn emit_numpy(doc: &Docstring, options: &EmitOptions) -> String {
    let mut out = String::new();

    // Summary
    if let Some(ref summary) = doc.summary {
        out.push_str(summary);
        out.push('\n');
    }

    // Directives (e.g. deprecation) — before the extended summary: the
    // parsers (and numpydoc convention) only recognize a directive directly
    // after the summary.
    for directive in &doc.directives {
        out.push('\n');
        super::emit_directive(&mut out, directive);
    }

    // Extended summary
    if let Some(ref ext) = doc.extended_summary {
        out.push('\n');
        out.push_str(ext);
        out.push('\n');
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

/// Section header name for NumPy style.
fn section_header(section: &Section) -> &str {
    match section {
        Section::Parameters(_) => "Parameters",
        Section::KeywordParameters(_) => "Keyword Parameters",
        Section::OtherParameters(_) => "Other Parameters",
        Section::Receives(_) => "Receives",
        Section::Returns(_) => "Returns",
        Section::Yields(_) => "Yields",
        Section::Raises(_) => "Raises",
        Section::Warns(_) => "Warns",
        Section::Attributes(_) => "Attributes",
        Section::Methods(_) => "Methods",
        Section::SeeAlso(_) => "See Also",
        Section::References(_) => "References",
        Section::FreeText { kind, .. } => free_section_name(kind),
    }
}

fn free_section_name(kind: &FreeSectionKind) -> &str {
    match kind {
        FreeSectionKind::Notes => "Notes",
        FreeSectionKind::Examples => "Examples",
        FreeSectionKind::Warnings => "Warnings",
        FreeSectionKind::Todo => "Todo",
        FreeSectionKind::Attention => "Attention",
        FreeSectionKind::Caution => "Caution",
        FreeSectionKind::Danger => "Danger",
        FreeSectionKind::Error => "Error",
        FreeSectionKind::Hint => "Hint",
        FreeSectionKind::Important => "Important",
        FreeSectionKind::Tip => "Tip",
        FreeSectionKind::Unknown(name) => name.as_str(),
    }
}

/// Emit NumPy section header with underline.
fn emit_section_header(out: &mut String, name: &str) {
    out.push_str(name);
    out.push('\n');
    for _ in 0..name.chars().count() {
        out.push('-');
    }
    out.push('\n');
}

fn emit_section(out: &mut String, section: &Section) {
    emit_section_header(out, section_header(section));

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
        Section::Raises(entries) | Section::Warns(entries) => {
            for e in entries {
                emit_exception(out, e);
            }
        }
        Section::Attributes(attrs) => {
            for a in attrs {
                emit_attribute(out, a);
            }
        }
        Section::Methods(methods) => {
            for m in methods {
                emit_method(out, m);
            }
        }
        Section::SeeAlso(items) => {
            for item in items {
                emit_see_also(out, item);
            }
        }
        Section::References(refs) => {
            for r in refs {
                emit_reference(out, r);
            }
        }
        Section::FreeText { body, .. } => {
            out.push_str(body);
            out.push('\n');
        }
    }
}

/// NumPy: `x, y : int, optional\n    default: 0\n    Description.`
fn emit_parameter(out: &mut String, p: &Parameter) {
    out.push_str(&p.names.join(", "));
    if p.type_annotation.is_some() || p.is_optional || p.default_value.is_some() {
        out.push_str(" : ");
        if let Some(ref ty) = p.type_annotation {
            out.push_str(ty);
        }
        if p.is_optional {
            if p.type_annotation.is_some() {
                out.push_str(", ");
            }
            out.push_str("optional");
        }
        if let Some(ref dv) = p.default_value {
            if p.type_annotation.is_some() || p.is_optional {
                out.push_str(", ");
            }
            out.push_str("default: ");
            out.push_str(dv);
        }
    }
    out.push('\n');
    if let Some(ref desc) = p.description {
        emit_indented_body(out, desc);
    }
}

/// NumPy: `name : type\n    Description.` or `type\n    Description.`
fn emit_return(out: &mut String, r: &Return) {
    if let Some(ref name) = r.name {
        out.push_str(name);
        if let Some(ref ty) = r.type_annotation {
            out.push_str(" : ");
            out.push_str(ty);
        }
    } else if let Some(ref ty) = r.type_annotation {
        out.push_str(ty);
    }
    out.push('\n');
    if let Some(ref desc) = r.description {
        emit_indented_body(out, desc);
    }
}

/// NumPy: `ValueError\n    Description.`
fn emit_exception(out: &mut String, e: &ExceptionEntry) {
    out.push_str(&e.type_name);
    out.push('\n');
    if let Some(ref desc) = e.description {
        emit_indented_body(out, desc);
    }
}

/// NumPy: `name : type\n    Description.`
fn emit_attribute(out: &mut String, a: &Attribute) {
    out.push_str(&a.name);
    if let Some(ref ty) = a.type_annotation {
        out.push_str(" : ");
        out.push_str(ty);
    }
    out.push('\n');
    if let Some(ref desc) = a.description {
        emit_indented_body(out, desc);
    }
}

/// NumPy: `name\n    Description.`
fn emit_method(out: &mut String, m: &Method) {
    out.push_str(&m.name);
    if let Some(ref ty) = m.type_annotation {
        out.push_str(" (");
        out.push_str(ty);
        out.push(')');
    }
    out.push('\n');
    if let Some(ref desc) = m.description {
        emit_indented_body(out, desc);
    }
}

/// NumPy: `func1, func2 : Description.`
fn emit_see_also(out: &mut String, item: &SeeAlsoEntry) {
    out.push_str(&item.names.join(", "));
    if let Some(ref desc) = item.description {
        out.push_str(" : ");
        out.push_str(desc);
    }
    out.push('\n');
}

/// NumPy: `.. [1] Content.`
fn emit_reference(out: &mut String, r: &Reference) {
    if let Some(ref label) = r.label {
        out.push_str(".. [");
        out.push_str(label);
        out.push(']');
        if let Some(ref content) = r.content {
            out.push(' ');
            emit_reference_content(out, content);
        }
    } else if let Some(ref content) = r.content {
        emit_reference_content(out, content);
    }
    out.push('\n');
}

/// Emit reference content, indenting continuation lines so they re-parse as
/// part of the same entry (deeper than the entry line, like descriptions).
fn emit_reference_content(out: &mut String, content: &str) {
    let mut lines = content.lines();
    if let Some(first_line) = lines.next() {
        out.push_str(first_line);
        for line in lines {
            out.push('\n');
            if !line.is_empty() {
                out.push_str("    ");
                out.push_str(line);
            }
        }
    }
}

/// Indent each line of a body by 4 spaces.
fn emit_indented_body(out: &mut String, body: &str) {
    for line in body.lines() {
        if !line.is_empty() {
            out.push_str("    ");
            out.push_str(line);
        }
        out.push('\n');
    }
}
