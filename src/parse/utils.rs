//! Shared utilities for docstring style parsers.

use crate::cursor::LineCursor;
use crate::cursor::indent_columns;
use crate::cursor::indent_len;
use crate::syntax::SyntaxElement;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::syntax::SyntaxToken;
use crate::text::TextRange;

// =============================================================================
// Text block builders (shared by all parsers)
// =============================================================================

/// Build a text block node of `kind` wrapping one [`SyntaxKind::TEXT_LINE`]
/// token per content line of `source[range]`.
///
/// `range` must span from the start of the first content line's text to the
/// end of the last content line's text (the parsers' trimmed content-range
/// convention). Interior indentation, newlines, and blank lines are *not*
/// tokenized here; the trivia pass fills those gaps inside the node.
pub(crate) fn build_text_block(kind: SyntaxKind, range: TextRange, source: &str) -> SyntaxNode {
    let start = usize::from(range.start());
    let end = usize::from(range.end());
    let mut children = Vec::new();
    let mut line_start = start;
    for line in source[start..end].split_inclusive('\n') {
        let content = line.trim();
        if !content.is_empty() {
            let lead = line.len() - line.trim_start().len();
            children.push(SyntaxElement::Token(SyntaxToken::new(
                SyntaxKind::TEXT_LINE,
                TextRange::from_offset_len(line_start + lead, content.len()),
            )));
        }
        line_start += line.len();
    }
    SyntaxNode::new(kind, range, children)
}

/// Build a single-line text block node of `kind` whose one `TEXT_LINE`
/// token covers exactly `range` (a trimmed span within one line).
pub(crate) fn text_block_single(kind: SyntaxKind, range: TextRange) -> SyntaxNode {
    let children = vec![SyntaxElement::Token(SyntaxToken::new(SyntaxKind::TEXT_LINE, range))];
    SyntaxNode::new(kind, range, children)
}

/// Build a zero-length placeholder text block node of `kind` at `pos`,
/// representing a syntactically missing element (no `TEXT_LINE` children).
pub(crate) fn missing_text_block(kind: SyntaxKind, pos: crate::text::TextSize) -> SyntaxNode {
    SyntaxNode::new(kind, TextRange::new(pos, pos), Vec::new())
}

/// Append a continuation line to an existing text block node: push a
/// `TEXT_LINE` token covering `cont` (a trimmed single-line span) and extend
/// the node's range to include it.
pub(crate) fn extend_text_block(block: &mut SyntaxNode, cont: TextRange) {
    block.push_child(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::TEXT_LINE, cont)));
    block.extend_range_to(cont.end());
}

// =============================================================================
// Deprecation directive parsing (shared by the NumPy and Google parsers)
// =============================================================================

/// Collect a directive body: the following lines that are blank or indented
/// deeper than `entry_indent_cols`. Advances the cursor past the collected
/// lines and returns the range spanning the first to last content line.
pub(crate) fn collect_description(cursor: &mut LineCursor, entry_indent_cols: usize) -> Option<TextRange> {
    let mut first_content_line: Option<usize> = None;
    let mut last_content_line = cursor.line;

    while !cursor.is_eof() {
        let line = cursor.current_line_text();
        if !line.trim().is_empty() && indent_columns(line) <= entry_indent_cols {
            break;
        }
        if !line.trim().is_empty() {
            if first_content_line.is_none() {
                first_content_line = Some(cursor.line);
            }
            last_content_line = cursor.line;
        }
        cursor.advance();
    }

    first_content_line.map(|first| {
        let first_line = cursor.line_text(first);
        let first_col = indent_len(first_line);
        let last_line = cursor.line_text(last_content_line);
        let last_col = indent_len(last_line) + last_line.trim().len();
        cursor.make_range(first, first_col, last_content_line, last_col)
    })
}

/// Try to parse an rST `.. deprecated:: <version>` directive at `cursor.line`.
///
/// On success, builds a deprecation node of `node_kind` (`NUMPY_DEPRECATION`
/// or `GOOGLE_DEPRECATION`) with `DIRECTIVE_MARKER`, `KEYWORD`, `DOUBLE_COLON`,
/// `VERSION`, and an optional `DESCRIPTION` collected from the following
/// more-indented lines, and advances the cursor past the directive.
/// Returns `None` (without advancing) if the current line is not a
/// deprecation directive.
pub(crate) fn try_parse_deprecation_directive(cursor: &mut LineCursor, node_kind: SyntaxKind) -> Option<SyntaxNode> {
    let line = cursor.current_line_text();
    let trimmed = line.trim();
    if !trimmed.starts_with(".. deprecated::") {
        return None;
    }

    let col = cursor.current_indent();
    let prefix = ".. deprecated::";
    let after_prefix = &trimmed[prefix.len()..];
    let ws_len = after_prefix.len() - after_prefix.trim_start().len();
    let version_str = after_prefix.trim();
    let version_col = col + prefix.len() + ws_len;

    let mut dep_children: Vec<SyntaxElement> = Vec::new();

    // `..` at col..col+2
    dep_children.push(SyntaxElement::Token(SyntaxToken::new(
        SyntaxKind::DIRECTIVE_MARKER,
        cursor.make_line_range(cursor.line, col, 2),
    )));
    // `deprecated` at col+3..col+13
    dep_children.push(SyntaxElement::Token(SyntaxToken::new(
        SyntaxKind::KEYWORD,
        cursor.make_line_range(cursor.line, col + 3, 10),
    )));
    // `::` at col+13..col+15
    dep_children.push(SyntaxElement::Token(SyntaxToken::new(
        SyntaxKind::DOUBLE_COLON,
        cursor.make_line_range(cursor.line, col + 13, 2),
    )));

    let version_range = cursor.make_line_range(cursor.line, version_col, version_str.len());
    dep_children.push(SyntaxElement::Token(SyntaxToken::new(
        SyntaxKind::VERSION,
        version_range,
    )));

    let dep_start_line = cursor.line;
    cursor.advance();

    let desc_range = collect_description(cursor, indent_columns(line));

    if let Some(desc) = desc_range {
        dep_children.push(SyntaxElement::Node(build_text_block(
            SyntaxKind::DESCRIPTION,
            desc,
            cursor.source(),
        )));
    }

    // Compute deprecation span
    let (dep_end_line, dep_end_col) = match desc_range {
        None => (dep_start_line, col + trimmed.len()),
        Some(d) => cursor.offset_to_line_col(d.end().raw() as usize),
    };

    let dep_range = cursor.make_range(dep_start_line, col, dep_end_line, dep_end_col);
    Some(SyntaxNode::new(node_kind, dep_range, dep_children))
}

/// Find the byte offset of the first entry-separating colon in `text`.
///
/// Skips colons that appear inside balanced brackets (`()`, `[]`, `{}`, `<>`)
/// so that type annotations such as `Dict[str, int]` never trigger a false split.
pub(crate) fn find_entry_colon(text: &str) -> Option<usize> {
    let mut depth: u32 = 0;
    for (i, b) in text.bytes().enumerate() {
        match b {
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => depth = depth.saturating_sub(1),
            b':' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Find the byte offset of the definition-list term/classifier separator colon.
///
/// NumPy and Google entries separate a term from its classifier (type) with a
/// colon, following the reStructuredText convention. A colon only acts as a
/// separator when it is either:
///
/// * preceded by whitespace (`name : type`), or
/// * directly attached to a single top-level token (`name:type`).
///
/// A colon embedded in multi-word prose (`Description with attributes:`) or one
/// that begins an rST role at the start of the line (`:attr:`...``) is **not** a
/// separator, so the line is kept intact. Colons inside balanced brackets are
/// skipped, just like [`find_entry_colon`].
pub(crate) fn find_term_colon(text: &str) -> Option<usize> {
    // A line that begins with a colon is an rST role / field marker, never a term.
    if text.trim_start().starts_with(':') {
        return None;
    }

    let bytes = text.as_bytes();
    let mut depth: u32 = 0;
    let mut top_level_ws = false;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => depth = depth.saturating_sub(1),
            b':' if depth == 0 => {
                // `name : type` (whitespace before) always separates; an attached
                // colon only separates when the term is a single top-level token,
                // so prose like `Some words:` is left intact.
                let ws_before = i > 0 && bytes[i - 1].is_ascii_whitespace();
                return if ws_before || !top_level_ws { Some(i) } else { None };
            }
            _ if depth == 0 && b.is_ascii_whitespace() => top_level_ws = true,
            _ => {}
        }
    }
    None
}

/// Split `text` by top-level commas (respecting `()`, `[]`, `{}`, and `<>` depth).
///
/// Returns a `Vec` of `(byte_offset, segment)` pairs where
/// `byte_offset` is the start position of each segment within `text`.
pub(crate) fn split_comma_parts(text: &str) -> Vec<(usize, &str)> {
    let mut parts = Vec::new();
    let mut depth: u32 = 0;
    let mut start = 0;

    for (i, b) in text.bytes().enumerate() {
        match b {
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                parts.push((start, &text[start..i]));
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push((start, &text[start..]));
    parts
}

/// Byte offsets of the *separator* commas in a type annotation `text`:
/// the top-level commas at or after `clean_len`, the end of the clean type
/// (the part left once trailing `optional` / `default …` marker segments
/// are stripped).
///
/// These commas become `COMMA` tokens; commas *before* the boundary lie
/// inside the clean type (e.g. `Dict[str, int]`, or a multi-segment type
/// kept whole) and stay covered by the `TYPE` token. All three parsers of
/// this logic family (google `split_type_markers`, the NumPy segment
/// scanner, and [`try_parse_bracket_entry`]) share this boundary rule so
/// it cannot drift between them.
pub(crate) fn separator_comma_offsets(text: &str, clean_len: usize) -> Vec<usize> {
    // The separator comma before each part after the first sits one byte
    // before that part.
    split_comma_parts(text)
        .iter()
        .skip(1)
        .map(|(seg_offset, _)| seg_offset - 1)
        .filter(|&pos| pos >= clean_len)
        .collect()
}

/// Find the matching closing bracket for an opening bracket at `open_pos`.
///
/// Only tracks the *same* bracket kind: `(` is matched by `)`, `[` by `]`,
/// `{` by `}`, and `<` by `>`.  Other bracket kinds are ignored.
///
/// Returns `Some(close_pos)` on success, `None` if unmatched.
pub(crate) fn find_matching_close(s: &str, open_pos: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let open = bytes[open_pos];
    let close = match open {
        b'(' => b')',
        b'[' => b']',
        b'{' => b'}',
        b'<' => b'>',
        _ => return None,
    };
    let mut depth: u32 = 1;
    for (i, &b) in bytes[open_pos + 1..].iter().enumerate() {
        if b == open {
            depth += 1;
        } else if b == close {
            depth -= 1;
            if depth == 0 {
                return Some(open_pos + 1 + i);
            }
        }
    }
    None
}

// =============================================================================
// Optional marker stripping
// =============================================================================

/// Strip a trailing `optional` marker from a type annotation.
///
/// Uses bracket-aware comma splitting so that commas inside type
/// annotations like `Dict[str, int]` are never mistaken for the
/// separator before `optional`.
///
/// Returns `(clean_type, optional_byte_offset)` where the offset is
/// relative to the start of `type_content` and points to the `o` in
/// `optional`.
pub(crate) fn strip_optional(type_content: &str) -> (&str, Option<usize>) {
    let parts = split_comma_parts(type_content);
    let mut optional_offset = None;
    let mut type_end = 0;

    for &(seg_offset, seg_raw) in &parts {
        let seg = seg_raw.trim();
        if seg == "optional" {
            let ws_lead = seg_raw.len() - seg_raw.trim_start().len();
            optional_offset = Some(seg_offset + ws_lead);
        } else if !seg.is_empty() {
            type_end = seg_offset + seg_raw.trim_end().len();
        }
    }

    if let Some(opt) = optional_offset {
        let clean = type_content[..type_end].trim_end_matches(',').trim_end();
        (clean, Some(opt))
    } else {
        (type_content, None)
    }
}

// =============================================================================
// Bracket-style entry parsing
// =============================================================================

/// Parsed byte-offset information for a bracket-style entry: `name(type): desc`.
///
/// All byte offsets are relative to the start of the input `text`.
pub(crate) struct BracketEntry<'a> {
    /// Name text before the bracket (end-trimmed).
    pub name: &'a str,
    /// Byte offset of the opening bracket.
    pub open_bracket: usize,
    /// Byte offset of the matching closing bracket.
    pub close_bracket: usize,
    /// Clean type text (optional stripped) inside brackets.
    pub clean_type: &'a str,
    /// Byte offset of the type text start.
    pub type_offset: usize,
    /// Byte offsets of top-level separator commas after the clean type
    /// (e.g. the comma before `optional`).
    pub commas: Vec<usize>,
    /// Byte offset of `optional` keyword, if present.
    pub optional_offset: Option<usize>,
    /// Byte offset of the colon after the close bracket, if present.
    pub colon: Option<usize>,
    /// Description text after the colon (trimmed), if present.
    pub description: Option<&'a str>,
    /// Byte offset of the description start, if present.
    pub description_offset: Option<usize>,
}

/// Try to parse a bracket-style entry `name(type): desc` or `name (type): desc`.
///
/// Returns `Some(BracketEntry)` when a bracket appears before the first
/// top-level colon and has a matching close, followed by `:` or end-of-text.
/// Returns `None` otherwise, so the caller can fall through to other patterns.
pub(crate) fn try_parse_bracket_entry(text: &str) -> Option<BracketEntry<'_>> {
    // Find the first opening bracket that comes after at least one character.
    let bracket_pos = text.bytes().enumerate().find_map(|(i, b)| {
        if i > 0 && matches!(b, b'(' | b'[' | b'{' | b'<') {
            Some(i)
        } else {
            None
        }
    })?;

    // The bracket must appear before any top-level colon.
    if let Some(colon_pos) = find_entry_colon(text) {
        if colon_pos < bracket_pos {
            return None;
        }
    }

    let close_pos = find_matching_close(text, bracket_pos)?;

    // After the closing bracket there must be `:` (with optional whitespace)
    // or end-of-text.
    let after_close = text[close_pos + 1..].trim_start();
    if !after_close.is_empty() && !after_close.starts_with(':') {
        return None;
    }

    let name = text[..bracket_pos].trim_end();

    // Determine colon, description, and description_offset first,
    // since we need to know the colon position to compute the type range
    // (when colon is inside brackets, the type ends at the colon).
    let (colon, description, description_offset) = if after_close.starts_with(':') {
        let colon_byte = text[close_pos + 1..].find(':').unwrap() + close_pos + 1;
        let after_colon = &text[colon_byte + 1..];
        let desc = after_colon.trim();
        if desc.is_empty() {
            (Some(colon_byte), None, None)
        } else {
            let ws = after_colon.len() - after_colon.trim_start().len();
            (Some(colon_byte), Some(desc), Some(colon_byte + 1 + ws))
        }
    } else {
        (None, None, None)
    };

    // Determine where the type portion ends. Normally at close_pos,
    // but if a colon is inside the brackets, it ends at the colon.
    let type_end = if let Some(c) = colon {
        if c > bracket_pos && c < close_pos { c } else { close_pos }
    } else {
        close_pos
    };

    let type_raw = &text[bracket_pos + 1..type_end];
    let type_trimmed = type_raw.trim();
    let leading_ws = type_raw.len() - type_raw.trim_start().len();
    let type_offset = bracket_pos + 1 + leading_ws;

    let (clean_type, opt_rel) = strip_optional(type_trimmed);
    let optional_offset = opt_rel.map(|r| type_offset + r);

    let commas: Vec<usize> = separator_comma_offsets(type_trimmed, clean_type.len())
        .into_iter()
        .map(|rel| type_offset + rel)
        .collect();

    Some(BracketEntry {
        name,
        open_bracket: bracket_pos,
        close_bracket: close_pos,
        clean_type,
        type_offset,
        commas,
        optional_offset,
        colon,
        description,
        description_offset,
    })
}

/// Find the byte offset of the first `:` in `text[start..]` that is not inside
/// `[]`, `{}`, or `<>` brackets.  Unlike [`find_entry_colon`] this does **not**
/// track `()` depth, which is useful when parsing inside an unclosed `(`.
///
/// Returns an absolute byte offset into `text`.
pub(crate) fn find_colon_ignoring_parens(text: &str, start: usize) -> Option<usize> {
    let mut depth: u32 = 0;
    for (i, b) in text[start..].bytes().enumerate() {
        match b {
            b'[' | b'{' | b'<' => depth += 1,
            b']' | b'}' | b'>' => depth = depth.saturating_sub(1),
            b':' if depth == 0 => return Some(start + i),
            _ => {}
        }
    }
    None
}

/// Try to find an opening bracket for a bracket-style entry.
///
/// Returns `Some(bracket_pos)` when there is a `(`, `[`, `{`, or `<` after at
/// least one character, and that bracket appears before any top-level colon.
/// Returns `None` otherwise.
pub(crate) fn find_entry_open_bracket(text: &str) -> Option<usize> {
    let bracket_pos = text.bytes().enumerate().find_map(|(i, b)| {
        if i > 0 && matches!(b, b'(' | b'[' | b'{' | b'<') {
            Some(i)
        } else {
            None
        }
    })?;

    // The bracket must appear before any top-level colon.
    if let Some(colon_pos) = find_entry_colon(text) {
        if colon_pos < bracket_pos {
            return None;
        }
    }

    Some(bracket_pos)
}

/// Convert a multi-line description with potential leading indentation to
/// an owned string with the leading indentation removed.
pub(crate) fn convert_multiline_with_indentation(text: &str) -> String {
    let description_indent = text
        .lines()
        .skip(1)
        .filter_map(|line| {
            let trimmed_len = line.trim_start().len();
            if trimmed_len == 0 {
                None
            } else {
                Some(line.len() - trimmed_len)
            }
        })
        .min()
        .unwrap_or(0);
    let mut lines = text.lines();
    if let Some(first_line) = lines.next() {
        lines
            .map(|line| {
                if description_indent >= line.len() {
                    // empty line
                    &line[0..0]
                } else {
                    line[description_indent..].trim_end()
                }
            })
            .fold(first_line.trim_end().to_owned(), |a, b| a + "\n" + b)
    } else {
        String::new()
    }
}

// =============================================================================
// Reference entries (shared by the NumPy and Google parsers)
// =============================================================================

/// Build a reference node of `kind` for an rST-style line: `.. [N] content`.
fn build_reference_node_rst(
    kind: SyntaxKind,
    directive_marker: TextRange,
    open_bracket: TextRange,
    number: Option<TextRange>,
    close_bracket: TextRange,
    content: Option<TextRange>,
    range: TextRange,
) -> SyntaxNode {
    let mut children = Vec::new();
    children.push(SyntaxElement::Token(SyntaxToken::new(
        SyntaxKind::DIRECTIVE_MARKER,
        directive_marker,
    )));
    children.push(SyntaxElement::Token(SyntaxToken::new(
        SyntaxKind::OPEN_BRACKET,
        open_bracket,
    )));
    if let Some(n) = number {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::NUMBER, n)));
    }
    children.push(SyntaxElement::Token(SyntaxToken::new(
        SyntaxKind::CLOSE_BRACKET,
        close_bracket,
    )));
    if let Some(c) = content {
        children.push(SyntaxElement::Node(text_block_single(SyntaxKind::CONTENT, c)));
    }
    SyntaxNode::new(kind, range, children)
}

/// Build a reference node of `kind` for a plain-text line (content only).
fn build_reference_node_plain(kind: SyntaxKind, content: TextRange, range: TextRange) -> SyntaxNode {
    let children = vec![SyntaxElement::Node(text_block_single(SyntaxKind::CONTENT, content))];
    SyntaxNode::new(kind, range, children)
}

/// Extend the `CONTENT` block of the last reference node, or add one.
fn extend_last_ref_content(nodes: &mut [SyntaxElement], cont: TextRange) {
    if let Some(SyntaxElement::Node(node)) = nodes.last_mut() {
        let mut found_content = false;
        for child in node.children_mut() {
            if let SyntaxElement::Node(n) = child {
                if n.kind() == SyntaxKind::CONTENT {
                    extend_text_block(n, cont);
                    found_content = true;
                    break;
                }
            }
        }
        if !found_content {
            node.push_child(SyntaxElement::Node(text_block_single(SyntaxKind::CONTENT, cont)));
        }
        node.extend_range_to(cont.end());
    }
}

/// Process one line of a References section body, appending a reference node
/// of `node_kind` (or extending the previous one for continuation lines).
///
/// Handles rST-marker lines (`.. [N] content`), plain-content lines, and
/// more-indented continuation lines that extend the previous entry's content.
pub(crate) fn process_reference_line(
    cursor: &LineCursor,
    node_kind: SyntaxKind,
    nodes: &mut Vec<SyntaxElement>,
    entry_indent: &mut Option<usize>,
) {
    let indent_cols = cursor.current_indent_columns();
    if let Some(base) = *entry_indent {
        if indent_cols > base {
            extend_last_ref_content(nodes, cursor.current_trimmed_range());
            return;
        }
    }
    if entry_indent.is_none() {
        *entry_indent = Some(indent_cols);
    }

    let col = cursor.current_indent();
    let trimmed = cursor.current_trimmed();
    let is_directive = trimmed.starts_with("..") && trimmed[2..].trim_start().starts_with('[');

    if is_directive {
        let rel_open = trimmed.find('[').unwrap();
        let abs_open = cursor.substr_offset(trimmed) + rel_open;
        // Bound the close-bracket search to the current line: a `]` on a later
        // line must not match, or the content slice below would panic. An
        // unmatched marker falls through to the plain-text reference path.
        let line_end = cursor.substr_offset(cursor.current_line_text()) + cursor.current_line_text().len();
        if let Some(abs_close) = find_matching_close(&cursor.source()[..line_end], abs_open) {
            let directive_marker = cursor.make_line_range(cursor.line, col, 2);
            let open_bracket = TextRange::from_offset_len(abs_open, 1);
            let close_bracket = TextRange::from_offset_len(abs_close, 1);
            let num_raw = &cursor.source()[abs_open + 1..abs_close];
            let num_str = num_raw.trim();
            let number = if !num_str.is_empty() {
                let num_abs = cursor.substr_offset(num_str);
                Some(TextRange::from_offset_len(num_abs, num_str.len()))
            } else {
                None
            };
            let line_end_offset = cursor.substr_offset(cursor.current_line_text()) + cursor.current_line_text().len();
            let after_on_line = &cursor.source()[abs_close + 1..line_end_offset.min(cursor.source().len())];
            let content_str = after_on_line.trim();
            let content = if !content_str.is_empty() {
                Some(TextRange::from_offset_len(
                    cursor.substr_offset(content_str),
                    content_str.len(),
                ))
            } else {
                None
            };

            nodes.push(SyntaxElement::Node(build_reference_node_rst(
                node_kind,
                directive_marker,
                open_bracket,
                number,
                close_bracket,
                content,
                cursor.current_trimmed_range(),
            )));
            return;
        }
    }

    // Plain text reference
    nodes.push(SyntaxElement::Node(build_reference_node_plain(
        node_kind,
        cursor.current_trimmed_range(),
        cursor.current_trimmed_range(),
    )));
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- find_entry_colon ----

    #[test]
    fn test_find_entry_colon() {
        // Basic colon
        assert_eq!(find_entry_colon("name: desc"), Some(4));
        assert_eq!(find_entry_colon("name:desc"), Some(4));
        assert_eq!(find_entry_colon("name:"), Some(4));
        // No colon
        assert_eq!(find_entry_colon("name"), None);
        // Colon inside brackets is skipped
        assert_eq!(find_entry_colon("Dict[str, int]: desc"), Some(14));
        assert_eq!(find_entry_colon("Tuple(a, b): desc"), Some(11));
        // Nested brackets
        assert_eq!(find_entry_colon("Dict[str, List[int]]: desc"), Some(20));
        // Only colon inside brackets — no match
        assert_eq!(find_entry_colon("Dict[k: v]"), None);
    }

    // ---- find_term_colon ----

    #[test]
    fn test_find_term_colon() {
        // `name : type` — whitespace before the colon separates.
        assert_eq!(find_term_colon("x : int"), Some(2));
        assert_eq!(find_term_colon("x, y : int"), Some(5));
        // Attached colon on a single top-level token separates (lenient form).
        assert_eq!(find_term_colon("result:int"), Some(6));
        assert_eq!(find_term_colon("func_a: Description"), Some(6));
        // Bracket-internal whitespace is not top-level, so an attached colon still
        // separates and the colon inside the brackets is skipped.
        assert_eq!(find_term_colon("Dict[str, int]: desc"), Some(14));
        // Prose with an attached trailing colon is NOT split (Issue #26).
        assert_eq!(find_term_colon("Description with attributes:"), None);
        // A line that starts with an rST role colon is never a term separator.
        assert_eq!(find_term_colon(":attr:`~module.ClassName.attr1`"), None);
        // No colon at all.
        assert_eq!(find_term_colon("name"), None);
    }

    #[test]
    fn test_split_comma_parts() {
        let parts: Vec<_> = split_comma_parts("int, optional")
            .iter()
            .map(|(_, s)| s.trim())
            .collect();
        assert_eq!(parts, vec!["int", "optional"]);

        // Brackets respected
        let parts: Vec<_> = split_comma_parts("Dict[str, int], optional")
            .iter()
            .map(|(_, s)| s.trim())
            .collect();
        assert_eq!(parts, vec!["Dict[str, int]", "optional"]);

        // Offsets
        let parts = split_comma_parts("int, optional");
        assert_eq!(parts[0].0, 0);
        assert_eq!(parts[1].0, 4);
    }

    #[test]
    fn test_separator_comma_offsets() {
        // Comma before the `optional` marker is a separator.
        assert_eq!(separator_comma_offsets("int, optional", 3), vec![3]);
        assert_eq!(separator_comma_offsets("Dict[str, int], optional", 14), vec![14]);
        // Comma inside the clean type is not.
        assert_eq!(separator_comma_offsets("int, float", 10), Vec::<usize>::new());
        // No markers stripped: whole text is the clean type.
        assert_eq!(separator_comma_offsets("int", 3), Vec::<usize>::new());
        // Two separators: before `optional` and before `default 5`.
        assert_eq!(separator_comma_offsets("int, optional, default 5", 3), vec![3, 13]);
    }

    #[test]
    fn test_find_matching_close_basic() {
        assert_eq!(find_matching_close("(abc)", 0), Some(4));
    }

    #[test]
    fn test_find_matching_close_nested_same() {
        assert_eq!(find_matching_close("(a(b)c)", 0), Some(6));
    }

    #[test]
    fn test_find_matching_close_nested_mixed() {
        assert_eq!(find_matching_close("(a[b]c)", 0), Some(6));
    }

    #[test]
    fn test_find_matching_close_mismatched_ignored() {
        // `]` is not `)`, so it is ignored — `)` closes the `(`.
        assert_eq!(find_matching_close("(a]b)", 0), Some(4));
    }

    #[test]
    fn test_find_matching_close_no_match() {
        assert_eq!(find_matching_close("(abc", 0), None);
    }

    #[test]
    fn test_find_matching_close_angle_brackets() {
        assert_eq!(find_matching_close("<int>", 0), Some(4));
    }

    // ---- strip_optional ----

    #[test]
    fn test_strip_optional_basic() {
        assert_eq!(strip_optional("int, optional"), ("int", Some(5)));
        assert_eq!(strip_optional("int"), ("int", None));
        assert_eq!(strip_optional("Dict[str, int], optional"), ("Dict[str, int]", Some(16)));
        assert_eq!(strip_optional("optional"), ("", Some(0)));
        assert_eq!(strip_optional("int,optional"), ("int", Some(4)));
        assert_eq!(strip_optional("int,  optional"), ("int", Some(6)));
        assert_eq!(strip_optional("int, optional  "), ("int", Some(5)));
    }

    // ---- try_parse_bracket_entry ----

    #[test]
    fn test_bracket_entry_basic() {
        let e = try_parse_bracket_entry("name (int): desc").unwrap();
        assert_eq!(e.name, "name");
        assert_eq!(e.clean_type, "int");
        assert_eq!(e.description, Some("desc"));
    }

    #[test]
    fn test_bracket_entry_no_space() {
        let e = try_parse_bracket_entry("name(int): desc").unwrap();
        assert_eq!(e.name, "name");
        assert_eq!(e.clean_type, "int");
    }

    #[test]
    fn test_bracket_entry_optional() {
        let e = try_parse_bracket_entry("name (int, optional): desc").unwrap();
        assert_eq!(e.clean_type, "int");
        assert!(e.optional_offset.is_some());
    }

    #[test]
    fn test_bracket_entry_complex_type() {
        let e = try_parse_bracket_entry("data (Dict[str, int]): values").unwrap();
        assert_eq!(e.clean_type, "Dict[str, int]");
        assert_eq!(e.description, Some("values"));
    }

    #[test]
    fn test_bracket_entry_no_colon() {
        let e = try_parse_bracket_entry("name (int)").unwrap();
        assert_eq!(e.name, "name");
        assert_eq!(e.clean_type, "int");
        assert!(e.colon.is_none());
        assert!(e.description.is_none());
    }

    #[test]
    fn test_bracket_entry_empty_desc() {
        let e = try_parse_bracket_entry("name (int):").unwrap();
        assert_eq!(e.clean_type, "int");
        assert!(e.colon.is_some());
        assert!(e.description.is_none());
    }

    #[test]
    fn test_bracket_entry_colon_before_bracket() {
        // `name : (int)` should NOT match — colon is before bracket.
        assert!(try_parse_bracket_entry("name : (int)").is_none());
    }

    #[test]
    fn test_bracket_entry_no_bracket() {
        assert!(try_parse_bracket_entry("name : int").is_none());
    }

    #[test]
    fn test_bracket_entry_text_after_bracket() {
        // `name (int) not_colon` — non-colon text after bracket.
        assert!(try_parse_bracket_entry("name (int) not_colon").is_none());
    }

    // ---- find_colon_ignoring_parens ----

    #[test]
    fn test_find_colon_ignoring_parens_basic() {
        assert_eq!(find_colon_ignoring_parens("int : desc", 0), Some(4));
    }

    #[test]
    fn test_find_colon_ignoring_parens_inside_brackets() {
        // `:` inside `[]` is skipped.
        assert_eq!(find_colon_ignoring_parens("Dict[k: v] : desc", 0), Some(11));
    }

    #[test]
    fn test_find_colon_ignoring_parens_inside_parens() {
        // Parens are NOT tracked, so `:` inside `(` is found.
        assert_eq!(find_colon_ignoring_parens("(int : desc", 1), Some(5));
    }

    #[test]
    fn test_find_colon_ignoring_parens_none() {
        assert_eq!(find_colon_ignoring_parens("int desc", 0), None);
    }

    // ---- find_entry_open_bracket ----

    #[test]
    fn test_find_entry_open_bracket_basic() {
        assert_eq!(find_entry_open_bracket("name (int)"), Some(5));
    }

    #[test]
    fn test_find_entry_open_bracket_colon_first() {
        // Colon before bracket → None.
        assert_eq!(find_entry_open_bracket("name : (int)"), None);
    }

    #[test]
    fn test_find_entry_open_bracket_no_bracket() {
        assert_eq!(find_entry_open_bracket("name : int"), None);
    }

    #[test]
    fn test_find_entry_open_bracket_at_start() {
        // Bracket at position 0 is not valid (no name before it).
        assert_eq!(find_entry_open_bracket("(int)"), None);
    }

    #[test]
    fn test_convert_multiline_with_indentation() {
        assert_eq!(
            convert_multiline_with_indentation(
                "First line.

        Description line.
        More description.

            Blockquote.
            Another.

        Some text.

        .. directive:: option
           directive_option"
            ),
            "First line.\n\nDescription line.\nMore description.\n\n    Blockquote.\n    Another.\n\nSome text.\n\n.. directive:: option\n   directive_option"
        );
    }
}
