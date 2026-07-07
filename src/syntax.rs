//! Unified syntax tree types.
//!
//! This module defines the core tree data structures shared by all docstring
//! styles.  Every parsed docstring is represented as a tree of [`SyntaxNode`]s
//! (branches) and [`SyntaxToken`]s (leaves), each tagged with a [`SyntaxKind`].
//!
//! The [`Parsed`] struct owns the source text and the root node, and provides
//! a convenience [`pretty_print`](Parsed::pretty_print) method for debugging.

use core::fmt;
use core::fmt::Write;

use crate::text::LineColumn;
use crate::text::LineIndex;
use crate::text::TextRange;

// =============================================================================
// SyntaxKind
// =============================================================================

/// Node and token kinds for all docstring styles.
///
/// Google and NumPy variants coexist in a single enum, just as Biome puts
/// `JsIfStatement` and `TsInterface` in one `SyntaxKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum SyntaxKind {
    // ── Common tokens ──────────────────────────────────────────────────
    /// Section name, parameter name, exception type name, etc.
    NAME,
    /// Type annotation.
    TYPE,
    /// `:` separator.
    COLON,
    /// Description text.
    DESCRIPTION,
    /// Opening bracket: `(`, `[`, `{`, or `<`.
    OPEN_BRACKET,
    /// Closing bracket: `)`, `]`, `}`, or `>`.
    CLOSE_BRACKET,
    /// `optional` marker.
    OPTIONAL,
    /// Free-text section body.
    BODY_TEXT,
    /// Summary line.
    SUMMARY,
    /// Extended summary paragraph.
    EXTENDED_SUMMARY,
    /// Stray line between sections.
    STRAY_LINE,

    // ── Trivia tokens ──────────────────────────────────────────────────
    /// A run of spaces/tabs within a line (indentation, inter-token
    /// spacing). Never contains a newline.
    WHITESPACE,
    /// A single line break: `\n` (or `\r\n` as one two-byte token).
    /// Never part of a blank line — see [`SyntaxKind::BLANK_LINE`].
    NEWLINE,
    /// One entire line consisting only of whitespace, *including* its
    /// terminating newline (a whitespace-only line at end of input keeps
    /// no newline). A zero-width line — a bare `\n` at line start — is
    /// also a blank line. Consecutive blank lines yield one `BLANK_LINE`
    /// token per line.
    BLANK_LINE,

    // ── Google-specific tokens ─────────────────────────────────────────
    /// Warning type (e.g. `UserWarning`).
    WARNING_TYPE,

    // ── NumPy-specific tokens ──────────────────────────────────────────
    /// Section header underline (`----------`).
    UNDERLINE,
    /// RST directive marker (`..`).
    DIRECTIVE_MARKER,
    /// Keyword such as `deprecated`.
    KEYWORD,
    /// RST double colon (`::`).
    DOUBLE_COLON,
    /// Deprecation version string.
    VERSION,
    /// Return type (NumPy-style).
    RETURN_TYPE,
    /// `default` keyword.
    DEFAULT_KEYWORD,
    /// Default value separator (`=` or `:`).
    DEFAULT_SEPARATOR,
    /// Default value text.
    DEFAULT_VALUE,
    /// Reference number.
    NUMBER,
    /// Reference content text.
    CONTENT,

    // ── Google nodes ───────────────────────────────────────────────────
    /// Root node for a Google-style docstring.
    GOOGLE_DOCSTRING,
    /// A complete Google section (header + body items).
    GOOGLE_SECTION,
    /// Section header (`Args:`, `Returns:`, etc.).
    GOOGLE_SECTION_HEADER,
    /// Deprecation directive block.
    GOOGLE_DEPRECATION,
    /// A single argument entry.
    GOOGLE_ARG,
    /// A single return value entry.
    GOOGLE_RETURNS,
    /// A single yield value entry.
    GOOGLE_YIELDS,
    /// A single exception entry.
    GOOGLE_EXCEPTION,
    /// A single warning entry.
    GOOGLE_WARNING,
    /// A single "See Also" item.
    GOOGLE_SEE_ALSO_ITEM,
    /// A single reference entry.
    GOOGLE_REFERENCE,
    /// A single attribute entry.
    GOOGLE_ATTRIBUTE,
    /// A single method entry.
    GOOGLE_METHOD,

    // ── NumPy nodes ────────────────────────────────────────────────────
    /// Root node for a NumPy-style docstring.
    NUMPY_DOCSTRING,
    /// A complete NumPy section (header + body items).
    NUMPY_SECTION,
    /// Section header (name + underline).
    NUMPY_SECTION_HEADER,
    /// Deprecation directive block.
    NUMPY_DEPRECATION,
    /// A single parameter entry.
    NUMPY_PARAMETER,
    /// A single return value entry.
    NUMPY_RETURNS,
    /// A single yield value entry.
    NUMPY_YIELDS,
    /// A single exception entry.
    NUMPY_EXCEPTION,
    /// A single warning entry.
    NUMPY_WARNING,
    /// A single "See Also" item.
    NUMPY_SEE_ALSO_ITEM,
    /// A single reference entry.
    NUMPY_REFERENCE,
    /// A single attribute entry.
    NUMPY_ATTRIBUTE,
    /// A single method entry.
    NUMPY_METHOD,

    // ── Plain node ─────────────────────────────────────────────────────
    /// Root node for a plain docstring (summary/extended summary only,
    /// no NumPy or Google style section markers).
    /// Also used for unrecognised styles such as Sphinx.
    PLAIN_DOCSTRING,
}

impl SyntaxKind {
    /// Whether this kind represents a node (branch) rather than a token (leaf).
    pub const fn is_node(self) -> bool {
        matches!(
            self,
            Self::PLAIN_DOCSTRING
                | Self::GOOGLE_DOCSTRING
                | Self::GOOGLE_SECTION
                | Self::GOOGLE_SECTION_HEADER
                | Self::GOOGLE_DEPRECATION
                | Self::GOOGLE_ARG
                | Self::GOOGLE_RETURNS
                | Self::GOOGLE_YIELDS
                | Self::GOOGLE_EXCEPTION
                | Self::GOOGLE_WARNING
                | Self::GOOGLE_SEE_ALSO_ITEM
                | Self::GOOGLE_REFERENCE
                | Self::GOOGLE_ATTRIBUTE
                | Self::GOOGLE_METHOD
                | Self::NUMPY_DOCSTRING
                | Self::NUMPY_SECTION
                | Self::NUMPY_SECTION_HEADER
                | Self::NUMPY_DEPRECATION
                | Self::NUMPY_PARAMETER
                | Self::NUMPY_RETURNS
                | Self::NUMPY_YIELDS
                | Self::NUMPY_EXCEPTION
                | Self::NUMPY_WARNING
                | Self::NUMPY_SEE_ALSO_ITEM
                | Self::NUMPY_REFERENCE
                | Self::NUMPY_ATTRIBUTE
                | Self::NUMPY_METHOD
        )
    }

    /// Whether this kind represents a token (leaf) rather than a node (branch).
    pub const fn is_token(self) -> bool {
        !self.is_node()
    }

    /// Whether this kind is a trivia token (whitespace, newline, blank line).
    ///
    /// Trivia tokens carry no docstring content; they account for the layout
    /// bytes between content tokens.
    pub const fn is_trivia(self) -> bool {
        matches!(self, Self::WHITESPACE | Self::NEWLINE | Self::BLANK_LINE)
    }

    /// Display name for pretty-printing (e.g. `"GOOGLE_ARG"`, `"NAME"`).
    pub const fn name(self) -> &'static str {
        match self {
            // Common tokens
            Self::NAME => "NAME",
            Self::TYPE => "TYPE",
            Self::COLON => "COLON",
            Self::DESCRIPTION => "DESCRIPTION",
            Self::OPEN_BRACKET => "OPEN_BRACKET",
            Self::CLOSE_BRACKET => "CLOSE_BRACKET",
            Self::OPTIONAL => "OPTIONAL",
            Self::BODY_TEXT => "BODY_TEXT",
            Self::SUMMARY => "SUMMARY",
            Self::EXTENDED_SUMMARY => "EXTENDED_SUMMARY",
            Self::STRAY_LINE => "STRAY_LINE",
            // Trivia tokens
            Self::WHITESPACE => "WHITESPACE",
            Self::NEWLINE => "NEWLINE",
            Self::BLANK_LINE => "BLANK_LINE",
            // Google tokens
            Self::WARNING_TYPE => "WARNING_TYPE",
            // NumPy tokens
            Self::UNDERLINE => "UNDERLINE",
            Self::DIRECTIVE_MARKER => "DIRECTIVE_MARKER",
            Self::KEYWORD => "KEYWORD",
            Self::DOUBLE_COLON => "DOUBLE_COLON",
            Self::VERSION => "VERSION",
            Self::RETURN_TYPE => "RETURN_TYPE",
            Self::DEFAULT_KEYWORD => "DEFAULT_KEYWORD",
            Self::DEFAULT_SEPARATOR => "DEFAULT_SEPARATOR",
            Self::DEFAULT_VALUE => "DEFAULT_VALUE",
            Self::NUMBER => "NUMBER",
            Self::CONTENT => "CONTENT",
            // Google nodes
            Self::GOOGLE_DOCSTRING => "GOOGLE_DOCSTRING",
            Self::GOOGLE_SECTION => "GOOGLE_SECTION",
            Self::GOOGLE_SECTION_HEADER => "GOOGLE_SECTION_HEADER",
            Self::GOOGLE_DEPRECATION => "GOOGLE_DEPRECATION",
            Self::GOOGLE_ARG => "GOOGLE_ARG",
            Self::GOOGLE_RETURNS => "GOOGLE_RETURNS",
            Self::GOOGLE_YIELDS => "GOOGLE_YIELDS",
            Self::GOOGLE_EXCEPTION => "GOOGLE_EXCEPTION",
            Self::GOOGLE_WARNING => "GOOGLE_WARNING",
            Self::GOOGLE_SEE_ALSO_ITEM => "GOOGLE_SEE_ALSO_ITEM",
            Self::GOOGLE_REFERENCE => "GOOGLE_REFERENCE",
            Self::GOOGLE_ATTRIBUTE => "GOOGLE_ATTRIBUTE",
            Self::GOOGLE_METHOD => "GOOGLE_METHOD",
            // Plain node
            Self::PLAIN_DOCSTRING => "PLAIN_DOCSTRING",
            // NumPy nodes
            Self::NUMPY_DOCSTRING => "NUMPY_DOCSTRING",
            Self::NUMPY_SECTION => "NUMPY_SECTION",
            Self::NUMPY_SECTION_HEADER => "NUMPY_SECTION_HEADER",
            Self::NUMPY_DEPRECATION => "NUMPY_DEPRECATION",
            Self::NUMPY_PARAMETER => "NUMPY_PARAMETER",
            Self::NUMPY_RETURNS => "NUMPY_RETURNS",
            Self::NUMPY_YIELDS => "NUMPY_YIELDS",
            Self::NUMPY_EXCEPTION => "NUMPY_EXCEPTION",
            Self::NUMPY_WARNING => "NUMPY_WARNING",
            Self::NUMPY_SEE_ALSO_ITEM => "NUMPY_SEE_ALSO_ITEM",
            Self::NUMPY_REFERENCE => "NUMPY_REFERENCE",
            Self::NUMPY_ATTRIBUTE => "NUMPY_ATTRIBUTE",
            Self::NUMPY_METHOD => "NUMPY_METHOD",
        }
    }
}

impl fmt::Display for SyntaxKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// =============================================================================
// SyntaxNode / SyntaxToken / SyntaxElement
// =============================================================================

/// A branch node in the syntax tree.
///
/// Holds an ordered list of child [`SyntaxElement`]s (nodes or tokens).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxNode {
    kind: SyntaxKind,
    range: TextRange,
    children: Vec<SyntaxElement>,
}

impl SyntaxNode {
    /// Creates a new node with the given kind, range, and children.
    pub fn new(kind: SyntaxKind, range: TextRange, children: Vec<SyntaxElement>) -> Self {
        Self { kind, range, children }
    }

    /// The kind of this node.
    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }

    /// The source range of this node.
    pub fn range(&self) -> &TextRange {
        &self.range
    }

    /// The ordered child elements.
    pub fn children(&self) -> &[SyntaxElement] {
        &self.children
    }

    /// Mutable access to the ordered child elements.
    pub fn children_mut(&mut self) -> &mut [SyntaxElement] {
        &mut self.children
    }

    /// Append a child element.
    pub fn push_child(&mut self, child: SyntaxElement) {
        self.children.push(child);
    }

    /// Take ownership of the child list, leaving it empty.
    ///
    /// Used by the trivia pass to splice trivia tokens between children.
    pub(crate) fn take_children(&mut self) -> Vec<SyntaxElement> {
        core::mem::take(&mut self.children)
    }

    /// Replace the child list.
    pub(crate) fn set_children(&mut self, children: Vec<SyntaxElement>) {
        self.children = children;
    }

    /// Extend this node's range end to `end`.
    pub fn extend_range_to(&mut self, end: crate::text::TextSize) {
        self.range = TextRange::new(self.range.start(), end);
    }

    /// Find the first present (non-missing) token child with the given kind.
    ///
    /// Zero-length tokens are considered missing and are excluded.
    /// Use [`find_missing`](Self::find_missing) to find missing tokens.
    pub fn find_token(&self, kind: SyntaxKind) -> Option<&SyntaxToken> {
        self.children.iter().find_map(|c| match c {
            SyntaxElement::Token(t) if t.kind() == kind && !t.is_missing() => Some(t),
            _ => None,
        })
    }

    /// Find the first missing (zero-length) token child with the given kind.
    pub fn find_missing(&self, kind: SyntaxKind) -> Option<&SyntaxToken> {
        self.children.iter().find_map(|c| match c {
            SyntaxElement::Token(t) if t.kind() == kind && t.is_missing() => Some(t),
            _ => None,
        })
    }

    /// Return the first token child with the given kind.
    ///
    /// Unlike [`find_token`] this also matches zero-length (missing) tokens,
    /// so it never panics due to a token being present but empty.  Callers
    /// that need to distinguish a real token from a placeholder should check
    /// [`SyntaxToken::is_missing`] on the returned value.
    ///
    /// # Panics
    ///
    /// Panics only if no child token of the given kind exists at all, which
    /// indicates a structural bug in the parser.
    pub fn required_token(&self, kind: SyntaxKind) -> &SyntaxToken {
        self.children
            .iter()
            .find_map(|c| match c {
                SyntaxElement::Token(t) if t.kind() == kind => Some(t),
                _ => None,
            })
            .unwrap_or_else(|| panic!("required token {:?} not found in {:?}", kind, self.kind))
    }

    /// Iterate over all token children with the given kind.
    pub fn tokens(&self, kind: SyntaxKind) -> impl Iterator<Item = &SyntaxToken> {
        self.children.iter().filter_map(move |c| match c {
            SyntaxElement::Token(t) if t.kind() == kind => Some(t),
            _ => None,
        })
    }

    /// Find the first child node with the given kind.
    pub fn find_node(&self, kind: SyntaxKind) -> Option<&SyntaxNode> {
        self.children.iter().find_map(|c| match c {
            SyntaxElement::Node(n) if n.kind() == kind => Some(n),
            _ => None,
        })
    }

    /// Iterate over all child nodes with the given kind.
    pub fn nodes(&self, kind: SyntaxKind) -> impl Iterator<Item = &SyntaxNode> {
        self.children.iter().filter_map(move |c| match c {
            SyntaxElement::Node(n) if n.kind() == kind => Some(n),
            _ => None,
        })
    }

    /// Write a pretty-printed tree representation.
    pub fn pretty_fmt(&self, src: &str, indent: usize, out: &mut String) {
        for _ in 0..indent {
            out.push_str("  ");
        }
        let _ = writeln!(out, "{}@{} {{", self.kind.name(), self.range);
        for child in &self.children {
            match child {
                SyntaxElement::Node(n) => n.pretty_fmt(src, indent + 1, out),
                SyntaxElement::Token(t) => t.pretty_fmt(src, indent + 1, out),
            }
        }
        for _ in 0..indent {
            out.push_str("  ");
        }
        out.push_str("}\n");
    }
}

/// A leaf token in the syntax tree.
///
/// Represents a contiguous span of source text with a known kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxToken {
    kind: SyntaxKind,
    range: TextRange,
}

impl SyntaxToken {
    /// Creates a new token with the given kind and range.
    pub fn new(kind: SyntaxKind, range: TextRange) -> Self {
        Self { kind, range }
    }

    /// The kind of this token.
    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }

    /// The source range of this token.
    pub fn range(&self) -> &TextRange {
        &self.range
    }

    /// Whether this token is missing from the source (zero-length placeholder).
    pub fn is_missing(&self) -> bool {
        self.range.is_empty()
    }

    /// Extract the corresponding text slice from source.
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        self.range.source_text(source)
    }

    /// Extend this token's range to include `other`.
    pub fn extend_range(&mut self, other: TextRange) {
        self.range.extend(other);
    }

    /// Write a pretty-printed token line.
    pub fn pretty_fmt(&self, src: &str, indent: usize, out: &mut String) {
        for _ in 0..indent {
            out.push_str("  ");
        }
        let _ = writeln!(out, "{}: {:?}@{}", self.kind.name(), self.text(src), self.range);
    }
}

/// A child element of a [`SyntaxNode`] — either a node or a token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxElement {
    /// A branch node.
    Node(SyntaxNode),
    /// A leaf token.
    Token(SyntaxToken),
}

impl SyntaxElement {
    /// The source range of this element.
    pub fn range(&self) -> &TextRange {
        match self {
            Self::Node(n) => n.range(),
            Self::Token(t) => t.range(),
        }
    }

    /// The kind of this element.
    pub fn kind(&self) -> SyntaxKind {
        match self {
            Self::Node(n) => n.kind(),
            Self::Token(t) => t.kind(),
        }
    }
}

// =============================================================================
// Parsed
// =============================================================================

/// The result of parsing a docstring.
///
/// Owns the source text and the root [`SyntaxNode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    source: String,
    root: SyntaxNode,
    line_index: LineIndex,
}

impl Parsed {
    /// Creates a new `Parsed` from source text and root node.
    pub fn new(source: String, root: SyntaxNode) -> Self {
        let line_index = LineIndex::new(&source);
        Self {
            source,
            root,
            line_index,
        }
    }

    /// The full source text.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The root node of the syntax tree.
    pub fn root(&self) -> &SyntaxNode {
        &self.root
    }

    /// Convert a byte offset to a [`LineColumn`] position.
    ///
    /// `lineno` is 1-based; `col` is the 0-based byte column within the line.
    pub fn line_col(&self, offset: crate::text::TextSize) -> LineColumn {
        self.line_index.line_col(offset)
    }

    /// Produce a Biome-style pretty-printed representation of the tree.
    pub fn pretty_print(&self) -> String {
        let mut out = String::new();
        self.root.pretty_fmt(&self.source, 0, &mut out);
        out
    }
}

// =============================================================================
// Visitor
// =============================================================================

/// Trait for visiting syntax tree nodes and tokens.
///
/// Implement this trait and pass it to [`walk`] for depth-first traversal.
pub trait Visitor {
    /// Called when entering a node (before visiting its children).
    fn enter(&mut self, _node: &SyntaxNode) {}
    /// Called when leaving a node (after visiting its children).
    fn leave(&mut self, _node: &SyntaxNode) {}
    /// Called for each token leaf.
    fn visit_token(&mut self, _token: &SyntaxToken) {}
}

/// Walk the syntax tree depth-first, calling the visitor methods.
pub fn walk(node: &SyntaxNode, visitor: &mut dyn Visitor) {
    visitor.enter(node);
    for child in node.children() {
        match child {
            SyntaxElement::Node(n) => walk(n, visitor),
            SyntaxElement::Token(t) => visitor.visit_token(t),
        }
    }
    visitor.leave(node);
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::TextRange;
    use crate::text::TextSize;

    #[test]
    fn test_syntax_kind_name() {
        assert_eq!(SyntaxKind::GOOGLE_ARG.name(), "GOOGLE_ARG");
        assert_eq!(SyntaxKind::NAME.name(), "NAME");
        assert_eq!(SyntaxKind::NUMPY_PARAMETER.name(), "NUMPY_PARAMETER");
    }

    #[test]
    fn test_syntax_kind_is_node_is_token() {
        assert!(SyntaxKind::GOOGLE_DOCSTRING.is_node());
        assert!(!SyntaxKind::GOOGLE_DOCSTRING.is_token());
        assert!(SyntaxKind::NAME.is_token());
        assert!(!SyntaxKind::NAME.is_node());
    }

    #[test]
    fn test_syntax_token_text() {
        let source = "hello world";
        let token = SyntaxToken::new(SyntaxKind::NAME, TextRange::new(TextSize::new(0), TextSize::new(5)));
        assert_eq!(token.text(source), "hello");
    }

    #[test]
    fn test_syntax_node_find_token() {
        let node = SyntaxNode::new(
            SyntaxKind::GOOGLE_ARG,
            TextRange::new(TextSize::new(0), TextSize::new(10)),
            vec![
                SyntaxElement::Token(SyntaxToken::new(
                    SyntaxKind::NAME,
                    TextRange::new(TextSize::new(0), TextSize::new(3)),
                )),
                SyntaxElement::Token(SyntaxToken::new(
                    SyntaxKind::COLON,
                    TextRange::new(TextSize::new(3), TextSize::new(4)),
                )),
                SyntaxElement::Token(SyntaxToken::new(
                    SyntaxKind::DESCRIPTION,
                    TextRange::new(TextSize::new(5), TextSize::new(10)),
                )),
            ],
        );

        assert!(node.find_token(SyntaxKind::NAME).is_some());
        assert!(node.find_token(SyntaxKind::COLON).is_some());
        assert!(node.find_token(SyntaxKind::TYPE).is_none());
        assert_eq!(node.tokens(SyntaxKind::NAME).count(), 1);
    }

    #[test]
    fn test_syntax_node_find_node() {
        let child = SyntaxNode::new(
            SyntaxKind::GOOGLE_SECTION_HEADER,
            TextRange::new(TextSize::new(0), TextSize::new(5)),
            vec![SyntaxElement::Token(SyntaxToken::new(
                SyntaxKind::NAME,
                TextRange::new(TextSize::new(0), TextSize::new(4)),
            ))],
        );
        let parent = SyntaxNode::new(
            SyntaxKind::GOOGLE_SECTION,
            TextRange::new(TextSize::new(0), TextSize::new(20)),
            vec![SyntaxElement::Node(child)],
        );

        assert!(parent.find_node(SyntaxKind::GOOGLE_SECTION_HEADER).is_some());
        assert!(parent.find_node(SyntaxKind::GOOGLE_ARG).is_none());
        assert_eq!(parent.nodes(SyntaxKind::GOOGLE_SECTION_HEADER).count(), 1);
    }

    #[test]
    fn test_pretty_print() {
        let source = "Args:\n    x: int";
        let root = SyntaxNode::new(
            SyntaxKind::GOOGLE_DOCSTRING,
            TextRange::new(TextSize::new(0), TextSize::new(source.len() as u32)),
            vec![SyntaxElement::Node(SyntaxNode::new(
                SyntaxKind::GOOGLE_SECTION,
                TextRange::new(TextSize::new(0), TextSize::new(source.len() as u32)),
                vec![
                    SyntaxElement::Node(SyntaxNode::new(
                        SyntaxKind::GOOGLE_SECTION_HEADER,
                        TextRange::new(TextSize::new(0), TextSize::new(5)),
                        vec![
                            SyntaxElement::Token(SyntaxToken::new(
                                SyntaxKind::NAME,
                                TextRange::new(TextSize::new(0), TextSize::new(4)),
                            )),
                            SyntaxElement::Token(SyntaxToken::new(
                                SyntaxKind::COLON,
                                TextRange::new(TextSize::new(4), TextSize::new(5)),
                            )),
                        ],
                    )),
                    SyntaxElement::Node(SyntaxNode::new(
                        SyntaxKind::GOOGLE_ARG,
                        TextRange::new(TextSize::new(10), TextSize::new(source.len() as u32)),
                        vec![
                            SyntaxElement::Token(SyntaxToken::new(
                                SyntaxKind::NAME,
                                TextRange::new(TextSize::new(10), TextSize::new(11)),
                            )),
                            SyntaxElement::Token(SyntaxToken::new(
                                SyntaxKind::COLON,
                                TextRange::new(TextSize::new(11), TextSize::new(12)),
                            )),
                            SyntaxElement::Token(SyntaxToken::new(
                                SyntaxKind::DESCRIPTION,
                                TextRange::new(TextSize::new(13), TextSize::new(source.len() as u32)),
                            )),
                        ],
                    )),
                ],
            ))],
        );

        let parsed = Parsed::new(source.to_string(), root);
        let output = parsed.pretty_print();

        // Verify structure is present
        assert!(output.contains("GOOGLE_DOCSTRING@"));
        assert!(output.contains("GOOGLE_SECTION@"));
        assert!(output.contains("GOOGLE_SECTION_HEADER@"));
        assert!(output.contains("GOOGLE_ARG@"));
        assert!(output.contains("NAME: \"Args\"@"));
        assert!(output.contains("COLON: \":\"@"));
        assert!(output.contains("NAME: \"x\"@"));
        assert!(output.contains("DESCRIPTION: \"int\"@"));
    }

    #[test]
    fn test_visitor_walk() {
        let source = "hello";
        let root = SyntaxNode::new(
            SyntaxKind::GOOGLE_DOCSTRING,
            TextRange::new(TextSize::new(0), TextSize::new(5)),
            vec![SyntaxElement::Token(SyntaxToken::new(
                SyntaxKind::SUMMARY,
                TextRange::new(TextSize::new(0), TextSize::new(5)),
            ))],
        );

        struct Counter {
            nodes: usize,
            tokens: usize,
        }
        impl Visitor for Counter {
            fn enter(&mut self, _node: &SyntaxNode) {
                self.nodes += 1;
            }
            fn visit_token(&mut self, _token: &SyntaxToken) {
                self.tokens += 1;
            }
        }

        let mut counter = Counter { nodes: 0, tokens: 0 };
        walk(&root, &mut counter);
        assert_eq!(counter.nodes, 1);
        assert_eq!(counter.tokens, 1);

        // verify text extraction
        let tok = root.required_token(SyntaxKind::SUMMARY);
        assert_eq!(tok.text(source), "hello");
    }
}
