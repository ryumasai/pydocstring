//! Plain docstring parser (SyntaxNode-based).
//!
//! Parses docstrings that contain no NumPy or Google style section markers.
//! Produces a [`Parsed`] with a [`SyntaxKind::DOCUMENT`] root that may
//! contain a [`SyntaxKind::SUMMARY`] node and an
//! [`SyntaxKind::EXTENDED_SUMMARY`] node.

use crate::cursor::LineCursor;
use crate::cursor::indent_len;
use crate::parse::utils::build_text_block;
use crate::syntax::Parsed;
use crate::syntax::SyntaxElement;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::text::TextRange;

// =============================================================================
// Parser
// =============================================================================

/// Build the TextRange covering `first..=last` content lines (trimmed).
fn build_content_range(cursor: &LineCursor, first: Option<usize>, last: usize) -> Option<TextRange> {
    first.map(|f| {
        let first_line = cursor.line_text(f);
        let first_col = indent_len(first_line);
        let last_line = cursor.line_text(last);
        let last_col = indent_len(last_line) + last_line.trim().len();
        cursor.make_range(f, first_col, last, last_col)
    })
}

/// Parse a plain docstring (no NumPy or Google section markers).
///
/// The returned [`Parsed`] has a [`SyntaxKind::DOCUMENT`] root that
/// contains at most one `SUMMARY` node and one `EXTENDED_SUMMARY` node.
/// Unrecognised styles (e.g. Sphinx) are also parsed this way.
///
/// # Example
///
/// ```rust
/// use pydocstring::parse::plain::{parse_plain, nodes::PlainDocstring};
/// use pydocstring::syntax::SyntaxKind;
///
/// let result = parse_plain("Summary.\n\nMore details here.");
/// assert_eq!(result.root().kind(), SyntaxKind::DOCUMENT);
///
/// let doc = PlainDocstring::cast(result.root()).unwrap();
/// assert_eq!(doc.summary().unwrap().text(result.source()), "Summary.");
/// assert_eq!(doc.extended_summary().unwrap().text(result.source()), "More details here.");
/// ```
pub fn parse_plain(input: &str) -> Parsed {
    let mut line_cursor = LineCursor::new(input);
    let mut root_children: Vec<SyntaxElement> = Vec::new();

    line_cursor.skip_blanks();
    if line_cursor.is_eof() {
        let mut root = SyntaxNode::new(SyntaxKind::DOCUMENT, line_cursor.full_range(), root_children);
        crate::parse::trivia::attach_trivia(&mut root, input);
        return Parsed::new(input.to_string(), root, crate::parse::Style::Plain);
    }

    let mut summary_done = false;
    let mut summary_first: Option<usize> = None;
    let mut summary_last: usize = 0;
    let mut ext_first: Option<usize> = None;
    let mut ext_last: usize = 0;

    while !line_cursor.is_eof() {
        if line_cursor.current_trimmed().is_empty() {
            // Blank line: flush summary if not done yet.
            if !summary_done && summary_first.is_some() {
                root_children.push(SyntaxElement::Node(build_text_block(
                    SyntaxKind::SUMMARY,
                    build_content_range(&line_cursor, summary_first, summary_last).unwrap(),
                    input,
                )));
                summary_done = true;
            }
            line_cursor.advance();
            continue;
        }

        if !summary_done {
            if summary_first.is_none() {
                summary_first = Some(line_cursor.line);
            }
            summary_last = line_cursor.line;
        } else {
            if ext_first.is_none() {
                ext_first = Some(line_cursor.line);
            }
            ext_last = line_cursor.line;
        }

        line_cursor.advance();
    }

    // Finalise at EOF.
    if !summary_done && summary_first.is_some() {
        root_children.push(SyntaxElement::Node(build_text_block(
            SyntaxKind::SUMMARY,
            build_content_range(&line_cursor, summary_first, summary_last).unwrap(),
            input,
        )));
    }
    if ext_first.is_some() {
        root_children.push(SyntaxElement::Node(build_text_block(
            SyntaxKind::EXTENDED_SUMMARY,
            build_content_range(&line_cursor, ext_first, ext_last).unwrap(),
            input,
        )));
    }

    let mut root = SyntaxNode::new(SyntaxKind::DOCUMENT, line_cursor.full_range(), root_children);
    crate::parse::trivia::attach_trivia(&mut root, input);
    Parsed::new(input.to_string(), root, crate::parse::Style::Plain)
}
