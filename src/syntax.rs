//! Unified syntax tree types.
//!
//! This module defines the core tree data structures shared by all docstring
//! styles.  Every parsed docstring is represented as a tree of [`SyntaxNode`]s
//! (branches) and [`SyntaxToken`]s (leaves), each tagged with a [`SyntaxKind`].
//!
//! The [`Parsed`] struct owns the source text and the root node, and provides
//! a convenience [`pretty_print`](Parsed::pretty_print) method for debugging.
//!
//! # Missing placeholders
//!
//! A zero-length element is a **missing placeholder**: the parsers insert one
//! wherever a syntactically expected element is absent from the source (e.g.
//! the `TYPE` in `a ()`, the `CLOSE_BRACKET` in `a (int`, or the
//! `DESCRIPTION` in `a (int):`). The equivalence is exact — zero-length ⇔
//! missing placeholder — and placeholders sit at the offset where the missing
//! element would be inserted, making them the edit API's insertion anchors.
//! Placeholders are only ever *replaced* by a real element, never extended in
//! place, and trivia tokens are never zero-length. [`Parsed::pretty_print`]
//! renders a missing token as `<missing>`; use [`SyntaxToken::is_missing`] /
//! [`SyntaxNode::find_missing`] to detect them programmatically. The
//! invariants are pinned corpus-wide in `tests/trivia.rs`.

use core::fmt;
use core::fmt::Write;

use crate::parse::Style;
use crate::text::LineColumn;
use crate::text::LineIndex;
use crate::text::TextRange;

// =============================================================================
// SyntaxKind
// =============================================================================

/// Node and token kinds for all docstring styles.
///
/// Node kinds are style-neutral: a Google-style and a NumPy-style docstring
/// produce the same [`SyntaxKind::DOCUMENT`] / [`SyntaxKind::SECTION`] /
/// [`SyntaxKind::ENTRY`] shapes. Style differences live only in the section
/// header ([`SyntaxKind::COLON`] vs [`SyntaxKind::UNDERLINE`]) and in the
/// parsers/renderers; [`Parsed::style`] reports the source style. The
/// vocabulary borrows reST concepts (document, section, directive, citation)
/// so a future reST-flavoured parser can join the same tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
#[non_exhaustive]
pub enum SyntaxKind {
    // ── Common tokens ──────────────────────────────────────────────────
    /// Section name, parameter/attribute name, or a return/yield name
    /// (exception and warning entries carry a [`SyntaxKind::TYPE`]
    /// instead of a name).
    NAME,
    /// Type annotation: a parameter/attribute type, a return/yield type,
    /// or the exception/warning type name of a raises/warns entry.
    TYPE,
    /// `:` separator.
    COLON,
    /// `,` separator: between multiple names, or before an `optional` /
    /// `default …` marker inside a type annotation.
    COMMA,
    /// Prose text block (node wrapping one [`SyntaxKind::TEXT_LINE`] token
    /// per content line): an entry/directive description, a free-text
    /// section body, or a citation's content.
    DESCRIPTION,
    /// Opening bracket: `(`, `[`, `{`, or `<`.
    OPEN_BRACKET,
    /// Closing bracket: `)`, `]`, `}`, or `>`.
    CLOSE_BRACKET,
    /// `optional` marker.
    OPTIONAL,
    /// Summary block (node wrapping one [`SyntaxKind::TEXT_LINE`] token per
    /// content line).
    SUMMARY,
    /// Extended summary paragraph (node wrapping one
    /// [`SyntaxKind::TEXT_LINE`] token per content line).
    EXTENDED_SUMMARY,
    /// The content span of one line inside a text block node
    /// ([`SyntaxKind::SUMMARY`], [`SyntaxKind::EXTENDED_SUMMARY`],
    /// [`SyntaxKind::DESCRIPTION`], [`SyntaxKind::PARAGRAPH`]): excludes
    /// leading indentation and the trailing newline. Never contains a
    /// newline.
    TEXT_LINE,

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

    // ── reST-flavoured tokens ──────────────────────────────────────────
    /// Section header underline (`----------`, NumPy-style headers).
    UNDERLINE,
    /// RST directive marker (`..`).
    DIRECTIVE_MARKER,
    /// Directive name such as `deprecated`.
    DIRECTIVE_NAME,
    /// RST double colon (`::`).
    DOUBLE_COLON,
    /// Directive argument (e.g. the version of a `.. deprecated::`).
    ARGUMENT,
    /// `default` keyword.
    DEFAULT_KEYWORD,
    /// Default value separator (`=` or `:`).
    DEFAULT_SEPARATOR,
    /// Default value text.
    DEFAULT_VALUE,
    /// Citation label (`1`, `CIT2002`, `#f1`, … inside `.. [label]`).
    LABEL,

    // ── Nodes (style-neutral) ──────────────────────────────────────────
    /// Root node of a parsed docstring, whatever its style.
    /// Use [`Parsed::style`] to recover the source style.
    DOCUMENT,
    /// A complete section (header + body items).
    SECTION,
    /// Section header (`Args:` in Google style, name + underline in
    /// NumPy style).
    SECTION_HEADER,
    /// A single section body entry (argument/parameter, return, yield,
    /// exception, warning, attribute, method, or "See Also" item).
    /// Corresponds to a reST `definition_list_item`: NAME ≈ term,
    /// TYPE ≈ classifier, DESCRIPTION ≈ definition.
    ENTRY,
    /// An rST directive block (currently only `.. deprecated:: <version>`;
    /// the version is an [`SyntaxKind::ARGUMENT`] token).
    DIRECTIVE,
    /// A citation/footnote entry in a References section
    /// (`.. [label] content`).
    CITATION,
    /// One `default …` marker occurrence inside a type annotation, wrapping
    /// its [`SyntaxKind::DEFAULT_KEYWORD`], optional
    /// [`SyntaxKind::DEFAULT_SEPARATOR`], and [`SyntaxKind::DEFAULT_VALUE`]
    /// tokens. Markers are repeatable: `x : int, default 1, default 2`
    /// produces one `DEFAULT` node per occurrence, in source order (which
    /// occurrence *wins* is a model-layer rule: the first).
    DEFAULT,
    /// A paragraph of stray prose lines between sections (node wrapping one
    /// [`SyntaxKind::TEXT_LINE`] token per content line, like the other text
    /// block kinds). Consecutive stray lines separated only by a newline form
    /// one paragraph; a blank line splits paragraphs (reST semantics).
    PARAGRAPH,
}

impl SyntaxKind {
    /// Whether this kind represents a node (branch) rather than a token (leaf).
    pub const fn is_node(self) -> bool {
        matches!(
            self,
            Self::SUMMARY
                | Self::EXTENDED_SUMMARY
                | Self::DESCRIPTION
                | Self::DOCUMENT
                | Self::SECTION
                | Self::SECTION_HEADER
                | Self::ENTRY
                | Self::DIRECTIVE
                | Self::CITATION
                | Self::DEFAULT
                | Self::PARAGRAPH
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

    /// Display name for pretty-printing (e.g. `"ENTRY"`, `"NAME"`).
    pub const fn name(self) -> &'static str {
        match self {
            // Common tokens
            Self::NAME => "NAME",
            Self::TYPE => "TYPE",
            Self::COLON => "COLON",
            Self::COMMA => "COMMA",
            Self::DESCRIPTION => "DESCRIPTION",
            Self::OPEN_BRACKET => "OPEN_BRACKET",
            Self::CLOSE_BRACKET => "CLOSE_BRACKET",
            Self::OPTIONAL => "OPTIONAL",
            Self::SUMMARY => "SUMMARY",
            Self::EXTENDED_SUMMARY => "EXTENDED_SUMMARY",
            Self::TEXT_LINE => "TEXT_LINE",
            // Trivia tokens
            Self::WHITESPACE => "WHITESPACE",
            Self::NEWLINE => "NEWLINE",
            Self::BLANK_LINE => "BLANK_LINE",
            // reST-flavoured tokens
            Self::UNDERLINE => "UNDERLINE",
            Self::DIRECTIVE_MARKER => "DIRECTIVE_MARKER",
            Self::DIRECTIVE_NAME => "DIRECTIVE_NAME",
            Self::DOUBLE_COLON => "DOUBLE_COLON",
            Self::ARGUMENT => "ARGUMENT",
            Self::DEFAULT_KEYWORD => "DEFAULT_KEYWORD",
            Self::DEFAULT_SEPARATOR => "DEFAULT_SEPARATOR",
            Self::DEFAULT_VALUE => "DEFAULT_VALUE",
            Self::LABEL => "LABEL",
            // Nodes
            Self::DOCUMENT => "DOCUMENT",
            Self::SECTION => "SECTION",
            Self::SECTION_HEADER => "SECTION_HEADER",
            Self::ENTRY => "ENTRY",
            Self::DIRECTIVE => "DIRECTIVE",
            Self::CITATION => "CITATION",
            Self::DEFAULT => "DEFAULT",
            Self::PARAGRAPH => "PARAGRAPH",
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
    ///
    /// Crate-private: mutating children can break the coverage/containment
    /// invariants that [`Parsed`] guarantees; only the parsers may do so.
    pub(crate) fn children_mut(&mut self) -> &mut [SyntaxElement] {
        &mut self.children
    }

    /// Append a child element.
    ///
    /// Crate-private: see [`children_mut`](Self::children_mut).
    pub(crate) fn push_child(&mut self, child: SyntaxElement) {
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
    ///
    /// Crate-private: see [`children_mut`](Self::children_mut).
    pub(crate) fn extend_range_to(&mut self, end: crate::text::TextSize) {
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
    /// Unlike [`find_token`](Self::find_token) this also matches zero-length
    /// (missing) tokens,
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

    /// Iterate over the *present* token children with the given kind.
    ///
    /// Zero-length missing placeholders are excluded, exactly as
    /// [`find_token`](SyntaxNode::find_token) excludes them — the two are the
    /// plural and singular form of the same question. Reach a placeholder with
    /// [`find_missing`](SyntaxNode::find_missing), which is the only accessor
    /// that returns one.
    pub fn tokens(&self, kind: SyntaxKind) -> impl Iterator<Item = &SyntaxToken> {
        self.children.iter().filter_map(move |c| match c {
            SyntaxElement::Token(t) if t.kind() == kind && !t.is_missing() => Some(t),
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
    ///
    /// A missing token marks the exact offset where the absent element would
    /// be inserted — the edit API's insertion anchor (see the
    /// [module docs](self#missing-placeholders)). Placeholders are only ever
    /// replaced by a real token, never extended in place.
    pub fn is_missing(&self) -> bool {
        self.range.is_empty()
    }

    /// Extract the corresponding text slice from source.
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        self.range.source_text(source)
    }

    /// Write a pretty-printed token line.
    ///
    /// A zero-length (missing placeholder) token renders as `<missing>`
    /// instead of an empty text literal.
    pub fn pretty_fmt(&self, src: &str, indent: usize, out: &mut String) {
        for _ in 0..indent {
            out.push_str("  ");
        }
        if self.is_missing() {
            let _ = writeln!(out, "{}: <missing>@{}", self.kind.name(), self.range);
        } else {
            let _ = writeln!(out, "{}: {:?}@{}", self.kind.name(), self.text(src), self.range);
        }
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
/// Owns the source text and the root [`SyntaxNode`], and records the
/// [`Style`] the docstring was parsed as (the root node kind is the
/// style-neutral [`SyntaxKind::DOCUMENT`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    source: String,
    root: SyntaxNode,
    style: Style,
    line_index: LineIndex,
}

impl Parsed {
    /// Creates a new `Parsed` from source text, root node, and source style.
    ///
    /// # Invariants
    ///
    /// A `Parsed` is expected to originate from one of this crate's parsers
    /// ([`parse`](crate::parse::parse) or a per-style `parse_*` function).
    /// Constructing one by hand is possible but the tree must then uphold the
    /// laws the parsers guarantee and downstream consumers rely on:
    ///
    /// - every element's range lies within `source` and within its parent's
    ///   range, and siblings appear in source order (containment/ordering);
    /// - a node's children plus trivia tokens exactly cover the node's range
    ///   (coverage — pinned corpus-wide in `tests/trivia.rs`);
    /// - token text is a slice of `source` (no synthesized text, see the
    ///   source-backed decision on issue #42);
    /// - zero-length elements are missing placeholders and nothing else.
    ///
    /// A hand-built tree that violates these laws produces unspecified (but
    /// memory-safe) results from the typed views, the visitor, and `to_model`.
    pub fn new(source: String, root: SyntaxNode, style: Style) -> Self {
        let line_index = LineIndex::new(&source);
        Self {
            source,
            root,
            style,
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

    /// The style this docstring was parsed as.
    pub fn style(&self) -> Style {
        self.style
    }

    /// Convert a byte offset to a [`LineColumn`] position.
    ///
    /// `lineno` is 1-based; `col` is the 0-based byte column within the line.
    pub fn line_col(&self, offset: crate::text::TextSize) -> LineColumn {
        self.line_index.line_col(offset)
    }

    /// Produce a Biome-style pretty-printed representation of the tree.
    ///
    /// This is a debugging aid: the exact output format is not stable and may
    /// change in any release — do not parse or snapshot it in downstream code.
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
/// Implement this trait and pass it to [`walk_tree`] for depth-first
/// traversal.
pub trait Visitor {
    /// Called when entering a node (before visiting its children).
    fn enter(&mut self, _node: &SyntaxNode) {}
    /// Called when leaving a node (after visiting its children).
    fn leave(&mut self, _node: &SyntaxNode) {}
    /// Called for each token leaf.
    fn visit_token(&mut self, _token: &SyntaxToken) {}
}

/// Walk the raw syntax tree depth-first, calling the visitor methods.
///
/// The traversal is kind-agnostic: it visits every node and token in the
/// tree, and the [`Visitor`] decides what to do with each [`SyntaxKind`].
pub fn walk_tree(node: &SyntaxNode, visitor: &mut dyn Visitor) {
    visitor.enter(node);
    for child in node.children() {
        match child {
            SyntaxElement::Node(n) => walk_tree(n, visitor),
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
        assert_eq!(SyntaxKind::ENTRY.name(), "ENTRY");
        assert_eq!(SyntaxKind::NAME.name(), "NAME");
        assert_eq!(SyntaxKind::SECTION.name(), "SECTION");
    }

    #[test]
    fn test_syntax_kind_is_node_is_token() {
        assert!(SyntaxKind::DOCUMENT.is_node());
        assert!(!SyntaxKind::DOCUMENT.is_token());
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
            SyntaxKind::ENTRY,
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
                    SyntaxKind::TEXT_LINE,
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
            SyntaxKind::SECTION_HEADER,
            TextRange::new(TextSize::new(0), TextSize::new(5)),
            vec![SyntaxElement::Token(SyntaxToken::new(
                SyntaxKind::NAME,
                TextRange::new(TextSize::new(0), TextSize::new(4)),
            ))],
        );
        let parent = SyntaxNode::new(
            SyntaxKind::SECTION,
            TextRange::new(TextSize::new(0), TextSize::new(20)),
            vec![SyntaxElement::Node(child)],
        );

        assert!(parent.find_node(SyntaxKind::SECTION_HEADER).is_some());
        assert!(parent.find_node(SyntaxKind::ENTRY).is_none());
        assert_eq!(parent.nodes(SyntaxKind::SECTION_HEADER).count(), 1);
    }

    #[test]
    fn test_pretty_print() {
        let source = "Args:\n    x: int";
        let root = SyntaxNode::new(
            SyntaxKind::DOCUMENT,
            TextRange::new(TextSize::new(0), TextSize::new(source.len() as u32)),
            vec![SyntaxElement::Node(SyntaxNode::new(
                SyntaxKind::SECTION,
                TextRange::new(TextSize::new(0), TextSize::new(source.len() as u32)),
                vec![
                    SyntaxElement::Node(SyntaxNode::new(
                        SyntaxKind::SECTION_HEADER,
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
                        SyntaxKind::ENTRY,
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
                                SyntaxKind::TEXT_LINE,
                                TextRange::new(TextSize::new(13), TextSize::new(source.len() as u32)),
                            )),
                        ],
                    )),
                ],
            ))],
        );

        let parsed = Parsed::new(source.to_string(), root, crate::parse::Style::Google);
        let output = parsed.pretty_print();

        // Verify structure is present
        assert!(output.contains("DOCUMENT@"));
        assert!(output.contains("SECTION@"));
        assert!(output.contains("SECTION_HEADER@"));
        assert!(output.contains("ENTRY@"));
        assert!(output.contains("NAME: \"Args\"@"));
        assert!(output.contains("COLON: \":\"@"));
        assert!(output.contains("NAME: \"x\"@"));
        assert!(output.contains("TEXT_LINE: \"int\"@"));
    }

    #[test]
    fn test_pretty_print_missing_placeholder() {
        // A zero-length token renders as `<missing>`, not as `""`.
        let source = "a ()";
        let mut out = String::new();
        SyntaxToken::new(SyntaxKind::TYPE, TextRange::new(TextSize::new(3), TextSize::new(3)))
            .pretty_fmt(source, 0, &mut out);
        assert_eq!(out, "TYPE: <missing>@3..3\n");
    }

    #[test]
    fn test_token_accessors_partition_present_from_missing() {
        // `find_token` and `tokens` are the singular and plural form of the same
        // question — "which tokens of this kind are *present*?" — so both exclude
        // zero-length placeholders. `find_missing` is the only door to one.
        let present = SyntaxToken::new(SyntaxKind::NAME, TextRange::new(TextSize::new(0), TextSize::new(1)));
        let missing = SyntaxToken::new(SyntaxKind::TYPE, TextRange::new(TextSize::new(3), TextSize::new(3)));
        let node = SyntaxNode::new(
            SyntaxKind::ENTRY,
            TextRange::new(TextSize::new(0), TextSize::new(4)),
            vec![SyntaxElement::Token(present), SyntaxElement::Token(missing)],
        );

        assert!(node.find_token(SyntaxKind::TYPE).is_none());
        assert_eq!(node.tokens(SyntaxKind::TYPE).count(), 0);
        assert!(node.find_missing(SyntaxKind::TYPE).is_some());

        assert!(node.find_token(SyntaxKind::NAME).is_some());
        assert_eq!(node.tokens(SyntaxKind::NAME).count(), 1);
        assert!(node.find_missing(SyntaxKind::NAME).is_none());
    }

    #[test]
    fn test_visitor_walk() {
        let source = "hello";
        let root = SyntaxNode::new(
            SyntaxKind::DOCUMENT,
            TextRange::new(TextSize::new(0), TextSize::new(5)),
            vec![SyntaxElement::Token(SyntaxToken::new(
                SyntaxKind::TEXT_LINE,
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
        walk_tree(&root, &mut counter);
        assert_eq!(counter.nodes, 1);
        assert_eq!(counter.tokens, 1);

        // verify text extraction
        let tok = root.required_token(SyntaxKind::TEXT_LINE);
        assert_eq!(tok.text(source), "hello");
    }
}
