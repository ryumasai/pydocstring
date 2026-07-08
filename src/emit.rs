//! Emit (code generation) from the style-independent document model.
//!
//! Each sub-module converts a [`Docstring`](crate::model::Docstring) into a
//! formatted string for a particular docstring style. All emitters take an
//! [`EmitOptions`] (by reference, so one options value can drive many calls).

pub mod google;
pub mod numpy;
pub mod sphinx;

/// Options controlling docstring emission.
///
/// This struct is `#[non_exhaustive]`: new options may be added in minor
/// releases. Construct it via [`Default`] and adjust fields, or use the
/// [`with_base_indent`](EmitOptions::with_base_indent) builder:
///
/// ```rust
/// use pydocstring::emit::EmitOptions;
///
/// let default = EmitOptions::default();
/// assert_eq!(default.base_indent, 0);
///
/// let indented = EmitOptions::default().with_base_indent(4);
/// assert_eq!(indented.base_indent, 4);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct EmitOptions {
    /// Number of spaces prepended to every non-empty line of output, so the
    /// result can be embedded at the correct indentation level in a Python
    /// file. Defaults to `0`.
    pub base_indent: usize,
}

impl EmitOptions {
    /// Returns these options with `base_indent` replaced.
    #[must_use]
    pub fn with_base_indent(mut self, base_indent: usize) -> Self {
        self.base_indent = base_indent;
        self
    }
}

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
