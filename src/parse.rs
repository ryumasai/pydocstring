//! Parsing: source text in, [`Parsed`] out.
//!
//! [`parse`] auto-detects the style; [`parse_google`], [`parse_numpy`] and
//! [`parse_plain`] force one. All four return the same [`Parsed`], so nothing
//! downstream branches on which one you called — read it through the
//! style-independent [`Document`] view, the raw CST ([`Parsed::root`]), or the
//! normalized model ([`Parsed::to_model`]).
//!
//! The per-style parsers are an implementation detail: the tree they build has
//! no per-style structure, and [`detect_style`] is the only thing that cares
//! which one runs.

use core::fmt;

use google::kind::GoogleSectionKind;

use crate::model::Docstring;
use crate::syntax::Parsed;

pub(crate) mod google;
pub(crate) mod numpy;
pub(crate) mod plain;
pub mod text_block;
pub mod token_ref;
pub(crate) mod trivia;
pub mod unified;
pub(crate) mod utils;

pub use google::parse_google;
pub use numpy::parse_numpy;
pub use plain::parse_plain;
pub use text_block::TextBlock;
pub use token_ref::TokenRef;
pub use unified::Citation;
pub use unified::DefaultMarker;
pub use unified::Directive;
pub use unified::Document;
pub use unified::Entry;
pub use unified::Section;

impl Parsed {
    /// Convert to the normalized model IR ([`Docstring`]).
    ///
    /// This is the third read lens, next to the [`Document`] view and the raw
    /// CST: it drops byte positions and normalizes the text, which is what
    /// makes it the input to [`emit`](crate::emit). The style is dispatched
    /// on internally — a Google and a NumPy docstring with the same content
    /// produce the same model.
    ///
    /// ```rust
    /// use pydocstring::parse::parse;
    ///
    /// let model = parse("Summary.\n\nArgs:\n    x (int): The value.\n").to_model();
    /// assert_eq!(model.summary.as_deref(), Some("Summary."));
    /// ```
    pub fn to_model(&self) -> Docstring {
        let model = match self.style() {
            Style::Google => google::to_model::to_model(self),
            Style::NumPy => numpy::to_model::to_model(self),
            Style::Plain => plain::to_model::to_model(self),
        };
        // Each per-style converter returns `None` only on a style mismatch,
        // which the dispatch above rules out.
        model.expect("to_model dispatched on the parsed style")
    }
}

// =============================================================================
// Style
// =============================================================================

/// Docstring style identifier.
///
/// This enum is `#[non_exhaustive]`: new styles may be added in minor
/// releases (see [`detect_style`]), so downstream `match`es need a wildcard
/// arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
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
/// The set of recognised styles may grow in minor releases (e.g. Sphinx
/// field lists): input that detects as [`Style::Plain`] today may detect as
/// a new, more specific [`Style`] variant later. `Style` is
/// `#[non_exhaustive]` for exactly this reason.
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
        if let Some(name) = trimmed.strip_suffix(':')
            && GoogleSectionKind::is_known(&name.to_ascii_lowercase())
        {
            return Style::Google;
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
