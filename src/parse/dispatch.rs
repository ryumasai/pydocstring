//! The shared block dispatcher (#148, the structure #107's module headers
//! pointed at): one document loop parses every sectioned style.
//!
//! Both dialects read a docstring the same way — summary until a blank line
//! or header, an optional run of rST directives, extended summary until a
//! header, then sections whose bodies are processed line by line. The
//! dialects differ in exactly three places, expressed by [`Dialect`]:
//!
//! 1. **What a section header looks like** (`Name:` vs name + underline) —
//!    [`Dialect::try_header`], with the marker shape carried in
//!    [`HeaderMarker`].
//! 2. **Which grammar reads a section body's lines** —
//!    [`Dialect::body`] picks a [`SectionBody`], whose entry processors are
//!    plain functions supplied by the dialect's module.
//! 3. **Whether indentation can close a section** —
//!    [`Dialect::flush_by_indent`]: Google sections end when a line returns
//!    to the header's indent; NumPy sections end only at the next header
//!    (entries sit at the header's own indent, so indent says nothing).

use crate::cursor::LineCursor;
use crate::parse::Style;
use crate::parse::google::parser::ReturnsState;
use crate::parse::kind::SectionName;
use crate::parse::utils::build_paragraph;
use crate::parse::utils::build_text_block;
use crate::syntax::Parsed;
use crate::syntax::SyntaxElement;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::syntax::SyntaxToken;
use crate::text::TextRange;

// =============================================================================
// Section headers
// =============================================================================

/// A recognized section header, before its `SECTION_HEADER` node is built.
pub(crate) struct SectionHeaderInfo {
    /// The header's source range (both lines, for an underlined header).
    pub(crate) range: TextRange,
    pub(crate) kind: SectionName,
    /// The name token's range.
    pub(crate) name: TextRange,
    /// The style-specific header marker.
    pub(crate) marker: HeaderMarker,
    /// The header line's indent, in columns (drives Google's indent flush).
    pub(crate) indent_columns: usize,
}

/// The marker that makes a line a section header.
pub(crate) enum HeaderMarker {
    /// Google: a trailing colon. `None` for a bare known name — the colon is
    /// grammatically required, so a zero-length placeholder is emitted at
    /// the position where it belongs.
    Colon(Option<TextRange>),
    /// NumPy: the dash underline on the following line.
    Underline(TextRange),
}

impl HeaderMarker {
    /// How many source lines the header occupies.
    fn lines(&self) -> usize {
        match self {
            Self::Colon(_) => 1,
            Self::Underline(_) => 2,
        }
    }
}

/// Build the `SECTION_HEADER` node: `NAME` plus the marker token.
fn build_section_header_node(info: &SectionHeaderInfo) -> SyntaxNode {
    let mut children = Vec::new();
    children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::NAME, info.name)));
    match info.marker {
        HeaderMarker::Colon(Some(colon)) => {
            children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::COLON, colon)));
        }
        HeaderMarker::Colon(None) => {
            children.push(SyntaxElement::Token(SyntaxToken::new(
                SyntaxKind::COLON,
                TextRange::new(info.name.end(), info.name.end()),
            )));
        }
        HeaderMarker::Underline(underline) => {
            children.push(SyntaxElement::Token(SyntaxToken::new(SyntaxKind::UNDERLINE, underline)));
        }
    }
    SyntaxNode::new(SyntaxKind::SECTION_HEADER, info.range, children)
}

// =============================================================================
// Section bodies
// =============================================================================

/// A per-line entry processor: reads the cursor's line and appends to (or
/// extends the last of) the accumulated body children.
pub(crate) type LineProcessor = fn(&LineCursor, &mut Vec<SyntaxElement>, &mut Option<usize>);

/// Accumulates a section body while its lines are consumed.
pub(crate) enum SectionBody {
    /// One `ENTRY`/`CITATION` per line (plus continuations), read by the
    /// dialect-supplied processor.
    Entries(LineProcessor, Vec<SyntaxElement>),
    /// Google Returns/Yields: the whole body collapses into a single entry.
    Collapsed(ReturnsState),
    /// Free-text body (Notes, Examples, unknown, …): one `DESCRIPTION`.
    FreeText(Option<TextRange>),
}

impl SectionBody {
    fn process_line(&mut self, cursor: &LineCursor, entry_indent: &mut Option<usize>) {
        match self {
            Self::Entries(process, nodes) => process(cursor, nodes, entry_indent),
            Self::Collapsed(state) => state.process_line(cursor),
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
            Self::Entries(_, nodes) => nodes,
            Self::Collapsed(state) => match state.into_node(source) {
                Some(node) => vec![SyntaxElement::Node(node)],
                None => vec![],
            },
            Self::FreeText(range) => match range {
                Some(r) => vec![SyntaxElement::Node(build_text_block(
                    SyntaxKind::DESCRIPTION,
                    r,
                    source,
                ))],
                None => vec![],
            },
        }
    }
}

// =============================================================================
// Dialect
// =============================================================================

/// The three style-specific decisions the document loop defers.
pub(crate) trait Dialect {
    /// The style tag recorded on the [`Parsed`] result.
    fn style(&self) -> Style;

    /// Try to recognize a section header at the cursor's line. `current` is
    /// the enclosing section's header, if any — Google refuses candidates
    /// indented deeper than it (they are body entries, e.g. `b :` inside an
    /// Args block); NumPy ignores it.
    fn try_header(&self, cursor: &LineCursor, current: Option<&SectionHeaderInfo>) -> Option<SectionHeaderInfo>;

    /// The body accumulator for a section kind.
    fn body(&self, kind: SectionName) -> SectionBody;

    /// Whether a line at (or above) the header's indent closes the section.
    fn flush_by_indent(&self) -> bool;
}

// =============================================================================
// The document loop
// =============================================================================

/// Parse a sectioned docstring: summary, directives, extended summary, then
/// sections — with every style-specific decision deferred to `dialect`.
pub(crate) fn parse_document(input: &str, dialect: &dyn Dialect) -> Parsed {
    let mut cursor = LineCursor::new(input);
    let mut root_children: Vec<SyntaxElement> = Vec::new();

    cursor.skip_blanks();

    // --- Summary: lines until a blank line or a section header ---
    if !cursor.is_eof() && dialect.try_header(&cursor, None).is_none() {
        let start_line = cursor.line;
        let start_col = cursor.current_indent();
        let mut last_line = start_line;

        while !cursor.is_eof() {
            if dialect.try_header(&cursor, None).is_some() || cursor.current_trimmed().is_empty() {
                break;
            }
            last_line = cursor.line;
            cursor.advance();
        }

        let last_text = cursor.line_text(last_line);
        let last_col = crate::cursor::indent_len(last_text) + last_text.trim().len();
        let range = cursor.make_range(start_line, start_col, last_line, last_col);
        if !range.is_empty() {
            root_children.push(SyntaxElement::Node(build_text_block(SyntaxKind::SUMMARY, range, input)));
        }
    }

    cursor.skip_blanks();

    // --- rST directives at the post-summary slot ---
    // Any directive name is accepted (`deprecated`, `versionadded`, `note`,
    // …), and a run of consecutive directives is recognized (numpydoc stacks
    // e.g. `.. deprecated::` and `.. versionadded::`). Block-level
    // directives inside section bodies stay prose (deferred). A `.. name::`
    // line never matches header detection (a header name must start with an
    // ASCII letter), so the order is safe.
    while !cursor.is_eof()
        && dialect.try_header(&cursor, None).is_none()
        && let Some(node) = crate::parse::utils::try_parse_directive(&mut cursor)
    {
        root_children.push(SyntaxElement::Node(node));
        cursor.skip_blanks();
    }

    // --- Extended summary: lines (blank lines included) until a header ---
    if !cursor.is_eof() && dialect.try_header(&cursor, None).is_none() {
        let start_line = cursor.line;
        let mut last_non_empty_line = cursor.line;
        let mut has_content = false;

        while !cursor.is_eof() {
            if dialect.try_header(&cursor, None).is_some() {
                break;
            }
            if !cursor.current_trimmed().is_empty() {
                last_non_empty_line = cursor.line;
                has_content = true;
            }
            cursor.advance();
        }

        if has_content {
            let first_line = cursor.line_text(start_line);
            let first_col = crate::cursor::indent_len(first_line);
            let last_line = cursor.line_text(last_non_empty_line);
            let last_col = crate::cursor::indent_len(last_line) + last_line.trim().len();
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
    // Google's indent tracking: whether the body sits deeper than the header
    // (see the flush rule below). Unused when the dialect never flushes by
    // indent.
    let mut body_is_deeper: Option<bool> = None;

    // Pending run of stray prose lines (first line, last line): flushed as
    // one PARAGRAPH node at a blank line, a section header, or EOF.
    let mut para_first: Option<usize> = None;
    let mut para_last: usize = 0;

    while !cursor.is_eof() {
        // --- Blank lines split stray-line paragraphs (reST semantics) ---
        if cursor.current_trimmed().is_empty() {
            if let Some(first) = para_first.take() {
                root_children.push(SyntaxElement::Node(build_paragraph(&cursor, first, para_last)));
            }
            cursor.advance();
            continue;
        }

        // --- Section header: flush the previous section and start anew ---
        if let Some(header_info) = dialect.try_header(&cursor, current_header.as_ref()) {
            if let Some(prev_header) = current_header.take() {
                let node = flush_section(&cursor, prev_header, current_body.take().unwrap());
                root_children.push(SyntaxElement::Node(node));
            }
            // Flush a pending stray-line paragraph (a header line right
            // after a stray run, with no blank line in between).
            if let Some(first) = para_first.take() {
                root_children.push(SyntaxElement::Node(build_paragraph(&cursor, first, para_last)));
            }

            current_body = Some(dialect.body(header_info.kind));
            cursor.line += header_info.marker.lines();
            current_header = Some(header_info);
            entry_indent = None;
            body_is_deeper = None;
            continue;
        }

        // --- Indent flush (Google only) ---
        //
        // body_is_deeper tracks whether the section body is indented deeper
        // than the section header:
        //   None        – no body line seen yet; flush only if STRICTLY
        //                 shallower than the header (lets zero-indent first
        //                 entries through)
        //   Some(true)  – body is deeper; flush when a line returns to the
        //                 header indent
        //   Some(false) – body is at same/shallower level (zero-indent
        //                 style); never flush by indent — rely on
        //                 section-header detection
        if dialect.flush_by_indent() {
            let l = cursor.current_indent_columns();
            let should_flush = current_header.as_ref().is_some_and(|h| match body_is_deeper {
                None => l < h.indent_columns,
                Some(true) => l <= h.indent_columns,
                Some(false) => false,
            });
            if should_flush {
                if let Some(prev_header) = current_header.take() {
                    let node = flush_section(&cursor, prev_header, current_body.take().unwrap());
                    root_children.push(SyntaxElement::Node(node));
                }
                body_is_deeper = None;
            }
        }

        // --- Body line or stray prose ---
        if let Some(body) = current_body.as_mut() {
            if dialect.flush_by_indent() && body_is_deeper.is_none() {
                let entry_l = cursor.current_indent_columns();
                body_is_deeper = Some(current_header.as_ref().is_some_and(|h| entry_l > h.indent_columns));
            }
            body.process_line(&cursor, &mut entry_indent);
        } else {
            // Stray prose line: accumulate into the pending paragraph run
            // (consecutive lines separated only by a newline form one
            // PARAGRAPH).
            if para_first.is_none() {
                para_first = Some(cursor.line);
            }
            para_last = cursor.line;
        }

        cursor.advance();
    }

    // --- EOF flushes ---
    if let Some(header) = current_header.take() {
        let node = flush_section(&cursor, header, current_body.take().unwrap());
        root_children.push(SyntaxElement::Node(node));
    }
    if let Some(first) = para_first.take() {
        root_children.push(SyntaxElement::Node(build_paragraph(&cursor, first, para_last)));
    }

    let mut root = SyntaxNode::new(SyntaxKind::DOCUMENT, cursor.full_range(), root_children);
    crate::parse::trivia::attach_trivia(&mut root, input);
    Parsed::new(input.to_string(), root, dialect.style())
}

/// Build the `SECTION` node for a finished section: header node plus the
/// accumulated body children, spanning from the header to the last content
/// line before the cursor.
fn flush_section(cursor: &LineCursor, header: SectionHeaderInfo, body: SectionBody) -> SyntaxNode {
    let header_start = header.range.start().raw() as usize;
    let section_range = cursor.span_back_from_cursor(header_start);

    let mut section_children = vec![SyntaxElement::Node(build_section_header_node(&header))];
    section_children.extend(body.into_children(cursor.source()));

    SyntaxNode::new(SyntaxKind::SECTION, section_range, section_children)
}
