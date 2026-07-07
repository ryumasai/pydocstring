//! Typed wrappers for NumPy-style syntax nodes.
//!
//! Each wrapper is a newtype over `&SyntaxNode` that provides typed accessors
//! for the node's children (tokens and sub-nodes).

use crate::parse::EntryRole;
use crate::parse::numpy::kind::NumPySectionKind;
use crate::parse::text_block::TextBlock;
use crate::parse::text_block::find_text_block;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::syntax::SyntaxToken;

// =============================================================================
// Macro for defining typed node wrappers
// =============================================================================

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
// NumPyDocstring
// =============================================================================

define_node!(NumPyDocstring, DOCUMENT);

impl<'a> NumPyDocstring<'a> {
    /// Brief summary block, if present.
    pub fn summary(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::SUMMARY)
    }

    /// Extended summary block, if present.
    pub fn extended_summary(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::EXTENDED_SUMMARY)
    }

    /// Deprecation node, if present.
    pub fn deprecation(&self) -> Option<NumPyDeprecation<'a>> {
        self.0.find_node(SyntaxKind::DIRECTIVE).and_then(NumPyDeprecation::cast)
    }

    /// Iterate over all section nodes.
    pub fn sections(&self) -> impl Iterator<Item = NumPySection<'a>> {
        self.0.nodes(SyntaxKind::SECTION).filter_map(NumPySection::cast)
    }

    /// Iterate over stray line tokens.
    pub fn stray_lines(&self) -> impl Iterator<Item = &'a SyntaxToken> {
        self.0.tokens(SyntaxKind::STRAY_LINE)
    }
}

// =============================================================================
// NumPySection
// =============================================================================

define_node!(NumPySection, SECTION);

impl<'a> NumPySection<'a> {
    /// The section header node.
    pub fn header(&self) -> NumPySectionHeader<'a> {
        NumPySectionHeader::cast(
            self.0
                .find_node(SyntaxKind::SECTION_HEADER)
                .expect("SECTION must have a SECTION_HEADER child"),
        )
        .unwrap()
    }

    /// Determine the section kind from the header name text.
    pub fn section_kind(&self, source: &str) -> NumPySectionKind {
        let name_text = self.header().name().text(source);
        NumPySectionKind::from_name(&name_text.to_ascii_lowercase())
    }

    /// Iterate over the `ENTRY` children when this section's entries have
    /// `role`; empty for any other section kind.
    ///
    /// All entries share the `ENTRY` node kind, so without this guard a
    /// mismatched accessor (e.g. `parameters()` on a Raises section) would
    /// wrap foreign entries whose typed accessors then panic in
    /// `required_token`.
    fn entries_with_role(&self, source: &str, role: EntryRole) -> impl Iterator<Item = &'a SyntaxNode> {
        let matches = self.section_kind(source).entry_role() == role;
        self.0.nodes(SyntaxKind::ENTRY).filter(move |_| matches)
    }

    /// Iterate over parameter entry nodes.
    ///
    /// Empty when this is not a Parameters-like section (Parameters, Other
    /// Parameters, Keyword Parameters, Receives).
    pub fn parameters(&self, source: &str) -> impl Iterator<Item = NumPyParameter<'a>> {
        self.entries_with_role(source, EntryRole::Parameter)
            .filter_map(NumPyParameter::cast)
    }

    /// Iterate over returns entry nodes.
    ///
    /// Empty when this is not a Returns section.
    pub fn returns(&self, source: &str) -> impl Iterator<Item = NumPyReturns<'a>> {
        self.entries_with_role(source, EntryRole::Return)
            .filter_map(NumPyReturns::cast)
    }

    /// Iterate over yields entry nodes.
    ///
    /// Empty when this is not a Yields section.
    pub fn yields(&self, source: &str) -> impl Iterator<Item = NumPyYields<'a>> {
        self.entries_with_role(source, EntryRole::Yield)
            .filter_map(NumPyYields::cast)
    }

    /// Iterate over exception entry nodes.
    ///
    /// Empty when this is not a Raises section.
    pub fn exceptions(&self, source: &str) -> impl Iterator<Item = NumPyException<'a>> {
        self.entries_with_role(source, EntryRole::Exception)
            .filter_map(NumPyException::cast)
    }

    /// Iterate over warning entry nodes.
    ///
    /// Empty when this is not a Warns section.
    pub fn warnings(&self, source: &str) -> impl Iterator<Item = NumPyWarning<'a>> {
        self.entries_with_role(source, EntryRole::Warning)
            .filter_map(NumPyWarning::cast)
    }

    /// Iterate over see-also item nodes.
    ///
    /// Empty when this is not a See Also section.
    pub fn see_also_items(&self, source: &str) -> impl Iterator<Item = NumPySeeAlsoItem<'a>> {
        self.entries_with_role(source, EntryRole::SeeAlsoItem)
            .filter_map(NumPySeeAlsoItem::cast)
    }

    /// Iterate over reference nodes.
    ///
    /// `CITATION` nodes only occur in References sections, so no section-kind
    /// guard is needed: other sections have no such children.
    pub fn references(&self) -> impl Iterator<Item = NumPyReference<'a>> {
        self.0.nodes(SyntaxKind::CITATION).filter_map(NumPyReference::cast)
    }

    /// Iterate over attribute entry nodes.
    ///
    /// Empty when this is not an Attributes section.
    pub fn attributes(&self, source: &str) -> impl Iterator<Item = NumPyAttribute<'a>> {
        self.entries_with_role(source, EntryRole::Attribute)
            .filter_map(NumPyAttribute::cast)
    }

    /// Iterate over method entry nodes.
    ///
    /// Empty when this is not a Methods section.
    pub fn methods(&self, source: &str) -> impl Iterator<Item = NumPyMethod<'a>> {
        self.entries_with_role(source, EntryRole::Method)
            .filter_map(NumPyMethod::cast)
    }

    /// Free-text body content block, if this is a free-text section.
    pub fn body_text(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPySectionHeader
// =============================================================================

define_node!(NumPySectionHeader, SECTION_HEADER);

impl<'a> NumPySectionHeader<'a> {
    /// Section name token (e.g. "Parameters", "Returns").
    pub fn name(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::NAME)
    }

    /// Underline token (the `----------` line).
    pub fn underline(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::UNDERLINE)
    }
}

// =============================================================================
// NumPyDeprecation
// =============================================================================

define_node!(NumPyDeprecation, DIRECTIVE);

impl<'a> NumPyDeprecation<'a> {
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
// NumPyParameter
// =============================================================================

define_node!(NumPyParameter, ENTRY);

impl<'a> NumPyParameter<'a> {
    /// Parameter name tokens (supports multiple names like `x1, x2`).
    pub fn names(&self) -> impl Iterator<Item = &'a SyntaxToken> {
        self.0.tokens(SyntaxKind::NAME)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Type annotation token, if present.
    pub fn r#type(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::TYPE)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }

    /// `optional` marker token, if present.
    pub fn optional(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::OPTIONAL)
    }

    /// `default` keyword token, if present.
    pub fn default_keyword(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DEFAULT_KEYWORD)
    }

    /// Default value separator token (`=` or `:`), if present.
    pub fn default_separator(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DEFAULT_SEPARATOR)
    }

    /// Default value text token, if present.
    pub fn default_value(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DEFAULT_VALUE)
    }
}

// =============================================================================
// NumPyReturns
// =============================================================================

define_node!(NumPyReturns, ENTRY);

impl<'a> NumPyReturns<'a> {
    /// Return name token, if present.
    pub fn name(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::NAME)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Return type annotation token, if present.
    pub fn return_type(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::RETURN_TYPE)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPyYields
// =============================================================================

define_node!(NumPyYields, ENTRY);

impl<'a> NumPyYields<'a> {
    /// Yield name token, if present.
    pub fn name(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::NAME)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Yield type annotation token, if present.
    pub fn return_type(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::RETURN_TYPE)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPyException
// =============================================================================

define_node!(NumPyException, ENTRY);

impl<'a> NumPyException<'a> {
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
// NumPyWarning
// =============================================================================

define_node!(NumPyWarning, ENTRY);

impl<'a> NumPyWarning<'a> {
    /// Warning type name token.
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
// NumPySeeAlsoItem
// =============================================================================

define_node!(NumPySeeAlsoItem, ENTRY);

impl<'a> NumPySeeAlsoItem<'a> {
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
// NumPyReference
// =============================================================================

define_node!(NumPyReference, CITATION);

impl<'a> NumPyReference<'a> {
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
// NumPyAttribute
// =============================================================================

define_node!(NumPyAttribute, ENTRY);

impl<'a> NumPyAttribute<'a> {
    /// Attribute name token.
    pub fn name(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::NAME)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Type annotation token, if present.
    pub fn r#type(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::TYPE)
    }

    /// Description text block, if present.
    pub fn description(&self) -> Option<TextBlock<'a>> {
        find_text_block(self.0, SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPyMethod
// =============================================================================

define_node!(NumPyMethod, ENTRY);

impl<'a> NumPyMethod<'a> {
    /// Method name token.
    pub fn name(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::NAME)
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
