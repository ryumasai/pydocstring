//! Typed wrapper for the plain-style docstring root node.

use crate::parse::text_block::TextBlock;
use crate::parse::text_block::find_text_block;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;

// =============================================================================
// PlainDocstring
// =============================================================================

/// Typed wrapper for plain-style [`SyntaxKind::DOCUMENT`] nodes.
#[derive(Debug)]
pub struct PlainDocstring<'a>(pub(crate) &'a SyntaxNode);

impl<'a> PlainDocstring<'a> {
    /// Try to cast a `SyntaxNode` reference into this typed wrapper.
    pub fn cast(node: &'a SyntaxNode) -> Option<Self> {
        (node.kind() == SyntaxKind::DOCUMENT).then_some(Self(node))
    }

    /// Access the underlying `SyntaxNode`.
    pub fn syntax(&self) -> &'a SyntaxNode {
        self.0
    }

    /// Brief summary block, if present.
    pub fn summary(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::SUMMARY)
    }

    /// Extended summary block, if present.
    pub fn extended_summary(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::EXTENDED_SUMMARY)
    }
}
