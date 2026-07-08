//! Typed wrapper for the plain-style docstring root node.

use crate::parse::text_block::TextBlock;
use crate::parse::text_block::find_text_block;
use crate::syntax::Parsed;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;

// =============================================================================
// PlainDocstring
// =============================================================================

/// Typed wrapper for plain-style [`SyntaxKind::DOCUMENT`] nodes.
#[derive(Debug, Clone, Copy)]
pub struct PlainDocstring<'a> {
    pub(crate) parsed: &'a Parsed,
    pub(crate) node: &'a SyntaxNode,
}

impl<'a> PlainDocstring<'a> {
    /// Try to cast a `SyntaxNode` from `parsed`'s tree into this typed
    /// wrapper.
    pub fn cast(parsed: &'a Parsed, node: &'a SyntaxNode) -> Option<Self> {
        (node.kind() == SyntaxKind::DOCUMENT).then_some(Self { parsed, node })
    }

    /// Access the underlying `SyntaxNode`.
    pub fn syntax(&self) -> &'a SyntaxNode {
        self.node
    }

    /// Brief summary block, if present.
    pub fn summary(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::SUMMARY)
    }

    /// Extended summary block, if present.
    pub fn extended_summary(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::EXTENDED_SUMMARY)
    }
}
