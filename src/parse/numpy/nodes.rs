//! Typed wrappers for NumPy-style syntax nodes.
//!
//! Each wrapper pairs a `&SyntaxNode` with the [`Parsed`] result it came
//! from and provides typed accessors for the node's children (tokens and
//! sub-nodes). Token accessors return [`TokenRef`]s, so no accessor takes a
//! `source` argument: `param.names().next().unwrap().text()`.

use crate::parse::EntryRole;
use crate::parse::numpy::kind::NumPySectionKind;
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
// NumPyDocstring
// =============================================================================

define_node!(NumPyDocstring, DOCUMENT);

impl<'a> NumPyDocstring<'a> {
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
    pub fn deprecation(&self) -> Option<NumPyDeprecation<'a>> {
        let parsed = self.parsed;
        self.node
            .nodes(SyntaxKind::DIRECTIVE)
            .find(|n| crate::parse::utils::directive_is_deprecated(parsed, n))
            .map(|node| NumPyDeprecation { parsed, node })
    }

    /// Iterate over all section nodes.
    pub fn sections(&self) -> impl Iterator<Item = NumPySection<'a>> {
        let parsed = self.parsed;
        self.node
            .nodes(SyntaxKind::SECTION)
            .map(move |node| NumPySection { parsed, node })
    }

    /// Iterate over stray-prose paragraph blocks (`PARAGRAPH` nodes) between
    /// sections, in source order.
    pub fn paragraphs(&self) -> impl Iterator<Item = TextBlock<'a>> {
        let parsed = self.parsed;
        self.node
            .nodes(SyntaxKind::PARAGRAPH)
            .filter_map(move |node| TextBlock::cast(parsed, node))
    }

    /// Deprecated alias for [`NumPyDocstring::paragraphs`]: stray lines are
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
// NumPySection
// =============================================================================

define_node!(NumPySection, SECTION);

impl<'a> NumPySection<'a> {
    /// The section header node.
    pub fn header(&self) -> NumPySectionHeader<'a> {
        NumPySectionHeader {
            parsed: self.parsed,
            node: self
                .node
                .find_node(SyntaxKind::SECTION_HEADER)
                .expect("SECTION must have a SECTION_HEADER child"),
        }
    }

    /// Determine the section kind from the header name text.
    pub fn section_kind(&self) -> NumPySectionKind {
        let name_text = self.header().name().text();
        NumPySectionKind::from_name(&name_text.to_ascii_lowercase())
    }

    /// Iterate over the `ENTRY` children when this section's entries have
    /// `role`; empty for any other section kind.
    ///
    /// All entries share the `ENTRY` node kind, so without this guard a
    /// mismatched accessor (e.g. `parameters()` on a Raises section) would
    /// wrap foreign entries whose typed accessors then panic in
    /// `required_token`.
    fn entries_with_role(&self, role: EntryRole) -> impl Iterator<Item = &'a SyntaxNode> {
        let matches = self.section_kind().entry_role() == role;
        self.node.nodes(SyntaxKind::ENTRY).filter(move |_| matches)
    }

    /// Iterate over parameter entry nodes.
    ///
    /// Empty when this is not a Parameters-like section (Parameters, Other
    /// Parameters, Keyword Parameters, Receives).
    pub fn parameters(&self) -> impl Iterator<Item = NumPyParameter<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::Parameter)
            .map(move |node| NumPyParameter { parsed, node })
    }

    /// Iterate over returns entry nodes.
    ///
    /// Empty when this is not a Returns section.
    pub fn returns(&self) -> impl Iterator<Item = NumPyReturns<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::Return)
            .map(move |node| NumPyReturns { parsed, node })
    }

    /// Iterate over yields entry nodes.
    ///
    /// Empty when this is not a Yields section.
    pub fn yields(&self) -> impl Iterator<Item = NumPyYields<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::Yield)
            .map(move |node| NumPyYields { parsed, node })
    }

    /// Iterate over exception entry nodes.
    ///
    /// Empty when this is not a Raises section.
    pub fn exceptions(&self) -> impl Iterator<Item = NumPyException<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::Exception)
            .map(move |node| NumPyException { parsed, node })
    }

    /// Iterate over warning entry nodes.
    ///
    /// Empty when this is not a Warns section.
    pub fn warnings(&self) -> impl Iterator<Item = NumPyWarning<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::Warning)
            .map(move |node| NumPyWarning { parsed, node })
    }

    /// Iterate over see-also item nodes.
    ///
    /// Empty when this is not a See Also section.
    pub fn see_also_items(&self) -> impl Iterator<Item = NumPySeeAlsoItem<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::SeeAlsoItem)
            .map(move |node| NumPySeeAlsoItem { parsed, node })
    }

    /// Iterate over reference nodes.
    ///
    /// `CITATION` nodes only occur in References sections, so no section-kind
    /// guard is needed: other sections have no such children.
    pub fn references(&self) -> impl Iterator<Item = NumPyReference<'a>> {
        let parsed = self.parsed;
        self.node
            .nodes(SyntaxKind::CITATION)
            .map(move |node| NumPyReference { parsed, node })
    }

    /// Iterate over attribute entry nodes.
    ///
    /// Empty when this is not an Attributes section.
    pub fn attributes(&self) -> impl Iterator<Item = NumPyAttribute<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::Attribute)
            .map(move |node| NumPyAttribute { parsed, node })
    }

    /// Iterate over method entry nodes.
    ///
    /// Empty when this is not a Methods section.
    pub fn methods(&self) -> impl Iterator<Item = NumPyMethod<'a>> {
        let parsed = self.parsed;
        self.entries_with_role(EntryRole::Method)
            .map(move |node| NumPyMethod { parsed, node })
    }

    /// Free-text body content block, if this is a free-text section.
    pub fn body_text(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPySectionHeader
// =============================================================================

define_node!(NumPySectionHeader, SECTION_HEADER);

impl<'a> NumPySectionHeader<'a> {
    /// Section name token (e.g. "Parameters", "Returns").
    pub fn name(&self) -> TokenRef<'a> {
        self.token_ref(self.node.required_token(SyntaxKind::NAME))
    }

    /// Underline token (the `----------` line).
    pub fn underline(&self) -> TokenRef<'a> {
        self.token_ref(self.node.required_token(SyntaxKind::UNDERLINE))
    }
}

// =============================================================================
// NumPyDeprecation
// =============================================================================

define_node!(NumPyDeprecation, DIRECTIVE);

impl<'a> NumPyDeprecation<'a> {
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
// NumPyParameter
// =============================================================================

define_node!(NumPyParameter, ENTRY);

impl<'a> NumPyParameter<'a> {
    /// Parameter name tokens (supports multiple names like `x1, x2`).
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

    /// Type annotation token, if present.
    pub fn type_annotation(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::TYPE)
    }

    /// Deprecated alias for [`NumPyParameter::type_annotation`].
    #[deprecated(since = "0.3.0", note = "renamed to type_annotation")]
    pub fn r#type(&self) -> Option<TokenRef<'a>> {
        self.type_annotation()
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
    /// Markers are repeatable; use [`NumPyParameter::optionals`] to see
    /// every occurrence, or [`NumPyParameter::is_optional`] for the boolean
    /// question.
    pub fn optional_marker(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::OPTIONAL)
    }

    /// Deprecated alias for [`NumPyParameter::optional_marker`].
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
    /// applies. Use [`NumPyParameter::defaults`] to see every occurrence.
    pub fn default_value(&self) -> Option<TokenRef<'a>> {
        self.defaults().next().and_then(|d| d.value())
    }
}

// =============================================================================
// NumPyReturns
// =============================================================================

define_node!(NumPyReturns, ENTRY);

impl<'a> NumPyReturns<'a> {
    /// Return name token, if present.
    pub fn name(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::NAME)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::COLON)
    }

    /// Return type annotation token, if present.
    pub fn type_annotation(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::TYPE)
    }

    /// Deprecated alias for [`NumPyReturns::type_annotation`].
    #[deprecated(since = "0.3.0", note = "renamed to type_annotation")]
    pub fn return_type(&self) -> Option<TokenRef<'a>> {
        self.type_annotation()
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPyYields
// =============================================================================

define_node!(NumPyYields, ENTRY);

impl<'a> NumPyYields<'a> {
    /// Yield name token, if present.
    pub fn name(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::NAME)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::COLON)
    }

    /// Yield type annotation token, if present.
    pub fn type_annotation(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::TYPE)
    }

    /// Deprecated alias for [`NumPyYields::type_annotation`].
    #[deprecated(since = "0.3.0", note = "renamed to type_annotation")]
    pub fn return_type(&self) -> Option<TokenRef<'a>> {
        self.type_annotation()
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPyException
// =============================================================================

define_node!(NumPyException, ENTRY);

impl<'a> NumPyException<'a> {
    /// Exception type name token.
    pub fn type_annotation(&self) -> TokenRef<'a> {
        self.token_ref(self.node.required_token(SyntaxKind::TYPE))
    }

    /// Deprecated alias for [`NumPyException::type_annotation`].
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
// NumPyWarning
// =============================================================================

define_node!(NumPyWarning, ENTRY);

impl<'a> NumPyWarning<'a> {
    /// Warning type name token.
    pub fn type_annotation(&self) -> TokenRef<'a> {
        self.token_ref(self.node.required_token(SyntaxKind::TYPE))
    }

    /// Deprecated alias for [`NumPyWarning::type_annotation`].
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
// NumPySeeAlsoItem
// =============================================================================

define_node!(NumPySeeAlsoItem, ENTRY);

impl<'a> NumPySeeAlsoItem<'a> {
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
// NumPyReference
// =============================================================================

define_node!(NumPyReference, CITATION);

impl<'a> NumPyReference<'a> {
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

    /// Deprecated alias for [`NumPyReference::label`].
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
// NumPyAttribute
// =============================================================================

define_node!(NumPyAttribute, ENTRY);

impl<'a> NumPyAttribute<'a> {
    /// Attribute name token.
    pub fn name(&self) -> TokenRef<'a> {
        self.token_ref(self.node.required_token(SyntaxKind::NAME))
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::COLON)
    }

    /// Type annotation token, if present.
    pub fn type_annotation(&self) -> Option<TokenRef<'a>> {
        self.find_token_ref(SyntaxKind::TYPE)
    }

    /// Deprecated alias for [`NumPyAttribute::type_annotation`].
    #[deprecated(since = "0.3.0", note = "renamed to type_annotation")]
    pub fn r#type(&self) -> Option<TokenRef<'a>> {
        self.type_annotation()
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.parsed, self.node, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPyMethod
// =============================================================================

define_node!(NumPyMethod, ENTRY);

impl<'a> NumPyMethod<'a> {
    /// Method name token.
    pub fn name(&self) -> TokenRef<'a> {
        self.token_ref(self.node.required_token(SyntaxKind::NAME))
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
