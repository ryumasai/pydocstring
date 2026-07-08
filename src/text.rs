//! Source location types (offset-only).
//!
//! This module provides [`TextSize`] (a byte offset) and [`TextRange`]
//! (a half-open byte range) for tracking source positions.
//! Inspired by ruff / rust-analyzer's `text-size` crate.

use core::fmt;
use core::ops;

// =============================================================================
// TextSize
// =============================================================================

/// A byte offset in the source text.
///
/// Newtype over `u32` for type safety (prevents mixing with line numbers, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TextSize(u32);

impl TextSize {
    /// Creates a new text size from a raw byte offset.
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw byte offset.
    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl From<u32> for TextSize {
    fn from(raw: u32) -> Self {
        Self(raw)
    }
}

impl From<TextSize> for u32 {
    fn from(size: TextSize) -> Self {
        size.0
    }
}

impl From<TextSize> for usize {
    fn from(size: TextSize) -> Self {
        size.0 as usize
    }
}

impl From<usize> for TextSize {
    fn from(raw: usize) -> Self {
        Self(raw as u32)
    }
}

impl ops::Add for TextSize {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl ops::Sub for TextSize {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl fmt::Display for TextSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// =============================================================================
// TextRange
// =============================================================================

/// A range in the source text `[start, end)`, represented as byte offsets.
///
/// Stores only offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextRange {
    start: TextSize,
    end: TextSize,
}

impl TextRange {
    /// Creates a new range from start (inclusive) and end (exclusive) offsets.
    pub const fn new(start: TextSize, end: TextSize) -> Self {
        Self { start, end }
    }

    /// Start offset (inclusive).
    pub const fn start(self) -> TextSize {
        self.start
    }

    /// End offset (exclusive).
    pub const fn end(self) -> TextSize {
        self.end
    }

    /// Length of the range in bytes.
    pub const fn len(self) -> TextSize {
        TextSize::new(self.end.0 - self.start.0)
    }

    /// Whether the range is empty.
    pub const fn is_empty(self) -> bool {
        self.start.0 == self.end.0
    }

    /// Whether `offset` is contained in this range.
    pub const fn contains(self, offset: TextSize) -> bool {
        self.start.0 <= offset.0 && offset.0 < self.end.0
    }

    /// Creates a range from an absolute byte offset and a length.
    pub const fn from_offset_len(offset: usize, len: usize) -> Self {
        Self {
            start: TextSize::new(offset as u32),
            end: TextSize::new((offset + len) as u32),
        }
    }

    /// Extracts the corresponding slice from the source text.
    ///
    /// Returns an empty string if the range is empty or offsets are out of bounds.
    pub fn source_text<'a>(&self, source: &'a str) -> &'a str {
        let start = self.start.0 as usize;
        let end = self.end.0 as usize;
        if start <= end && end <= source.len() {
            &source[start..end]
        } else {
            ""
        }
    }

    /// Grow this range's end to cover `other`.
    ///
    /// The end only ever moves forward: if `other` ends before this range
    /// does, the range is left unchanged (this can never shrink a range).
    /// The start is not touched.
    pub(crate) fn extend(&mut self, other: TextRange) {
        if other.end > self.end {
            self.end = other.end;
        }
    }
}

impl fmt::Display for TextRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

// =============================================================================
// LineColumn
// =============================================================================

/// A line/column position in the source text.
///
/// `lineno` is 1-based; `col` is the 0-based byte offset from the start of
/// the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineColumn {
    /// 1-based line number.
    pub lineno: u32,
    /// 0-based byte column offset from the start of the line.
    pub col: u32,
}

impl fmt::Display for LineColumn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.lineno, self.col)
    }
}

// =============================================================================
// LineIndex
// =============================================================================

/// A lookup table for converting byte offsets to [`LineColumn`] positions.
///
/// Build once from the source text with [`LineIndex::new`], then call
/// [`LineIndex::line_col`] for any [`TextSize`] offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    /// Byte offset of the first character of each line.
    /// `line_starts[0]` is always 0 (start of the first line).
    line_starts: Vec<u32>,
}

impl LineIndex {
    /// Build a `LineIndex` from the source text.
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0u32];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self { line_starts }
    }

    /// Convert a byte offset to a [`LineColumn`] position.
    ///
    /// `lineno` is 1-based; `col` is the 0-based byte offset within the line.
    pub fn line_col(&self, offset: TextSize) -> LineColumn {
        let offset = offset.raw();
        // The index of the last line that starts at or before `offset`.
        let line = self.line_starts.partition_point(|&s| s <= offset) - 1;
        let col = offset - self.line_starts[line];
        LineColumn {
            lineno: line as u32 + 1,
            col,
        }
    }
}
