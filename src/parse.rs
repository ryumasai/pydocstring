//! Docstring style implementations.
//!
//! Each sub-module provides an AST and parser for its respective style.
//! This module also provides [`detect_style`] for automatic style detection.

use core::fmt;

use google::GoogleSectionKind;

pub mod google;
pub mod numpy;
pub mod plain;
pub mod text_block;
pub(crate) mod trivia;
pub(crate) mod utils;
pub mod visitor;

pub use text_block::TextBlock;

// =============================================================================
// Style
// =============================================================================

/// Docstring style identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Style {
    /// NumPy style (section headers with underlines).
    NumPy,
    /// Google style (section headers with colons).
    Google,
    /// Plain docstring: no recognised style markers (summary/extended summary
    /// only). Also covers unrecognised styles such as Sphinx.
    Plain,
}

impl fmt::Display for Style {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Style::NumPy => write!(f, "numpy"),
            Style::Google => write!(f, "google"),
            Style::Plain => write!(f, "plain"),
        }
    }
}

// =============================================================================
// EntryRole
// =============================================================================

/// The role of the `ENTRY` nodes in a section body, derived from the section
/// kind.
///
/// This is the single mapping used both by the visitor (to route an `ENTRY`
/// to the right `visit_*` method) and by the typed section accessors (to
/// return empty for sections outside the accessor's role, e.g. `args()` on a
/// `Raises:` section).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryRole {
    /// Argument/parameter entries (Args, Keyword Args, Other Parameters, Receives).
    Parameter,
    /// Return entries.
    Return,
    /// Yield entries.
    Yield,
    /// Exception entries (Raises).
    Exception,
    /// Warning entries (Warns).
    Warning,
    /// "See Also" items.
    SeeAlsoItem,
    /// Attribute entries.
    Attribute,
    /// Method entries.
    Method,
    /// References sections: body items are `CITATION` nodes, never `ENTRY`.
    Citation,
    /// Free-text sections (Notes, Examples, unknown, …): no entries at all.
    FreeText,
}

// =============================================================================
// Style detection
// =============================================================================

/// Detect the docstring style from its content.
///
/// Uses heuristics to identify the style:
/// 1. **NumPy**: Section headers followed by `---` underlines
/// 2. **Google**: Section headers ending with `:` (e.g., `Args:`, `Returns:`)
/// 3. Falls back to [`Style::Plain`] if no style-specific patterns are found.
///    This includes summary-only docstrings and unrecognised styles such as
///    Sphinx.
///
/// # Example
///
/// ```rust
/// use pydocstring::parse::detect_style;
/// use pydocstring::parse::Style;
///
/// let numpy = "Summary.\n\nParameters\n----------\nx : int\n    Description.";
/// assert_eq!(detect_style(numpy), Style::NumPy);
///
/// let google = "Summary.\n\nArgs:\n    x: Description.";
/// assert_eq!(detect_style(google), Style::Google);
///
/// let plain = "Just a summary.";
/// assert_eq!(detect_style(plain), Style::Plain);
/// ```
pub fn detect_style(input: &str) -> Style {
    let lines: Vec<&str> = input.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // NumPy: non-empty line followed by a line of 3+ dashes.
        if let Some(next) = lines.get(i + 1) {
            let next_trimmed = next.trim();
            if !next_trimmed.is_empty() && next_trimmed.len() >= 3 && next_trimmed.bytes().all(|b| b == b'-') {
                return Style::NumPy;
            }
        }

        // Google: known section name ending with `:`.
        if let Some(name) = trimmed.strip_suffix(':') {
            if GoogleSectionKind::is_known(&name.to_ascii_lowercase()) {
                return Style::Google;
            }
        }
    }

    Style::Plain
}

// =============================================================================
// Unified parse entry point
// =============================================================================

/// Parse a docstring, auto-detecting its style.
///
/// Internally calls [`detect_style`] and dispatches to the appropriate parser.
/// The root node kind is always the style-neutral
/// [`SyntaxKind::DOCUMENT`](crate::syntax::SyntaxKind::DOCUMENT); the detected
/// style is recorded on the result and reported by
/// [`Parsed::style`](crate::syntax::Parsed::style).
///
/// # Example
///
/// ```rust
/// use pydocstring::parse::parse;
/// use pydocstring::parse::Style;
/// use pydocstring::syntax::SyntaxKind;
///
/// let result = parse("Summary.\n\nArgs:\n    x: Description.");
/// assert_eq!(result.root().kind(), SyntaxKind::DOCUMENT);
/// assert_eq!(result.style(), Style::Google);
///
/// let plain = parse("Just a summary.");
/// assert_eq!(plain.style(), Style::Plain);
/// ```
pub fn parse(input: &str) -> crate::syntax::Parsed {
    match detect_style(input) {
        Style::NumPy => numpy::parse_numpy(input),
        Style::Google => google::parse_google(input),
        Style::Plain => plain::parse_plain(input),
    }
}
