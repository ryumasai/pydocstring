//! Typed wrapper for multi-line text block nodes.
//!
//! Four [`SyntaxKind`]s carry free-form text content: [`SyntaxKind::SUMMARY`],
//! [`SyntaxKind::EXTENDED_SUMMARY`], [`SyntaxKind::DESCRIPTION`], and
//! [`SyntaxKind::PARAGRAPH`]. Each is a node
//! that wraps one [`SyntaxKind::TEXT_LINE`] token per content line; the
//! interior layout bytes (indentation, newlines, paragraph-break blank
//! lines) are filled in as trivia tokens by the post-parse trivia pass.
//!
//! [`TextBlock`] is the shared typed wrapper over any of these nodes.

use crate::parse::token_ref::TokenRef;
use crate::parse::utils::convert_multiline_with_indentation;
use crate::syntax::Parsed;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::text::TextRange;

/// Typed wrapper for a multi-line text block node.
///
/// Covers the four text-content kinds: `SUMMARY`, `EXTENDED_SUMMARY`,
/// `DESCRIPTION`, and `PARAGRAPH`. A single-line block still
/// wraps exactly one [`SyntaxKind::TEXT_LINE`] token (uniformity).
///
/// Holds a reference to the [`Parsed`] result it came from, so text access
/// needs no `source` argument ([`TextBlock::text`],
/// [`TextBlock::logical_text`]).
#[derive(Debug, Clone, Copy)]
pub struct TextBlock<'a> {
    pub(crate) parsed: &'a Parsed,
    pub(crate) node: &'a SyntaxNode,
}

impl<'a> TextBlock<'a> {
    /// Whether `kind` is one of the text block node kinds.
    pub const fn can_cast(kind: SyntaxKind) -> bool {
        matches!(
            kind,
            SyntaxKind::SUMMARY | SyntaxKind::EXTENDED_SUMMARY | SyntaxKind::DESCRIPTION | SyntaxKind::PARAGRAPH
        )
    }

    /// Try to cast a `SyntaxNode` from `parsed`'s tree into this typed
    /// wrapper.
    pub fn cast(parsed: &'a Parsed, node: &'a SyntaxNode) -> Option<Self> {
        Self::can_cast(node.kind()).then_some(Self { parsed, node })
    }

    /// Access the underlying `SyntaxNode`.
    pub fn syntax(&self) -> &'a SyntaxNode {
        self.node
    }

    /// The source range of the block: from the start of the first content
    /// line's text to the end of the last content line's text.
    pub fn range(&self) -> TextRange {
        self.node.range()
    }

    /// Whether this block is a zero-length placeholder inserted by the
    /// parser for a syntactically missing element (e.g. the description in
    /// `arg (int):`).
    pub fn is_missing(&self) -> bool {
        self.node.range().is_empty()
    }

    /// Iterate over the per-line [`SyntaxKind::TEXT_LINE`] tokens.
    pub fn lines(&self) -> impl Iterator<Item = TokenRef<'a>> {
        let parsed = self.parsed;
        self.node
            .tokens(SyntaxKind::TEXT_LINE)
            .map(move |t| TokenRef::new(parsed, t))
    }

    /// The raw source slice of the block's range, byte-identical to the
    /// source text between the first and last content line (interior
    /// indentation and newlines included).
    pub fn text(&self) -> &'a str {
        self.node.range().source_text(self.parsed.source())
    }

    /// The logical text of the block: continuation lines dedented by the
    /// common indentation and joined with `\n`.
    pub fn logical_text(&self) -> String {
        convert_multiline_with_indentation(self.text())
    }
}

/// Find the first non-missing (non-empty) text block child of `kind`.
///
/// Mirrors [`SyntaxNode::find_token`]'s exclusion of zero-length
/// placeholders, so typed accessors keep their pre-#38 semantics.
pub(crate) fn find_text_block<'a>(parsed: &'a Parsed, node: &'a SyntaxNode, kind: SyntaxKind) -> Option<TextBlock<'a>> {
    node.nodes(kind)
        .find(|n| !n.range().is_empty())
        .and_then(|n| TextBlock::cast(parsed, n))
}
