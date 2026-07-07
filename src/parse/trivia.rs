//! Post-parse pass that tokenizes trivia (whitespace, newlines, blank lines).
//!
//! The parsers produce content tokens only; the bytes between them —
//! indentation, inter-token spacing, line breaks, blank lines — are not
//! represented in the tree. [`attach_trivia`] walks the finished tree and
//! splices flat [`SyntaxKind::WHITESPACE`] / [`SyntaxKind::NEWLINE`] /
//! [`SyntaxKind::BLANK_LINE`] tokens into the child list of the deepest node
//! whose range covers each gap, so the CST structurally accounts for every
//! whitespace byte of the source.
//!
//! Placement falls out of the recursion: a gap between two siblings becomes
//! trivia children of their common parent, and leading/trailing blank lines
//! live at the docstring root, whose range spans the whole input.
//!
//! Non-whitespace bytes found in a gap are content that a parser dropped —
//! always a bug, outlawed by the coverage law (`tests/coverage.rs`, #39).
//! They are deliberately left uncovered — the whitespace runs around them
//! are tokenized normally — so that the coverage law exposes them instead
//! of having them masked by a trivia token.

use crate::syntax::SyntaxElement;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::syntax::SyntaxToken;
use crate::text::TextRange;
use crate::text::TextSize;

/// Splice trivia tokens into `root` for every gap between content tokens.
///
/// Called by each style parser as its final step, just before the tree is
/// wrapped in a [`Parsed`](crate::syntax::Parsed). Node ranges are unchanged,
/// except that the root's range is extended to the end of `source` when the
/// parser left a trailing newline (or trailing blank lines) outside of it.
pub(crate) fn attach_trivia(root: &mut SyntaxNode, source: &str) {
    let src_end = TextSize::new(source.len() as u32);
    if root.range().end() < src_end {
        root.extend_range_to(src_end);
    }
    attach(root, source);
}

/// Recursively splice trivia into `node`'s child list.
///
/// Gaps are the bytes inside the node's range that no child covers. They
/// are computed over the child ranges *sorted by position*: some parsers
/// store entry children in canonical rather than source order (e.g. a
/// google-style `name (type):` entry in a NumPy section lists `COLON`
/// before `TYPE`), and a later-listed child must still shield its bytes
/// from being lexed as trivia.
fn attach(node: &mut SyntaxNode, source: &str) {
    let mut old = node.take_children();
    for child in &mut old {
        if let SyntaxElement::Node(n) = child {
            attach(n, source);
        }
    }

    let mut ranges: Vec<TextRange> = old.iter().map(|c| *c.range()).collect();
    ranges.sort_by_key(|r| (r.start(), r.end()));
    let mut trivia: Vec<SyntaxElement> = Vec::new();
    let mut pos = usize::from(node.range().start());
    for range in ranges {
        let start = usize::from(range.start());
        if pos < start {
            lex_gap(source, pos, start, &mut trivia);
        }
        pos = pos.max(usize::from(range.end()));
    }
    let end = usize::from(node.range().end());
    if pos < end {
        lex_gap(source, pos, end, &mut trivia);
    }

    // Merge the (position-sorted) trivia into the child list: each trivia
    // token goes before the first remaining child that starts after it.
    let mut children = Vec::with_capacity(old.len() + trivia.len());
    let mut pending = trivia.into_iter().peekable();
    for child in old {
        while pending
            .peek()
            .is_some_and(|t| t.range().start() < child.range().start())
        {
            children.push(pending.next().unwrap());
        }
        children.push(child);
    }
    children.extend(pending);
    node.set_children(children);
}

/// Lex `source[start..end]` into trivia tokens appended to `out`.
///
/// Line-start detection consults `source` (offset 0 or a preceding `\n`),
/// not the gap slice, so indentation and blank lines are classified
/// correctly regardless of where the gap begins.
fn lex_gap(source: &str, start: usize, end: usize, out: &mut Vec<SyntaxElement>) {
    let bytes = source.as_bytes();
    let mut pos = start;
    while pos < end {
        // Blank line: at a line start, a whitespace-only line taken whole,
        // including its terminating newline (absent only at end of input).
        if (pos == 0 || bytes[pos - 1] == b'\n')
            && let Some(line_end) = blank_line_end(bytes, pos, end)
        {
            push(out, SyntaxKind::BLANK_LINE, pos, line_end);
            pos = line_end;
            continue;
        }
        match bytes[pos] {
            b'\n' => {
                push(out, SyntaxKind::NEWLINE, pos, pos + 1);
                pos += 1;
            }
            b'\r' if pos + 1 < end && bytes[pos + 1] == b'\n' => {
                push(out, SyntaxKind::NEWLINE, pos, pos + 2);
                pos += 2;
            }
            b' ' | b'\t' | b'\r' => {
                let run_start = pos;
                while pos < end && matches!(bytes[pos], b' ' | b'\t' | b'\r') {
                    if bytes[pos] == b'\r' && pos + 1 < end && bytes[pos + 1] == b'\n' {
                        // Leave `\r\n` to become a NEWLINE token.
                        break;
                    }
                    pos += 1;
                }
                push(out, SyntaxKind::WHITESPACE, run_start, pos);
            }
            _ => {
                // Non-whitespace bytes in a gap: content dropped by the
                // parser (#39). Skip without emitting a token.
                while pos < end && !matches!(bytes[pos], b' ' | b'\t' | b'\r' | b'\n') {
                    pos += 1;
                }
            }
        }
    }
}

/// If the line starting at `pos` is whitespace-only and lies entirely within
/// the gap `[pos, end)`, return the end offset of its `BLANK_LINE` token
/// (past the terminating newline, or at end of input for an unterminated
/// final line).
///
/// The line is scanned in the full source: a blank line whose bytes extend
/// past `end` is not claimed here (the caller falls back to `WHITESPACE` /
/// `NEWLINE` tokens for the covered part).
fn blank_line_end(bytes: &[u8], pos: usize, end: usize) -> Option<usize> {
    let mut i = pos;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
        i += 1;
    }
    let term = match bytes.get(i) {
        Some(b'\n') => i + 1,
        Some(b'\r') if bytes.get(i + 1) == Some(&b'\n') => i + 2,
        // Non-whitespace on this line: not a blank line.
        Some(_) => return None,
        // Whitespace-only final line without a newline; zero-width lines
        // at end of input are nothing at all, not a blank line.
        None if i > pos => i,
        None => return None,
    };
    (term <= end).then_some(term)
}

/// Append a trivia token covering `source[start..end]`.
fn push(out: &mut Vec<SyntaxElement>, kind: SyntaxKind, start: usize, end: usize) {
    out.push(SyntaxElement::Token(SyntaxToken::new(
        kind,
        TextRange::new(TextSize::new(start as u32), TextSize::new(end as u32)),
    )));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collect `(kind, text)` for the tokens produced by lexing the whole of
    /// `source` as one gap.
    fn lex_all(source: &str) -> Vec<(SyntaxKind, String)> {
        let mut out = Vec::new();
        lex_gap(source, 0, source.len(), &mut out);
        out.iter()
            .map(|e| match e {
                SyntaxElement::Token(t) => (t.kind(), t.text(source).to_owned()),
                SyntaxElement::Node(_) => unreachable!("lex_gap emits tokens only"),
            })
            .collect()
    }

    #[test]
    fn test_lex_gap_newline_then_blank_line() {
        // A newline that terminates a content line is NEWLINE; the following
        // zero-width line is BLANK_LINE.
        let src = "x\n\ny";
        let mut out = Vec::new();
        lex_gap(src, 1, 3, &mut out);
        let kinds: Vec<_> = out.iter().map(|e| e.kind()).collect();
        assert_eq!(kinds, vec![SyntaxKind::NEWLINE, SyntaxKind::BLANK_LINE]);
    }

    #[test]
    fn test_lex_gap_whitespace_only_line_is_blank_line_with_newline() {
        assert_eq!(lex_all("  \t\n"), vec![(SyntaxKind::BLANK_LINE, "  \t\n".to_owned())]);
    }

    #[test]
    fn test_lex_gap_consecutive_blank_lines_one_token_each() {
        assert_eq!(
            lex_all("\n \n\n"),
            vec![
                (SyntaxKind::BLANK_LINE, "\n".to_owned()),
                (SyntaxKind::BLANK_LINE, " \n".to_owned()),
                (SyntaxKind::BLANK_LINE, "\n".to_owned()),
            ]
        );
    }

    #[test]
    fn test_lex_gap_trailing_whitespace_at_eof_is_blank_line_without_newline() {
        assert_eq!(
            lex_all("\n   "),
            vec![
                (SyntaxKind::BLANK_LINE, "\n".to_owned()),
                (SyntaxKind::BLANK_LINE, "   ".to_owned()),
            ]
        );
    }

    #[test]
    fn test_lex_gap_indentation_before_content_is_whitespace() {
        // Line-start whitespace followed (in the source) by content is plain
        // WHITESPACE, even though the content is outside the gap.
        let src = "    x";
        let mut out = Vec::new();
        lex_gap(src, 0, 4, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind(), SyntaxKind::WHITESPACE);
    }

    #[test]
    fn test_lex_gap_skips_dropped_content() {
        // Non-whitespace gap bytes stay uncovered; surrounding whitespace is
        // tokenized normally.
        assert_eq!(
            lex_all("dropped words\n"),
            vec![
                (SyntaxKind::WHITESPACE, " ".to_owned()),
                (SyntaxKind::NEWLINE, "\n".to_owned()),
            ]
        );
    }

    #[test]
    fn test_lex_gap_crlf_is_one_newline_token() {
        assert_eq!(lex_all("x\r\n"), vec![(SyntaxKind::NEWLINE, "\r\n".to_owned())]);
    }
}
