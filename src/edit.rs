//! Anchored splice edits over a parsed docstring.
//!
//! This is the low-level layer of the edit API (issue #44): an edit list
//! anchored on byte ranges, [`SyntaxNode`]s, or [`SyntaxToken`]s of a
//! [`Parsed`] result. Edits are collected with [`Parsed::edit`], validated
//! and spliced by [`Edits::apply`] into a new source string, and optionally
//! re-parsed with the same style parser by [`Edits::apply_reparsed`] — trees
//! are never mutated in place; per the source-backed convention (issue #42),
//! an edit renders to a string and the string is re-parsed.
//!
//! Everything an edit does not explicitly touch is preserved byte-for-byte:
//! an empty edit list reproduces the source exactly, and replacing any
//! element with its own text is the identity (both laws are property-tested
//! over the whole corpus in `tests/edit.rs`).
//!
//! Zero-length missing placeholders (see [`SyntaxToken::is_missing`]) are the
//! insertion anchors: replacing one splices new text at exactly the offset
//! where the absent element belongs.
//!
//! # Example
//!
//! Replace one entry's description while the rest of the docstring stays
//! byte-identical (the issue #26 use case):
//!
//! ```rust
//! use pydocstring::parse::{parse, Document};
//!
//! let src = "Summary.\n\nArgs:\n    x (int): Old description.\n    y: Stays.\n";
//! let parsed = parse(src);
//!
//! let doc = Document::new(&parsed);
//! let entry = doc.sections().next().unwrap().entries().next().unwrap();
//! let desc = entry.description().unwrap();
//!
//! let mut edits = parsed.edit();
//! edits.replace_node(desc.syntax(), "New description.");
//! assert_eq!(
//!     edits.apply().unwrap(),
//!     "Summary.\n\nArgs:\n    x (int): New description.\n    y: Stays.\n",
//! );
//!
//! // apply_reparsed re-parses with the same style parser:
//! let reparsed = edits.apply_reparsed().unwrap();
//! assert_eq!(reparsed.style(), parsed.style());
//! ```

use core::fmt;

use crate::parse::Style;
use crate::syntax::Parsed;
use crate::syntax::SyntaxElement;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::syntax::SyntaxToken;
use crate::text::TextRange;
use crate::text::TextSize;

// =============================================================================
// EditError
// =============================================================================

/// An error raised by [`Edits::apply`] when the edit list is invalid.
///
/// This enum is `#[non_exhaustive]`: later phases of the edit API may add
/// validation failures, so downstream `match`es need a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditError {
    /// Two edits cover overlapping byte ranges, so their splice order would
    /// be ambiguous. Touching ranges (one's end equals the other's start) do
    /// not overlap; see [`Edits::apply`] for how boundary inserts are
    /// ordered.
    Overlap {
        /// The earlier edit's range (in the position-sorted order).
        a: TextRange,
        /// The later edit's range that overlaps `a`.
        b: TextRange,
    },
    /// An edit's range does not denote a valid span of the parse result's
    /// source: it extends past the end of the source, its start is greater
    /// than its end, or an offset falls inside a multi-byte UTF-8 character.
    OutOfBounds {
        /// The offending range.
        range: TextRange,
    },
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overlap { a, b } => write!(f, "overlapping edits at {a} and {b}"),
            Self::OutOfBounds { range } => write!(
                f,
                "edit range {range} is not a valid span of the source: it runs past the end, \
                 its start is after its end, or an endpoint falls inside a multi-byte character"
            ),
        }
    }
}

impl std::error::Error for EditError {}

// =============================================================================
// Edits
// =============================================================================

/// One pending splice: replace `range` with `text` (an insert has an empty
/// range; a delete has empty text).
#[derive(Debug, Clone)]
struct SpliceEdit {
    range: TextRange,
    text: String,
}

/// A list of pending edits anchored on one [`Parsed`] result.
///
/// Created by [`Parsed::edit`]. Collect edits with the builder methods (each
/// returns `&mut Self` for chaining), then splice them with
/// [`apply`](Edits::apply) or [`apply_reparsed`](Edits::apply_reparsed).
/// Applying borrows the list (`&self`), so a builder can be applied more
/// than once or inspected afterwards.
///
/// All node/token conveniences delegate to the three range-anchored core
/// methods [`replace`](Edits::replace), [`insert`](Edits::insert), and
/// [`delete`](Edits::delete). Validation (bounds and overlaps) happens in
/// `apply`, not at collection time.
#[derive(Debug)]
pub struct Edits<'a> {
    parsed: &'a Parsed,
    edits: Vec<SpliceEdit>,
}

impl Parsed {
    /// Start an empty edit list anchored on this parse result.
    ///
    /// See the [`edit`](crate::edit) module docs for the model and an
    /// end-to-end example.
    pub fn edit(&self) -> Edits<'_> {
        Edits {
            parsed: self,
            edits: Vec::new(),
        }
    }
}

impl<'a> Edits<'a> {
    /// The parse result this edit list is anchored on.
    pub fn parsed(&self) -> &'a Parsed {
        self.parsed
    }

    // ── Range-anchored core ────────────────────────────────────────────

    /// Replace the bytes of `range` with `text`.
    ///
    /// A zero-length `range` inserts `text` at that offset (this is how
    /// missing placeholders work as insertion anchors); empty `text` deletes
    /// the range. The range is validated by [`apply`](Edits::apply), not
    /// here.
    pub fn replace(&mut self, range: TextRange, text: impl Into<String>) -> &mut Self {
        self.edits.push(SpliceEdit {
            range,
            text: text.into(),
        });
        self
    }

    /// Insert `text` at byte offset `at`.
    ///
    /// Equivalent to replacing the empty range `at..at`. Multiple inserts at
    /// the same offset are applied in call order (first call's text ends up
    /// first); see [`apply`](Edits::apply) for inserts at the boundary of a
    /// replaced range.
    pub fn insert(&mut self, at: TextSize, text: impl Into<String>) -> &mut Self {
        self.replace(TextRange::new(at, at), text)
    }

    /// Delete the bytes of `range`.
    ///
    /// Equivalent to replacing the range with the empty string. For removing
    /// a whole construct together with its line layout, prefer
    /// [`remove_lines`](Edits::remove_lines).
    pub fn delete(&mut self, range: TextRange) -> &mut Self {
        self.replace(range, String::new())
    }

    // ── Node / token conveniences ──────────────────────────────────────

    /// Replace `node`'s exact content span with `text`.
    ///
    /// A node's range is its content span only — leading indentation and the
    /// trailing `NEWLINE` are siblings in the parent, so they are preserved.
    /// Use [`remove_lines`](Edits::remove_lines) to remove a node together
    /// with its lines.
    pub fn replace_node(&mut self, node: &SyntaxNode, text: impl Into<String>) -> &mut Self {
        self.replace(node.range(), text)
    }

    /// Replace `token`'s exact span with `text`.
    ///
    /// Works on zero-length missing placeholders (see
    /// [`SyntaxToken::is_missing`]): replacing one inserts `text` at the
    /// anchor offset where the missing element belongs.
    pub fn replace_token(&mut self, token: &SyntaxToken, text: impl Into<String>) -> &mut Self {
        self.replace(token.range(), text)
    }

    /// Delete `node` together with the whole line(s) it occupies.
    ///
    /// The node's content span is expanded to a *line extent* and one
    /// adjacent trailing blank line is consumed, in three documented steps:
    ///
    /// 1. **Start** — moved back to the start of the node's first line if
    ///    every byte between the line start and the node is a space or tab
    ///    (the leading indentation); otherwise left at the node start.
    /// 2. **End** — trailing spaces/tabs after the node on its last line are
    ///    consumed together with that line's newline (`\n` or `\r\n`). If
    ///    the node ends the source without a newline, trailing whitespace up
    ///    to the end of the source is consumed instead.
    /// 3. **Blank line** — if a `BLANK_LINE` token of the tree starts
    ///    exactly at the resulting end offset, exactly one such token is
    ///    consumed as well (the RFC blank-line convention: a construct
    ///    separated from the next by a blank line takes that separator with
    ///    it). Preceding blank lines are never touched.
    ///
    /// The expanded extent is recorded as a single [`delete`](Edits::delete).
    pub fn remove_lines(&mut self, node: &SyntaxNode) -> &mut Self {
        self.remove_lines_range(node.range())
    }

    /// Delete `range` together with the whole line(s) it occupies.
    ///
    /// The range-anchored form of [`remove_lines`](Edits::remove_lines), which
    /// only ever reads its node's range: the blank-line step (3) resolves
    /// against the whole tree, not against the node. Use this when you hold a
    /// span rather than a node — e.g. across an FFI boundary, where the
    /// caller's handle is a range.
    pub fn remove_lines_range(&mut self, range: TextRange) -> &mut Self {
        let source = self.parsed.source();
        let bytes = source.as_bytes();
        let mut start = usize::from(range.start());
        let mut end = usize::from(range.end());
        if start > end || end > bytes.len() || !source.is_char_boundary(start) || !source.is_char_boundary(end) {
            // Foreign/corrupt range (out of bounds, inverted, or splitting a
            // multi-byte character in a user-built tree): record it as-is and
            // let apply() report OutOfBounds instead of panicking here.
            return self.delete(range);
        }

        // 1. Expand start over the first line's leading indentation.
        let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
        if bytes[line_start..start].iter().all(|&b| b == b' ' || b == b'\t') {
            start = line_start;
        }

        // 2. Expand end over trailing spaces/tabs plus the line's newline.
        let mut scan = end;
        while scan < bytes.len() && (bytes[scan] == b' ' || bytes[scan] == b'\t') {
            scan += 1;
        }
        if scan + 1 < bytes.len() && bytes[scan] == b'\r' && bytes[scan + 1] == b'\n' {
            end = scan + 2;
        } else if scan < bytes.len() && bytes[scan] == b'\n' {
            end = scan + 1;
        } else if scan == bytes.len() {
            end = scan;
        }

        // 3. Consume one trailing BLANK_LINE sibling, if the tree has one
        //    starting exactly at the extent end.
        if let Some(blank_end) = blank_line_end_at(self.parsed.root(), TextSize::from(end)) {
            end = usize::from(blank_end);
        }

        self.delete(TextRange::new(TextSize::from(start), TextSize::from(end)))
    }

    // ── Application ────────────────────────────────────────────────────

    /// Validate the edit list and splice it into a new source string.
    ///
    /// Non-consuming: the builder can be inspected or applied again.
    ///
    /// # Ordering
    ///
    /// Edits are sorted by position (start offset, then end offset); the
    /// sort is stable, so edits with the *same* range — in particular
    /// multiple inserts at one offset — are applied in call order. An insert
    /// at the **start** boundary of a replaced/deleted range sorts before it
    /// (its text lands *before* the replacement); an insert at the **end**
    /// boundary lands *after* the replacement.
    ///
    /// # Errors
    ///
    /// * [`EditError::OutOfBounds`] — an edit range extends past the end of
    ///   the source, is inverted (start > end), or splits a multi-byte
    ///   UTF-8 character.
    /// * [`EditError::Overlap`] — two edits cover overlapping ranges
    ///   (touching ranges are fine). A zero-length insert strictly inside
    ///   another edit's range also overlaps.
    pub fn apply(&self) -> Result<String, EditError> {
        let source = self.parsed.source();
        for edit in &self.edits {
            let start = usize::from(edit.range.start());
            let end = usize::from(edit.range.end());
            if start > end || end > source.len() || !source.is_char_boundary(start) || !source.is_char_boundary(end) {
                return Err(EditError::OutOfBounds { range: edit.range });
            }
        }

        let mut order: Vec<usize> = (0..self.edits.len()).collect();
        // Stable: equal (start, end) keys keep insertion-call order.
        order.sort_by_key(|&i| (self.edits[i].range.start(), self.edits[i].range.end()));
        for pair in order.windows(2) {
            let a = self.edits[pair[0]].range;
            let b = self.edits[pair[1]].range;
            if a.end() > b.start() {
                return Err(EditError::Overlap { a, b });
            }
        }

        let mut out = String::with_capacity(source.len());
        let mut pos = 0usize;
        for &i in &order {
            let edit = &self.edits[i];
            out.push_str(&source[pos..usize::from(edit.range.start())]);
            out.push_str(&edit.text);
            pos = usize::from(edit.range.end());
        }
        out.push_str(&source[pos..]);
        Ok(out)
    }

    /// [`apply`](Edits::apply) the edits, then re-parse the result with the
    /// parser of the *same style* as the original ([`Parsed::style`]).
    ///
    /// The style is deliberately not re-detected: editing must not silently
    /// reinterpret the docstring as another style, even if the edited text
    /// would auto-detect differently.
    pub fn apply_reparsed(&self) -> Result<Parsed, EditError> {
        let text = self.apply()?;
        Ok(match self.parsed.style() {
            Style::NumPy => crate::parse::numpy::parse_numpy(&text),
            Style::Google => crate::parse::google::parse_google(&text),
            Style::Plain => crate::parse::plain::parse_plain(&text),
        })
    }
}

/// Find a `BLANK_LINE` token starting exactly at `at` anywhere in the tree,
/// returning its end offset.
///
/// The coverage law guarantees every blank-line byte is owned by a
/// `BLANK_LINE` token, so this tree walk — not text scanning — decides
/// whether the bytes following a line extent are a blank line.
fn blank_line_end_at(node: &SyntaxNode, at: TextSize) -> Option<TextSize> {
    for child in node.children() {
        match child {
            SyntaxElement::Token(t) => {
                if t.kind() == SyntaxKind::BLANK_LINE && t.range().start() == at {
                    return Some(t.range().end());
                }
            }
            SyntaxElement::Node(n) => {
                if let Some(end) = blank_line_end_at(n, at) {
                    return Some(end);
                }
            }
        }
    }
    None
}
