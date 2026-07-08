//! Emit a [`Docstring`] as a Google-style docstring.

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

/// Emit a [`Docstring`] as a Google-style docstring string.
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
/// let text = emit_google(&doc, &EmitOptions::default());
/// assert!(text.contains("Args:"));
/// ```
pub fn emit_google(doc: &Docstring, options: &EmitOptions) -> String {
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

/// Google: `    name (type, optional, default value): Description.`
fn emit_parameter(out: &mut String, p: &Parameter) {
    out.push_str("    ");
    out.push_str(&p.names.join(", "));
    if p.type_annotation.is_some() || p.is_optional || p.default_value.is_some() {
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
        if let Some(ref dv) = p.default_value {
            if p.type_annotation.is_some() || p.is_optional {
                out.push_str(", ");
            }
            out.push_str("default ");
            out.push_str(dv);
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
        // Indent continuation lines into the section body: written raw at
        // column 0 they dedent out of the Returns section and the re-parse
        // silently drops every line after the first (#93).
        emit_multiline_with_indentation(out, desc, 4);
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

/// Google: `    name1, name2 (type): Description.`
fn emit_attribute(out: &mut String, a: &Attribute) {
    out.push_str("    ");
    out.push_str(&a.names.join(", "));
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

/// Google: `    func1, func2\n        Description.`
///
/// The description always goes on the following deeper-indented line(s) —
/// the form the parser reads back for every name. The `name: desc` one-liner
/// is NOT round-trippable when the name starts with an rST role
/// (`:func:`x``): find_term_colon's leading-colon guard (the #26 rule)
/// rejects the line on re-parse and the description comma-splits into fake
/// names (#91).
fn emit_see_also(out: &mut String, item: &SeeAlsoEntry) {
    out.push_str("    ");
    out.push_str(&item.names.join(", "));
    out.push('\n');
    if let Some(ref desc) = item.description {
        for line in desc.lines() {
            if !line.is_empty() {
                out.push_str("        ");
                out.push_str(line);
            }
            out.push('\n');
        }
    }
}

/// Google: `    .. [1] Content.`
fn emit_reference(out: &mut String, r: &Reference) {
    out.push_str("    ");
    if let Some(ref label) = r.label {
        out.push_str(".. [");
        out.push_str(label);
        out.push_str("] ");
    }
    if let Some(ref content) = r.content {
        emit_multiline_with_indentation(out, content, 8);
    }
    out.push('\n');
}
