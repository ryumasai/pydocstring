//! Typed wrapper for the plain-style docstring root node.

use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::syntax::SyntaxToken;

// =============================================================================
// PlainDocstring
// =============================================================================

/// Typed wrapper for [`SyntaxKind::PLAIN_DOCSTRING`] nodes.
#[derive(Debug)]
pub struct PlainDocstring<'a>(pub(crate) &'a SyntaxNode);

impl<'a> PlainDocstring<'a> {
    /// Try to cast a `SyntaxNode` reference into this typed wrapper.
    pub fn cast(node: &'a SyntaxNode) -> Option<Self> {
        (node.kind() == SyntaxKind::PLAIN_DOCSTRING).then_some(Self(node))
    }

    /// Access the underlying `SyntaxNode`.
    pub fn syntax(&self) -> &'a SyntaxNode {
        self.0
    }

    /// Brief summary token, if present.
    pub fn summary(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::SUMMARY)
    }

    /// Extended summary token, if present.
    pub fn extended_summary(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::EXTENDED_SUMMARY)
    }
}
