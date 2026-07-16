//! The model reading layer (#148): style-neutral typed wrappers over the
//! nodes `to_model` converts.
//!
//! The CST carries no per-style structure — `ENTRY`, `CITATION` and the text
//! blocks have the same shape whichever parser built them — so one set of
//! wrappers reads all styles. The single style-sensitive question, which
//! alias table resolves a section header name, is answered from
//! [`Parsed::style`] inside [`SectionNode::section_kind`].
//!
//! This replaces the three per-style `nodes.rs` twins, whose surface was far
//! wider than `to_model` used (a `dead_code` allow apologized for it). The
//! public read lenses are `parse::unified` and the raw CST; these wrappers
//! carry exactly what `to_model` reads and nothing else.

use crate::parse::Style;
use crate::parse::kind::SectionName;
use crate::parse::text_block::TextBlock;
use crate::parse::text_block::find_text_block;
use crate::parse::token_ref::TokenRef;
use crate::parse::unified::DefaultMarker;
use crate::syntax::Parsed;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;

/// Define a typed node wrapper over a `(Parsed, SyntaxNode)` pair.
macro_rules! define_node {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy)]
        pub(crate) struct $name<'a> {
            pub(crate) parsed: &'a Parsed,
            pub(crate) node: &'a SyntaxNode,
        }

        impl<'a> $name<'a> {
            /// First token child of `kind` as a [`TokenRef`], if present.
            #[allow(dead_code)]
            fn find_token_ref(&self, kind: SyntaxKind) -> Option<TokenRef<'a>> {
                self.node.find_token(kind).map(|t| TokenRef::new(self.parsed, t))
            }

            /// All token children of `kind` as [`TokenRef`]s.
            #[allow(dead_code)]
            fn token_refs(&self, kind: SyntaxKind) -> impl Iterator<Item = TokenRef<'a>> {
                let parsed = self.parsed;
                self.node.tokens(kind).map(move |t| TokenRef::new(parsed, t))
            }
        }
    };
}

// ─── Document ────────────────────────────────────────────────────────────────

define_node!(DocNode);

impl<'a> DocNode<'a> {
    /// Wrap `parsed`'s root `DOCUMENT` node.
    pub(crate) fn root(parsed: &'a Parsed) -> Self {
        Self {
            parsed,
            node: parsed.root(),
        }
    }

    /// Brief summary block, if present.
    pub(crate) fn summary(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::SUMMARY)
    }

    /// Extended summary block, if present.
    pub(crate) fn extended_summary(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::EXTENDED_SUMMARY)
    }

    /// Iterate over section nodes (none in a plain-parsed document).
    pub(crate) fn sections(&self) -> impl Iterator<Item = SectionNode<'a>> {
        let parsed = self.parsed;
        self.node
            .nodes(SyntaxKind::SECTION)
            .map(move |node| SectionNode { parsed, node })
    }
}

// ─── Section ─────────────────────────────────────────────────────────────────

define_node!(SectionNode);

impl<'a> SectionNode<'a> {
    /// The section header's name text, as written.
    pub(crate) fn header_name(&self) -> &'a str {
        let header = self
            .node
            .find_node(SyntaxKind::SECTION_HEADER)
            .expect("SECTION must have a SECTION_HEADER child");
        TokenRef::new(self.parsed, header.required_token(SyntaxKind::NAME)).text()
    }

    /// Resolve the section kind from the header name, through the alias
    /// table of the style this document was parsed as.
    pub(crate) fn section_kind(&self) -> SectionName {
        let lower = self.header_name().to_ascii_lowercase();
        match self.parsed.style() {
            Style::NumPy => SectionName::from_numpy_name(&lower),
            _ => SectionName::from_google_name(&lower),
        }
    }

    /// Access the underlying `SyntaxNode`.
    pub(crate) fn syntax(&self) -> &'a SyntaxNode {
        self.node
    }
}

// ─── Entries ─────────────────────────────────────────────────────────────────

define_node!(ParameterNode);

impl<'a> ParameterNode<'a> {
    /// All name tokens (can be multiple, e.g. `x1, x2`).
    pub(crate) fn names(&self) -> impl Iterator<Item = TokenRef<'a>> {
        self.token_refs(SyntaxKind::NAME)
    }

    /// Type annotation token, if present.
    pub(crate) fn type_annotation(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::TYPE)
    }

    /// Description text block, if present.
    pub(crate) fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }

    /// Whether the entry carries at least one `optional` marker.
    pub(crate) fn is_optional(&self) -> bool {
        self.find_token_ref(SyntaxKind::OPTIONAL).is_some()
    }

    /// The first `default …` marker's value token, if present (first
    /// occurrence wins — the model layer's normalization rule).
    pub(crate) fn default_value(&self) -> Option<TokenRef<'a>> {
        let parsed = self.parsed;
        self.node
            .nodes(SyntaxKind::DEFAULT)
            .filter_map(|node| DefaultMarker::cast(parsed, node))
            .next()
            .and_then(|d| d.value())
    }
}

define_node!(ReturnNode);

impl<'a> ReturnNode<'a> {
    /// Return name token, if present (`name : type`, NumPy only — a
    /// Google-parsed return entry never carries a `NAME` token).
    pub(crate) fn name(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::NAME)
    }

    /// Type annotation token, if present.
    pub(crate) fn type_annotation(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::TYPE)
    }

    /// Description text block, if present.
    pub(crate) fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

define_node!(ExceptionNode);

impl<'a> ExceptionNode<'a> {
    /// Exception / warning type name token.
    pub(crate) fn type_annotation(&self) -> TokenRef<'a> {
        TokenRef::new(self.parsed, self.node.required_token(SyntaxKind::TYPE))
    }

    /// Description text block, if present.
    pub(crate) fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

define_node!(SeeAlsoNode);

impl<'a> SeeAlsoNode<'a> {
    /// All name tokens (can be multiple, e.g. `func_a, func_b`).
    pub(crate) fn names(&self) -> impl Iterator<Item = TokenRef<'a>> {
        self.token_refs(SyntaxKind::NAME)
    }

    /// Description text block, if present.
    pub(crate) fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

define_node!(MethodNode);

impl<'a> MethodNode<'a> {
    /// Method name token.
    pub(crate) fn name(&self) -> TokenRef<'a> {
        TokenRef::new(self.parsed, self.node.required_token(SyntaxKind::NAME))
    }

    /// Type annotation token, if present. (Neither parser currently emits a
    /// `TYPE` token in a method entry; read anyway so the model converter
    /// has one code path.)
    pub(crate) fn type_annotation(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::TYPE)
    }

    /// Description text block, if present.
    pub(crate) fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

define_node!(ReferenceNode);

impl<'a> ReferenceNode<'a> {
    /// Citation label token (`1`, `CIT2002`, `#f1`, …), if present.
    pub(crate) fn label(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::LABEL)
    }

    /// Reference content text block, if present.
    pub(crate) fn content(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}
