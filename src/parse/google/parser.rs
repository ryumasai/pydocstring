//! Google style docstring parser (SyntaxNode-based).
//!
//! Parses docstrings in Google format and produces a [`Parsed`] result
//! containing a tree of [`SyntaxNode`]s and [`SyntaxToken`]s.

use crate::cursor::{LineCursor, indent_len};
use crate::parse::google::kind::GoogleSectionKind;
use crate::parse::utils::{
    find_colon_ignoring_parens, find_entry_open_bracket, find_matching_close, find_term_colon, strip_optional,
};
use crate::syntax::{Parsed, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
use crate::text::TextRange;

// =============================================================================
// Section detection
// =============================================================================

/// Extract the section name from a trimmed header line.
///
/// Strips the trailing colon (and any whitespace before it) if present.
/// Returns `(name, has_colon)` where `name` is the clean section name.
fn extract_section_name(trimmed: &str) -> (&str, bool) {
    if let Some(stripped) = trimmed.strip_suffix(':') {
        (stripped.trim_end(), true)
    } else {
        (trimmed, false)
    }
}

// =============================================================================
// Entry header parsing
// =============================================================================

/// Type information from a parsed entry header.
struct TypeInfo {
    open_bracket: TextRange,
    r#type: Option<TextRange>,
    close_bracket: Option<TextRange>,
    optional: Option<TextRange>,
}

/// Parsed components of a Google-style entry header.
struct EntryHeader {
    range: TextRange,
    name: TextRange,
    type_info: Option<TypeInfo>,
    colon: Option<TextRange>,
    first_description: Option<TextRange>,
}

/// Parse a Google-style entry header at `cursor.line`.
///
/// Uses a left-to-right confirmation algorithm:
///   1. Find opening bracket → NAME is everything before it
///   2. Find matching close bracket → TYPE is inside brackets
///   3. Check character after bracket/whitespace for `:` → COLON, rest is DESC
///   4. Otherwise remaining text is DESC (missing COLON) or nothing
fn parse_entry_header(cursor: &LineCursor, parse_type: bool) -> EntryHeader {
    let line = cursor.current_line_text();
    let trimmed = line.trim();
    let entry_start = cursor.substr_offset(trimmed);

    // --- Bracket entry: `name (type): desc` and variants ---
    if parse_type {
        if let Some(bracket_pos) = find_entry_open_bracket(trimmed) {
            let name = trimmed[..bracket_pos].trim_end();
            let name_span = TextRange::from_offset_len(entry_start, name.len());
            let open_bracket = TextRange::from_offset_len(entry_start + bracket_pos, 1);

            let close_pos = find_matching_close(trimmed, bracket_pos);
            let (type_text, close_bracket, colon, first_description) = match close_pos {
                Some(cp) => {
                    // Bracket matched — check what follows.
                    let cb = Some(TextRange::from_offset_len(entry_start + cp, 1));
                    let after_close = &trimmed[cp + 1..];
                    let after_trimmed = after_close.trim_start();
                    if after_trimmed.starts_with(':') {
                        // `:` confirmed → COLON + DESC
                        let colon_abs = cp + 1 + (after_close.len() - after_trimmed.len());
                        let colon_span = Some(TextRange::from_offset_len(entry_start + colon_abs, 1));
                        let after_colon = &trimmed[colon_abs + 1..];
                        let desc = after_colon.trim();
                        let desc_span = if desc.is_empty() {
                            None
                        } else {
                            let ws = after_colon.len() - after_colon.trim_start().len();
                            Some(TextRange::from_offset_len(entry_start + colon_abs + 1 + ws, desc.len()))
                        };
                        (&trimmed[bracket_pos + 1..cp], cb, colon_span, desc_span)
                    } else if !after_trimmed.is_empty() {
                        // Text without colon → DESC (missing COLON)
                        let ws = after_close.len() - after_trimmed.len();
                        let desc_span = Some(TextRange::from_offset_len(
                            entry_start + cp + 1 + ws,
                            after_trimmed.len(),
                        ));
                        (&trimmed[bracket_pos + 1..cp], cb, None, desc_span)
                    } else {
                        // Nothing after close bracket
                        (&trimmed[bracket_pos + 1..cp], cb, None, None)
                    }
                }
                None => {
                    // No matching close bracket — look for colon ignoring paren depth.
                    if let Some(colon_abs) = find_colon_ignoring_parens(trimmed, bracket_pos + 1) {
                        let type_raw = &trimmed[bracket_pos + 1..colon_abs];
                        let colon_span = Some(TextRange::from_offset_len(entry_start + colon_abs, 1));
                        let after_colon = &trimmed[colon_abs + 1..];
                        let desc = after_colon.trim();
                        let desc_span = if desc.is_empty() {
                            None
                        } else {
                            let ws = after_colon.len() - after_colon.trim_start().len();
                            Some(TextRange::from_offset_len(entry_start + colon_abs + 1 + ws, desc.len()))
                        };
                        (type_raw, None, colon_span, desc_span)
                    } else {
                        (&trimmed[bracket_pos + 1..], None, None, None)
                    }
                }
            };

            let type_trimmed = type_text.trim();
            let leading_ws = type_text.len() - type_text.trim_start().len();
            let type_offset = bracket_pos + 1 + leading_ws;
            let (clean_type, opt_rel) = strip_optional(type_trimmed);

            let type_span = if !clean_type.is_empty() {
                Some(TextRange::from_offset_len(entry_start + type_offset, clean_type.len()))
            } else {
                None
            };
            let opt_span = opt_rel.map(|r| TextRange::from_offset_len(entry_start + type_offset + r, "optional".len()));

            let type_info = Some(TypeInfo {
                open_bracket,
                r#type: type_span,
                close_bracket,
                optional: opt_span,
            });

            let range_end = first_description
                .as_ref()
                .map(|d| d.end())
                .or_else(|| colon.as_ref().map(|c| c.end()))
                .or_else(|| close_bracket.map(|cb| cb.end()))
                .unwrap_or_else(|| TextRange::from_offset_len(entry_start, trimmed.len()).end());

            return EntryHeader {
                range: TextRange::new(name_span.start(), range_end),
                name: name_span,
                type_info,
                colon,
                first_description,
            };
        }
    }

    // --- `name: desc` ---
    if let Some(colon_rel) = find_term_colon(trimmed) {
        let name = trimmed[..colon_rel].trim_end();
        // If the colon is at position 0 (e.g. RST-style `:param foo:`), the
        // name would be empty which is invalid.  Fall through to the bare-name
        // fallback so the whole line is preserved as-is rather than producing
        // an empty NAME token that later panics in `required_token`.
        if !name.is_empty() {
            let after_colon = &trimmed[colon_rel + 1..];
            let desc = after_colon.trim_start();
            let ws_after = after_colon.len() - desc.len();
            let desc_start = entry_start + colon_rel + 1 + ws_after;
            let colon_span = TextRange::from_offset_len(entry_start + colon_rel, 1);
            let first_description = if desc.is_empty() {
                None
            } else {
                Some(TextRange::from_offset_len(desc_start, desc.len()))
            };
            let range_end = if let Some(ref d) = first_description {
                d.end()
            } else {
                colon_span.end()
            };
            let name_span = TextRange::from_offset_len(entry_start, name.len());
            return EntryHeader {
                range: TextRange::new(name_span.start(), range_end),
                name: name_span,
                type_info: None,
                colon: Some(colon_span),
                first_description,
            };
        }
    }

    // --- Fallback: bare name ---
    let name_span = TextRange::from_offset_len(entry_start, trimmed.len());
    EntryHeader {
        range: name_span,
        name: name_span,
        type_info: None,
        colon: None,
        first_description: None,
    }
}

// =============================================================================
// Section header parsing
// =============================================================================

/// Parsed section header info (internal representation before building SyntaxNode).
struct SectionHeaderInfo {
    range: TextRange,
    kind: GoogleSectionKind,
    name: TextRange,
    colon: Option<TextRange>,
    indent_columns: usize,
}

fn try_parse_section_header(cursor: &LineCursor) -> Option<SectionHeaderInfo> {
    let trimmed = cursor.current_trimmed();
    let (name, has_colon) = extract_section_name(trimmed);

    if name.is_empty() || !name.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }

    let is_header = if has_colon {
        !name.contains(':') && name.chars().all(|c| c.is_alphanumeric() || c.is_ascii_whitespace())
    } else {
        GoogleSectionKind::is_known(&name.to_ascii_lowercase())
    };

    if !is_header {
        return None;
    }

    let col = cursor.current_indent();
    let header_name = name.trim_end();

    let colon = if has_colon {
        let colon_col = col + trimmed.len() - 1;
        Some(cursor.make_line_range(cursor.line, colon_col, 1))
    } else {
        None
    };

    let normalized = header_name.to_ascii_lowercase();
    let kind = GoogleSectionKind::from_name(&normalized);

    Some(SectionHeaderInfo {
        range: cursor.current_trimmed_range(),
        kind,
        name: cursor.make_line_range(cursor.line, col, header_name.len()),
        colon,
        indent_columns: cursor.current_indent_columns(),
    })
}

// =============================================================================
// SyntaxNode builders
// =============================================================================

fn build_section_header_node(info: &SectionHeaderInfo) -> SyntaxNode {
    let mut children = Vec::new();
    children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::NAME, info.name)));
    if let Some(colon) = info.colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COLON, colon)));
    } else {
        // Colon is grammatically required; emit a zero-length COLON token
        // at the position where it should appear (right after the name).
        children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::COLON,
            TextRange::new(info.name.end(), info.name.end()),
        )));
    }
    SyntaxNode::new(SyntaxKind::GOOGLE_SECTION_HEADER, info.range, children)
}

/// Build a SyntaxNode for an arg-like entry (GoogleArg, GoogleAttribute, GoogleMethod).
fn build_arg_node(kind: SyntaxKind, header: &EntryHeader, range: TextRange) -> SyntaxNode {
    let mut children = Vec::new();
    children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::NAME, header.name)));
    if let Some(ti) = &header.type_info {
        children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::OPEN_BRACKET,
            ti.open_bracket,
        )));
        if let Some(t) = ti.r#type {
            children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::TYPE, t)));
        } else {
            // Empty brackets `()`: emit a zero-length missing TYPE token right
            // after the open bracket so callers can distinguish `a ()` from `a:`.
            let missing_pos = ti.open_bracket.end();
            children.push(SyntaxElement::Token(SyntaxToken::new(
                SyntaxKind::TYPE,
                TextRange::new(missing_pos, missing_pos),
            )));
        }
        if let Some(cb) = ti.close_bracket {
            children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::CLOSE_BRACKET, cb)));
        } else {
            // Close bracket expected but missing.
            let missing_pos = ti.r#type.map(|t| t.end()).unwrap_or(ti.open_bracket.end());
            children.push(SyntaxElement::Token(SyntaxToken::new(
                SyntaxKind::CLOSE_BRACKET,
                TextRange::new(missing_pos, missing_pos),
            )));
        }
        if let Some(opt) = ti.optional {
            children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::OPTIONAL, opt)));
        }
    }
    if let Some(colon) = header.colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COLON, colon)));
    } else if header.type_info.is_some() && header.first_description.is_some() {
        // Bracket-style entry with text after it but no colon.
        let missing_pos = header
            .type_info
            .as_ref()
            .and_then(|ti| ti.close_bracket.map(|cb| cb.end()))
            .or_else(|| header.type_info.as_ref().and_then(|ti| ti.r#type.map(|t| t.end())))
            .unwrap_or(header.name.end());
        children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::COLON,
            TextRange::new(missing_pos, missing_pos),
        )));
    }
    if let Some(desc) = header.first_description {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::DESCRIPTION, desc)));
    } else if let Some(colon) = header.colon {
        // Colon present but no description: zero-length placeholder so callers
        // can distinguish `a (int):` from `a (int)` via find_missing(DESCRIPTION).
        children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::DESCRIPTION,
            TextRange::new(colon.end(), colon.end()),
        )));
    }
    // Ensure children are in source order (needed when colon/description
    // appear before the close bracket, e.g., `arg (int:desc.)`).
    children.sort_by_key(|c| c.range().start());
    SyntaxNode::new(kind, range, children)
}

/// Build a SyntaxNode for an exception entry.
fn build_exception_node(header: &EntryHeader, range: TextRange) -> SyntaxNode {
    let mut children = Vec::new();
    children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::TYPE, header.name)));
    if let Some(colon) = header.colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COLON, colon)));
    }
    if let Some(desc) = header.first_description {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::DESCRIPTION, desc)));
    } else if let Some(colon) = header.colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::DESCRIPTION,
            TextRange::new(colon.end(), colon.end()),
        )));
    }
    SyntaxNode::new(SyntaxKind::GOOGLE_EXCEPTION, range, children)
}

/// Build a SyntaxNode for a warning entry.
fn build_warning_node(header: &EntryHeader, range: TextRange) -> SyntaxNode {
    let mut children = Vec::new();
    children.push(SyntaxElement::Token(SyntaxToken::new(
        SyntaxKind::WARNING_TYPE,
        header.name,
    )));
    if let Some(colon) = header.colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COLON, colon)));
    }
    if let Some(desc) = header.first_description {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::DESCRIPTION, desc)));
    } else if let Some(colon) = header.colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::DESCRIPTION,
            TextRange::new(colon.end(), colon.end()),
        )));
    }
    SyntaxNode::new(SyntaxKind::GOOGLE_WARNING, range, children)
}

/// Build a SyntaxNode for a see-also entry.
fn build_see_also_node(header: &EntryHeader, range: TextRange, source: &str) -> SyntaxNode {
    let mut children = Vec::new();
    // Split name by comma into individual name tokens
    let name_text = header.name.source_text(source);
    let base = header.name.start().raw() as usize;
    let mut offset = 0;
    for part in name_text.split(',') {
        let name = part.trim();
        if !name.is_empty() {
            let lead = part.len() - part.trim_start().len();
            children.push(SyntaxElement::Token(SyntaxToken::new(
                SyntaxKind::NAME,
                TextRange::from_offset_len(base + offset + lead, name.len()),
            )));
        }
        offset += part.len() + 1;
    }
    if let Some(colon) = header.colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COLON, colon)));
    }
    if let Some(desc) = header.first_description {
        children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::DESCRIPTION, desc)));
    } else if let Some(colon) = header.colon {
        children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::DESCRIPTION,
            TextRange::new(colon.end(), colon.end()),
        )));
    }
    SyntaxNode::new(SyntaxKind::GOOGLE_SEE_ALSO_ITEM, range, children)
}

// =============================================================================
// Section body helpers
// =============================================================================

fn parse_entry(cursor: &LineCursor, parse_type: bool) -> (EntryHeader, TextRange) {
    let header = parse_entry_header(cursor, parse_type);
    let entry_col = cursor.current_indent();
    let range_end = header
        .first_description
        .as_ref()
        .map_or(header.range.end(), |d| d.end());
    let (end_line, end_col_pos) = cursor.offset_to_line_col(range_end.raw() as usize);
    let entry_range = cursor.make_range(cursor.line, entry_col, end_line, end_col_pos);
    (header, entry_range)
}

fn build_content_range(cursor: &LineCursor, first: Option<usize>, last: usize) -> Option<TextRange> {
    first.map(|f| {
        let first_line = cursor.line_text(f);
        let first_col = indent_len(first_line);
        let last_line = cursor.line_text(last);
        let last_col = indent_len(last_line) + last_line.trim().len();
        cursor.make_range(f, first_col, last, last_col)
    })
}

// =============================================================================
// Per-line section body processors
// =============================================================================

/// Extend the DESCRIPTION token of the last child node, or add one.
fn extend_last_node_description(nodes: &mut [SyntaxElement], cont: TextRange) {
    if let Some(SyntaxElement::Node(node)) = nodes.last_mut() {
        // Find or add description token, extend range
        let mut found_desc = false;
        for child in node.children_mut() {
            if let SyntaxElement::Token(t) = child {
                if t.kind() == SyntaxKind::DESCRIPTION {
                    if t.is_missing() {
                        // Zero-length placeholder: replace range entirely rather
                        // than extending from the old (wrong) start position.
                        *t = SyntaxToken::new(SyntaxKind::DESCRIPTION, cont);
                    } else {
                        t.extend_range(cont);
                    }
                    found_desc = true;
                    break;
                }
            }
        }
        if !found_desc {
            node.push_child(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::DESCRIPTION, cont)));
        }
        // Extend node range
        node.extend_range_to(cont.end());
    }
}

fn process_arg_line(
    cursor: &LineCursor,
    node_kind: SyntaxKind,
    nodes: &mut Vec<SyntaxElement>,
    entry_indent: &mut Option<usize>,
) {
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
    let (header, entry_range) = parse_entry(cursor, node_kind != SyntaxKind::GOOGLE_METHOD);
    nodes.push(SyntaxElement::Node(build_arg_node(node_kind, &header, entry_range)));
}

fn process_exception_line(cursor: &LineCursor, nodes: &mut Vec<SyntaxElement>, entry_indent: &mut Option<usize>) {
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
    let (header, entry_range) = parse_entry(cursor, false);
    nodes.push(SyntaxElement::Node(build_exception_node(&header, entry_range)));
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
    let (header, entry_range) = parse_entry(cursor, false);
    nodes.push(SyntaxElement::Node(build_warning_node(&header, entry_range)));
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
    let (header, entry_range) = parse_entry(cursor, false);
    nodes.push(SyntaxElement::Node(build_see_also_node(
        &header,
        entry_range,
        cursor.source(),
    )));
}

/// Returns/Yields section state during parsing.
struct ReturnsState {
    range: Option<TextRange>,
    return_type: Option<TextRange>,
    colon: Option<TextRange>,
    description: Option<TextRange>,
}

impl ReturnsState {
    fn new() -> Self {
        Self {
            range: None,
            return_type: None,
            colon: None,
            description: None,
        }
    }

    fn process_line(&mut self, cursor: &LineCursor) {
        let trimmed_range = cursor.current_trimmed_range();
        if self.range.is_none() {
            self.range = Some(trimmed_range);
            let trimmed = cursor.current_trimmed();
            let col = cursor.current_indent();
            if let Some(colon_pos) = find_term_colon(trimmed) {
                let type_str = trimmed[..colon_pos].trim_end();
                let after_colon = &trimmed[colon_pos + 1..];
                let desc_str = after_colon.trim_start();
                let ws_after = after_colon.len() - desc_str.len();
                self.return_type = Some(cursor.make_line_range(cursor.line, col, type_str.len()));
                self.colon = Some(cursor.make_line_range(cursor.line, col + colon_pos, 1));
                let desc_start = col + colon_pos + 1 + ws_after;
                self.description = if desc_str.is_empty() {
                    None
                } else {
                    Some(cursor.make_line_range(cursor.line, desc_start, desc_str.len()))
                };
            } else {
                self.description = Some(trimmed_range);
            }
        } else {
            match self.description {
                Some(ref mut desc) => desc.extend(trimmed_range),
                None => self.description = Some(trimmed_range),
            }
            if let Some(ref mut r) = self.range {
                r.extend(trimmed_range);
            }
        }
    }

    fn into_node(self, kind: SyntaxKind) -> Option<SyntaxNode> {
        let range = self.range?;
        let mut children = Vec::new();
        if let Some(rt) = self.return_type {
            children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::RETURN_TYPE, rt)));
        }
        if let Some(colon) = self.colon {
            children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COLON, colon)));
        }
        if let Some(desc) = self.description {
            children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::DESCRIPTION, desc)));
        }
        Some(SyntaxNode::new(kind, range, children))
    }
}

// =============================================================================
// Section body kind tracking
// =============================================================================

/// Tracks the current section being parsed and accumulates its body children.
enum SectionBody {
    /// Args-like entries (Args, KeywordArgs, OtherParameters, Receives, Attributes, Methods)
    Args(SyntaxKind, Vec<SyntaxElement>),
    /// Returns/Yields
    Returns(SyntaxKind, ReturnsState),
    /// Raises
    Raises(Vec<SyntaxElement>),
    /// Warns
    Warns(Vec<SyntaxElement>),
    /// SeeAlso
    SeeAlso(Vec<SyntaxElement>),
    /// Free-text (Notes, Examples, etc.)
    FreeText(Option<TextRange>),
}

impl SectionBody {
    #[rustfmt::skip]
    fn new(kind: GoogleSectionKind) -> Self {
        match kind {
            GoogleSectionKind::Args => Self::Args(SyntaxKind::GOOGLE_ARG, Vec::new()),
            GoogleSectionKind::KeywordArgs => Self::Args(SyntaxKind::GOOGLE_ARG, Vec::new()),
            GoogleSectionKind::OtherParameters => Self::Args(SyntaxKind::GOOGLE_ARG, Vec::new()),
            GoogleSectionKind::Receives => Self::Args(SyntaxKind::GOOGLE_ARG, Vec::new()),
            GoogleSectionKind::Attributes => Self::Args(SyntaxKind::GOOGLE_ATTRIBUTE, Vec::new()),
            GoogleSectionKind::Methods => Self::Args(SyntaxKind::GOOGLE_METHOD, Vec::new()),
            GoogleSectionKind::Returns => Self::Returns(SyntaxKind::GOOGLE_RETURNS, ReturnsState::new()),
            GoogleSectionKind::Yields => Self::Returns(SyntaxKind::GOOGLE_YIELDS, ReturnsState::new()),
            GoogleSectionKind::Raises => Self::Raises(Vec::new()),
            GoogleSectionKind::Warns => Self::Warns(Vec::new()),
            GoogleSectionKind::SeeAlso => Self::SeeAlso(Vec::new()),
            _ => Self::FreeText(None),
        }
    }

    #[rustfmt::skip]
    fn process_line(&mut self, cursor: &LineCursor, entry_indent: &mut Option<usize>) {
        match self {
            Self::Args(node_kind, nodes) => process_arg_line(cursor, *node_kind, nodes, entry_indent),
            Self::Returns(_, state) => state.process_line(cursor),
            Self::Raises(nodes) => process_exception_line(cursor, nodes, entry_indent),
            Self::Warns(nodes) => process_warning_line(cursor, nodes, entry_indent),
            Self::SeeAlso(nodes) => process_see_also_line(cursor, nodes, entry_indent),
            Self::FreeText(range) => {
                let r = cursor.current_trimmed_range();
                match range {
                    Some(existing) => existing.extend(r),
                    None => *range = Some(r),
                }
            }
        }
    }

    fn into_children(self) -> Vec<SyntaxElement> {
        match self {
            Self::Args(_, nodes) => nodes,
            Self::Returns(kind, state) => match state.into_node(kind) {
                Some(node) => vec![SyntaxElement::Node(node)],
                None => vec![],
            },
            Self::Raises(nodes) => nodes,
            Self::Warns(nodes) => nodes,
            Self::SeeAlso(nodes) => nodes,
            Self::FreeText(range) => match range {
                Some(r) => vec![SyntaxElement::Token(SyntaxToken::new(SyntaxKind::BODY_TEXT, r))],
                None => vec![],
            },
        }
    }
}

// =============================================================================
// Main parser
// =============================================================================

/// Parse a Google-style docstring into a [`Parsed`] result.
///
/// # Example
///
/// ```rust
/// use pydocstring::parse::google::parse_google;
/// use pydocstring::syntax::SyntaxKind;
///
/// let input = "Summary.\n\nArgs:\n    x (int): The value.\n\nReturns:\n    int: The result.";
/// let parsed = parse_google(input);
/// let source = parsed.source();
/// let root = parsed.root();
///
/// // Access summary
/// let summary = root.find_token(SyntaxKind::SUMMARY).unwrap();
/// assert_eq!(summary.text(source), "Summary.");
///
/// // Access sections
/// let sections: Vec<_> = root.nodes(SyntaxKind::GOOGLE_SECTION).collect();
/// assert_eq!(sections.len(), 2);
/// ```
pub fn parse_google(input: &str) -> Parsed {
    let mut line_cursor = LineCursor::new(input);
    let mut root_children: Vec<SyntaxElement> = Vec::new();

    line_cursor.skip_blanks();
    if line_cursor.is_eof() {
        let root = SyntaxNode::new(SyntaxKind::GOOGLE_DOCSTRING, line_cursor.full_range(), root_children);
        return Parsed::new(input.to_string(), root);
    }

    let mut summary_done = false;
    let mut extended_done = false;
    let mut summary_first: Option<usize> = None;
    let mut summary_last: usize = 0;
    let mut ext_first: Option<usize> = None;
    let mut ext_last: usize = 0;

    let mut current_header: Option<SectionHeaderInfo> = None;
    let mut current_body: Option<SectionBody> = None;
    let mut entry_indent: Option<usize> = None;
    let mut body_is_deeper: Option<bool> = None;

    while !line_cursor.is_eof() {
        // --- Blank lines ---
        if line_cursor.current_trimmed().is_empty() {
            if !summary_done && summary_first.is_some() {
                root_children.push(SyntaxElement::Token(SyntaxToken::new(
                    SyntaxKind::SUMMARY,
                    build_content_range(&line_cursor, summary_first, summary_last).unwrap(),
                )));
                summary_done = true;
            }
            line_cursor.advance();
            continue;
        }

        // --- Detect section header ---
        // Lines that are strictly more indented than the current section header
        // are body entries (e.g., `b :` inside an Args block) and must never
        // be mistaken for a new section header.
        let may_be_header = current_header
            .as_ref()
            .is_none_or(|h| line_cursor.current_indent_columns() <= h.indent_columns);
        if may_be_header && let Some(header_info) = try_parse_section_header(&line_cursor) {
            // Finalise pending pre-section content
            if !summary_done {
                if summary_first.is_some() {
                    root_children.push(SyntaxElement::Token(SyntaxToken::new(
                        SyntaxKind::SUMMARY,
                        build_content_range(&line_cursor, summary_first, summary_last).unwrap(),
                    )));
                }
                summary_done = true;
            }
            if !extended_done {
                if ext_first.is_some() {
                    root_children.push(SyntaxElement::Token(SyntaxToken::new(
                        SyntaxKind::EXTENDED_SUMMARY,
                        build_content_range(&line_cursor, ext_first, ext_last).unwrap(),
                    )));
                }
                extended_done = true;
            }

            // Flush previous section
            if let Some(prev_header) = current_header.take() {
                flush_section(
                    &line_cursor,
                    &mut root_children,
                    prev_header,
                    current_body.take().unwrap(),
                );
            }

            // Start new section
            current_body = Some(SectionBody::new(header_info.kind));
            current_header = Some(header_info);
            entry_indent = None;
            body_is_deeper = None;
            line_cursor.advance();
            continue;
        }

        // --- Flush section when a stray line is detected ---
        //
        // body_is_deeper tracks whether the section body is indented deeper than
        // the section header:
        //   None        – no body line seen yet; flush only if STRICTLY shallower
        //                 than the header (lets zero-indent first entries through)
        //   Some(true)  – body is deeper; flush when line returns to header indent
        //   Some(false) – body is at same/shallower level (zero-indent style);
        //                 never flush by indent — rely on section-header detection
        {
            let l = line_cursor.current_indent_columns();
            let should_flush = current_header.as_ref().is_some_and(|h| match body_is_deeper {
                None => l < h.indent_columns,
                Some(true) => l <= h.indent_columns,
                Some(false) => false,
            });
            if should_flush {
                if let Some(prev_header) = current_header.take() {
                    flush_section(
                        &line_cursor,
                        &mut root_children,
                        prev_header,
                        current_body.take().unwrap(),
                    );
                }
                body_is_deeper = None;
            }
        }

        // --- Process line based on current state ---
        if let Some(ref mut body) = current_body {
            if body_is_deeper.is_none() {
                let entry_l = line_cursor.current_indent_columns();
                body_is_deeper = Some(current_header.as_ref().is_some_and(|h| entry_l > h.indent_columns));
            }
            body.process_line(&line_cursor, &mut entry_indent);
        } else if !summary_done {
            if summary_first.is_none() {
                summary_first = Some(line_cursor.line);
            }
            summary_last = line_cursor.line;
        } else if !extended_done {
            if ext_first.is_none() {
                ext_first = Some(line_cursor.line);
            }
            ext_last = line_cursor.line;
        } else {
            root_children.push(SyntaxElement::Token(SyntaxToken::new(
                SyntaxKind::STRAY_LINE,
                line_cursor.current_trimmed_range(),
            )));
        }

        line_cursor.advance();
    }

    // Flush final section
    if let Some(header) = current_header.take() {
        flush_section(&line_cursor, &mut root_children, header, current_body.take().unwrap());
    }

    // Finalise at EOF
    if !summary_done && summary_first.is_some() {
        root_children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::SUMMARY,
            build_content_range(&line_cursor, summary_first, summary_last).unwrap(),
        )));
    }
    if !extended_done && ext_first.is_some() {
        root_children.push(SyntaxElement::Token(SyntaxToken::new(
            SyntaxKind::EXTENDED_SUMMARY,
            build_content_range(&line_cursor, ext_first, ext_last).unwrap(),
        )));
    }

    let root = SyntaxNode::new(SyntaxKind::GOOGLE_DOCSTRING, line_cursor.full_range(), root_children);
    Parsed::new(input.to_string(), root)
}

fn flush_section(
    cursor: &LineCursor,
    root_children: &mut Vec<SyntaxElement>,
    header_info: SectionHeaderInfo,
    body: SectionBody,
) {
    let header_start = header_info.range.start().raw() as usize;
    let section_range = cursor.span_back_from_cursor(header_start);

    let header_node = build_section_header_node(&header_info);
    let mut section_children = vec![SyntaxElement::Node(header_node)];
    section_children.extend(body.into_children());

    root_children.push(SyntaxElement::Node(SyntaxNode::new(
        SyntaxKind::GOOGLE_SECTION,
        section_range,
        section_children,
    )));
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn is_header(text: &str) -> bool {
        let cursor = LineCursor::new(text);
        try_parse_section_header(&cursor).is_some()
    }

    #[test]
    fn test_is_section_header() {
        assert!(is_header("Args:"));
        assert!(is_header("NotASection:"));
        assert!(is_header("Returns:"));
        assert!(is_header("Custom:"));
        assert!(is_header("args:"));
        assert!(is_header("RETURNS:"));
        assert!(!is_header("key: value:"));
        assert!(is_header(
            "This is a very long line that should not be a section header:"
        ));
        assert!(is_header("Args :"));
        assert!(is_header("Returns :"));
        assert!(is_header("Args"));
        assert!(is_header("Returns"));
        assert!(is_header("args"));
        assert!(is_header("RETURNS"));
        assert!(is_header("See Also"));
        assert!(!is_header("NotASection"));
        assert!(!is_header("SomeWord"));
    }

    fn header_from(text: &str) -> EntryHeader {
        let cursor = LineCursor::new(text);
        parse_entry_header(&cursor, false)
    }

    fn header_from_lenient(text: &str) -> EntryHeader {
        let cursor = LineCursor::new(text);
        parse_entry_header(&cursor, true)
    }

    #[test]
    fn test_parse_entry_header_with_type() {
        let src = "name (int): Description";
        let header = header_from_lenient(src);
        assert_eq!(header.name.source_text(src), "name");
        assert!(header.type_info.is_some());
        let ti = header.type_info.unwrap();
        assert_eq!(ti.r#type.unwrap().source_text(src), "int");
        assert_eq!(header.first_description.unwrap().source_text(src), "Description");
    }

    #[test]
    fn test_parse_entry_header_optional() {
        let src = "name (int, optional): Description";
        let header = header_from_lenient(src);
        assert_eq!(header.name.source_text(src), "name");
        let ti = header.type_info.unwrap();
        assert_eq!(ti.r#type.unwrap().source_text(src), "int");
        assert!(ti.optional.is_some());
        assert_eq!(ti.optional.unwrap().source_text(src), "optional");
    }

    #[test]
    fn test_parse_entry_header_no_type() {
        let src = "name: Description";
        let header = header_from(src);
        assert_eq!(header.name.source_text(src), "name");
        assert!(header.type_info.is_none());
        assert_eq!(header.first_description.unwrap().source_text(src), "Description");
    }

    #[test]
    fn test_parse_entry_header_complex_type() {
        let src = "data (Dict[str, List[int]]): Values";
        let header = header_from_lenient(src);
        assert_eq!(header.name.source_text(src), "data");
        let ti = header.type_info.unwrap();
        assert_eq!(ti.r#type.unwrap().source_text(src), "Dict[str, List[int]]");
        assert_eq!(header.first_description.unwrap().source_text(src), "Values");
    }

    #[test]
    fn test_parse_entry_header_colon_only() {
        let src = "x:";
        let header = header_from(src);
        assert_eq!(header.name.source_text(src), "x");
        assert!(header.type_info.is_none());
        assert!(header.first_description.is_none());
    }

    #[test]
    fn test_parse_entry_header_varargs() {
        let src1 = "*args: Positional arguments";
        let header = header_from(src1);
        assert_eq!(header.name.source_text(src1), "*args");
        assert_eq!(
            header.first_description.unwrap().source_text(src1),
            "Positional arguments"
        );

        let src2 = "**kwargs (dict): Keyword arguments";
        let header = header_from_lenient(src2);
        assert_eq!(header.name.source_text(src2), "**kwargs");
        let ti = header.type_info.unwrap();
        assert_eq!(ti.r#type.unwrap().source_text(src2), "dict");
    }

    #[test]
    fn test_parse_entry_header_no_space_after_colon() {
        let src = "name:Description";
        let header = header_from(src);
        assert_eq!(header.name.source_text(src), "name");
        assert!(header.type_info.is_none());
        assert_eq!(header.first_description.unwrap().source_text(src), "Description");
    }

    #[test]
    fn test_parse_entry_header_extra_spaces_after_colon() {
        let src = "name:   Description";
        let header = header_from(src);
        assert_eq!(header.name.source_text(src), "name");
        assert!(header.type_info.is_none());
        assert_eq!(header.first_description.unwrap().source_text(src), "Description");
    }

    #[test]
    fn test_parse_entry_header_no_space_before_bracket_strict() {
        let src = "name(int): Description";
        let header = header_from(src);
        // Strict mode: brackets without space are NOT treated as type
        assert_eq!(header.name.source_text(src), "name(int)");
        assert!(header.type_info.is_none());
        assert_eq!(header.first_description.unwrap().source_text(src), "Description");
    }

    #[test]
    fn test_parse_entry_header_no_space_before_bracket_lenient() {
        let src = "name(int): Description";
        let header = header_from_lenient(src);
        // Lenient mode: brackets without space ARE treated as type
        assert_eq!(header.name.source_text(src), "name");
        assert!(header.type_info.is_some());
        let ti = header.type_info.unwrap();
        assert_eq!(ti.r#type.unwrap().source_text(src), "int");
        assert_eq!(header.first_description.unwrap().source_text(src), "Description");
    }

    #[test]
    fn test_parse_entry_header_no_space_complex_type_lenient() {
        let src = "data(Dict[str, int]): Values";
        let header = header_from_lenient(src);
        assert_eq!(header.name.source_text(src), "data");
        let ti = header.type_info.unwrap();
        assert_eq!(ti.r#type.unwrap().source_text(src), "Dict[str, int]");
        assert_eq!(header.first_description.unwrap().source_text(src), "Values");
    }
}
