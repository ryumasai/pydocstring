//! Source cursor for line-oriented docstring parsing.
//!
//! [`LineCursor`] bundles the source text, line-offset table, and current
//! line position into a single struct, eliminating the need to thread
//! `(source, &offsets, total_lines)` through every helper function.

use crate::text::TextRange;
use crate::text::TextSize;

// =============================================================================
// LineCursor
// =============================================================================

/// A read/write cursor over a source string, providing line-oriented
/// navigation and span construction helpers.
///
/// Callers advance the cursor by mutating [`LineCursor::line`] directly
/// (or via convenience methods like [`advance`](LineCursor::advance) and
/// [`skip_blank_lines`](LineCursor::skip_blank_lines)).  Sub-parsers
/// receive `&mut LineCursor` and leave it positioned after the last
/// consumed line.
pub(crate) struct LineCursor<'a> {
    source: &'a str,
    offsets: Vec<usize>,
    total: usize,
    /// Current line index (0-based).
    pub line: usize,
}

impl<'a> LineCursor<'a> {
    /// Create a new cursor over `source`, starting at line 0.
    pub fn new(source: &'a str) -> Self {
        let offsets = build_line_offsets(source);
        let total = count_lines(source, &offsets);
        Self {
            source,
            offsets,
            total,
            line: 0,
        }
    }

    // ── Source access ────────────────────────────────────────────────

    /// The full source text.
    pub fn source(&self) -> &'a str {
        self.source
    }

    /// A `TextRange` spanning the entire source text.
    pub fn full_range(&self) -> TextRange {
        let last_line = self.total.saturating_sub(1);
        let last_col = self.line_text(last_line).len();
        self.make_range(0, 0, last_line, last_col)
    }

    // ── Position ────────────────────────────────────────────────────

    /// Whether the cursor has reached or passed the end of the source.
    pub fn is_eof(&self) -> bool {
        self.line >= self.total
    }

    /// Total number of lines in the source.
    pub fn total_lines(&self) -> usize {
        self.total
    }

    /// Advance the cursor by one line.
    pub fn advance(&mut self) {
        self.line += 1;
    }

    /// Skip blank (whitespace-only) lines starting at the current position.
    pub fn skip_blanks(&mut self) {
        while !self.is_eof() && self.current_line_text().trim().is_empty() {
            self.line += 1;
        }
    }

    // ── Current-line helpers ────────────────────────────────────────

    /// Text of the current line (without trailing newline).
    pub fn current_line_text(&self) -> &'a str {
        self.line_text(self.line)
    }

    /// Trimmed text of the current line.
    pub fn current_trimmed(&self) -> &'a str {
        self.current_line_text().trim()
    }

    /// Leading-whitespace byte count of the current line.
    pub fn current_indent(&self) -> usize {
        indent_len(self.current_line_text())
    }

    /// Visual column width of leading whitespace on the current line.
    ///
    /// Expands tabs to 4-column stops.  Use this for indentation-level
    /// comparison; use [`current_indent`](Self::current_indent) when you
    /// need a byte offset.
    pub fn current_indent_columns(&self) -> usize {
        indent_columns(self.current_line_text())
    }

    /// A [`TextRange`] spanning the trimmed (non-whitespace) content of
    /// the current line.
    ///
    /// Equivalent to
    /// `make_line_range(line, current_indent(), current_trimmed().len())`.
    pub fn current_trimmed_range(&self) -> TextRange {
        self.make_line_range(self.line, self.current_indent(), self.current_trimmed().len())
    }

    // ── Arbitrary-line helpers ──────────────────────────────────────

    /// Text of line `idx` (without trailing newline).
    ///
    /// Returns `""` if `idx` is out of bounds.
    pub fn line_text(&self, idx: usize) -> &'a str {
        if idx >= self.offsets.len() {
            return "";
        }
        let start = self.offsets[idx];
        let end = if idx + 1 < self.offsets.len() {
            self.offsets[idx + 1].saturating_sub(1)
        } else {
            self.source.len()
        };
        if start >= self.source.len() {
            return "";
        }
        &self.source[start..end]
    }

    // ── Span construction ──────────────────────────────────────────

    /// Build a [`TextRange`] from (line, col) pairs.
    pub fn make_range(&self, start_line: usize, start_col: usize, end_line: usize, end_col: usize) -> TextRange {
        TextRange::new(
            TextSize::new((self.offsets[start_line] + start_col) as u32),
            TextSize::new((self.offsets[end_line] + end_col) as u32),
        )
    }

    /// Build a [`TextRange`] spanning `len` bytes on a single line.
    ///
    /// Equivalent to `make_range(line, col, line, col + len)`.
    pub fn make_line_range(&self, line: usize, col: usize, len: usize) -> TextRange {
        let start = self.offsets[line] + col;
        TextRange::from_offset_len(start, len)
    }

    /// Build a [`TextRange`] from a starting offset to the end of the
    /// last non-blank line before the current cursor position.
    ///
    /// Walks backwards from `cursor.line - 1`, skipping trailing blank
    /// lines, to find the true content end.  The returned range starts at
    /// `start_offset` and ends at the last non-whitespace byte on the
    /// found line.
    pub fn span_back_from_cursor(&self, start_offset: usize) -> TextRange {
        let (start_line, start_col) = self.offset_to_line_col(start_offset);
        let mut end_line = self.line.saturating_sub(1);
        while end_line > start_line {
            if !self.line_text(end_line).trim().is_empty() {
                break;
            }
            end_line -= 1;
        }
        let end_text = self.line_text(end_line);
        let end_col = indent_len(end_text) + end_text.trim().len();
        self.make_range(start_line, start_col, end_line, end_col)
    }

    // ── Offset utilities ───────────────────────────────────────────

    /// Convert a byte offset to `(line, col)`.
    pub fn offset_to_line_col(&self, offset: usize) -> (usize, usize) {
        let line = self.offsets.partition_point(|&o| o <= offset).saturating_sub(1);
        let col = offset - self.offsets[line];
        (line, col)
    }

    /// Byte offset of `inner` within the source string.
    ///
    /// Both `inner` and the source must point into the same allocation.
    pub fn substr_offset(&self, inner: &str) -> usize {
        inner.as_ptr() as usize - self.source.as_ptr() as usize
    }
}

// =============================================================================
// Standalone helpers (still useful outside LineCursor)
// =============================================================================

/// Number of leading whitespace bytes in `line`.
///
/// Use this for **byte-offset** calculations (e.g. column parameters to
/// [`LineCursor::make_range`]).  For indentation-level *comparison*, prefer
/// [`indent_columns`] which handles tab characters correctly.
pub(crate) fn indent_len(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Visual column width of leading whitespace in `line`.
///
/// Each tab character advances the column to the next multiple of
/// `TAB_WIDTH` (4), matching the most common Python editor convention.
/// Regular spaces count as one column each.
///
/// Use this for indentation-level *comparison* (not byte offsets).
pub(crate) fn indent_columns(line: &str) -> usize {
    const TAB_WIDTH: usize = 4;
    let mut col = 0;
    for byte in line.bytes() {
        match byte {
            b'\t' => col = (col / TAB_WIDTH + 1) * TAB_WIDTH,
            b' ' => col += 1,
            _ => break,
        }
    }
    col
}

// =============================================================================
// Internal helpers
// =============================================================================

/// Build a table mapping each line index to its starting byte offset.
fn build_line_offsets(input: &str) -> Vec<usize> {
    let mut offsets = vec![0usize];
    for (i, byte) in input.bytes().enumerate() {
        if byte == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Number of text lines in source.
fn count_lines(source: &str, offsets: &[usize]) -> usize {
    if source.is_empty() {
        0
    } else if source.ends_with('\n') {
        offsets.len() - 1
    } else {
        offsets.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indent_columns_spaces_only() {
        assert_eq!(indent_columns("    hello"), 4);
        assert_eq!(indent_columns("hello"), 0);
        assert_eq!(indent_columns("  x"), 2);
    }

    #[test]
    fn test_indent_columns_tab() {
        // A single tab at the start → next multiple of 4 → 4
        assert_eq!(indent_columns("\thello"), 4);
        // Two tabs → 8
        assert_eq!(indent_columns("\t\thello"), 8);
    }

    #[test]
    fn test_indent_columns_mixed_tab_space() {
        // 2 spaces then tab → col=2, tab → next mult of 4 → 4
        assert_eq!(indent_columns("  \thello"), 4);
        // 3 spaces then tab → col=3, tab → next mult of 4 → 4
        assert_eq!(indent_columns("   \thello"), 4);
        // tab then 2 spaces → col=4, +2 → 6
        assert_eq!(indent_columns("\t  hello"), 6);
    }

    #[test]
    fn test_indent_len_unchanged() {
        // indent_len counts bytes, not visual columns
        assert_eq!(indent_len("\thello"), 1);
        assert_eq!(indent_len("    hello"), 4);
        assert_eq!(indent_len("  \thello"), 3);
    }
}
