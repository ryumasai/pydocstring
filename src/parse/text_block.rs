//! Typed wrapper for multi-line text block nodes.
//!
//! Five [`SyntaxKind`]s carry free-form text content: [`SyntaxKind::SUMMARY`],
//! [`SyntaxKind::EXTENDED_SUMMARY`], [`SyntaxKind::DESCRIPTION`],
//! [`SyntaxKind::BODY_TEXT`], and [`SyntaxKind::CONTENT`]. Each is a node
//! that wraps one [`SyntaxKind::TEXT_LINE`] token per content line; the
//! interior layout bytes (indentation, newlines, paragraph-break blank
//! lines) are filled in as trivia tokens by the post-parse trivia pass.
//!
//! [`TextBlock`] is the shared typed wrapper over any of these nodes.

use crate::parse::utils::convert_multiline_with_indentation;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::syntax::SyntaxToken;
use crate::text::TextRange;

/// Typed wrapper for a multi-line text block node.
///
/// Covers the five text-content kinds: `SUMMARY`, `EXTENDED_SUMMARY`,
/// `DESCRIPTION`, `BODY_TEXT`, and `CONTENT`. A single-line block still
/// wraps exactly one [`SyntaxKind::TEXT_LINE`] token (uniformity).
#[derive(Debug)]
pub struct TextBlock<'a>(pub(crate) &'a SyntaxNode);

impl<'a> TextBlock<'a> {
    /// Whether `kind` is one of the text block node kinds.
    pub const fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::SUMMARY
                | SyntaxKind::EXTENDED_SUMMARY
                | SyntaxKind::DESCRIPTION
                | SyntaxKind::BODY_TEXT
                | SyntaxKind::CONTENT
        )
    }

    /// Try to cast a `SyntaxNode` reference into this typed wrapper.
    pub fn cast(node: &'a SyntaxNode) -> Option<Self> {
        Self::can_cast(node.kind()).then_some(Self(node))
    }

    /// Access the underlying `SyntaxNode`.
    pub fn syntax(&self) -> &'a SyntaxNode {
        self.0
    }

    /// The source range of the block: from the start of the first content
    /// line's text to the end of the last content line's text.
    pub fn range(&self) -> &'a TextRange {
        self.0.range()
    }

    /// Whether this block is a zero-length placeholder inserted by the
    /// parser for a syntactically missing element (e.g. the description in
    /// `arg (int):`).
    pub fn is_missing(&self) -> bool {
        self.0.range().is_empty()
    }

    /// Iterate over the per-line [`SyntaxKind::TEXT_LINE`] tokens.
    pub fn lines(&self) -> impl Iterator<Item = &'a SyntaxToken> {
        self.0.tokens(SyntaxKind::TEXT_LINE)
    }

    /// The raw source slice of the block's range, byte-identical to the
    /// source text between the first and last content line (interior
    /// indentation and newlines included).
    pub fn text(&self, source: &'a str) -> &'a str {
        self.0.range().source_text(source)
    }

    /// The logical text of the block: continuation lines dedented by the
    /// common indentation and joined with `\n`.
    pub fn logical_text(&self, source: &str) -> String {
        convert_multiline_with_indentation(self.text(source))
    }
}

/// Find the first non-missing (non-empty) text block child of `kind`.
///
/// Mirrors [`SyntaxNode::find_token`]'s exclusion of zero-length
/// placeholders, so typed accessors keep their pre-#38 semantics.
pub(crate) fn find_text_block<'a>(node: &'a SyntaxNode, kind: SyntaxKind) -> Option<TextBlock<'a>> {
    node.nodes(kind)
        .find(|n| !n.range().is_empty())
        .and_then(TextBlock::cast)
}
