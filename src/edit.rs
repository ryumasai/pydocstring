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

use crate::parse::Entry;
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

    // ── Semantic edits ─────────────────────────────────────────────────

    /// Replace `entry`'s description with `text`, or write one where the entry
    /// has none.
    ///
    /// Unlike the splice methods this one needs no anchor: an entry with no
    /// description has nothing to replace, and this places one — after the
    /// colon in Google style, on its own continuation line in NumPy style,
    /// adding the colon if the entry lacks it.
    ///
    /// # Placement
    ///
    /// A **single-line** `text` keeps the entry's existing shape: a Google
    /// description written inline (`x (int): …`) stays inline, one written on
    /// its own line stays there.
    ///
    /// A **multi-line** `text` is always placed on its own line, at the
    /// entry's continuation indent, and its interior indentation is preserved
    /// relative to that. Splicing a multi-line block inline would start it at
    /// a column nobody controls — its second line would land *shallower than
    /// its first*, which is malformed rST that only survives because napoleon
    /// dedents a field body before docutils sees it.
    ///
    /// The continuation indent is read from the description's own second line
    /// where there is one, so a docstring that continues at an unusual depth
    /// keeps it. Only when the entry has no continuation line to read is it
    /// derived as the entry's indent plus four spaces.
    ///
    /// ```rust
    /// use pydocstring::parse::{parse, Document};
    ///
    /// let parsed = parse("S.\n\nArgs:\n    x (int): Old.\n    y (str):\n");
    /// let doc = Document::new(&parsed);
    /// let args = doc.sections().next().unwrap();
    /// let mut entries = args.entries();
    /// let (x, y) = (entries.next().unwrap(), entries.next().unwrap());
    ///
    /// let mut edits = parsed.edit();
    /// edits.set_description(x, "New.");   // replaces, inline
    /// edits.set_description(y, "Fresh."); // writes one where there was none
    /// assert_eq!(
    ///     edits.apply().unwrap(),
    ///     "S.\n\nArgs:\n    x (int): New.\n    y (str): Fresh.\n",
    /// );
    /// ```
    pub fn set_description(&mut self, entry: Entry<'_>, text: &str) -> &mut Self {
        let node = entry.syntax().nodes(SyntaxKind::DESCRIPTION).next();
        let indent = self.continuation_indent(entry, node);
        // NumPy has no inline description: it is always its own line.
        let own_line = self.numpy_entry_grammar() || text.contains('\n');
        let block = format!("\n{indent}{}", reindent(text, &indent));

        match node {
            // An existing description: replace it where it stands, unless the
            // new text is a block that has to own its line.
            Some(node) if !node.range().is_empty() => {
                if own_line {
                    self.replace(widened_node(entry, node), block)
                } else {
                    self.replace(node.range(), text)
                }
            }
            // A zero-length DESCRIPTION placeholder (`x (int):`) — the anchor
            // is already at the offset where a description belongs.
            Some(placeholder) => {
                let range = self.over_trailing_blanks(widened_node(entry, placeholder));
                let text = if own_line { block } else { format!(" {text}") };
                self.replace(range, text)
            }
            // No description node at all (`x`, `x (int)`, NumPy's `x : int`):
            // nothing in the tree marks where one would go, so the grammar
            // does — which is the whole reason this method exists.
            None => {
                // A NumPy description does not follow the colon — the colon of
                // `x : int` separates the name from the type — so it needs
                // none. A Google one does, and the entry may not have one yet.
                let colon = if self.numpy_entry_grammar() || entry.syntax().find_token(SyntaxKind::COLON).is_some() {
                    ""
                } else {
                    ":"
                };
                let anchor = TextRange::new(entry.range().end(), entry.range().end());
                let text = if own_line { block } else { format!(" {text}") };
                self.replace(self.over_trailing_blanks(anchor), format!("{colon}{text}"))
            }
        }
    }

    /// `range` extended forward over the spaces and tabs that follow it.
    ///
    /// The whitespace an author left after `x (int):` is not a child of the
    /// entry — the ENTRY ends at the colon and the space belongs to the line —
    /// so no walk over siblings can see it. Writing a description at that
    /// anchor without consuming it would strand the space at the end of the
    /// line. Used only where the anchor *is* the end of the line, so the run
    /// this eats is always trailing whitespace.
    fn over_trailing_blanks(&self, range: TextRange) -> TextRange {
        let bytes = self.parsed.source().as_bytes();
        let mut end = usize::from(range.end()).min(bytes.len());
        while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
            end += 1;
        }
        TextRange::new(range.start(), TextSize::from(end))
    }

    /// Insert `text` as a new paragraph before `entry`'s description, keeping
    /// the description itself byte-for-byte.
    ///
    /// This is [`set_description`](Edits::set_description)'s placement applied
    /// to a prepended block: `text` lands on its own line at the entry's
    /// continuation indent, a blank line separates it from the existing
    /// description, and the description's own bytes — including the interior
    /// indentation of its continuation lines — are spliced back untouched
    /// rather than re-rendered.
    ///
    /// An entry with no description gets `text` as its description, exactly as
    /// [`set_description`](Edits::set_description) would write it.
    ///
    /// ```rust
    /// use pydocstring::parse::{parse, Document};
    ///
    /// let parsed = parse("S.\n\nArgs:\n    x (int): Keep me.\n");
    /// let doc = Document::new(&parsed);
    /// let entry = doc.sections().next().unwrap().entries().next().unwrap();
    ///
    /// let mut edits = parsed.edit();
    /// edits.prepend_to_description(entry, ".. deprecated:: 1.10\n   Use `y`.");
    /// assert_eq!(
    ///     edits.apply().unwrap(),
    ///     "S.\n\nArgs:\n    x (int):\n        .. deprecated:: 1.10\n           Use `y`.\n\n        Keep me.\n",
    /// );
    /// ```
    pub fn prepend_to_description(&mut self, entry: Entry<'_>, text: &str) -> &mut Self {
        let node = entry
            .syntax()
            .nodes(SyntaxKind::DESCRIPTION)
            .next()
            .filter(|n| !n.range().is_empty());
        let Some(node) = node else {
            // Nothing to prepend to — the description is the text.
            return self.set_description(entry, text);
        };

        let indent = self.continuation_indent(entry, Some(node));
        // The block's own text already carries the indentation of its
        // continuation lines; only its first line lost its indent to the
        // range's start. So `indent + text` puts it back byte-for-byte, and
        // no line of the author's prose is re-rendered.
        let kept = node.range().source_text(self.parsed.source());
        let range = widened_node(entry, node);
        self.replace(
            range,
            format!("\n{indent}{}\n\n{indent}{kept}", reindent(text, &indent)),
        )
    }

    /// Set `entry`'s type annotation to `text`, or write one where the entry
    /// has none.
    ///
    /// A type that is present is replaced, and so is a zero-length placeholder
    /// (`x ():`). Where the *marker itself* is absent — `x: The value.` has no
    /// brackets to hold a type, and neither has NumPy's `x` — there is nothing
    /// to anchor on, and this writes the marker too: `x (int): The value.` in
    /// Google style, `x : int` in NumPy style.
    ///
    /// ```rust
    /// use pydocstring::parse::{parse, Document};
    ///
    /// let parsed = parse("S.\n\nArgs:\n    x: The value.\n");
    /// let doc = Document::new(&parsed);
    /// let entry = doc.sections().next().unwrap().entries().next().unwrap();
    ///
    /// let mut edits = parsed.edit();
    /// edits.set_type(entry, "int");
    /// assert_eq!(edits.apply().unwrap(), "S.\n\nArgs:\n    x (int): The value.\n");
    /// ```
    pub fn set_type(&mut self, entry: Entry<'_>, text: &str) -> &mut Self {
        let numpy = self.numpy_entry_grammar();
        let node = entry.syntax();

        if let Some(token) = node
            .find_token(SyntaxKind::TYPE)
            .or_else(|| node.find_missing(SyntaxKind::TYPE))
        {
            // A NumPy placeholder sits flush against the colon (`x :`), so it
            // needs the separating space that Google's brackets already give.
            if token.is_missing() && numpy {
                return self.replace(widened_token(entry, token), format!(" {text}"));
            }
            return self.replace_token(token, text);
        }

        // No type marker at all: write one where the grammar puts it — after
        // the *last* name, since an entry may declare several
        // (`x, y (int)` / `x, y : int`), and the type annotates all of them.
        if let Some(name) = node.tokens(SyntaxKind::NAME).last() {
            let written = if numpy {
                format!(" : {text}")
            } else {
                format!(" ({text})")
            };
            return self.insert(name.range().end(), written);
        }

        // No name either — a Google `Returns:` / `Raises:` entry that is all
        // description (`The value.`), where the type is written in front of it.
        let written = if numpy {
            let indent = self.continuation_indent(entry, node.nodes(SyntaxKind::DESCRIPTION).next());
            format!("{text}\n{indent}")
        } else {
            format!("{text}: ")
        };
        self.insert(entry.range().start(), written)
    }

    /// Whether this parse result's entries follow the NumPy grammar — a
    /// description on its own continuation line, a type after the colon — as
    /// opposed to Google's inline `x (int): desc`.
    ///
    /// The `match` is **exhaustive on purpose**, even though [`Style`] is
    /// `#[non_exhaustive]` to the outside world. Phase 5 adds Sphinx field
    /// lists as a third style (#99), and their entry grammar is neither of
    /// these two. A wildcard arm here would silently write Google's brackets
    /// and colon into a field list; instead, adding a `Style` variant stops
    /// this function from compiling and makes its author choose.
    fn numpy_entry_grammar(&self) -> bool {
        match self.parsed.style() {
            Style::NumPy => true,
            // Plain has no sections, so no entries, so no entry grammar: it is
            // only ever reached through a hand-built tree, and Google's is the
            // right guess for the shape of one.
            Style::Google | Style::Plain => false,
        }
    }

    /// The indent an entry's description continuation lines use.
    ///
    /// A description is a list of lines, so ask its second one where it
    /// starts: the entry's indent plus four spaces is a guess, and it is wrong
    /// for a docstring that continues at another depth. The guess is the
    /// fallback only for an entry with no continuation line to read.
    fn continuation_indent(&self, entry: Entry<'_>, description: Option<&SyntaxNode>) -> String {
        let second_line = description.and_then(|d| d.tokens(SyntaxKind::TEXT_LINE).nth(1));
        match second_line {
            Some(line) => self.parsed.line_indent(line.range().start()).to_string(),
            None => format!("{}    ", self.parsed.line_indent(entry.range().start())),
        }
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

/// `node`'s range, widened backwards over the trivia that separates it from
/// its previous sibling. See [`widen_from`].
fn widened_node(entry: Entry<'_>, node: &SyntaxNode) -> TextRange {
    let index = entry
        .syntax()
        .children()
        .iter()
        .position(|child| matches!(child, SyntaxElement::Node(n) if core::ptr::eq(n, node)));
    widen_from(entry, index, node.range())
}

/// `token`'s range, widened backwards over the trivia that separates it from
/// its previous sibling. See [`widen_from`].
fn widened_token(entry: Entry<'_>, token: &SyntaxToken) -> TextRange {
    let index = entry
        .syntax()
        .children()
        .iter()
        .position(|child| matches!(child, SyntaxElement::Token(t) if core::ptr::eq(t, token)));
    widen_from(entry, index, token.range())
}

/// Widen `range` backwards from the entry child at `index`, over the trivia
/// tokens in front of it, consuming **at most one line break**.
///
/// This is what lets one code path serve both styles. A Google description is
/// written inline (`x (int): desc`) and a NumPy one on its own line, and the
/// tree says which: the siblings in front of the `DESCRIPTION` are
/// `WHITESPACE` in the first, `NEWLINE` + `WHITESPACE` in the second. Eating
/// them and re-emitting on a fresh line collapses the two into one edit.
///
/// One line break, not all of them, is what keeps that a rule rather than a
/// guess: `NEWLINE` and `BLANK_LINE` are distinct kinds, so a blank line the
/// author wrote in front of the description survives the widening.
///
/// This is deliberately **not** a public "widen over trivia" operation. It is
/// the *entry* grammar, and it is correct only there: an extended summary is
/// preceded by `BLANK_LINE` `NEWLINE`, where eating one line break would
/// destroy the paragraph break, and a section body by `NEWLINE`, where it
/// would pull the body onto the header's line.
fn widen_from(entry: Entry<'_>, index: Option<usize>, range: TextRange) -> TextRange {
    let Some(index) = index else {
        // Not a child of this entry (a caller mixing trees). Leave the range
        // as it is and let `apply` judge it.
        return range;
    };

    let mut start = range.start();
    let mut took_line_break = false;
    for child in entry.syntax().children()[..index].iter().rev() {
        let SyntaxElement::Token(token) = child else { break };
        if !token.kind().is_trivia() {
            break;
        }
        if matches!(token.kind(), SyntaxKind::NEWLINE | SyntaxKind::BLANK_LINE) {
            if took_line_break {
                break;
            }
            took_line_break = true;
        }
        start = token.range().start();
    }
    TextRange::new(start, range.end())
}

/// `text`'s continuation lines pushed under `indent`, its interior
/// indentation kept relative to it.
///
/// The first line is placed by the caller — it follows the indent that is
/// written before it — and a blank line is left blank rather than filled with
/// trailing whitespace.
fn reindent(text: &str, indent: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            if !line.is_empty() {
                out.push_str(indent);
            }
        }
        out.push_str(line);
    }
    out
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
