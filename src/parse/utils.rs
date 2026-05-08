//! Shared utilities for docstring style parsers.

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
    /// Clean type text (optional stripped) inside brackets.
    pub clean_type: &'a str,
    /// Byte offset of the type text start.
    pub type_offset: usize,
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

    Some(BracketEntry {
        name,
        clean_type,
        type_offset,
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
    let description_indent = text.lines().skip(1).filter_map(|line| {
        let trimmed_len = line.trim_start().len();
        if trimmed_len == 0 {
            None
        } else {
            Some(line.len() - trimmed_len)
        }
    }).min().unwrap_or(0);
    let mut lines = text.lines();
    let first_line = lines.next().unwrap().trim_end(); // at least one line => we can safely unwrap
    lines.map(|line| {
        if description_indent >= line.len() { // empty line
            &line[0..0]
        } else {
            line[description_indent..].trim_end()
        }
    }).fold(first_line.to_owned(), |a, b| a + "\n" + b)
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
        assert_eq!(convert_multiline_with_indentation("First line.

        Description line.
        More description.

            Blockquote.
            Another.

        Some text.

        .. directive:: option
           directive_option"), "First line.\n\nDescription line.\nMore description.\n\n    Blockquote.\n    Another.\n\nSome text.\n\n.. directive:: option\n   directive_option");
    }
}
