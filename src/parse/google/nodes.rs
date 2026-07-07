//! Typed wrappers for Google-style syntax nodes.
//!
//! Each wrapper is a newtype over `&SyntaxNode` that provides typed accessors
//! for the node's children (tokens and sub-nodes).

use crate::parse::google::kind::GoogleSectionKind;
use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

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

define_node!(GoogleDocstring, GOOGLE_DOCSTRING);

impl<'a> GoogleDocstring<'a> {
    /// Brief summary token, if present.
    pub fn summary(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::SUMMARY)
    }

    /// Extended summary token, if present.
    pub fn extended_summary(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::EXTENDED_SUMMARY)
    }

    /// Deprecation node, if present.
    pub fn deprecation(&self) -> Option<GoogleDeprecation<'a>> {
        self.0
            .find_node(SyntaxKind::GOOGLE_DEPRECATION)
            .and_then(GoogleDeprecation::cast)
    }

    /// Iterate over all section nodes.
    pub fn sections(&self) -> impl Iterator<Item = GoogleSection<'a>> {
        self.0.nodes(SyntaxKind::GOOGLE_SECTION).filter_map(GoogleSection::cast)
    }

    /// Iterate over stray line tokens.
    pub fn stray_lines(&self) -> impl Iterator<Item = &'a SyntaxToken> {
        self.0.tokens(SyntaxKind::STRAY_LINE)
    }
}

// =============================================================================
// GoogleDeprecation
// =============================================================================

define_node!(GoogleDeprecation, GOOGLE_DEPRECATION);

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
        self.0.required_token(SyntaxKind::VERSION)
    }

    /// Description / reason for deprecation.
    pub fn description(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleSection
// =============================================================================

define_node!(GoogleSection, GOOGLE_SECTION);

impl<'a> GoogleSection<'a> {
    /// The section header node.
    pub fn header(&self) -> GoogleSectionHeader<'a> {
        GoogleSectionHeader::cast(
            self.0
                .find_node(SyntaxKind::GOOGLE_SECTION_HEADER)
                .expect("GOOGLE_SECTION must have a GOOGLE_SECTION_HEADER child"),
        )
        .unwrap()
    }

    /// Determine the section kind from the header name text.
    pub fn section_kind(&self, source: &str) -> GoogleSectionKind {
        let name_text = self.header().name().text(source);
        GoogleSectionKind::from_name(&name_text.to_ascii_lowercase())
    }

    /// Iterate over arg entry nodes in this section.
    pub fn args(&self) -> impl Iterator<Item = GoogleArg<'a>> {
        self.0.nodes(SyntaxKind::GOOGLE_ARG).filter_map(GoogleArg::cast)
    }

    /// Returns entry node in this section, if present.
    pub fn returns(&self) -> Option<GoogleReturn<'a>> {
        self.0
            .find_node(SyntaxKind::GOOGLE_RETURNS)
            .and_then(GoogleReturn::cast)
    }

    /// Yields entry node in this section, if present.
    pub fn yields(&self) -> Option<GoogleYield<'a>> {
        self.0.find_node(SyntaxKind::GOOGLE_YIELDS).and_then(GoogleYield::cast)
    }

    /// Iterate over exception entry nodes.
    pub fn exceptions(&self) -> impl Iterator<Item = GoogleException<'a>> {
        self.0
            .nodes(SyntaxKind::GOOGLE_EXCEPTION)
            .filter_map(GoogleException::cast)
    }

    /// Iterate over warning entry nodes.
    pub fn warnings(&self) -> impl Iterator<Item = GoogleWarning<'a>> {
        self.0.nodes(SyntaxKind::GOOGLE_WARNING).filter_map(GoogleWarning::cast)
    }

    /// Iterate over see-also item nodes.
    pub fn see_also_items(&self) -> impl Iterator<Item = GoogleSeeAlsoItem<'a>> {
        self.0
            .nodes(SyntaxKind::GOOGLE_SEE_ALSO_ITEM)
            .filter_map(GoogleSeeAlsoItem::cast)
    }

    /// Iterate over reference nodes.
    pub fn references(&self) -> impl Iterator<Item = GoogleReference<'a>> {
        self.0
            .nodes(SyntaxKind::GOOGLE_REFERENCE)
            .filter_map(GoogleReference::cast)
    }

    /// Iterate over attribute entry nodes.
    pub fn attributes(&self) -> impl Iterator<Item = GoogleAttribute<'a>> {
        self.0
            .nodes(SyntaxKind::GOOGLE_ATTRIBUTE)
            .filter_map(GoogleAttribute::cast)
    }

    /// Iterate over method entry nodes.
    pub fn methods(&self) -> impl Iterator<Item = GoogleMethod<'a>> {
        self.0.nodes(SyntaxKind::GOOGLE_METHOD).filter_map(GoogleMethod::cast)
    }

    /// Free-text body content, if this is a free-text section.
    pub fn body_text(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::BODY_TEXT)
    }
}

// =============================================================================
// GoogleSectionHeader
// =============================================================================

define_node!(GoogleSectionHeader, GOOGLE_SECTION_HEADER);

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

define_node!(GoogleArg, GOOGLE_ARG);

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
// GoogleReturn
// =============================================================================

define_node!(GoogleReturn, GOOGLE_RETURNS);

impl<'a> GoogleReturn<'a> {
    /// Return type annotation token, if present.
    pub fn return_type(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::RETURN_TYPE)
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
// GoogleYield
// =============================================================================

define_node!(GoogleYield, GOOGLE_YIELDS);

impl<'a> GoogleYield<'a> {
    /// Yield type annotation token, if present.
    pub fn return_type(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::RETURN_TYPE)
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
// GoogleException
// =============================================================================

define_node!(GoogleException, GOOGLE_EXCEPTION);

impl<'a> GoogleException<'a> {
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
// GoogleWarning
// =============================================================================

define_node!(GoogleWarning, GOOGLE_WARNING);

impl<'a> GoogleWarning<'a> {
    /// Warning type name token (e.g. `UserWarning`).
    pub fn warning_type(&self) -> &'a SyntaxToken {
        self.0.required_token(SyntaxKind::WARNING_TYPE)
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
// GoogleSeeAlsoItem
// =============================================================================

define_node!(GoogleSeeAlsoItem, GOOGLE_SEE_ALSO_ITEM);

impl<'a> GoogleSeeAlsoItem<'a> {
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
// GoogleReference
// =============================================================================

define_node!(GoogleReference, GOOGLE_REFERENCE);

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
// GoogleAttribute
// =============================================================================

define_node!(GoogleAttribute, GOOGLE_ATTRIBUTE);

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

    /// Description text token, if present.
    pub fn description(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DESCRIPTION)
    }
}

// =============================================================================
// GoogleMethod
// =============================================================================

define_node!(GoogleMethod, GOOGLE_METHOD);

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

    /// Description text token, if present.
    pub fn description(&self) -> Option<&'a SyntaxToken> {
        self.0.find_token(SyntaxKind::DESCRIPTION)
    }
}
