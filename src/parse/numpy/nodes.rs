//! Typed wrappers for NumPy-style syntax nodes.
//!
//! Each wrapper is a newtype over `&SyntaxNode` that provides typed accessors
//! for the node's children (tokens and sub-nodes).

use crate::parse::numpy::kind::NumPySectionKind;
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

define_node!(NumPyDocstring, NUMPY_DOCSTRING);

impl<'a> NumPyDocstring<'a> {
    /// Brief summary token, if present.
    pub fn summary(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::SUMMARY)
    }

    /// Extended summary token, if present.
    pub fn extended_summary(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::EXTENDED_SUMMARY)
    }

    /// Deprecation node, if present.
    pub fn deprecation(&self) -> Option<NumPyDeprecation<'a>> {
        self.0
            .find_node(SyntaxKind::NUMPY_DEPRECATION)
            .and_then(NumPyDeprecation::cast)
    }

    /// Iterate over all section nodes.
    pub fn sections(&self) -> impl Iterator<Item = NumPySection<'a>> {
        self.0.nodes(SyntaxKind::NUMPY_SECTION).filter_map(NumPySection::cast)
    }

    /// Iterate over stray line tokens.
    pub fn stray_lines(&self) -> impl Iterator<Item = &'a SyntaxToken> {
        self.0.tokens(SyntaxKind::STRAY_LINE)
    }
}

// =============================================================================
// NumPySection
// =============================================================================

define_node!(NumPySection, NUMPY_SECTION);

impl<'a> NumPySection<'a> {
    /// The section header node.
    pub fn header(&self) -> NumPySectionHeader<'a> {
        NumPySectionHeader::cast(
            self.0
                .find_node(SyntaxKind::NUMPY_SECTION_HEADER)
                .expect("NUMPY_SECTION must have a NUMPY_SECTION_HEADER child"),
        )
        .unwrap()
    }

    /// Determine the section kind from the header name text.
    pub fn section_kind(&self, source: &str) -> NumPySectionKind {
        let name_text = self.header().name().text(source);
        NumPySectionKind::from_name(&name_text.to_ascii_lowercase())
    }

    /// Iterate over parameter entry nodes.
    pub fn parameters(&self) -> impl Iterator<Item = NumPyParameter<'a>> {
        self.0
            .nodes(SyntaxKind::NUMPY_PARAMETER)
            .filter_map(NumPyParameter::cast)
    }

    /// Iterate over returns entry nodes.
    pub fn returns(&self) -> impl Iterator<Item = NumPyReturns<'a>> {
        self.0.nodes(SyntaxKind::NUMPY_RETURNS).filter_map(NumPyReturns::cast)
    }

    /// Iterate over yields entry nodes.
    pub fn yields(&self) -> impl Iterator<Item = NumPyYields<'a>> {
        self.0.nodes(SyntaxKind::NUMPY_YIELDS).filter_map(NumPyYields::cast)
    }

    /// Iterate over exception entry nodes.
    pub fn exceptions(&self) -> impl Iterator<Item = NumPyException<'a>> {
        self.0
            .nodes(SyntaxKind::NUMPY_EXCEPTION)
            .filter_map(NumPyException::cast)
    }

    /// Iterate over warning entry nodes.
    pub fn warnings(&self) -> impl Iterator<Item = NumPyWarning<'a>> {
        self.0.nodes(SyntaxKind::NUMPY_WARNING).filter_map(NumPyWarning::cast)
    }

    /// Iterate over see-also item nodes.
    pub fn see_also_items(&self) -> impl Iterator<Item = NumPySeeAlsoItem<'a>> {
        self.0
            .nodes(SyntaxKind::NUMPY_SEE_ALSO_ITEM)
            .filter_map(NumPySeeAlsoItem::cast)
    }

    /// Iterate over reference nodes.
    pub fn references(&self) -> impl Iterator<Item = NumPyReference<'a>> {
        self.0
            .nodes(SyntaxKind::NUMPY_REFERENCE)
            .filter_map(NumPyReference::cast)
    }

    /// Iterate over attribute entry nodes.
    pub fn attributes(&self) -> impl Iterator<Item = NumPyAttribute<'a>> {
        self.0
            .nodes(SyntaxKind::NUMPY_ATTRIBUTE)
            .filter_map(NumPyAttribute::cast)
    }

    /// Iterate over method entry nodes.
    pub fn methods(&self) -> impl Iterator<Item = NumPyMethod<'a>> {
        self.0.nodes(SyntaxKind::NUMPY_METHOD).filter_map(NumPyMethod::cast)
    }

    /// Free-text body content, if this is a free-text section.
    pub fn body_text(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::BODY_TEXT)
    }
}

// =============================================================================
// NumPySectionHeader
// =============================================================================

define_node!(NumPySectionHeader, NUMPY_SECTION_HEADER);

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

define_node!(NumPyDeprecation, NUMPY_DEPRECATION);

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
        self.0.required_token(SyntaxKind::VERSION)
    }

    /// Description / reason for deprecation.
    pub fn description(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPyParameter
// =============================================================================

define_node!(NumPyParameter, NUMPY_PARAMETER);

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

    /// Description text token, if present.
    pub fn description(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DESCRIPTION)
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

define_node!(NumPyReturns, NUMPY_RETURNS);

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

    /// Description text token, if present.
    pub fn description(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPyYields
// =============================================================================

define_node!(NumPyYields, NUMPY_YIELDS);

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

    /// Description text token, if present.
    pub fn description(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPyException
// =============================================================================

define_node!(NumPyException, NUMPY_EXCEPTION);

impl<'a> NumPyException<'a> {
    /// Exception type name token.
    pub fn r#type(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::TYPE)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Description text token, if present.
    pub fn description(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPyWarning
// =============================================================================

define_node!(NumPyWarning, NUMPY_WARNING);

impl<'a> NumPyWarning<'a> {
    /// Warning type name token.
    pub fn r#type(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::TYPE)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Description text token, if present.
    pub fn description(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPySeeAlsoItem
// =============================================================================

define_node!(NumPySeeAlsoItem, NUMPY_SEE_ALSO_ITEM);

impl<'a> NumPySeeAlsoItem<'a> {
    /// All name tokens (can be multiple, e.g. `func_a, func_b`).
    pub fn names(&self) -> impl Iterator<Item = &'a SyntaxToken> {
        self.0.tokens(SyntaxKind::NAME)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Description text token, if present.
    pub fn description(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPyReference
// =============================================================================

define_node!(NumPyReference, NUMPY_REFERENCE);

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
        self.0.find_token(SyntaxKind::NUMBER)
    }

    /// Closing bracket token, if present.
    pub fn close_bracket(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::CLOSE_BRACKET)
    }

    /// Reference content text token, if present.
    pub fn content(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::CONTENT)
    }
}

// =============================================================================
// NumPyAttribute
// =============================================================================

define_node!(NumPyAttribute, NUMPY_ATTRIBUTE);

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

    /// Description text token, if present.
    pub fn description(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// NumPyMethod
// =============================================================================

define_node!(NumPyMethod, NUMPY_METHOD);

impl<'a> NumPyMethod<'a> {
    /// Method name token.
    pub fn name(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::NAME)
    }

    /// Colon separator token, if present.
    pub fn colon(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::COLON)
    }

    /// Description text token, if present.
    pub fn description(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DESCRIPTION)
    }
}
