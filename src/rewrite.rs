//! Pattern-based rewriting (#47): the capstone of the edit API (RFC #48).
//!
//! [`Parsed::replace`] finds every [`Match`] of a [`Pattern`] and splices a
//! rendered *template* over each one, returning a new source string.
//! [`Parsed::replace_in`] does the same scoped to an anchor node. Both build
//! on the layers below them — the matcher (#46) supplies the matches and the
//! captures, and the splice list ([`crate::edit`], #44) applies the batch —
//! so everything outside the rewritten regions is preserved **byte-for-byte
//! by construction**.
//!
//! # Templates
//!
//! A template is **literal text with substitution holes**, not a fragment to
//! parse. Its `$NAME` / `$$$NAME` holes are recognised by the exact same
//! metavariable rules as patterns (the shared #45 scanner,
//! [`crate::pattern`]); everything else is copied verbatim. For each match a
//! replacement string is rendered:
//!
//! - literal template text is emitted as written;
//! - a `$NAME` hole is replaced by the **original target bytes** of the
//!   capture the match bound to `NAME` ([`Capture::text`](crate::matcher::Capture::text) — the RFC
//!   preservation guarantee: captured content is copied byte-exact, never
//!   reformatted);
//! - a `$$$NAME` hole is likewise replaced by the captured sibling
//!   sequence's original bytes (the capture range already spans the
//!   separators between siblings, so they too are verbatim).
//!
//! A hole name may repeat (each occurrence substitutes the same capture) and
//! a captured name may go unused (that region of the target is simply
//! dropped — an intentional deletion). A hole that names a metavariable the
//! match's [reading](crate::pattern::Reading) did **not** bind is a
//! [`RewriteError::UnknownMetavar`].
//!
//! # Re-indentation
//!
//! The replacement is spliced at [`Match::range`], whose start sits *after*
//! the matched line's leading indentation (node ranges exclude it), so the
//! template's **first line** lands right after that existing indentation with
//! nothing added. Each subsequent **template** line is a continuation line
//! and is prefixed with the match site's **base indent** — the run of spaces
//! / tabs at the start of the line the match begins on — so multi-line
//! templates stay aligned under the construct they replace. Blank template
//! lines get no indent (no trailing whitespace is introduced).
//!
//! Crucially, re-indentation applies only to the template's *own* literal
//! newlines. The bytes of a capture are emitted verbatim, newlines included:
//! a multi-line `$DESC` substitutes its original source, which already
//! carries the correct relative indentation of same-indented source, so it is
//! never re-indented. This is what makes the preservation law hold — a
//! template that re-emits a match's captures reproduces the match's bytes
//! exactly.
//!
//! # Style strictness and no-ops
//!
//! Rewriting inherits the matcher's [style strictness](crate::matcher): a
//! pattern of a different [`Style`](crate::parse::Style) than the target
//! yields no matches, so the source is returned unchanged (`Ok`). Likewise a
//! pattern that simply matches nothing is a no-op. To re-parse the result,
//! feed the returned string back through the same-style parser (e.g.
//! [`parse`](crate::parse::parse) or [`parse_google`](crate::parse::parse_google)).
//!
//! # Example
//!
//! The issue #26 use case — annotate one entry's line while every other byte
//! of the docstring stays identical:
//!
//! ```rust
//! use pydocstring::parse::{parse, Style};
//! use pydocstring::pattern::Pattern;
//!
//! let src = "Summary.\n\nArgs:\n    x (int): The value.\n    y (str): Kept.\n";
//! let parsed = parse(src);
//! let pattern = Pattern::new(Style::Google, "$NAME (int): $DESC").unwrap();
//!
//! let out = parsed.replace(&pattern, "$NAME (int): $DESC (deprecated)").unwrap();
//! assert_eq!(
//!     out,
//!     "Summary.\n\nArgs:\n    x (int): The value. (deprecated)\n    y (str): Kept.\n",
//! );
//! ```

use core::fmt;

use crate::edit::EditError;
use crate::matcher::Match;
use crate::pattern::MetaVarToken;
use crate::pattern::Pattern;
use crate::pattern::lex_metavars;
use crate::syntax::Parsed;
use crate::syntax::SyntaxNode;

// =============================================================================
// RewriteError
// =============================================================================

/// Why [`Parsed::replace`] / [`Parsed::replace_in`] could not produce a
/// result.
///
/// `#[non_exhaustive]`: later phases may add rewrite-time failures, so
/// downstream `match`es need a wildcard arm.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RewriteError {
    /// A template hole named a metavariable that the match's reading did not
    /// bind (a typo, or a name that exists only under a different reading).
    UnknownMetavar {
        /// The offending name, without the `$` / `$$$` sigil.
        name: String,
    },
    /// The underlying splice failed. In practice this cannot arise from
    /// matcher-produced ranges (they are valid, non-overlapping spans of the
    /// target); it is surfaced only so the edit layer's validation is never
    /// swallowed.
    Edit(EditError),
}

impl fmt::Display for RewriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMetavar { name } => {
                write!(
                    f,
                    "template references ${name}, which the matched reading does not bind"
                )
            }
            Self::Edit(error) => write!(f, "rewrite splice failed: {error}"),
        }
    }
}

impl std::error::Error for RewriteError {}

impl From<EditError> for RewriteError {
    fn from(error: EditError) -> Self {
        Self::Edit(error)
    }
}

// =============================================================================
// Entry points
// =============================================================================

impl Parsed {
    /// Replace every match of `pattern` in this document with `template`
    /// rendered against that match's captures, returning the new source.
    ///
    /// Matches are found globally with [`Pattern::matches`] (all readings,
    /// document order, non-overlapping). Style-strict: a pattern of a
    /// different style, or one that matches nothing, returns the source
    /// unchanged. See the [module docs](self) for the template and
    /// re-indentation semantics.
    ///
    /// # Errors
    ///
    /// [`RewriteError::UnknownMetavar`] if the template names a metavariable
    /// a match's reading did not bind.
    pub fn replace(&self, pattern: &Pattern, template: &str) -> Result<String, RewriteError> {
        self.rewrite(&pattern.matches(self), template)
    }

    /// Like [`replace`](Parsed::replace), but scoped to `anchor`'s subtree via
    /// [`Pattern::matches_in`]: the anchor's grammar selects the readings, so
    /// the same pattern rewrites `$TYPE`-shaped entries under a `Raises:`
    /// anchor and `$NAME`-shaped ones under an `Args:` anchor. An `anchor`
    /// that is not a node of this document's tree matches nothing (the source
    /// is returned unchanged).
    pub fn replace_in(&self, pattern: &Pattern, anchor: &SyntaxNode, template: &str) -> Result<String, RewriteError> {
        self.rewrite(&pattern.matches_in(self, anchor), template)
    }

    /// Render `template` against each match and splice the batch.
    fn rewrite(&self, matches: &[Match<'_>], template: &str) -> Result<String, RewriteError> {
        let source = self.source();
        let mut edits = self.edit();
        for m in matches {
            let rendered = render(template, m, source)?;
            edits.replace(m.range(), rendered);
        }
        edits.apply().map_err(RewriteError::from)
    }
}

// =============================================================================
// Template rendering
// =============================================================================

/// Render `template` for one `match`, substituting captured original bytes
/// for holes and re-indenting the template's own continuation lines to the
/// match's base indent (see the [module docs](self#re-indentation)).
fn render(template: &str, m: &Match<'_>, source: &str) -> Result<String, RewriteError> {
    let base = base_indent(source, usize::from(m.range().start()));
    let mut out = String::new();
    // `pending` means a template newline was just emitted and the next line's
    // base indent is still owed. It is set only by *template* newlines, never
    // by newlines inside a capture, so capture content is emitted verbatim.
    let mut pending = false;
    for token in lex_metavars(template) {
        match token {
            MetaVarToken::Literal(literal) => {
                for ch in literal.chars() {
                    if ch == '\n' {
                        out.push('\n');
                        pending = true; // A blank line keeps the indent owed.
                    } else if ch == '\r' {
                        // Part of a CRLF terminator, not line content: emit it
                        // without fulfilling the owed indent, so a blank CRLF
                        // line stays blank rather than gaining whitespace.
                        out.push('\r');
                    } else {
                        if pending {
                            out.push_str(base);
                            pending = false;
                        }
                        out.push(ch);
                    }
                }
            }
            MetaVarToken::Var { name, .. } => {
                let capture = m
                    .capture(name)
                    .ok_or_else(|| RewriteError::UnknownMetavar { name: name.to_owned() })?;
                if pending {
                    out.push_str(base);
                    pending = false;
                }
                out.push_str(capture.text());
            }
        }
    }
    Ok(out)
}

/// The base indent of the line that byte offset `start` sits on: the leading
/// run of spaces / tabs at that line's start.
fn base_indent(source: &str, start: usize) -> &str {
    let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
    let indent_len = source[line_start..start]
        .bytes()
        .take_while(|&b| b == b' ' || b == b'\t')
        .count();
    &source[line_start..line_start + indent_len]
}
