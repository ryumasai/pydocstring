//! Typed wrappers for Google-style syntax nodes.
//!
//! Each wrapper is a newtype over `&SyntaxNode` that provides typed accessors
//! for the node's children (tokens and sub-nodes).

use crate::parse::EntryRole;
use crate::parse::google::kind::GoogleSectionKind;
use crate::parse::text_block::TextBlock;
use crate::parse::text_block::find_text_block;
use crate::parse::unified::DefaultMarker;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::syntax::SyntaxToken;

// =============================================================================
// Macro for defining typed node wrappers
// =============================================================================

/// Define a typed node wrapper that casts from `&SyntaxNode`.
macro_rules! define_node {
    ($name:ident, $kind:ident) => {
        #[doc = concat!("Typed wrapper for `", stringify!($kind), "` syntax nodes.")]
        #[derive(Debug)]
        pub struct $name<'a>(pub(crate) &'a SyntaxNode);

        impl<'a> $name<'a> {
            /// Try to cast a `SyntaxNode` reference into this typed wrapper.
            pub fn cast(node: &'a SyntaxNode) -> Option<Self> {
                (node.kind() == SyntaxKind::$kind).then(|| Self(node))
            }

            /// Access the underlying `SyntaxNode`.
            pub fn syntax(&self) -> &'a SyntaxNode {
                self.0
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
        find_text_block(self.0, SyntaxKind::SUMMARY)
    }

    /// Extended summary block, if present.
    pub fn extended_summary(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::EXTENDED_SUMMARY)
    }

    /// Deprecation node, if present.
    pub fn deprecation(&self) -> Option<GoogleDeprecation<'a>> {
        self.0
            .find_node(SyntaxKind::DIRECTIVE)
            .and_then(GoogleDeprecation::cast)
    }

    /// Iterate over all section nodes.
    pub fn sections(&self) -> impl Iterator<Item = GoogleSection<'a>> {
        self.0.nodes(SyntaxKind::SECTION).filter_map(GoogleSection::cast)
    }

    /// Iterate over stray line tokens.
    pub fn stray_lines(&self) -> impl Iterator<Item = &'a SyntaxToken> {
        self.0.tokens(SyntaxKind::STRAY_LINE)
    }
}

// =============================================================================
// GoogleDeprecation
// =============================================================================

define_node!(GoogleDeprecation, DIRECTIVE);

impl<'a> GoogleDeprecation<'a> {
    /// The `..` RST directive marker.
    pub fn directive_marker(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DIRECTIVE_MARKER)
    }

    /// The `deprecated` keyword.
    pub fn keyword(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::KEYWORD)
    }

    /// The `::` double-colon separator.
    pub fn double_colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DOUBLE_COLON)
    }

    /// Version when deprecated.
    pub fn version(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::ARGUMENT)
    }

    /// Description / reason for deprecation.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleSection
// =============================================================================

define_node!(GoogleSection, SECTION);

impl<'a> GoogleSection<'a> {
    /// The section header node.
    pub fn header(&self) -> GoogleSectionHeader<'a> {
        GoogleSectionHeader::cast(
            self.0
                .find_node(SyntaxKind::SECTION_HEADER)
                .expect("SECTION must have a SECTION_HEADER child"),
        )
        .unwrap()
    }

    /// Determine the section kind from the header name text.
    pub fn section_kind(&self, source: &str) -> GoogleSectionKind {
        let name_text = self.header().name().text(source);
        GoogleSectionKind::from_name(&name_text.to_ascii_lowercase())
    }

    /// Iterate over the `ENTRY` children when this section's entries have
    /// `role`; empty for any other section kind.
    ///
    /// All entries share the `ENTRY` node kind, so without this guard a
    /// mismatched accessor (e.g. `args()` on a `Raises:` section) would wrap
    /// foreign entries whose typed accessors then panic in `required_token`.
    fn entries_with_role(&self, source: &str, role: EntryRole) -> impl Iterator<Item = &'a SyntaxNode> {
        let matches = self.section_kind(source).entry_role() == role;
        self.0.nodes(SyntaxKind::ENTRY).filter(move |_| matches)
    }

    /// Iterate over arg entry nodes in this section.
    ///
    /// Empty when this is not an Args-like section (Args, Keyword Args,
    /// Other Parameters, Receives).
    pub fn args(&self, source: &str) -> impl Iterator<Item = GoogleArg<'a>> {
        self.entries_with_role(source, EntryRole::Parameter)
            .filter_map(GoogleArg::cast)
    }

    /// Returns entry node in this section, if present.
    ///
    /// `None` when this is not a Returns section.
    pub fn returns(&self, source: &str) -> Option<GoogleReturn<'a>> {
        self.entries_with_role(source, EntryRole::Return)
            .next()
            .and_then(GoogleReturn::cast)
    }

    /// Yields entry node in this section, if present.
    ///
    /// `None` when this is not a Yields section.
    pub fn yields(&self, source: &str) -> Option<GoogleYield<'a>> {
        self.entries_with_role(source, EntryRole::Yield)
            .next()
            .and_then(GoogleYield::cast)
    }

    /// Iterate over exception entry nodes.
    ///
    /// Empty when this is not a Raises section.
    pub fn exceptions(&self, source: &str) -> impl Iterator<Item = GoogleException<'a>> {
        self.entries_with_role(source, EntryRole::Exception)
            .filter_map(GoogleException::cast)
    }

    /// Iterate over warning entry nodes.
    ///
    /// Empty when this is not a Warns section.
    pub fn warnings(&self, source: &str) -> impl Iterator<Item = GoogleWarning<'a>> {
        self.entries_with_role(source, EntryRole::Warning)
            .filter_map(GoogleWarning::cast)
    }

    /// Iterate over see-also item nodes.
    ///
    /// Empty when this is not a See Also section.
    pub fn see_also_items(&self, source: &str) -> impl Iterator<Item = GoogleSeeAlsoItem<'a>> {
        self.entries_with_role(source, EntryRole::SeeAlsoItem)
            .filter_map(GoogleSeeAlsoItem::cast)
    }

    /// Iterate over reference nodes.
    ///
    /// `CITATION` nodes only occur in References sections, so no section-kind
    /// guard is needed: other sections have no such children.
    pub fn references(&self) -> impl Iterator<Item = GoogleReference<'a>> {
        self.0.nodes(SyntaxKind::CITATION).filter_map(GoogleReference::cast)
    }

    /// Iterate over attribute entry nodes.
    ///
    /// Empty when this is not an Attributes section.
    pub fn attributes(&self, source: &str) -> impl Iterator<Item = GoogleAttribute<'a>> {
        self.entries_with_role(source, EntryRole::Attribute)
            .filter_map(GoogleAttribute::cast)
    }

    /// Iterate over method entry nodes.
    ///
    /// Empty when this is not a Methods section.
    pub fn methods(&self, source: &str) -> impl Iterator<Item = GoogleMethod<'a>> {
        self.entries_with_role(source, EntryRole::Method)
            .filter_map(GoogleMethod::cast)
    }

    /// Free-text body content block, if this is a free-text section.
    pub fn body_text(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleSectionHeader
// =============================================================================

define_node!(GoogleSectionHeader, SECTION_HEADER);

impl<'a> GoogleSectionHeader<'a> {
    /// Section name token (e.g. "Args", "Returns").
    pub fn name(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::NAME)
    }

    /// Colon token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
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
    pub fn name(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::NAME)
    }

    /// All argument name tokens (can be multiple, e.g. `x1, x2`).
    pub fn names(&self) -> impl Iterator<Item = &'a SyntaxToken> {
        self.0.tokens(SyntaxKind::NAME)
    }

    /// Opening bracket token (e.g. `(`), if present.
    pub fn open_bracket(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::OPEN_BRACKET)
    }

    /// Type annotation token, if present.
    pub fn r#type(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::TYPE)
    }

    /// Closing bracket token (e.g. `)`), if present.
    pub fn close_bracket(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::CLOSE_BRACKET)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }

    /// First `optional` marker token, if present.
    ///
    /// Markers are repeatable; use [`GoogleArg::optionals`] to see every
    /// occurrence.
    pub fn optional(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::OPTIONAL)
    }

    /// All `optional` marker tokens, one per occurrence, in source order.
    pub fn optionals(&self) -> impl Iterator<Item = &'a SyntaxToken> {
        self.0.tokens(SyntaxKind::OPTIONAL)
    }

    /// All `default …` markers, one [`DefaultMarker`] per occurrence, in source
    /// order.
    pub fn defaults(&self) -> impl Iterator<Item = DefaultMarker<'a>> {
        self.0.nodes(SyntaxKind::DEFAULT).filter_map(DefaultMarker::cast)
    }

    /// The first `default …` marker's keyword token, if present.
    pub fn default_keyword(&self) -> Option<&'a SyntaxToken> {
        self.defaults().next().map(|d| d.keyword())
    }

    /// The first `default …` marker's separator token (`=` or `:`), if
    /// present.
    pub fn default_separator(&self) -> Option<&'a SyntaxToken> {
        self.defaults().next().and_then(|d| d.separator())
    }

    /// The first `default …` marker's value token, if present.
    ///
    /// First occurrence wins — the same normalization rule the model layer
    /// applies. Use [`GoogleArg::defaults`] to see every occurrence.
    pub fn default_value(&self) -> Option<&'a SyntaxToken> {
        self.defaults().next().and_then(|d| d.value())
    }
}

// =============================================================================
// GoogleReturn
// =============================================================================

define_node!(GoogleReturn, ENTRY);

impl<'a> GoogleReturn<'a> {
    /// Return type annotation token, if present.
    pub fn return_type(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::RETURN_TYPE)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleYield
// =============================================================================

define_node!(GoogleYield, ENTRY);

impl<'a> GoogleYield<'a> {
    /// Yield type annotation token, if present.
    pub fn return_type(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::RETURN_TYPE)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleException
// =============================================================================

define_node!(GoogleException, ENTRY);

impl<'a> GoogleException<'a> {
    /// Exception type name token.
    pub fn r#type(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::TYPE)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleWarning
// =============================================================================

define_node!(GoogleWarning, ENTRY);

impl<'a> GoogleWarning<'a> {
    /// Warning type name token (e.g. `UserWarning`).
    pub fn warning_type(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::WARNING_TYPE)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleSeeAlsoItem
// =============================================================================

define_node!(GoogleSeeAlsoItem, ENTRY);

impl<'a> GoogleSeeAlsoItem<'a> {
    /// All name tokens (can be multiple, e.g. `func_a, func_b`).
    pub fn names(&self) -> impl Iterator<Item = &'a SyntaxToken> {
        self.0.tokens(SyntaxKind::NAME)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleReference
// =============================================================================

define_node!(GoogleReference, CITATION);

impl<'a> GoogleReference<'a> {
    /// RST directive marker (`..`), if present.
    pub fn directive_marker(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DIRECTIVE_MARKER)
    }

    /// Opening bracket token, if present.
    pub fn open_bracket(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::OPEN_BRACKET)
    }

    /// Reference number token, if present.
    pub fn number(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::LABEL)
    }

    /// Closing bracket token, if present.
    pub fn close_bracket(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::CLOSE_BRACKET)
    }

    /// Reference content text block, if present.
    pub fn content(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleAttribute
// =============================================================================

define_node!(GoogleAttribute, ENTRY);

impl<'a> GoogleAttribute<'a> {
    /// Attribute name token.
    pub fn name(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::NAME)
    }

    /// Opening bracket token (e.g. `(`), if present.
    pub fn open_bracket(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::OPEN_BRACKET)
    }

    /// Type annotation token, if present.
    pub fn r#type(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::TYPE)
    }

    /// Closing bracket token (e.g. `)`), if present.
    pub fn close_bracket(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::CLOSE_BRACKET)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleMethod
// =============================================================================

define_node!(GoogleMethod, ENTRY);

impl<'a> GoogleMethod<'a> {
    /// Method name token.
    pub fn name(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::NAME)
    }

    /// Opening bracket token (e.g. `(`), if present.
    pub fn open_bracket(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::OPEN_BRACKET)
    }

    /// Type annotation token, if present.
    pub fn r#type(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::TYPE)
    }

    /// Closing bracket token (e.g. `)`), if present.
    pub fn close_bracket(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::CLOSE_BRACKET)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}
