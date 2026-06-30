//! Emit (code generation) from the style-independent document model.
//!
//! Each sub-module converts a [`Docstring`](crate::model::Docstring) into a
//! formatted string for a particular docstring style.

pub mod google;
pub mod numpy;
pub mod sphinx;

/// Prepend `base_indent` spaces to every non-empty line.
pub(crate) fn indent_lines(text: &str, base_indent: usize) -> String {
    let indent: String = " ".repeat(base_indent);
    let mut result = String::new();
    for line in text.lines() {
        if !line.is_empty() {
            result.push_str(&indent);
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}
