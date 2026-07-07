//! NumPy style docstring parser (SyntaxNode-based).
//!
//! Parses docstrings in NumPy format and produces a [`Parsed`] result
//! containing a tree of [`SyntaxNode`]s and [`SyntaxToken`]s.

use crate::cursor::LineCursor;
use crate::cursor::indent_len;
use crate::parse::numpy::kind::NumPySectionKind;
use crate::parse::utils::build_text_block;
use crate::parse::utils::extend_text_block;
use crate::parse::utils::find_term_colon;
use crate::parse::utils::missing_text_block;
use crate::parse::utils::process_reference_line;
use crate::parse::utils::separator_comma_offsets;
use crate::parse::utils::split_comma_parts;
use crate::parse::utils::text_block_single;
use crate::parse::utils::try_parse_bracket_entry;
use crate::parse::utils::try_parse_deprecation_directive;
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
fn is_underline(trimmed: &str) -> bool {
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
    let kind = NumPySectionKind::from_name(&normalized);

    Some(SectionHeaderInfo {
        range: cursor.make_range(
            cursor.line,
            header_col,
            cursor.line + 1,
            underline_col + underline_trimmed.len(),
        ),
        kind,
        name: cursor.make_line_range(cursor.line, header_col, header_trimmed.len()),
        underline: cursor.make_line_range(cursor.line + 1, underline_col, underline_trimmed.len()),
    })
}

struct SectionHeaderInfo {
    range: TextRange,
    kind: NumPySectionKind,
    name: TextRange,
    underline: TextRange,
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
    optional: Option<TextRange>,
    default_keyword: Option<TextRange>,
    default_separator: Option<TextRange>,
    default_value: Option<TextRange>,
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
            optional: None,
            default_keyword: None,
            default_separator: None,
            default_value: None,
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
            optional: None,
            default_keyword: None,
            default_separator: None,
            default_value: None,
            first_description: None,
        };
    }

    let type_abs_start = cursor.substr_offset(after_trimmed);
    let type_text = after_trimmed;

    let mut optional: Option<TextRange> = None;
    let mut default_keyword: Option<TextRange> = None;
    let mut default_separator: Option<TextRange> = None;
    let mut default_value: Option<TextRange> = None;
    let mut type_parts_end: usize = 0;

    for (seg_offset, seg_raw) in split_comma_parts(type_text) {
        let seg = seg_raw.trim();
        if seg.is_empty() {
            continue;
        }

        if seg == "optional" {
            let seg_abs = type_abs_start + seg_offset + (seg_raw.len() - seg_raw.trim_start().len());
            optional = Some(TextRange::from_offset_len(seg_abs, "optional".len()));
        } else if seg.starts_with("default")
            && seg["default".len()..]
                .chars()
                .next()
                .is_none_or(|c| matches!(c, ' ' | '\t' | '=' | ':'))
        {
            // Boundary guard: a type like `defaultdict` is not a default marker.
            let stripped = &seg["default".len()..];
            let ws_lead = seg_raw.len() - seg_raw.trim_start().len();
            let kw_abs = type_abs_start + seg_offset + ws_lead;
            default_keyword = Some(TextRange::from_offset_len(kw_abs, "default".len()));

            let after_kw = stripped.trim_start();
            if let Some(rest) = after_kw.strip_prefix('=') {
                let sep_pos = seg.find('=').unwrap();
                let sep_abs = kw_abs + sep_pos;
                default_separator = Some(TextRange::from_offset_len(sep_abs, 1));
                let val = rest.trim_start();
                if !val.is_empty() {
                    let val_abs = cursor.substr_offset(val);
                    default_value = Some(TextRange::from_offset_len(val_abs, val.len()));
                } else {
                    // Separator present but value absent: zero-length placeholder.
                    let missing_pos = sep_abs + 1;
                    default_value = Some(TextRange::from_offset_len(missing_pos, 0));
                }
            } else if let Some(rest) = after_kw.strip_prefix(':') {
                let sep_pos = seg.rfind(':').unwrap();
                let sep_abs = kw_abs + sep_pos;
                default_separator = Some(TextRange::from_offset_len(sep_abs, 1));
                let val = rest.trim_start();
                if !val.is_empty() {
                    let val_abs = cursor.substr_offset(val);
                    default_value = Some(TextRange::from_offset_len(val_abs, val.len()));
                } else {
                    // Separator present but value absent: zero-length placeholder.
                    let missing_pos = sep_abs + 1;
                    default_value = Some(TextRange::from_offset_len(missing_pos, 0));
                }
            } else {
                let val = after_kw.trim_start();
                if !val.is_empty() {
                    let val_abs = cursor.substr_offset(val);
                    default_value = Some(TextRange::from_offset_len(val_abs, val.len()));
                }
            }
        } else {
            type_parts_end = seg_offset + seg_raw.trim_end().len();
        }
    }

    let (param_type, clean_len) = if type_parts_end == 0 {
        (None, 0)
    } else {
        let clean = type_text[..type_parts_end].trim_end_matches(',').trim_end();
        (
            Some(TextRange::from_offset_len(type_abs_start, clean.len())),
            clean.len(),
        )
    };

    let type_commas: Vec<TextRange> = separator_comma_offsets(type_text, clean_len)
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
        optional,
        default_keyword,
        default_separator,
        default_value,
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

    let optional = entry
        .optional_offset
        .map(|r| cursor.make_line_range(line_idx, col_base + r, "optional".len()));

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
        optional,
        default_keyword: None,
        default_separator: None,
        default_value: None,
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

fn build_section_header_node(info: &SectionHeaderInfo) -> SyntaxNode {
    let children = vec![
        SyntaxElement::Token(SyntaxToken::new(SyntaxKind::NAME, info.name)),
        SyntaxElement::Token(SyntaxToken::new(SyntaxKind::UNDERLINE, info.underline)),
    ];
    SyntaxNode::new(SyntaxKind::NUMPY_SECTION_HEADER, info.range, children)
}

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
    if let Some(opt) = parts.optional {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::OPTIONAL, opt)));
    }
    if let Some(dk) = parts.default_keyword {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::DEFAULT_KEYWORD, dk)));
    }
    if let Some(ds) = parts.default_separator {
        children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::DEFAULT_SEPARATOR,
            ds,
        )));
    }
    if let Some(dv) = parts.default_value {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::DEFAULT_VALUE, dv)));
    }
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
    SyntaxNode::new(SyntaxKind::NUMPY_PARAMETER, range, children)
}

fn build_returns_node(
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
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::RETURN_TYPE, rt)));
    } else if let Some(c) = colon {
        // Colon present but no return type: zero-length placeholder so callers
        // can distinguish `name :` (missing type) from `type` (no name/colon).
        let missing_pos = c.end();
        children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::RETURN_TYPE,
            TextRange::new(missing_pos, missing_pos),
        )));
    }
    SyntaxNode::new(SyntaxKind::NUMPY_RETURNS, range, children)
}

fn build_yields_node(
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
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::RETURN_TYPE, rt)));
    } else if let Some(c) = colon {
        // Colon present but no yield type: zero-length placeholder.
        let missing_pos = c.end();
        children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::RETURN_TYPE,
            TextRange::new(missing_pos, missing_pos),
        )));
    }
    SyntaxNode::new(SyntaxKind::NUMPY_YIELDS, range, children)
}

fn build_exception_node(
    exc_type: TextRange,
    colon: Option<TextRange>,
    first_desc: Option<TextRange>,
    range: TextRange,
) -> SyntaxNode {
    let mut children = Vec::new();
    children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::TYPE, exc_type)));
    if let Some(c) = colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COLON, c)));
    }
    if let Some(d) = first_desc {
        children.push(SyntaxElement::Node(text_block_single(SyntaxKind::DESCRIPTION, d)));
    } else if let Some(c) = colon {
        // Colon present but no description: zero-length placeholder so callers
        // can distinguish `Exc:` (missing description) from `Exc` (no colon).
        children.push(SyntaxElement::Node(missing_text_block(
            SyntaxKind::DESCRIPTION,
            c.end(),
        )));
    }
    SyntaxNode::new(SyntaxKind::NUMPY_EXCEPTION, range, children)
}

fn build_warning_node(
    warn_type: TextRange,
    colon: Option<TextRange>,
    first_desc: Option<TextRange>,
    range: TextRange,
) -> SyntaxNode {
    let mut children = Vec::new();
    children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::TYPE, warn_type)));
    if let Some(c) = colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COLON, c)));
    }
    if let Some(d) = first_desc {
        children.push(SyntaxElement::Node(text_block_single(SyntaxKind::DESCRIPTION, d)));
    } else if let Some(c) = colon {
        // Colon present but no description: zero-length placeholder.
        children.push(SyntaxElement::Node(missing_text_block(
            SyntaxKind::DESCRIPTION,
            c.end(),
        )));
    }
    SyntaxNode::new(SyntaxKind::NUMPY_WARNING, range, children)
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
    SyntaxNode::new(SyntaxKind::NUMPY_SEE_ALSO_ITEM, range, children)
}

fn build_attribute_node(parts: &ParamHeaderParts, range: TextRange) -> SyntaxNode {
    let mut children = Vec::new();
    // Attributes use the first name only.
    if let Some((_, name)) = parts.names.iter().find(|(kind, _)| *kind == SyntaxKind::NAME) {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::NAME, *name)));
    }
    if let Some(colon) = parts.colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COLON, colon)));
    }
    if let Some(t) = parts.param_type {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::TYPE, t)));
    } else if let Some(c) = parts.colon {
        // Colon present but no type: zero-length placeholder so callers can
        // distinguish `attr :` (missing type) from `attr` (no type at all).
        let missing_pos = c.end();
        children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::TYPE,
            TextRange::new(missing_pos, missing_pos),
        )));
    }
    SyntaxNode::new(SyntaxKind::NUMPY_ATTRIBUTE, range, children)
}

fn build_method_node(
    name: TextRange,
    colon: Option<TextRange>,
    first_desc: Option<TextRange>,
    range: TextRange,
) -> SyntaxNode {
    let mut children = Vec::new();
    children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::NAME, name)));
    if let Some(c) = colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COLON, c)));
    }
    if let Some(d) = first_desc {
        children.push(SyntaxElement::Node(text_block_single(SyntaxKind::DESCRIPTION, d)));
    } else if let Some(c) = colon {
        // Colon present but no description: zero-length placeholder.
        children.push(SyntaxElement::Node(missing_text_block(
            SyntaxKind::DESCRIPTION,
            c.end(),
        )));
    }
    SyntaxNode::new(SyntaxKind::NUMPY_METHOD, range, children)
}

// =============================================================================
// Per-line section body processors
// =============================================================================

fn extend_last_node_description(nodes: &mut [SyntaxElement], cont: TextRange) {
    if let Some(SyntaxElement::Node(node)) = nodes.last_mut() {
        let mut found_desc = false;
        for child in node.children_mut() {
            if let SyntaxElement::Node(n) = child {
                if n.kind() == SyntaxKind::DESCRIPTION {
                    if n.range().is_empty() {
                        // Zero-length placeholder: replace the block entirely
                        // rather than extending from the old (wrong) start.
                        *n = text_block_single(SyntaxKind::DESCRIPTION, cont);
                    } else {
                        extend_text_block(n, cont);
                    }
                    found_desc = true;
                    break;
                }
            }
        }
        if !found_desc {
            node.push_child(SyntaxElement::Node(text_block_single(SyntaxKind::DESCRIPTION, cont)));
        }
        node.extend_range_to(cont.end());
    }
}

fn process_parameter_line(cursor: &LineCursor, nodes: &mut Vec<SyntaxElement>, entry_indent: &mut Option<usize>) {
    let indent_cols = cursor.current_indent_columns();
    if let Some(base) = *entry_indent {
        if indent_cols > base {
            extend_last_node_description(nodes, cursor.current_trimmed_range());
            return;
        }
    }
    if entry_indent.is_none() {
        *entry_indent = Some(indent_cols);
    }

    let col = cursor.current_indent();
    let trimmed = cursor.current_trimmed();
    let parts = parse_name_and_type(trimmed, cursor.line, col, cursor);
    let entry_range = cursor.current_trimmed_range();
    nodes.push(SyntaxElement::Node(build_parameter_node(&parts, entry_range)));
}

fn process_returns_line(cursor: &LineCursor, nodes: &mut Vec<SyntaxElement>, entry_indent: &mut Option<usize>) {
    let indent_cols = cursor.current_indent_columns();
    if let Some(base) = *entry_indent {
        if indent_cols > base {
            extend_last_node_description(nodes, cursor.current_trimmed_range());
            return;
        }
    }
    if entry_indent.is_none() {
        *entry_indent = Some(indent_cols);
    }

    let col = cursor.current_indent();
    let trimmed = cursor.current_trimmed();

    let (name, colon, return_type) = if let Some(colon_pos) = find_term_colon(trimmed) {
        let n = trimmed[..colon_pos].trim_end();
        let after_colon = &trimmed[colon_pos + 1..];
        let t = after_colon.trim();
        let ws_after = after_colon.len() - after_colon.trim_start().len();
        let type_col = col + colon_pos + 1 + ws_after;
        (
            Some(cursor.make_line_range(cursor.line, col, n.len())),
            Some(cursor.make_line_range(cursor.line, col + colon_pos, 1)),
            if t.is_empty() {
                None
            } else {
                Some(cursor.make_line_range(cursor.line, type_col, t.len()))
            },
        )
    } else {
        // Unnamed: type only (stored as RETURN_TYPE)
        (None, None, Some(cursor.current_trimmed_range()))
    };

    let entry_range = cursor.current_trimmed_range();
    nodes.push(SyntaxElement::Node(build_returns_node(
        name,
        colon,
        return_type,
        entry_range,
    )));
}

fn process_yields_line(cursor: &LineCursor, nodes: &mut Vec<SyntaxElement>, entry_indent: &mut Option<usize>) {
    let indent_cols = cursor.current_indent_columns();
    if let Some(base) = *entry_indent {
        if indent_cols > base {
            extend_last_node_description(nodes, cursor.current_trimmed_range());
            return;
        }
    }
    if entry_indent.is_none() {
        *entry_indent = Some(indent_cols);
    }

    let col = cursor.current_indent();
    let trimmed = cursor.current_trimmed();

    let (name, colon, return_type) = if let Some(colon_pos) = find_term_colon(trimmed) {
        let n = trimmed[..colon_pos].trim_end();
        let after_colon = &trimmed[colon_pos + 1..];
        let t = after_colon.trim();
        let ws_after = after_colon.len() - after_colon.trim_start().len();
        let type_col = col + colon_pos + 1 + ws_after;
        (
            Some(cursor.make_line_range(cursor.line, col, n.len())),
            Some(cursor.make_line_range(cursor.line, col + colon_pos, 1)),
            if t.is_empty() {
                None
            } else {
                Some(cursor.make_line_range(cursor.line, type_col, t.len()))
            },
        )
    } else {
        // Unnamed: type only (stored as RETURN_TYPE)
        (None, None, Some(cursor.current_trimmed_range()))
    };

    let entry_range = cursor.current_trimmed_range();
    nodes.push(SyntaxElement::Node(build_yields_node(
        name,
        colon,
        return_type,
        entry_range,
    )));
}

fn process_raises_line(cursor: &LineCursor, nodes: &mut Vec<SyntaxElement>, entry_indent: &mut Option<usize>) {
    let indent_cols = cursor.current_indent_columns();
    if let Some(base) = *entry_indent {
        if indent_cols > base {
            extend_last_node_description(nodes, cursor.current_trimmed_range());
            return;
        }
    }
    if entry_indent.is_none() {
        *entry_indent = Some(indent_cols);
    }

    let col = cursor.current_indent();
    let trimmed = cursor.current_trimmed();

    let (exc_type, colon, first_desc) = if let Some(colon_pos) = find_term_colon(trimmed) {
        let type_str = trimmed[..colon_pos].trim_end();
        let after_colon = &trimmed[colon_pos + 1..];
        let desc_str = after_colon.trim();
        let ws_after = after_colon.len() - after_colon.trim_start().len();
        let desc_col = col + colon_pos + 1 + ws_after;
        (
            cursor.make_line_range(cursor.line, col, type_str.len()),
            Some(cursor.make_line_range(cursor.line, col + colon_pos, 1)),
            if desc_str.is_empty() {
                None
            } else {
                Some(cursor.make_line_range(cursor.line, desc_col, desc_str.len()))
            },
        )
    } else {
        (cursor.current_trimmed_range(), None, None)
    };

    let entry_range = cursor.current_trimmed_range();
    nodes.push(SyntaxElement::Node(build_exception_node(
        exc_type,
        colon,
        first_desc,
        entry_range,
    )));
}

fn process_warning_line(cursor: &LineCursor, nodes: &mut Vec<SyntaxElement>, entry_indent: &mut Option<usize>) {
    let indent_cols = cursor.current_indent_columns();
    if let Some(base) = *entry_indent {
        if indent_cols > base {
            extend_last_node_description(nodes, cursor.current_trimmed_range());
            return;
        }
    }
    if entry_indent.is_none() {
        *entry_indent = Some(indent_cols);
    }

    let col = cursor.current_indent();
    let trimmed = cursor.current_trimmed();

    let (warn_type, colon, first_desc) = if let Some(colon_pos) = find_term_colon(trimmed) {
        let type_str = trimmed[..colon_pos].trim_end();
        let after_colon = &trimmed[colon_pos + 1..];
        let desc_str = after_colon.trim();
        let ws_after = after_colon.len() - after_colon.trim_start().len();
        let desc_col = col + colon_pos + 1 + ws_after;
        (
            cursor.make_line_range(cursor.line, col, type_str.len()),
            Some(cursor.make_line_range(cursor.line, col + colon_pos, 1)),
            if desc_str.is_empty() {
                None
            } else {
                Some(cursor.make_line_range(cursor.line, desc_col, desc_str.len()))
            },
        )
    } else {
        (cursor.current_trimmed_range(), None, None)
    };

    let entry_range = cursor.current_trimmed_range();
    nodes.push(SyntaxElement::Node(build_warning_node(
        warn_type,
        colon,
        first_desc,
        entry_range,
    )));
}

fn process_see_also_line(cursor: &LineCursor, nodes: &mut Vec<SyntaxElement>, entry_indent: &mut Option<usize>) {
    let indent_cols = cursor.current_indent_columns();
    if let Some(base) = *entry_indent {
        if indent_cols > base {
            extend_last_node_description(nodes, cursor.current_trimmed_range());
            return;
        }
    }
    if entry_indent.is_none() {
        *entry_indent = Some(indent_cols);
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
    let indent_cols = cursor.current_indent_columns();
    if let Some(base) = *entry_indent {
        if indent_cols > base {
            extend_last_node_description(nodes, cursor.current_trimmed_range());
            return;
        }
    }
    if entry_indent.is_none() {
        *entry_indent = Some(indent_cols);
    }

    let col = cursor.current_indent();
    let trimmed = cursor.current_trimmed();
    let parts = parse_name_and_type(trimmed, cursor.line, col, cursor);
    let entry_range = cursor.current_trimmed_range();
    nodes.push(SyntaxElement::Node(build_attribute_node(&parts, entry_range)));
}

fn process_method_line(cursor: &LineCursor, nodes: &mut Vec<SyntaxElement>, entry_indent: &mut Option<usize>) {
    let indent_cols = cursor.current_indent_columns();
    if let Some(base) = *entry_indent {
        if indent_cols > base {
            extend_last_node_description(nodes, cursor.current_trimmed_range());
            return;
        }
    }
    if entry_indent.is_none() {
        *entry_indent = Some(indent_cols);
    }

    let col = cursor.current_indent();
    let trimmed = cursor.current_trimmed();

    let (name, colon, first_desc) = if let Some(colon_pos) = find_term_colon(trimmed) {
        let n = trimmed[..colon_pos].trim_end();
        let after_colon = &trimmed[colon_pos + 1..];
        let desc_str = after_colon.trim();
        let ws_after = after_colon.len() - after_colon.trim_start().len();
        let desc_col = col + colon_pos + 1 + ws_after;
        (
            cursor.make_line_range(cursor.line, col, n.len()),
            Some(cursor.make_line_range(cursor.line, col + colon_pos, 1)),
            if desc_str.is_empty() {
                None
            } else {
                Some(cursor.make_line_range(cursor.line, desc_col, desc_str.len()))
            },
        )
    } else {
        (cursor.current_trimmed_range(), None, None)
    };

    let entry_range = cursor.current_trimmed_range();
    nodes.push(SyntaxElement::Node(build_method_node(
        name,
        colon,
        first_desc,
        entry_range,
    )));
}

// =============================================================================
// Section body kind tracking
// =============================================================================

enum SectionBody {
    Parameters(Vec<SyntaxElement>),
    Returns(Vec<SyntaxElement>),
    Yields(Vec<SyntaxElement>),
    Raises(Vec<SyntaxElement>),
    Warns(Vec<SyntaxElement>),
    SeeAlso(Vec<SyntaxElement>),
    References(Vec<SyntaxElement>),
    Attributes(Vec<SyntaxElement>),
    Methods(Vec<SyntaxElement>),
    FreeText(Option<TextRange>),
}

impl SectionBody {
    fn new(kind: NumPySectionKind) -> Self {
        match kind {
            NumPySectionKind::Parameters => Self::Parameters(Vec::new()),
            NumPySectionKind::OtherParameters => Self::Parameters(Vec::new()),
            NumPySectionKind::Receives => Self::Parameters(Vec::new()),
            NumPySectionKind::KeywordParameters => Self::Parameters(Vec::new()),
            NumPySectionKind::Returns => Self::Returns(Vec::new()),
            NumPySectionKind::Yields => Self::Yields(Vec::new()),
            NumPySectionKind::Raises => Self::Raises(Vec::new()),
            NumPySectionKind::Warns => Self::Warns(Vec::new()),
            NumPySectionKind::SeeAlso => Self::SeeAlso(Vec::new()),
            NumPySectionKind::References => Self::References(Vec::new()),
            NumPySectionKind::Attributes => Self::Attributes(Vec::new()),
            NumPySectionKind::Methods => Self::Methods(Vec::new()),
            _ => Self::FreeText(None),
        }
    }

    fn process_line(&mut self, cursor: &LineCursor, entry_indent: &mut Option<usize>) {
        match self {
            Self::Parameters(nodes) => process_parameter_line(cursor, nodes, entry_indent),
            Self::Returns(nodes) => process_returns_line(cursor, nodes, entry_indent),
            Self::Yields(nodes) => process_yields_line(cursor, nodes, entry_indent),
            Self::Raises(nodes) => process_raises_line(cursor, nodes, entry_indent),
            Self::Warns(nodes) => process_warning_line(cursor, nodes, entry_indent),
            Self::SeeAlso(nodes) => process_see_also_line(cursor, nodes, entry_indent),
            Self::References(nodes) => process_reference_line(cursor, SyntaxKind::NUMPY_REFERENCE, nodes, entry_indent),
            Self::Attributes(nodes) => process_attribute_line(cursor, nodes, entry_indent),
            Self::Methods(nodes) => process_method_line(cursor, nodes, entry_indent),
            Self::FreeText(range) => {
                let r = cursor.current_trimmed_range();
                match range {
                    Some(existing) => existing.extend(r),
                    None => *range = Some(r),
                }
            }
        }
    }

    fn into_children(self, source: &str) -> Vec<SyntaxElement> {
        match self {
            Self::Parameters(nodes) => nodes,
            Self::Returns(nodes) => nodes,
            Self::Yields(nodes) => nodes,
            Self::Raises(nodes) => nodes,
            Self::Warns(nodes) => nodes,
            Self::SeeAlso(nodes) => nodes,
            Self::References(nodes) => nodes,
            Self::Attributes(nodes) => nodes,
            Self::Methods(nodes) => nodes,
            Self::FreeText(range) => {
                if let Some(r) = range {
                    vec![SyntaxElement::Node(build_text_block(SyntaxKind::BODY_TEXT, r, source))]
                } else {
                    vec![]
                }
            }
        }
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
/// use pydocstring::parse::numpy::parse_numpy;
/// use pydocstring::syntax::SyntaxKind;
///
/// let input = "Summary.\n\nParameters\n----------\nx : int\n    The value.\n";
/// let parsed = parse_numpy(input);
/// let source = parsed.source();
/// let root = parsed.root();
///
/// // Access summary (a text block node wrapping per-line TEXT_LINE tokens)
/// let summary = pydocstring::parse::TextBlock::cast(root.find_node(SyntaxKind::SUMMARY).unwrap()).unwrap();
/// assert_eq!(summary.text(source), "Summary.");
///
/// // Access sections
/// let sections: Vec<_> = root.nodes(SyntaxKind::NUMPY_SECTION).collect();
/// assert_eq!(sections.len(), 1);
/// ```
pub fn parse_numpy(input: &str) -> Parsed {
    let mut cursor = LineCursor::new(input);
    let mut root_children: Vec<SyntaxElement> = Vec::new();

    cursor.skip_blanks();
    if cursor.is_eof() {
        let mut root = SyntaxNode::new(SyntaxKind::NUMPY_DOCSTRING, cursor.full_range(), root_children);
        crate::parse::trivia::attach_trivia(&mut root, input);
        return Parsed::new(input.to_string(), root);
    }

    // --- Summary (all lines until blank line or section header) ---
    if try_detect_header(&cursor).is_none() {
        let trimmed = cursor.current_trimmed();
        if !trimmed.is_empty() {
            let start_line = cursor.line;
            let start_col = cursor.current_indent();
            let mut last_line = start_line;

            while !cursor.is_eof() {
                if try_detect_header(&cursor).is_some() {
                    break;
                }
                let t = cursor.current_trimmed();
                if t.is_empty() {
                    break;
                }
                last_line = cursor.line;
                cursor.advance();
            }

            let last_text = cursor.line_text(last_line);
            let last_col = indent_len(last_text) + last_text.trim().len();
            let range = cursor.make_range(start_line, start_col, last_line, last_col);
            if !range.is_empty() {
                root_children.push(SyntaxElement::Node(build_text_block(SyntaxKind::SUMMARY, range, input)));
            }
        }
    }

    cursor.skip_blanks();

    // --- Deprecation directive ---
    if !cursor.is_eof()
        && try_detect_header(&cursor).is_none()
        && let Some(node) = try_parse_deprecation_directive(&mut cursor, SyntaxKind::NUMPY_DEPRECATION)
    {
        root_children.push(SyntaxElement::Node(node));
        cursor.skip_blanks();
    }

    // --- Extended summary ---
    if !cursor.is_eof() && try_detect_header(&cursor).is_none() {
        let start_line = cursor.line;
        let mut last_non_empty_line = cursor.line;
        let mut has_content = false;

        while !cursor.is_eof() {
            if try_detect_header(&cursor).is_some() {
                break;
            }
            let t = cursor.current_trimmed();
            if !t.is_empty() {
                last_non_empty_line = cursor.line;
                has_content = true;
            }
            cursor.advance();
        }

        if has_content {
            let first_line = cursor.line_text(start_line);
            let first_col = indent_len(first_line);
            let last_line = cursor.line_text(last_non_empty_line);
            let last_col = indent_len(last_line) + last_line.trim().len();
            let range = cursor.make_range(start_line, first_col, last_non_empty_line, last_col);
            root_children.push(SyntaxElement::Node(build_text_block(
                SyntaxKind::EXTENDED_SUMMARY,
                range,
                input,
            )));
        }
    }

    // --- Sections ---
    let mut current_header: Option<SectionHeaderInfo> = None;
    let mut current_body: Option<SectionBody> = None;
    let mut entry_indent: Option<usize> = None;

    while !cursor.is_eof() {
        if cursor.current_trimmed().is_empty() {
            cursor.advance();
            continue;
        }

        if let Some(header_info) = try_detect_header(&cursor) {
            // Flush previous section
            if let Some(prev_header) = current_header.take() {
                let section_node = flush_section(&cursor, prev_header, current_body.take().unwrap());
                root_children.push(SyntaxElement::Node(section_node));
            }

            current_body = Some(SectionBody::new(header_info.kind));
            current_header = Some(header_info);
            entry_indent = None;
            cursor.line += 2; // skip header + underline
            continue;
        }

        // NumPy entries sit at the same indentation level as the section header
        // (L = H = 0), so stray lines cannot be detected by indent or blank-line
        // heuristics alone.  Sections end only when the next header is detected.
        if let Some(ref mut body) = current_body {
            body.process_line(&cursor, &mut entry_indent);
        } else {
            // Stray line
            root_children.push(SyntaxElement::Token(SyntaxToken::new(
                SyntaxKind::STRAY_LINE,
                cursor.current_trimmed_range(),
            )));
        }

        cursor.advance();
    }

    // Flush final section
    if let Some(header) = current_header.take() {
        let section_node = flush_section(&cursor, header, current_body.take().unwrap());
        root_children.push(SyntaxElement::Node(section_node));
    }

    let mut root = SyntaxNode::new(SyntaxKind::NUMPY_DOCSTRING, cursor.full_range(), root_children);
    crate::parse::trivia::attach_trivia(&mut root, input);
    Parsed::new(input.to_string(), root)
}

fn flush_section(cursor: &LineCursor, header: SectionHeaderInfo, body: SectionBody) -> SyntaxNode {
    let header_start = header.range.start().raw() as usize;
    let section_range = cursor.span_back_from_cursor(header_start);

    let mut section_children = Vec::new();
    section_children.push(SyntaxElement::Node(build_section_header_node(&header)));
    section_children.extend(body.into_children(cursor.source()));

    SyntaxNode::new(SyntaxKind::NUMPY_SECTION, section_range, section_children)
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
        assert_eq!(try_detect_header(&c1).unwrap().kind, NumPySectionKind::Parameters);

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
        assert_eq!(try_detect_header(&c5).unwrap().kind, NumPySectionKind::Returns);
    }

    #[test]
    fn test_parse_name_and_type_basic() {
        let src = "x : int";
        let cursor = LineCursor::new(src);
        let p = parse_name_and_type(src, 0, 0, &cursor);
        assert_eq!(p.names[0].1.source_text(src), "x");
        assert!(p.colon.is_some());
        assert_eq!(p.param_type.unwrap().source_text(src), "int");
        assert!(p.optional.is_none());
    }

    #[test]
    fn test_parse_name_and_type_optional() {
        let src = "x : int, optional";
        let cursor = LineCursor::new(src);
        let p = parse_name_and_type(src, 0, 0, &cursor);
        assert_eq!(p.names[0].1.source_text(src), "x");
        assert!(p.colon.is_some());
        assert_eq!(p.param_type.unwrap().source_text(src), "int");
        assert!(p.optional.is_some());
    }

    #[test]
    fn test_parse_name_and_type_complex() {
        let src = "x : Dict[str, int], optional";
        let cursor = LineCursor::new(src);
        let p = parse_name_and_type(src, 0, 0, &cursor);
        assert!(p.colon.is_some());
        assert_eq!(p.param_type.unwrap().source_text(src), "Dict[str, int]");
        assert!(p.optional.is_some());
    }

    #[test]
    fn test_basic_parse() {
        let input = "Summary.\n\nParameters\n----------\nx : int\n    The value.\n";
        let parsed = parse_numpy(input);
        let root = parsed.root();
        assert_eq!(root.kind(), SyntaxKind::NUMPY_DOCSTRING);
        let summary = crate::parse::text_block::TextBlock::cast(root.find_node(SyntaxKind::SUMMARY).unwrap()).unwrap();
        assert_eq!(summary.text(parsed.source()), "Summary.");
        let sections: Vec<_> = root.nodes(SyntaxKind::NUMPY_SECTION).collect();
        assert_eq!(sections.len(), 1);
    }

    /// A google-style entry in a NumPy section stores its children in source
    /// order: the COLON (found after the close bracket) must not precede TYPE.
    #[test]
    fn test_google_style_entry_children_in_source_order() {
        let input = "Summary.\n\nParameters\n----------\nname (str): The name.\n";
        let parsed = parse_numpy(input);
        let section = parsed.root().find_node(SyntaxKind::NUMPY_SECTION).unwrap();
        let param = section.find_node(SyntaxKind::NUMPY_PARAMETER).unwrap();
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
        let section = parsed.root().find_node(SyntaxKind::NUMPY_SECTION).unwrap();
        let param = section.find_node(SyntaxKind::NUMPY_PARAMETER).unwrap();
        let open = param.find_token(SyntaxKind::OPEN_BRACKET).unwrap();
        let missing = param.find_missing(SyntaxKind::TYPE).unwrap();
        assert_eq!(missing.range().start(), open.range().end());
    }
}
