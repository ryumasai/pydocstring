//! Emit a [`Docstring`] as a Google-style docstring.

use crate::model::{
    Attribute, Docstring, ExceptionEntry, FreeSectionKind, Method, Parameter, Reference, Return, Section, SeeAlsoEntry,
};

/// Emit a [`Docstring`] as a Google-style docstring string.
///
/// `base_indent` is the number of spaces prepended to every non-empty line
/// of output, so the result can be embedded at the correct indentation level
/// in a Python file.
///
/// # Example
///
/// ```rust
/// use pydocstring::model::{Docstring, Section, Parameter};
/// use pydocstring::emit::google::emit_google;
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
/// let text = emit_google(&doc, 0);
/// assert!(text.contains("Args:"));
/// ```
pub fn emit_google(doc: &Docstring, base_indent: usize) -> String {
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

fn emit_multiline_with_indentation(out: &mut String, text: &str, indent_level: usize) {
    if indent_level == 0 {
        out.push_str(text);
    } else {
        let mut lines = text.lines();
        if let Some(first_line) = lines.next() {
            out.push_str(first_line);
            for line in lines {
                out.push('\n');
                if !line.is_empty() {
                    out.push_str(&" ".repeat(indent_level));
                    out.push_str(line);
                }
            }
        }
    }
}

/// Section header name for Google style.
fn section_header(section: &Section) -> &str {
    match section {
        Section::Parameters(_) => "Args",
        Section::KeywordParameters(_) => "Keyword Args",
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

fn emit_section(out: &mut String, section: &Section) {
    out.push_str(section_header(section));
    out.push_str(":\n");

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
            out.push_str("    ");
            emit_multiline_with_indentation(out, body, 4);
            out.push('\n');
        }
    }
}

/// Google: `    name (type, optional): Description.`
fn emit_parameter(out: &mut String, p: &Parameter) {
    out.push_str("    ");
    out.push_str(&p.names.join(", "));
    if p.type_annotation.is_some() || p.is_optional {
        out.push_str(" (");
        if let Some(ref ty) = p.type_annotation {
            out.push_str(ty);
        }
        if p.is_optional {
            if p.type_annotation.is_some() {
                out.push_str(", ");
            }
            out.push_str("optional");
        }
        out.push(')');
    }
    out.push(':');
    if let Some(ref desc) = p.description {
        out.push(' ');
        emit_multiline_with_indentation(out, desc, 8);
    }
    out.push('\n');
}

/// Google: `    type: Description.`
fn emit_return(out: &mut String, r: &Return) {
    out.push_str("    ");
    if let Some(ref ty) = r.type_annotation {
        out.push_str(ty);
        out.push(':');
        if let Some(ref desc) = r.description {
            out.push(' ');
            emit_multiline_with_indentation(out, desc, 4);
        }
    } else if let Some(ref desc) = r.description {
        out.push_str(desc);
    }
    out.push('\n');
}

/// Google: `    ValueError: Description.`
fn emit_exception(out: &mut String, e: &ExceptionEntry) {
    out.push_str("    ");
    out.push_str(&e.type_name);
    out.push(':');
    if let Some(ref desc) = e.description {
        out.push(' ');
        emit_multiline_with_indentation(out, desc, 8);
    }
    out.push('\n');
}

/// Google: `    name (type): Description.`
fn emit_attribute(out: &mut String, a: &Attribute) {
    out.push_str("    ");
    out.push_str(&a.name);
    if let Some(ref ty) = a.type_annotation {
        out.push_str(" (");
        out.push_str(ty);
        out.push(')');
    }
    out.push(':');
    if let Some(ref desc) = a.description {
        out.push(' ');
        emit_multiline_with_indentation(out, desc, 8);
    }
    out.push('\n');
}

/// Google: `    name(sig): Description.`
fn emit_method(out: &mut String, m: &Method) {
    out.push_str("    ");
    out.push_str(&m.name);
    if let Some(ref ty) = m.type_annotation {
        out.push_str(" (");
        out.push_str(ty);
        out.push(')');
    }
    out.push(':');
    if let Some(ref desc) = m.description {
        out.push(' ');
        emit_multiline_with_indentation(out, desc, 8);
    }
    out.push('\n');
}

/// Google: `    func1, func2: Description.`
fn emit_see_also(out: &mut String, item: &SeeAlsoEntry) {
    out.push_str("    ");
    out.push_str(&item.names.join(", "));
    if let Some(ref desc) = item.description {
        out.push_str(": ");
        emit_multiline_with_indentation(out, desc, 8);
    }
    out.push('\n');
}

/// Google: `    .. [1] Content.`
fn emit_reference(out: &mut String, r: &Reference) {
    out.push_str("    ");
    if let Some(ref num) = r.number {
        out.push_str(".. [");
        out.push_str(num);
        out.push_str("] ");
    }
    if let Some(ref content) = r.content {
        emit_multiline_with_indentation(out, content, 8);
    }
    out.push('\n');
}
