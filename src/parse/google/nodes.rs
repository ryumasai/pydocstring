//! Typed wrappers for Google-style syntax nodes.
//!
//! Each wrapper pairs a `&SyntaxNode` with the [`Parsed`] result it came
//! from and provides typed accessors for the node's children (tokens and
//! sub-nodes). Token accessors return [`TokenRef`]s, so no accessor takes a
//! `source` argument: `arg.name().text()`.

use crate::parse::EntryRole;
use crate::parse::google::kind::GoogleSectionKind;
use crate::parse::text_block::TextBlock;
use crate::parse::text_block::find_text_block;
use crate::parse::token_ref::TokenRef;
use crate::parse::unified::DefaultMarker;
use crate::syntax::Parsed;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;

// =============================================================================
// Macro for defining typed node wrappers
// =============================================================================

/// Define a typed node wrapper that casts from `&SyntaxNode`.
macro_rules! define_node {
    ($name:ident, $kind:ident) => {
        #[doc = concat!("Typed wrapper for `", stringify!($kind), "` syntax nodes.")]
        #[derive(Debug, Clone, Copy)]
        pub struct $name<'a> {
            pub(crate) parsed: &'a Parsed,
            pub(crate) node: &'a SyntaxNode,
        }

        impl<'a> $name<'a> {
            /// Try to cast a `SyntaxNode` from `parsed`'s tree into this
            /// typed wrapper.
            pub fn cast(parsed: &'a Parsed, node: &'a SyntaxNode) -> Option<Self> {
                (node.kind() == SyntaxKind::$kind).then(|| Self { parsed, node })
            }

            /// Access the underlying `SyntaxNode`.
            pub fn syntax(&self) -> &'a SyntaxNode {
                self.node
            }

            /// Bundle a token from this node's tree with the parse result.
            #[allow(dead_code)]
            fn token_ref(&self, token: &'a crate::syntax::SyntaxToken) -> TokenRef<'a> {
                TokenRef::new(self.parsed, token)
            }

            /// First token child of `kind` as a [`TokenRef`], if present.
            #[allow(dead_code)]
            fn find_token_ref(&self, kind: SyntaxKind) -> Option<TokenRef<'a>> {
                self.node.find_token(kind).map(|t| TokenRef::new(self.parsed, t))
            }
        }
    };
}

// =============================================================================
// GoogleDocstring
// =============================================================================

define_node!(GoogleDocstring, DOCUMENT);

impl<'a> GoogleDocstring<'a> {
    /// Brief summary block, if present.
    pub fn summary(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::SUMMARY)
    }

    /// Extended summary block, if present.
    pub fn extended_summary(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::EXTENDED_SUMMARY)
    }

    /// Deprecation node, if present: the first `DIRECTIVE` whose name is
    /// `deprecated`. Directives with other names are not deprecations and
    /// are skipped.
    pub fn deprecation(&self) -> Option<GoogleDeprecation<'a>> {
        let parsed = self.parsed;
        self.node
            .nodes(SyntaxKind::DIRECTIVE)
            .find(|n| crate::parse::utils::directive_is_deprecated(parsed, n))
            .map(|node| GoogleDeprecation { parsed, node })
    }

    /// Iterate over all section nodes.
    pub fn sections(&self) -> impl Iterator<Item = GoogleSection<'a>> {
        let parsed = self.parsed;
        self.node
            .nodes(SyntaxKind::SECTION)
            .map(move |node| GoogleSection { parsed, node })
    }

    /// Iterate over stray-prose paragraph blocks (`PARAGRAPH` nodes) between
    /// sections, in source order.
    pub fn paragraphs(&self) -> impl Iterator<Item = TextBlock<'a>> {
        let parsed = self.parsed;
        self.node
            .nodes(SyntaxKind::PARAGRAPH)
            .filter_map(move |node| TextBlock::cast(parsed, node))
    }

    /// Deprecated alias for [`GoogleDocstring::paragraphs`]: stray lines are
    /// now grouped into `PARAGRAPH` text blocks.
    #[deprecated(
        since = "0.3.0",
        note = "use `paragraphs()`; stray lines are now PARAGRAPH text blocks"
    )]
    pub fn stray_lines(&self) -> impl Iterator<Item = TextBlock<'a>> {
        self.paragraphs()
    }
}

// =============================================================================
// GoogleDeprecation
// =============================================================================

define_node!(GoogleDeprecation, DIRECTIVE);

impl<'a> GoogleDeprecation<'a> {
    /// The `..` RST directive marker.
    pub fn directive_marker(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::DIRECTIVE_MARKER)
    }

    /// The `deprecated` directive name.
    pub fn keyword(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::DIRECTIVE_NAME)
    }

    /// The `::` double-colon separator.
    pub fn double_colon(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::DOUBLE_COLON)
    }

    /// Version when deprecated.
    pub fn version(&self) -> TokenRef<'a> {
        self.token_ref(self.node.required_token(SyntaxKind::ARGUMENT))
    }

    /// Description / reason for deprecation.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleSection
// =============================================================================

define_node!(GoogleSection, SECTION);

impl<'a> GoogleSection<'a> {
    /// The section header node.
    pub fn header(&self) -> GoogleSectionHeader<'a> {
        GoogleSectionHeader {
            parsed: self.parsed,
            node: self
                .node
                .find_node(SyntaxKind::SECTION_HEADER)
                .expect("SECTION must have a SECTION_HEADER child"),
        }
    }

    /// Determine the section kind from the header name text.
    pub fn section_kind(&self) -> GoogleSectionKind {
        let name_text = self.header().name().text();
        GoogleSectionKind::from_name(&name_text.to_ascii_lowercase())
    }

    /// Iterate over the `ENTRY` children when this section's entries have
    /// `role`; empty for any other section kind.
    ///
    /// All entries share the `ENTRY` node kind, so without this guard a
    /// mismatched accessor (e.g. `args()` on a `Raises:` section) would wrap
    /// foreign entries whose typed accessors then panic in `required_token`.
    fn entries_with_role(&self, role: EntryRole) -> impl Iterator<Item = &'a SyntaxNode> {
        let matches = self.section_kind().entry_role() == role;
        self.node.nodes(SyntaxKind::ENTRY).filter(move |_| matches)
    }

    /// Iterate over arg entry nodes in this section.
    ///
    /// Empty when this is not an Args-like section (Args, Keyword Args,
    /// Other Parameters, Receives).
    pub fn args(&self) -> impl Iterator<Item = GoogleArg<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::Parameter)
            .map(move |node| GoogleArg { parsed, node })
    }

    /// Returns entry node in this section, if present.
    ///
    /// `None` when this is not a Returns section.
    pub fn returns(&self) -> Option<GoogleReturn<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::Return)
            .next()
            .map(|node| GoogleReturn { parsed, node })
    }

    /// Yields entry node in this section, if present.
    ///
    /// `None` when this is not a Yields section.
    pub fn yields(&self) -> Option<GoogleYield<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::Yield)
            .next()
            .map(|node| GoogleYield { parsed, node })
    }

    /// Iterate over exception entry nodes.
    ///
    /// Empty when this is not a Raises section.
    pub fn exceptions(&self) -> impl Iterator<Item = GoogleException<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::Exception)
            .map(move |node| GoogleException { parsed, node })
    }

    /// Iterate over warning entry nodes.
    ///
    /// Empty when this is not a Warns section.
    pub fn warnings(&self) -> impl Iterator<Item = GoogleWarning<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::Warning)
            .map(move |node| GoogleWarning { parsed, node })
    }

    /// Iterate over see-also item nodes.
    ///
    /// Empty when this is not a See Also section.
    pub fn see_also_items(&self) -> impl Iterator<Item = GoogleSeeAlsoItem<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::SeeAlsoItem)
            .map(move |node| GoogleSeeAlsoItem { parsed, node })
    }

    /// Iterate over reference nodes.
    ///
    /// `CITATION` nodes only occur in References sections, so no section-kind
    /// guard is needed: other sections have no such children.
    pub fn references(&self) -> impl Iterator<Item = GoogleReference<'a>> {
        let parsed = self.parsed;
        self.node
            .nodes(SyntaxKind::CITATION)
            .map(move |node| GoogleReference { parsed, node })
    }

    /// Iterate over attribute entry nodes.
    ///
    /// Empty when this is not an Attributes section.
    pub fn attributes(&self) -> impl Iterator<Item = GoogleAttribute<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::Attribute)
            .map(move |node| GoogleAttribute { parsed, node })
    }

    /// Iterate over method entry nodes.
    ///
    /// Empty when this is not a Methods section.
    pub fn methods(&self) -> impl Iterator<Item = GoogleMethod<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::Method)
            .map(move |node| GoogleMethod { parsed, node })
    }

    /// Free-text body content block, if this is a free-text section.
    pub fn body_text(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleSectionHeader
// =============================================================================

define_node!(GoogleSectionHeader, SECTION_HEADER);

impl<'a> GoogleSectionHeader<'a> {
    /// Section name token (e.g. "Args", "Returns").
    pub fn name(&self) -> TokenRef<'a> {
        self.token_ref(self.node.required_token(SyntaxKind::NAME))
    }

    /// Colon token, if present.
    pub fn colon(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::COLON)
    }
}

// =============================================================================
// GoogleArg
// =============================================================================

define_node!(GoogleArg, ENTRY);

impl<'a> GoogleArg<'a> {
    /// Argument name token.
    ///
    /// When the entry declares several comma-separated names, this returns
    /// the first one; use [`GoogleArg::names`] to access all of them.
    pub fn name(&self) -> TokenRef<'a> {
        self.token_ref(self.node.required_token(SyntaxKind::NAME))
    }

    /// All argument name tokens (can be multiple, e.g. `x1, x2`).
    pub fn names(&self) -> impl Iterator<Item = TokenRef<'a>> {
        let parsed = self.parsed;
        self.node
            .tokens(SyntaxKind::NAME)
            .map(move |t| TokenRef::new(parsed, t))
    }

    /// Opening bracket token (e.g. `(`), if present.
    pub fn open_bracket(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::OPEN_BRACKET)
    }

    /// Type annotation token, if present.
    pub fn type_annotation(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::TYPE)
    }

    /// Deprecated alias for [`GoogleArg::type_annotation`].
    #[deprecated(since = "0.3.0", note = "renamed to type_annotation")]
    pub fn r#type(&self) -> Option<TokenRef<'a>> {
        self.type_annotation()
    }

    /// Closing bracket token (e.g. `)`), if present.
    pub fn close_bracket(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::CLOSE_BRACKET)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }

    /// Whether the entry carries at least one `optional` marker.
    pub fn is_optional(&self) -> bool {
        self.optionals().next().is_some()
    }

    /// First `optional` marker token, if present.
    ///
    /// Markers are repeatable; use [`GoogleArg::optionals`] to see every
    /// occurrence, or [`GoogleArg::is_optional`] for the boolean question.
    pub fn optional_marker(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::OPTIONAL)
    }

    /// Deprecated alias for [`GoogleArg::optional_marker`].
    #[deprecated(since = "0.3.0", note = "renamed to optional_marker; see also is_optional")]
    pub fn optional(&self) -> Option<TokenRef<'a>> {
        self.optional_marker()
    }

    /// All `optional` marker tokens, one per occurrence, in source order.
    pub fn optionals(&self) -> impl Iterator<Item = TokenRef<'a>> {
        let parsed = self.parsed;
        self.node
            .tokens(SyntaxKind::OPTIONAL)
            .map(move |t| TokenRef::new(parsed, t))
    }

    /// All `default …` markers, one [`DefaultMarker`] per occurrence, in source
    /// order.
    pub fn defaults(&self) -> impl Iterator<Item = DefaultMarker<'a>> {
        let parsed = self.parsed;
        self.node
            .nodes(SyntaxKind::DEFAULT)
            .filter_map(move |node| DefaultMarker::cast(parsed, node))
    }

    /// The first `default …` marker's keyword token, if present.
    pub fn default_keyword(&self) -> Option<TokenRef<'a>> {
        self.defaults().next().map(|d| d.keyword())
    }

    /// The first `default …` marker's separator token (`=` or `:`), if
    /// present.
    pub fn default_separator(&self) -> Option<TokenRef<'a>> {
        self.defaults().next().and_then(|d| d.separator())
    }

    /// The first `default …` marker's value token, if present.
    ///
    /// First occurrence wins — the same normalization rule the model layer
    /// applies. Use [`GoogleArg::defaults`] to see every occurrence.
    pub fn default_value(&self) -> Option<TokenRef<'a>> {
        self.defaults().next().and_then(|d| d.value())
    }
}

// =============================================================================
// GoogleReturn
// =============================================================================

define_node!(GoogleReturn, ENTRY);

impl<'a> GoogleReturn<'a> {
    /// Return type annotation token, if present.
    pub fn type_annotation(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::TYPE)
    }

    /// Deprecated alias for [`GoogleReturn::type_annotation`].
    #[deprecated(since = "0.3.0", note = "renamed to type_annotation")]
    pub fn return_type(&self) -> Option<TokenRef<'a>> {
        self.type_annotation()
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleYield
// =============================================================================

define_node!(GoogleYield, ENTRY);

impl<'a> GoogleYield<'a> {
    /// Yield type annotation token, if present.
    pub fn type_annotation(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::TYPE)
    }

    /// Deprecated alias for [`GoogleYield::type_annotation`].
    #[deprecated(since = "0.3.0", note = "renamed to type_annotation")]
    pub fn return_type(&self) -> Option<TokenRef<'a>> {
        self.type_annotation()
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleException
// =============================================================================

define_node!(GoogleException, ENTRY);

impl<'a> GoogleException<'a> {
    /// Exception type name token.
    pub fn type_annotation(&self) -> TokenRef<'a> {
        self.token_ref(self.node.required_token(SyntaxKind::TYPE))
    }

    /// Deprecated alias for [`GoogleException::type_annotation`].
    #[deprecated(since = "0.3.0", note = "renamed to type_annotation")]
    pub fn r#type(&self) -> TokenRef<'a> {
        self.type_annotation()
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleWarning
// =============================================================================

define_node!(GoogleWarning, ENTRY);

impl<'a> GoogleWarning<'a> {
    /// Warning type name token (e.g. `UserWarning`).
    pub fn type_annotation(&self) -> TokenRef<'a> {
        self.token_ref(self.node.required_token(SyntaxKind::TYPE))
    }

    /// Deprecated alias for [`GoogleWarning::type_annotation`].
    #[deprecated(since = "0.3.0", note = "renamed to type_annotation")]
    pub fn warning_type(&self) -> TokenRef<'a> {
        self.type_annotation()
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleSeeAlsoItem
// =============================================================================

define_node!(GoogleSeeAlsoItem, ENTRY);

impl<'a> GoogleSeeAlsoItem<'a> {
    /// All name tokens (can be multiple, e.g. `func_a, func_b`).
    pub fn names(&self) -> impl Iterator<Item = TokenRef<'a>> {
        let parsed = self.parsed;
        self.node
            .tokens(SyntaxKind::NAME)
            .map(move |t| TokenRef::new(parsed, t))
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleReference
// =============================================================================

define_node!(GoogleReference, CITATION);

impl<'a> GoogleReference<'a> {
    /// RST directive marker (`..`), if present.
    pub fn directive_marker(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::DIRECTIVE_MARKER)
    }

    /// Opening bracket token, if present.
    pub fn open_bracket(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::OPEN_BRACKET)
    }

    /// Citation label token (`1`, `CIT2002`, `#f1`, …), if present.
    pub fn label(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::LABEL)
    }

    /// Deprecated alias for [`GoogleReference::label`].
    #[deprecated(since = "0.3.0", note = "renamed to label")]
    pub fn number(&self) -> Option<TokenRef<'a>> {
        self.label()
    }

    /// Closing bracket token, if present.
    pub fn close_bracket(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::CLOSE_BRACKET)
    }

    /// Reference content text block, if present.
    pub fn content(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleAttribute
// =============================================================================

define_node!(GoogleAttribute, ENTRY);

impl<'a> GoogleAttribute<'a> {
    /// Attribute name token.
    ///
    /// When the entry declares several comma-separated names, this returns
    /// the first one; use [`GoogleAttribute::names`] to access all of them.
    pub fn name(&self) -> TokenRef<'a> {
        self.token_ref(self.node.required_token(SyntaxKind::NAME))
    }

    /// All attribute name tokens (can be multiple, e.g. `jac, hess`).
    pub fn names(&self) -> impl Iterator<Item = TokenRef<'a>> {
        let parsed = self.parsed;
        self.node
            .tokens(SyntaxKind::NAME)
            .map(move |t| TokenRef::new(parsed, t))
    }

    /// Opening bracket token (e.g. `(`), if present.
    pub fn open_bracket(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::OPEN_BRACKET)
    }

    /// Type annotation token, if present.
    pub fn type_annotation(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::TYPE)
    }

    /// Deprecated alias for [`GoogleAttribute::type_annotation`].
    #[deprecated(since = "0.3.0", note = "renamed to type_annotation")]
    pub fn r#type(&self) -> Option<TokenRef<'a>> {
        self.type_annotation()
    }

    /// Closing bracket token (e.g. `)`), if present.
    pub fn close_bracket(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::CLOSE_BRACKET)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleMethod
// =============================================================================

define_node!(GoogleMethod, ENTRY);

impl<'a> GoogleMethod<'a> {
    /// Method name token.
    pub fn name(&self) -> TokenRef<'a> {
        self.token_ref(self.node.required_token(SyntaxKind::NAME))
    }

    /// Opening bracket token (e.g. `(`), if present.
    pub fn open_bracket(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::OPEN_BRACKET)
    }

    /// Type annotation token, if present.
    pub fn type_annotation(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::TYPE)
    }

    /// Deprecated alias for [`GoogleMethod::type_annotation`].
    #[deprecated(since = "0.3.0", note = "renamed to type_annotation")]
    pub fn r#type(&self) -> Option<TokenRef<'a>> {
        self.type_annotation()
    }

    /// Closing bracket token (e.g. `)`), if present.
    pub fn close_bracket(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::CLOSE_BRACKET)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}
