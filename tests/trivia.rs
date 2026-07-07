//! Trivia and text block spec tests and corpus-wide invariants (#37, #38).
//!
//! After parsing, the CST carries flat trivia tokens (`WHITESPACE`,
//! `NEWLINE`, `BLANK_LINE`) for the gap bytes between content tokens, and
//! multi-line content lives in text block nodes (`SUMMARY`,
//! `EXTENDED_SUMMARY`, `DESCRIPTION`, `BODY_TEXT`, `CONTENT`) wrapping one
//! `TEXT_LINE` token per content line. These tests pin the lexing rules on
//! hand-written inputs and enforce, for every corpus input, the structural
//! invariants the trivia pass guarantees:
//!
//! 1. No token contains `\n`, except `NEWLINE` / `BLANK_LINE`. This
//!    invariant is absolute.
//! 2. Tokens never overlap and appear in source order; trivia tokens fall
//!    inside their parent node's range.
//! 3. Every whitespace byte of the source is covered by some token
//!    (non-whitespace bytes may still be dropped by the parsers — full
//!    byte-for-byte coverage is #39's test).

mod common;

use std::fs;

use common::collect_inputs;
use common::corpus_name;
use common::style_dirs;
use pydocstring::parse::TextBlock;
use pydocstring::syntax::Parsed;
use pydocstring::syntax::SyntaxElement;
use pydocstring::syntax::SyntaxKind;
use pydocstring::syntax::SyntaxNode;
use pydocstring::syntax::SyntaxToken;

fn parse_for_style(style: &str, input: &str) -> Parsed {
    match style {
        "google" => pydocstring::parse::google::parse_google(input),
        "numpy" => pydocstring::parse::numpy::parse_numpy(input),
        "plain" => pydocstring::parse::plain::parse_plain(input),
        other => panic!("unknown corpus style directory: {other}"),
    }
}

/// Depth-first collection of all tokens, with their parent node.
fn collect_tokens<'a>(node: &'a SyntaxNode, out: &mut Vec<(&'a SyntaxNode, &'a SyntaxToken)>) {
    for child in node.children() {
        match child {
            SyntaxElement::Node(n) => collect_tokens(n, out),
            SyntaxElement::Token(t) => out.push((node, t)),
        }
    }
}

/// Run all corpus invariants for one parsed input, returning violations.
fn check_invariants(name: &str, parsed: &Parsed) -> Vec<String> {
    let source = parsed.source();
    let mut violations = Vec::new();
    let mut tokens = Vec::new();
    collect_tokens(parsed.root(), &mut tokens);

    // Invariant 1 (absolute): only NEWLINE / BLANK_LINE may contain a
    // newline.
    for (_, token) in &tokens {
        let kind = token.kind();
        let exempt = matches!(kind, SyntaxKind::NEWLINE | SyntaxKind::BLANK_LINE);
        if !exempt && token.text(source).contains('\n') {
            violations.push(format!("{name}: {kind} token {} contains a newline", token.range()));
        }
    }

    // Invariant 2: tokens never overlap (checked in source order — some
    // parsers store entry children in canonical rather than source order);
    // trivia tokens fall inside their parent node's range.
    let mut sorted: Vec<&SyntaxToken> = tokens.iter().map(|(_, t)| *t).collect();
    sorted.sort_by_key(|t| (t.range().start(), t.range().end()));
    for pair in sorted.windows(2) {
        let (prev, next) = (pair[0], pair[1]);
        if prev.range().end() > next.range().start() {
            violations.push(format!(
                "{name}: token {} {} overlaps or precedes token {} {}",
                prev.kind(),
                prev.range(),
                next.kind(),
                next.range()
            ));
        }
    }
    for (parent, token) in &tokens {
        if token.kind().is_trivia()
            && (token.range().start() < parent.range().start() || token.range().end() > parent.range().end())
        {
            violations.push(format!(
                "{name}: trivia {} {} outside parent {} {}",
                token.kind(),
                token.range(),
                parent.kind(),
                parent.range()
            ));
        }
    }

    // Invariant 3: every whitespace byte is covered by some token. (Tokens
    // are non-overlapping and sorted, so their concatenation is a
    // subsequence of the source by construction.)
    let mut covered = vec![false; source.len()];
    for (_, token) in &tokens {
        for slot in &mut covered[usize::from(token.range().start())..usize::from(token.range().end())] {
            *slot = true;
        }
    }
    for (i, byte) in source.bytes().enumerate() {
        if matches!(byte, b' ' | b'\t' | b'\r' | b'\n') && !covered[i] {
            violations.push(format!(
                "{name}: whitespace byte at offset {i} ({byte:?}) not covered by any token"
            ));
        }
    }

    violations
}

#[test]
fn corpus_trivia_invariants() {
    let mut violations = Vec::new();
    let mut checked = 0;

    for style_dir in style_dirs() {
        let style = style_dir.file_name().unwrap().to_str().unwrap().to_owned();
        for txt_path in collect_inputs(&style_dir) {
            checked += 1;
            let input = fs::read_to_string(&txt_path).unwrap();
            let parsed = parse_for_style(&style, &input);
            violations.extend(check_invariants(&corpus_name(&txt_path), &parsed));
        }
    }

    assert!(checked > 0, "no corpus input files found under tests/corpus");
    assert!(
        violations.is_empty(),
        "{} trivia invariant violation(s):\n{}",
        violations.len(),
        violations.join("\n")
    );
}

// =============================================================================
// Spec tests: pin the trivia lexing rules
// =============================================================================

/// `(kind, text)` for every direct token child of `node`.
fn token_children(node: &SyntaxNode, source: &str) -> Vec<(SyntaxKind, String)> {
    node.children()
        .iter()
        .filter_map(|c| match c {
            SyntaxElement::Token(t) => Some((t.kind(), t.text(source).to_owned())),
            SyntaxElement::Node(_) => None,
        })
        .collect()
}

#[test]
fn blank_line_between_sections_is_docstring_level() {
    let input = "Summary.\n\nArgs:\n    x: A.\n\nReturns:\n    int: B.\n";
    let parsed = pydocstring::parse::google::parse_google(input);
    let root_tokens = token_children(parsed.root(), parsed.source());

    // Both blank lines (after the summary, and between the two sections)
    // are BLANK_LINE tokens directly under the docstring root.
    let blanks: Vec<_> = root_tokens
        .iter()
        .filter(|(k, _)| *k == SyntaxKind::BLANK_LINE)
        .collect();
    assert_eq!(blanks.len(), 2, "root tokens: {root_tokens:?}");
    assert!(blanks.iter().all(|(_, text)| text == "\n"));

    // No BLANK_LINE hides inside a section.
    for section in parsed.root().nodes(SyntaxKind::GOOGLE_SECTION) {
        let mut tokens = Vec::new();
        collect_tokens(section, &mut tokens);
        assert!(
            tokens.iter().all(|(_, t)| t.kind() != SyntaxKind::BLANK_LINE),
            "BLANK_LINE inside section"
        );
    }
}

#[test]
fn entry_indentation_is_whitespace_inside_section() {
    let input = "Summary.\n\nArgs:\n    x: A.\n";
    let parsed = pydocstring::parse::google::parse_google(input);
    let section = parsed.root().find_node(SyntaxKind::GOOGLE_SECTION).unwrap();
    let tokens = token_children(section, parsed.source());
    assert!(
        tokens.contains(&(SyntaxKind::NEWLINE, "\n".to_owned())),
        "section tokens: {tokens:?}"
    );
    assert!(
        tokens.contains(&(SyntaxKind::WHITESPACE, "    ".to_owned())),
        "section tokens: {tokens:?}"
    );
}

#[test]
fn tab_indentation_is_whitespace() {
    let input = "Summary.\n\nArgs:\n\tx: A.\n";
    let parsed = pydocstring::parse::google::parse_google(input);
    let section = parsed.root().find_node(SyntaxKind::GOOGLE_SECTION).unwrap();
    let tokens = token_children(section, parsed.source());
    assert!(
        tokens.contains(&(SyntaxKind::WHITESPACE, "\t".to_owned())),
        "section tokens: {tokens:?}"
    );
}

#[test]
fn no_trailing_newline_token_without_trailing_newline() {
    let input = "Summary.\n\nArgs:\n    x: A.";
    let parsed = pydocstring::parse::google::parse_google(input);
    let mut tokens = Vec::new();
    collect_tokens(parsed.root(), &mut tokens);
    let last = tokens.last().unwrap().1;
    assert!(!last.kind().is_trivia(), "last token: {} {}", last.kind(), last.range());
}

#[test]
fn trailing_newline_becomes_newline_token() {
    let input = "Summary.\n\nArgs:\n    x: A.\n";
    let parsed = pydocstring::parse::google::parse_google(input);
    let mut tokens = Vec::new();
    collect_tokens(parsed.root(), &mut tokens);
    let (_, last) = tokens.last().unwrap();
    assert_eq!(last.kind(), SyntaxKind::NEWLINE);
    assert_eq!(usize::from(last.range().end()), input.len());
}

#[test]
fn consecutive_blank_lines_yield_one_token_each() {
    let input = "Summary.\n\n\n   \nExtended.\n";
    let parsed = pydocstring::parse::plain::parse_plain(input);
    let elements: Vec<_> = parsed
        .root()
        .children()
        .iter()
        .map(|c| (c.kind(), c.range().source_text(parsed.source()).to_owned()))
        .collect();
    assert_eq!(
        elements,
        vec![
            (SyntaxKind::SUMMARY, "Summary.".to_owned()),
            (SyntaxKind::NEWLINE, "\n".to_owned()),
            (SyntaxKind::BLANK_LINE, "\n".to_owned()),
            (SyntaxKind::BLANK_LINE, "\n".to_owned()),
            (SyntaxKind::BLANK_LINE, "   \n".to_owned()),
            (SyntaxKind::EXTENDED_SUMMARY, "Extended.".to_owned()),
            (SyntaxKind::NEWLINE, "\n".to_owned()),
        ]
    );
}

#[test]
fn leading_blank_lines_live_at_root_level() {
    let input = "\n\nSummary.";
    let parsed = pydocstring::parse::plain::parse_plain(input);
    let elements: Vec<_> = parsed
        .root()
        .children()
        .iter()
        .map(|c| (c.kind(), c.range().source_text(parsed.source()).to_owned()))
        .collect();
    assert_eq!(
        elements,
        vec![
            (SyntaxKind::BLANK_LINE, "\n".to_owned()),
            (SyntaxKind::BLANK_LINE, "\n".to_owned()),
            (SyntaxKind::SUMMARY, "Summary.".to_owned()),
        ]
    );
}

#[test]
fn empty_input_has_no_tokens() {
    let parsed = pydocstring::parse::plain::parse_plain("");
    assert!(parsed.root().children().is_empty());
}

#[test]
fn numpy_underline_gaps_are_newlines() {
    let input = "Parameters\n----------\nx : int\n    Desc.\n";
    let parsed = pydocstring::parse::numpy::parse_numpy(input);
    let mut tokens = Vec::new();
    collect_tokens(parsed.root(), &mut tokens);
    let texts: String = tokens.iter().map(|(_, t)| t.text(parsed.source())).collect();
    // Concatenating all tokens reproduces the whole input: nothing dropped.
    assert_eq!(texts, input);
}

// =============================================================================
// Spec tests: text block nodes (#38)
// =============================================================================

#[test]
fn multi_line_description_yields_one_text_line_token_per_line() {
    let input = "Summary.\n\nArgs:\n    x: First line of desc\n        cont.\n";
    let parsed = pydocstring::parse::google::parse_google(input);
    let source = parsed.source();
    let section = parsed.root().find_node(SyntaxKind::GOOGLE_SECTION).unwrap();
    let arg = section.find_node(SyntaxKind::GOOGLE_ARG).unwrap();
    let desc = TextBlock::cast(arg.find_node(SyntaxKind::DESCRIPTION).unwrap()).unwrap();

    let lines: Vec<_> = desc.lines().map(|t| t.text(source)).collect();
    assert_eq!(lines, vec!["First line of desc", "cont."]);

    // Raw text() is the byte-identical source slice of the block's range,
    // including the interior newline and indentation.
    assert_eq!(desc.text(source), "First line of desc\n        cont.");

    // Interior newline + indentation are trivia tokens inside the node.
    let tokens = token_children(desc.syntax(), source);
    assert_eq!(
        tokens,
        vec![
            (SyntaxKind::TEXT_LINE, "First line of desc".to_owned()),
            (SyntaxKind::NEWLINE, "\n".to_owned()),
            (SyntaxKind::WHITESPACE, "        ".to_owned()),
            (SyntaxKind::TEXT_LINE, "cont.".to_owned()),
        ]
    );
}

#[test]
fn single_line_summary_is_still_a_block_with_one_text_line() {
    let parsed = pydocstring::parse::plain::parse_plain("Summary.");
    let block = TextBlock::cast(parsed.root().find_node(SyntaxKind::SUMMARY).unwrap()).unwrap();
    let lines: Vec<_> = block.lines().map(|t| t.text(parsed.source())).collect();
    assert_eq!(lines, vec!["Summary."]);
    assert_eq!(block.text(parsed.source()), "Summary.");
}

#[test]
fn logical_text_dedents_indented_continuation() {
    let input = "Summary.\n\nArgs:\n    x: First line of desc\n        cont.\n";
    let parsed = pydocstring::parse::google::parse_google(input);
    let section = parsed.root().find_node(SyntaxKind::GOOGLE_SECTION).unwrap();
    let arg = section.find_node(SyntaxKind::GOOGLE_ARG).unwrap();
    let desc = TextBlock::cast(arg.find_node(SyntaxKind::DESCRIPTION).unwrap()).unwrap();
    // Continuation lines are dedented by their common indentation and
    // joined with `\n` (convert_multiline_with_indentation semantics).
    assert_eq!(desc.logical_text(parsed.source()), "First line of desc\ncont.");
}

#[test]
fn multi_paragraph_body_contains_blank_line_inside_node() {
    let input = "Summary.\n\nNotes:\n    Paragraph one.\n\n    Paragraph two.\n";
    let parsed = pydocstring::parse::google::parse_google(input);
    let source = parsed.source();
    let section = parsed.root().find_node(SyntaxKind::GOOGLE_SECTION).unwrap();
    let body = TextBlock::cast(section.find_node(SyntaxKind::BODY_TEXT).unwrap()).unwrap();

    let lines: Vec<_> = body.lines().map(|t| t.text(source)).collect();
    assert_eq!(lines, vec!["Paragraph one.", "Paragraph two."]);

    // The paragraph break is a BLANK_LINE token *inside* the block node.
    let tokens = token_children(body.syntax(), source);
    assert_eq!(
        tokens,
        vec![
            (SyntaxKind::TEXT_LINE, "Paragraph one.".to_owned()),
            (SyntaxKind::NEWLINE, "\n".to_owned()),
            (SyntaxKind::BLANK_LINE, "\n".to_owned()),
            (SyntaxKind::WHITESPACE, "    ".to_owned()),
            (SyntaxKind::TEXT_LINE, "Paragraph two.".to_owned()),
        ]
    );
}

/// SPEC: CONTENT (reference entries) is a TextBlock like the other four
/// kinds — per-line tokens, raw text, dedented logical text.
#[test]
fn content_block_lines_and_logical_text() {
    let src = "Summary.\n\nReferences\n----------\n.. [1] Author A, \"Title\",\n    with a continuation line.\n";
    let parsed = pydocstring::parse::numpy::parse_numpy(src);
    let section = parsed.root().find_node(SyntaxKind::NUMPY_SECTION).unwrap();
    let reference = section.find_node(SyntaxKind::NUMPY_REFERENCE).unwrap();
    let block = TextBlock::cast(reference.find_node(SyntaxKind::CONTENT).unwrap()).unwrap();
    let lines: Vec<_> = block.lines().map(|t| t.text(parsed.source())).collect();
    assert_eq!(lines, ["Author A, \"Title\",", "with a continuation line."]);
    assert!(block.text(parsed.source()).contains('\n'));
    assert_eq!(
        block.logical_text(parsed.source()),
        "Author A, \"Title\",\nwith a continuation line."
    );
}
