//! NumPy style docstring parser (SyntaxNode-based).
//!
//! Parses docstrings in NumPy format and produces a [`Parsed`] result
//! containing a tree of [`SyntaxNode`]s and [`SyntaxToken`]s.

use crate::cursor::LineCursor;
use crate::cursor::indent_len;
use crate::parse::dispatch::Dialect;
use crate::parse::dispatch::HeaderMarker;
use crate::parse::dispatch::SectionBody;
use crate::parse::dispatch::SectionHeaderInfo;
use crate::parse::kind::SectionName;
use crate::parse::utils::build_leading_token_entry;
use crate::parse::utils::entry_continuation_guard;
use crate::parse::utils::find_term_colon;
use crate::parse::utils::marker_syntax_elements;
use crate::parse::utils::missing_text_block;
use crate::parse::utils::process_reference_line;
use crate::parse::utils::scan_type_markers;
use crate::parse::utils::text_block_single;
use crate::parse::utils::try_parse_bracket_entry;
use crate::syntax::Parsed;
use crate::syntax::SyntaxElement;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::syntax::SyntaxToken;
use crate::text::TextRange;

// =============================================================================
// Section detection
// =============================================================================

/// Check if a trimmed line is a NumPy-style section underline (only dashes).
///
/// `pub(crate)` so [`detect_style`](crate::parse::detect_style) applies the
/// same underline rule as the parser it dispatches to (#142).
pub(crate) fn is_underline(trimmed: &str) -> bool {
    !trimmed.is_empty() && trimmed.bytes().all(|b| b == b'-')
}

/// Try to detect a NumPy-style section header at `cursor.line`.
///
/// A section header is a non-empty line immediately followed by a
/// line consisting only of dashes. Does **not** advance the cursor.
fn try_detect_header(cursor: &LineCursor) -> Option<SectionHeaderInfo> {
    let header_trimmed = cursor.current_trimmed();
    if header_trimmed.is_empty() {
        return None;
    }
    if cursor.line + 1 >= cursor.total_lines() {
        return None;
    }
    let underline_line = cursor.line_text(cursor.line + 1);
    let underline_trimmed = underline_line.trim();
    if !is_underline(underline_trimmed) {
        return None;
    }

    let header_col = cursor.current_indent();
    let underline_col = indent_len(underline_line);
    let normalized = header_trimmed.to_ascii_lowercase();
    let kind = SectionName::from_numpy_name(&normalized);

    Some(SectionHeaderInfo {
        range: cursor.make_range(
            cursor.line,
            header_col,
            cursor.line + 1,
            underline_col + underline_trimmed.len(),
        ),
        kind,
        name: cursor.make_line_range(cursor.line, header_col, header_trimmed.len()),
        marker: HeaderMarker::Underline(cursor.make_line_range(
            cursor.line + 1,
            underline_col,
            underline_trimmed.len(),
        )),
        indent_columns: cursor.current_indent_columns(),
    })
}

// =============================================================================
// Entry header parsing (parameter name : type, optional)
// =============================================================================

struct ParamHeaderParts {
    /// `NAME` and separator `COMMA` tokens, interleaved in source order.
    names: Vec<(SyntaxKind, TextRange)>,
    /// Brackets of a google-style entry (`name (type): desc`), if any.
    open_bracket: Option<TextRange>,
    close_bracket: Option<TextRange>,
    colon: Option<TextRange>,
    param_type: Option<TextRange>,
    /// Separator commas after the clean type (before `optional` / `default`).
    type_commas: Vec<TextRange>,
    /// `OPTIONAL` tokens and `DEFAULT` nodes (one per marker occurrence,
    /// in source order), with absolute source ranges.
    markers: Vec<SyntaxElement>,
    first_description: Option<TextRange>,
}

fn parse_name_and_type(text: &str, line_idx: usize, col_base: usize, cursor: &LineCursor) -> ParamHeaderParts {
    // --- Google-style bracket pattern: `name (type): desc` ---
    if let Some(result) = try_parse_google_style_entry(text, line_idx, col_base, cursor) {
        return result;
    }

    let Some(colon_pos) = find_term_colon(text) else {
        let names = parse_name_list(text, line_idx, col_base, cursor);
        return ParamHeaderParts {
            names,
            open_bracket: None,
            close_bracket: None,
            colon: None,
            param_type: None,
            type_commas: Vec::new(),
            markers: Vec::new(),
            first_description: None,
        };
    };

    let name_str = text[..colon_pos].trim_end();
    let colon_col = col_base + colon_pos;
    let colon_span = Some(cursor.make_line_range(line_idx, colon_col, 1));
    let names = parse_name_list(name_str, line_idx, col_base, cursor);

    let after_colon = &text[colon_pos + 1..];
    let after_trimmed = after_colon.trim();

    if after_trimmed.is_empty() {
        // Colon present but no type text: emit a zero-length TYPE so callers
        // can use `type_().is_missing()` to distinguish `a :` from `a`.
        let missing_type = cursor.make_line_range(line_idx, colon_col + 1, 0);
        return ParamHeaderParts {
            names,
            open_bracket: None,
            close_bracket: None,
            colon: colon_span,
            param_type: Some(missing_type),
            type_commas: Vec::new(),
            markers: Vec::new(),
            first_description: None,
        };
    }

    let type_abs_start = cursor.substr_offset(after_trimmed);
    let scanned = scan_type_markers(after_trimmed);

    let param_type = if scanned.clean_type.is_empty() {
        None
    } else {
        Some(TextRange::from_offset_len(type_abs_start, scanned.clean_type.len()))
    };

    let type_commas: Vec<TextRange> = scanned
        .commas
        .into_iter()
        .map(|rel| TextRange::from_offset_len(type_abs_start + rel, 1))
        .collect();

    ParamHeaderParts {
        names,
        open_bracket: None,
        close_bracket: None,
        colon: colon_span,
        param_type,
        type_commas,
        markers: marker_syntax_elements(&scanned.markers, type_abs_start),
        first_description: None,
    }
}

/// Try to parse a Google-style entry `name (type): desc` or `name(type): desc`.
///
/// Returns `Some(ParamHeaderParts)` when the line matches the bracket-style
/// pattern.  Otherwise returns `None` so that the caller falls through to
/// the normal NumPy parsing path.
fn try_parse_google_style_entry(
    text: &str,
    line_idx: usize,
    col_base: usize,
    cursor: &LineCursor,
) -> Option<ParamHeaderParts> {
    let entry = try_parse_bracket_entry(text)?;

    let names = parse_name_list(entry.name, line_idx, col_base, cursor);

    let open_bracket = Some(cursor.make_line_range(line_idx, col_base + entry.open_bracket, 1));
    let close_bracket = Some(cursor.make_line_range(line_idx, col_base + entry.close_bracket, 1));

    let param_type = if !entry.clean_type.is_empty() {
        Some(cursor.make_line_range(line_idx, col_base + entry.type_offset, entry.clean_type.len()))
    } else {
        None
    };

    let type_commas = entry
        .commas
        .iter()
        .map(|&c| cursor.make_line_range(line_idx, col_base + c, 1))
        .collect();

    // Marker offsets are relative to the type text; anchor them at its
    // absolute position on the line.
    let type_base = cursor
        .make_line_range(line_idx, col_base + entry.type_offset, 0)
        .start()
        .raw() as usize;
    let markers = marker_syntax_elements(&entry.markers, type_base);

    let colon = entry.colon.map(|c| cursor.make_line_range(line_idx, col_base + c, 1));

    let first_description = entry
        .description_offset
        .map(|d| cursor.make_line_range(line_idx, col_base + d, entry.description.unwrap().len()));

    Some(ParamHeaderParts {
        names,
        open_bracket,
        close_bracket,
        colon,
        param_type,
        type_commas,
        markers,
        first_description,
    })
}

/// Parse a comma-separated name list into interleaved `NAME` and separator
/// `COMMA` token specs, in source order.
fn parse_name_list(text: &str, line_idx: usize, col_base: usize, cursor: &LineCursor) -> Vec<(SyntaxKind, TextRange)> {
    let mut names = Vec::new();
    let mut byte_pos = 0usize;
    let parts: Vec<&str> = text.split(',').collect();

    for (i, part) in parts.iter().enumerate() {
        let leading = part.len() - part.trim_start().len();
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            let name_col = col_base + byte_pos + leading;
            names.push((
                SyntaxKind::NAME,
                cursor.make_line_range(line_idx, name_col, trimmed.len()),
            ));
        }
        // A separator comma follows every part but the last.
        if i + 1 < parts.len() {
            let comma_col = col_base + byte_pos + part.len();
            names.push((SyntaxKind::COMMA, cursor.make_line_range(line_idx, comma_col, 1)));
        }
        byte_pos += part.len() + 1;
    }

    names
}

// =============================================================================
// SyntaxNode builders
// =============================================================================

fn build_parameter_node(parts: &ParamHeaderParts, range: TextRange) -> SyntaxNode {
    let mut children = Vec::new();
    for (kind, range) in &parts.names {
        children.push(SyntaxElement::Token(SyntaxToken::new(*kind, *range)));
    }
    if let Some(ob) = parts.open_bracket {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::OPEN_BRACKET, ob)));
    }
    if let Some(colon) = parts.colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COLON, colon)));
    }
    if let Some(t) = parts.param_type {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::TYPE, t)));
    } else if parts.colon.is_some() || parts.open_bracket.is_some() {
        // Colon (or empty brackets) present but no type: zero-length
        // placeholder so callers can distinguish `name :` / `name ()`
        // (missing type) from `name` (no type at all). Anchored where the
        // type would appear: right after the open bracket for bracketed
        // entries, right after the colon otherwise.
        let missing_pos = parts
            .open_bracket
            .map(|ob| ob.end())
            .unwrap_or_else(|| parts.colon.unwrap().end());
        children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::TYPE,
            TextRange::new(missing_pos, missing_pos),
        )));
    }
    for comma in &parts.type_commas {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COMMA, *comma)));
    }
    // One element per marker occurrence: OPTIONAL tokens and DEFAULT nodes.
    children.extend(parts.markers.iter().cloned());
    if let Some(cb) = parts.close_bracket {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::CLOSE_BRACKET, cb)));
    }
    if let Some(desc) = parts.first_description {
        children.push(SyntaxElement::Node(text_block_single(SyntaxKind::DESCRIPTION, desc)));
    }
    // The tree stores children in source order. Google-style entries collect
    // them out of order above (COLON is found after the close bracket but
    // pushed before TYPE), so sort by position; zero-length placeholders
    // sort before a token starting at the same offset.
    children.sort_by_key(|c| (c.range().start(), c.range().end()));
    SyntaxNode::new(SyntaxKind::ENTRY, range, children)
}

/// Build a Returns/Yields `ENTRY`: `[NAME?, COLON?, TYPE?]`.
///
/// A colon with no type yields a zero-length `TYPE` placeholder so callers
/// can distinguish `name :` (missing type) from `type` (no name/colon).
fn build_return_entry_node(
    name: Option<TextRange>,
    colon: Option<TextRange>,
    return_type: Option<TextRange>,
    range: TextRange,
) -> SyntaxNode {
    let mut children = Vec::new();
    if let Some(n) = name {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::NAME, n)));
    }
    if let Some(c) = colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COLON, c)));
    }
    if let Some(rt) = return_type {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::TYPE, rt)));
    } else if let Some(c) = colon {
        let missing_pos = c.end();
        children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::TYPE,
            TextRange::new(missing_pos, missing_pos),
        )));
    }
    SyntaxNode::new(SyntaxKind::ENTRY, range, children)
}

fn build_see_also_node(
    names_str: &str,
    names_line: usize,
    names_col: usize,
    colon: Option<TextRange>,
    description: Option<TextRange>,
    range: TextRange,
    cursor: &LineCursor,
) -> SyntaxNode {
    let mut children = Vec::new();
    let names = parse_name_list(names_str, names_line, names_col, cursor);
    for (kind, range) in &names {
        children.push(SyntaxElement::Token(SyntaxToken::new(*kind, *range)));
    }
    if let Some(c) = colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COLON, c)));
    }
    if let Some(d) = description {
        children.push(SyntaxElement::Node(text_block_single(SyntaxKind::DESCRIPTION, d)));
    } else if let Some(c) = colon {
        // Colon present but no description: zero-length placeholder.
        children.push(SyntaxElement::Node(missing_text_block(
            SyntaxKind::DESCRIPTION,
            c.end(),
        )));
    }
    SyntaxNode::new(SyntaxKind::ENTRY, range, children)
}

// =============================================================================
// Per-line section body processors
// =============================================================================

fn process_parameter_line(cursor: &LineCursor, nodes: &mut Vec<SyntaxElement>, entry_indent: &mut Option<usize>) {
    if entry_continuation_guard(cursor, nodes, entry_indent) {
        return;
    }

    let col = cursor.current_indent();
    let trimmed = cursor.current_trimmed();
    let parts = parse_name_and_type(trimmed, cursor.line, col, cursor);
    let entry_range = cursor.current_trimmed_range();
    nodes.push(SyntaxElement::Node(build_parameter_node(&parts, entry_range)));
}

/// Split an entry line on its term colon: `(lead, colon, rest)` ranges, with
/// `rest` `None` when nothing follows the colon. `None` when the line has no
/// term colon at all.
fn split_on_term_colon(
    cursor: &LineCursor,
    col: usize,
    trimmed: &str,
) -> Option<(TextRange, TextRange, Option<TextRange>)> {
    let colon_pos = find_term_colon(trimmed)?;
    let lead = trimmed[..colon_pos].trim_end();
    let after_colon = &trimmed[colon_pos + 1..];
    let rest = after_colon.trim();
    let ws_after = after_colon.len() - after_colon.trim_start().len();
    let rest_col = col + colon_pos + 1 + ws_after;
    Some((
        cursor.make_line_range(cursor.line, col, lead.len()),
        cursor.make_line_range(cursor.line, col + colon_pos, 1),
        if rest.is_empty() {
            None
        } else {
            Some(cursor.make_line_range(cursor.line, rest_col, rest.len()))
        },
    ))
}

/// One line of a Returns/Yields body: `name : type`, or a bare type.
fn process_return_like_line(cursor: &LineCursor, nodes: &mut Vec<SyntaxElement>, entry_indent: &mut Option<usize>) {
    if entry_continuation_guard(cursor, nodes, entry_indent) {
        return;
    }

    let col = cursor.current_indent();
    let trimmed = cursor.current_trimmed();

    let (name, colon, return_type) = match split_on_term_colon(cursor, col, trimmed) {
        Some((lead, colon, rest)) => (Some(lead), Some(colon), rest),
        // Unnamed: type only (stored as TYPE)
        None => (None, None, Some(cursor.current_trimmed_range())),
    };

    let entry_range = cursor.current_trimmed_range();
    nodes.push(SyntaxElement::Node(build_return_entry_node(
        name,
        colon,
        return_type,
        entry_range,
    )));
}

/// One line of a Raises/Warns/Methods body: an entry led by a single token
/// (`TYPE` for exception and warning types, `NAME` for methods).
fn process_leading_token_line(
    cursor: &LineCursor,
    nodes: &mut Vec<SyntaxElement>,
    entry_indent: &mut Option<usize>,
    first_kind: SyntaxKind,
) {
    if entry_continuation_guard(cursor, nodes, entry_indent) {
        return;
    }

    let col = cursor.current_indent();
    let trimmed = cursor.current_trimmed();

    let (first, colon, first_desc) = match split_on_term_colon(cursor, col, trimmed) {
        Some((lead, colon, rest)) => (lead, Some(colon), rest),
        None => (cursor.current_trimmed_range(), None, None),
    };

    let entry_range = cursor.current_trimmed_range();
    nodes.push(SyntaxElement::Node(build_leading_token_entry(
        first_kind,
        first,
        colon,
        first_desc,
        entry_range,
    )));
}

fn process_see_also_line(cursor: &LineCursor, nodes: &mut Vec<SyntaxElement>, entry_indent: &mut Option<usize>) {
    if entry_continuation_guard(cursor, nodes, entry_indent) {
        return;
    }

    let col = cursor.current_indent();
    let trimmed = cursor.current_trimmed();

    let (names_str, colon, description) = if let Some(colon_pos) = find_term_colon(trimmed) {
        let after_colon = &trimmed[colon_pos + 1..];
        let desc_text = after_colon.trim();
        let ws_after = after_colon.len() - after_colon.trim_start().len();
        let desc_col = col + colon_pos + 1 + ws_after;
        (
            trimmed[..colon_pos].trim_end(),
            Some(cursor.make_line_range(cursor.line, col + colon_pos, 1)),
            if desc_text.is_empty() {
                None
            } else {
                Some(cursor.make_line_range(cursor.line, desc_col, desc_text.len()))
            },
        )
    } else {
        (trimmed, None, None)
    };

    let entry_range = cursor.make_line_range(cursor.line, col, trimmed.len());
    nodes.push(SyntaxElement::Node(build_see_also_node(
        names_str,
        cursor.line,
        col,
        colon,
        description,
        entry_range,
        cursor,
    )));
}

fn process_attribute_line(cursor: &LineCursor, nodes: &mut Vec<SyntaxElement>, entry_indent: &mut Option<usize>) {
    if entry_continuation_guard(cursor, nodes, entry_indent) {
        return;
    }

    let col = cursor.current_indent();
    let trimmed = cursor.current_trimmed();
    let parts = parse_name_and_type(trimmed, cursor.line, col, cursor);
    let entry_range = cursor.current_trimmed_range();
    // Attribute entries share the parameter grammar (`name1, name2 : type`),
    // so they reuse build_parameter_node: every NAME/COMMA token of the name
    // list reaches the CST (dropping the later names violated the coverage
    // law, #89).
    nodes.push(SyntaxElement::Node(build_parameter_node(&parts, entry_range)));
}

// =============================================================================
// The NumPy dialect
// =============================================================================

fn process_raises_line(cursor: &LineCursor, nodes: &mut Vec<SyntaxElement>, entry_indent: &mut Option<usize>) {
    process_leading_token_line(cursor, nodes, entry_indent, SyntaxKind::TYPE);
}

fn process_method_line(cursor: &LineCursor, nodes: &mut Vec<SyntaxElement>, entry_indent: &mut Option<usize>) {
    process_leading_token_line(cursor, nodes, entry_indent, SyntaxKind::NAME);
}

/// The NumPy dialect: underlined headers, one entry per body line.
///
/// NumPy entries sit at the same indentation level as the section header
/// (L = H = 0), so stray lines cannot be detected by indent or blank-line
/// heuristics alone: sections end only when the next header is detected.
struct NumPyDialect;

impl Dialect for NumPyDialect {
    fn style(&self) -> crate::parse::Style {
        crate::parse::Style::NumPy
    }

    fn try_header(&self, cursor: &LineCursor, _current: Option<&SectionHeaderInfo>) -> Option<SectionHeaderInfo> {
        try_detect_header(cursor)
    }

    #[rustfmt::skip]
    fn body(&self, kind: SectionName) -> SectionBody {
        match kind {
            SectionName::Parameters => SectionBody::Entries(process_parameter_line, Vec::new()),
            SectionName::OtherParameters => SectionBody::Entries(process_parameter_line, Vec::new()),
            SectionName::Receives => SectionBody::Entries(process_parameter_line, Vec::new()),
            SectionName::KeywordParameters => SectionBody::Entries(process_parameter_line, Vec::new()),
            SectionName::Returns => SectionBody::Entries(process_return_like_line, Vec::new()),
            SectionName::Yields => SectionBody::Entries(process_return_like_line, Vec::new()),
            SectionName::Raises => SectionBody::Entries(process_raises_line, Vec::new()),
            SectionName::Warns => SectionBody::Entries(process_raises_line, Vec::new()),
            SectionName::SeeAlso => SectionBody::Entries(process_see_also_line, Vec::new()),
            SectionName::References => SectionBody::Entries(process_reference_line, Vec::new()),
            SectionName::Attributes => SectionBody::Entries(process_attribute_line, Vec::new()),
            SectionName::Methods => SectionBody::Entries(process_method_line, Vec::new()),
            _ => SectionBody::FreeText(None),
        }
    }

    fn flush_by_indent(&self) -> bool {
        false
    }
}

// =============================================================================
// Main parser
// =============================================================================

/// Parse a NumPy-style docstring into a [`Parsed`] result.
///
/// # Example
///
/// ```rust
/// use pydocstring::parse::parse_numpy;
/// use pydocstring::syntax::SyntaxKind;
///
/// let input = "Summary.\n\nParameters\n----------\nx : int\n    The value.\n";
/// let parsed = parse_numpy(input);
/// let root = parsed.root();
///
/// // Access summary (a text block node wrapping per-line TEXT_LINE tokens)
/// let summary = pydocstring::parse::TextBlock::cast(&parsed, root.find_node(SyntaxKind::SUMMARY).unwrap()).unwrap();
/// assert_eq!(summary.text(), "Summary.");
///
/// // Access sections
/// let sections: Vec<_> = root.nodes(SyntaxKind::SECTION).collect();
/// assert_eq!(sections.len(), 1);
/// ```
pub fn parse_numpy(input: &str) -> Parsed {
    crate::parse::dispatch::parse_document(input, &NumPyDialect)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_underline() {
        assert!(is_underline("----------"));
        assert!(is_underline("---"));
        assert!(!is_underline(""));
        assert!(!is_underline("not dashes"));
        assert!(!is_underline("---x---"));
    }

    #[test]
    fn test_try_detect_header() {
        let c1 = LineCursor::new("Parameters\n----------");
        assert!(try_detect_header(&c1).is_some());
        assert_eq!(try_detect_header(&c1).unwrap().kind, SectionName::Parameters);

        let c2 = LineCursor::new("just text\nmore text");
        assert!(try_detect_header(&c2).is_none());

        let c3 = LineCursor::new("\n----------");
        assert!(try_detect_header(&c3).is_none());

        let c4 = LineCursor::new("Only one line");
        assert!(try_detect_header(&c4).is_none());

        let mut c5 = LineCursor::new("Parameters\n----------\nx : int\nReturns\n-------");
        assert!(try_detect_header(&c5).is_some());
        c5.line = 3;
        assert!(try_detect_header(&c5).is_some());
        assert_eq!(try_detect_header(&c5).unwrap().kind, SectionName::Returns);
    }

    #[test]
    fn test_parse_name_and_type_basic() {
        let src = "x : int";
        let cursor = LineCursor::new(src);
        let p = parse_name_and_type(src, 0, 0, &cursor);
        assert_eq!(p.names[0].1.source_text(src), "x");
        assert!(p.colon.is_some());
        assert_eq!(p.param_type.unwrap().source_text(src), "int");
        assert!(p.markers.is_empty());
    }

    #[test]
    fn test_parse_name_and_type_optional() {
        let src = "x : int, optional";
        let cursor = LineCursor::new(src);
        let p = parse_name_and_type(src, 0, 0, &cursor);
        assert_eq!(p.names[0].1.source_text(src), "x");
        assert!(p.colon.is_some());
        assert_eq!(p.param_type.unwrap().source_text(src), "int");
        assert_eq!(p.markers.len(), 1);
        assert_eq!(p.markers[0].kind(), SyntaxKind::OPTIONAL);
    }

    #[test]
    fn test_parse_name_and_type_complex() {
        let src = "x : Dict[str, int], optional";
        let cursor = LineCursor::new(src);
        let p = parse_name_and_type(src, 0, 0, &cursor);
        assert!(p.colon.is_some());
        assert_eq!(p.param_type.unwrap().source_text(src), "Dict[str, int]");
        assert_eq!(p.markers.len(), 1);
        assert_eq!(p.markers[0].kind(), SyntaxKind::OPTIONAL);
    }

    #[test]
    fn test_basic_parse() {
        let input = "Summary.\n\nParameters\n----------\nx : int\n    The value.\n";
        let parsed = parse_numpy(input);
        let root = parsed.root();
        assert_eq!(root.kind(), SyntaxKind::DOCUMENT);
        let summary =
            crate::parse::text_block::TextBlock::cast(&parsed, root.find_node(SyntaxKind::SUMMARY).unwrap()).unwrap();
        assert_eq!(summary.text(), "Summary.");
        let sections: Vec<_> = root.nodes(SyntaxKind::SECTION).collect();
        assert_eq!(sections.len(), 1);
    }

    /// A google-style entry in a NumPy section stores its children in source
    /// order: the COLON (found after the close bracket) must not precede TYPE.
    #[test]
    fn test_google_style_entry_children_in_source_order() {
        let input = "Summary.\n\nParameters\n----------\nname (str): The name.\n";
        let parsed = parse_numpy(input);
        let section = parsed.root().find_node(SyntaxKind::SECTION).unwrap();
        let param = section.find_node(SyntaxKind::ENTRY).unwrap();
        let mut last_start = None;
        for child in param.children() {
            assert!(
                last_start.is_none_or(|prev| prev <= child.range().start()),
                "children out of source order: {:?}",
                param
            );
            last_start = Some(child.range().start());
        }
        let kinds: Vec<SyntaxKind> = param
            .children()
            .iter()
            .filter(|c| !c.kind().is_trivia())
            .map(|c| c.kind())
            .collect();
        assert_eq!(
            kinds,
            vec![
                SyntaxKind::NAME,
                SyntaxKind::OPEN_BRACKET,
                SyntaxKind::TYPE,
                SyntaxKind::CLOSE_BRACKET,
                SyntaxKind::COLON,
                SyntaxKind::DESCRIPTION,
            ]
        );
    }

    /// Empty brackets: the zero-length missing TYPE placeholder is anchored
    /// right after the open bracket (where the type would appear), not at
    /// the colon.
    #[test]
    fn test_google_style_entry_missing_type_anchored_after_open_bracket() {
        let input = "Summary.\n\nParameters\n----------\nname (): The name.\n";
        let parsed = parse_numpy(input);
        let section = parsed.root().find_node(SyntaxKind::SECTION).unwrap();
        let param = section.find_node(SyntaxKind::ENTRY).unwrap();
        let open = param.find_token(SyntaxKind::OPEN_BRACKET).unwrap();
        let missing = param.find_missing(SyntaxKind::TYPE).unwrap();
        assert_eq!(missing.range().start(), open.range().end());
    }
}
