//! Source-aware token handle returned by the typed views.
//!
//! The raw tree layer keeps token text explicit — [`SyntaxToken::text`]
//! takes the source string, per the source-backed decision on issue #42.
//! The typed views ([`Document`](crate::parse::Document) and friends, the
//! per-style wrappers, [`TextBlock`](crate::parse::TextBlock)) already hold
//! the [`Parsed`] result they came from, so the tokens *they* hand out can
//! bundle it: a [`TokenRef`] is a token plus its parse result, giving
//! `token.text()` without threading `source` through every call.

use crate::syntax::Parsed;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxToken;
use crate::text::TextRange;

/// A [`SyntaxToken`] bundled with the [`Parsed`] result it belongs to.
///
/// Returned by every token accessor on the typed views so that token text is
/// available without passing the source around ([`TokenRef::text`]). The raw
/// token stays reachable via [`TokenRef::syntax`] for code that works on the
/// tree layer.
#[derive(Debug, Clone, Copy)]
pub struct TokenRef<'a> {
    parsed: &'a Parsed,
    token: &'a SyntaxToken,
}

impl<'a> TokenRef<'a> {
    /// Bundle `token` with the [`Parsed`] result it belongs to.
    ///
    /// `token` must come from `parsed`'s tree; a foreign token yields
    /// nonsensical (but memory-safe) [`text`](Self::text) results.
    pub fn new(parsed: &'a Parsed, token: &'a SyntaxToken) -> Self {
        Self { parsed, token }
    }

    /// Access the underlying raw [`SyntaxToken`].
    pub fn syntax(&self) -> &'a SyntaxToken {
        self.token
    }

    /// The kind of this token.
    pub fn kind(&self) -> SyntaxKind {
        self.token.kind()
    }

    /// The source range of this token.
    pub fn range(&self) -> TextRange {
        self.token.range()
    }

    /// Whether this token is missing from the source (zero-length
    /// placeholder). See [`SyntaxToken::is_missing`].
    pub fn is_missing(&self) -> bool {
        self.token.is_missing()
    }

    /// The token's text, sliced from the parse result's source.
    pub fn text(&self) -> &'a str {
        self.token.text(self.parsed.source())
    }
}
