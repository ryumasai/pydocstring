//! The coverage invariant — Phase 1's acceptance law (#39).
//!
//! Concatenating ALL tokens of a parse (content + trivia) in source order
//! must reproduce the source byte-for-byte: no gaps, no overlaps, first
//! token starts at offset 0, last token ends at `source.len()`. Any gap
//! means a parser silently dropped bytes.
//!
//! Known violations are tracked in `KNOWN_COVERAGE_FAILURES` with the same
//! discipline as `tests/roundtrip.rs`: the test fails when a NEW violation
//! appears *or* when a listed one starts passing (stale entry). Shrink the
//! list by fixing parser drops, never by relaxing the law.

mod common;

use std::fs;

use common::collect_inputs;
use common::corpus_name;
use common::style_dirs;
use pydocstring::syntax::Parsed;
use pydocstring::syntax::SyntaxElement;
use pydocstring::syntax::SyntaxNode;
use pydocstring::syntax::SyntaxToken;

/// Corpus inputs currently known to violate the coverage law.
const KNOWN_COVERAGE_FAILURES: &[&str] = &[
    // Multi-name Attributes entries (`jac, hess : ndarray`): unlike
    // build_parameter_node, build_attribute_node keeps the FIRST name only,
    // so the later NAME/COMMA tokens of the name list never reach the CST
    // and their bytes are dropped (src/parse/numpy/parser.rs,
    // "Attributes use the first name only").
    "numpy/realworld/scipy_optimize_optimizeresult.txt",
];

fn parse_for_style(style: &str, input: &str) -> Parsed {
    match style {
        "google" => pydocstring::parse::google::parse_google(input),
        "numpy" => pydocstring::parse::numpy::parse_numpy(input),
        "plain" => pydocstring::parse::plain::parse_plain(input),
        other => panic!("unknown corpus style directory: {other}"),
    }
}

/// Depth-first collection of all tokens.
fn collect_tokens<'a>(node: &'a SyntaxNode, out: &mut Vec<&'a SyntaxToken>) {
    for child in node.children() {
        match child {
            SyntaxElement::Node(n) => collect_tokens(n, out),
            SyntaxElement::Token(t) => out.push(t),
        }
    }
}

/// Checks the coverage law for one parsed input; returns violation details.
fn check_coverage(parsed: &Parsed) -> Vec<String> {
    let source = parsed.source();
    let mut violations = Vec::new();

    let mut tokens = Vec::new();
    collect_tokens(parsed.root(), &mut tokens);
    // Defensive: the parsers produce children in source order, but the law
    // is about coverage, not tree order — sort before checking adjacency.
    tokens.sort_by_key(|t| (t.range().start(), t.range().end()));

    if source.is_empty() {
        return violations;
    }
    let Some(first) = tokens.first() else {
        violations.push(format!("no tokens at all for {} source bytes", source.len()));
        return violations;
    };
    let last = tokens.last().unwrap();

    if usize::from(first.range().start()) != 0 {
        let end = usize::from(first.range().start());
        violations.push(format!(
            "gap at 0..{end}: leading bytes {:?} not covered",
            &source[..end]
        ));
    }
    for pair in tokens.windows(2) {
        let (prev, next) = (pair[0], pair[1]);
        let (gap_start, gap_end) = (usize::from(prev.range().end()), usize::from(next.range().start()));
        if gap_start > gap_end {
            violations.push(format!(
                "overlap: {} {} and {} {}",
                prev.kind(),
                prev.range(),
                next.kind(),
                next.range()
            ));
        } else if gap_start < gap_end {
            violations.push(format!(
                "gap at {gap_start}..{gap_end}: dropped bytes {:?} (between {} {} and {} {})",
                &source[gap_start..gap_end],
                prev.kind(),
                prev.range(),
                next.kind(),
                next.range()
            ));
        }
    }
    if usize::from(last.range().end()) != source.len() {
        let start = usize::from(last.range().end());
        violations.push(format!(
            "gap at {start}..{}: trailing bytes {:?} not covered",
            source.len(),
            &source[start..]
        ));
    }

    violations
}

/// LAW: every source byte is covered by exactly one token — concatenating
/// all tokens in source order reproduces the input byte-for-byte.
#[test]
fn every_source_byte_has_a_token() {
    let mut failures = Vec::new();
    let mut passed_known: Vec<&str> = KNOWN_COVERAGE_FAILURES.to_vec();
    let mut checked = 0;

    for style_dir in style_dirs() {
        let style = style_dir.file_name().unwrap().to_str().unwrap().to_owned();
        for txt_path in collect_inputs(&style_dir) {
            checked += 1;
            let input = fs::read_to_string(&txt_path).unwrap();
            let parsed = parse_for_style(&style, &input);
            let violations = check_coverage(&parsed);
            if violations.is_empty() {
                continue;
            }
            let name = corpus_name(&txt_path);
            if let Some(pos) = passed_known.iter().position(|k| *k == name) {
                passed_known.remove(pos);
            } else {
                failures.push(format!("{name}:\n  {}", violations.join("\n  ")));
            }
        }
    }

    assert!(checked > 0, "no corpus input files found under tests/corpus");
    assert!(
        failures.is_empty(),
        "{} new coverage violation(s):\n\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        passed_known.is_empty(),
        "these KNOWN coverage failures now pass — remove the stale entries:\n  {}",
        passed_known.join("\n  ")
    );
}

// =============================================================================
// Spec tests: separator bytes are content tokens (COMMA, brackets)
// =============================================================================

use pydocstring::syntax::SyntaxKind;

/// `(kind, text)` for every token child of `node`, depth-first.
fn token_texts(node: &SyntaxNode, source: &str) -> Vec<(SyntaxKind, String)> {
    let mut tokens = Vec::new();
    collect_tokens(node, &mut tokens);
    tokens.iter().map(|t| (t.kind(), t.text(source).to_owned())).collect()
}

/// SPEC: the comma between multiple names is a `COMMA` token between the
/// `NAME` tokens, in both styles.
#[test]
fn name_list_commas_are_comma_tokens() {
    let google = pydocstring::parse::google::parse_google("Summary.\n\nArgs:\n    x1, x2 (int): The values.\n");
    let arg = google.root().find_node(SyntaxKind::SECTION).unwrap();
    let arg = arg.find_node(SyntaxKind::ENTRY).unwrap();
    let tokens = token_texts(arg, google.source());
    let name_comma: Vec<_> = tokens
        .iter()
        .filter(|(k, _)| matches!(k, SyntaxKind::NAME | SyntaxKind::COMMA))
        .cloned()
        .collect();
    assert_eq!(
        name_comma,
        vec![
            (SyntaxKind::NAME, "x1".to_owned()),
            (SyntaxKind::COMMA, ",".to_owned()),
            (SyntaxKind::NAME, "x2".to_owned()),
        ]
    );

    let numpy =
        pydocstring::parse::numpy::parse_numpy("Summary.\n\nParameters\n----------\nx1, x2 : int\n    The values.\n");
    let param = numpy.root().find_node(SyntaxKind::SECTION).unwrap();
    let param = param.find_node(SyntaxKind::ENTRY).unwrap();
    let tokens = token_texts(param, numpy.source());
    let name_comma: Vec<_> = tokens
        .iter()
        .filter(|(k, _)| matches!(k, SyntaxKind::NAME | SyntaxKind::COMMA))
        .cloned()
        .collect();
    assert_eq!(
        name_comma,
        vec![
            (SyntaxKind::NAME, "x1".to_owned()),
            (SyntaxKind::COMMA, ",".to_owned()),
            (SyntaxKind::NAME, "x2".to_owned()),
        ]
    );
}

/// SPEC: the separator comma before an `optional` marker is a `COMMA` token,
/// so `(int, optional)` is covered byte-for-byte (commas *inside* a type like
/// `Dict[str, int]` stay part of the `TYPE` token).
#[test]
fn optional_marker_comma_is_a_comma_token() {
    let input = "Summary.\n\nArgs:\n    x (int, optional): The value.\n";
    let parsed = pydocstring::parse::google::parse_google(input);
    let arg = parsed.root().find_node(SyntaxKind::SECTION).unwrap();
    let arg = arg.find_node(SyntaxKind::ENTRY).unwrap();
    let tokens = token_texts(arg, parsed.source());
    let type_area: Vec<_> = tokens
        .iter()
        .filter(|(k, _)| matches!(k, SyntaxKind::TYPE | SyntaxKind::COMMA | SyntaxKind::OPTIONAL))
        .cloned()
        .collect();
    assert_eq!(
        type_area,
        vec![
            (SyntaxKind::TYPE, "int".to_owned()),
            (SyntaxKind::COMMA, ",".to_owned()),
            (SyntaxKind::OPTIONAL, "optional".to_owned()),
        ]
    );

    // A bracket-internal comma is not a separator: no COMMA token.
    let input = "Summary.\n\nArgs:\n    x (Dict[str, int]): The value.\n";
    let parsed = pydocstring::parse::google::parse_google(input);
    let arg = parsed.root().find_node(SyntaxKind::SECTION).unwrap();
    let arg = arg.find_node(SyntaxKind::ENTRY).unwrap();
    let tokens = token_texts(arg, parsed.source());
    assert!(
        tokens
            .iter()
            .any(|(k, t)| *k == SyntaxKind::TYPE && t == "Dict[str, int]")
    );
    assert!(tokens.iter().all(|(k, _)| *k != SyntaxKind::COMMA));
}

// =============================================================================
// Spec tests: markers are repeatable nodes (#41/#76)
// =============================================================================

/// SPEC: every `default …` occurrence becomes its own `DEFAULT` node (one
/// per occurrence, in source order) wrapping `DEFAULT_KEYWORD` /
/// `DEFAULT_SEPARATOR`? / `DEFAULT_VALUE`, so a repeated marker keeps every
/// byte in the tree — the #76 fix. The model takes the first occurrence
/// (pinned in tests/model.rs).
#[test]
fn repeated_default_markers_one_node_per_occurrence() {
    let input = "Summary.\n\nParameters\n----------\nx : int, default 1, default 2\n    Desc.\n";
    let parsed = pydocstring::parse::numpy::parse_numpy(input);
    assert!(check_coverage(&parsed).is_empty(), "coverage violated: {input:?}");

    let entry = parsed
        .root()
        .find_node(SyntaxKind::SECTION)
        .unwrap()
        .find_node(SyntaxKind::ENTRY)
        .unwrap();
    let defaults: Vec<_> = entry.nodes(SyntaxKind::DEFAULT).collect();
    assert_eq!(defaults.len(), 2);
    let values: Vec<_> = defaults
        .iter()
        .map(|d| {
            d.find_token(SyntaxKind::DEFAULT_VALUE)
                .unwrap()
                .text(parsed.source())
                .to_owned()
        })
        .collect();
    assert_eq!(values, vec!["1", "2"]);

    let input = "Summary.\n\nArgs:\n    x (int, default 1, default 2): Desc.\n";
    let parsed = pydocstring::parse::google::parse_google(input);
    assert!(check_coverage(&parsed).is_empty(), "coverage violated: {input:?}");
    let entry = parsed
        .root()
        .find_node(SyntaxKind::SECTION)
        .unwrap()
        .find_node(SyntaxKind::ENTRY)
        .unwrap();
    let values: Vec<_> = entry
        .nodes(SyntaxKind::DEFAULT)
        .map(|d| {
            d.find_token(SyntaxKind::DEFAULT_VALUE)
                .unwrap()
                .text(parsed.source())
                .to_owned()
        })
        .collect();
    assert_eq!(values, vec!["1", "2"]);
}

/// SPEC: every `optional` occurrence becomes its own `OPTIONAL` token, in
/// both styles.
#[test]
fn repeated_optional_markers_one_token_per_occurrence() {
    let input = "Summary.\n\nParameters\n----------\nx : int, optional, optional\n    Desc.\n";
    let parsed = pydocstring::parse::numpy::parse_numpy(input);
    assert!(check_coverage(&parsed).is_empty(), "coverage violated: {input:?}");
    let entry = parsed
        .root()
        .find_node(SyntaxKind::SECTION)
        .unwrap()
        .find_node(SyntaxKind::ENTRY)
        .unwrap();
    assert_eq!(entry.tokens(SyntaxKind::OPTIONAL).count(), 2);

    let input = "Summary.\n\nArgs:\n    x (int, optional, optional): Desc.\n";
    let parsed = pydocstring::parse::google::parse_google(input);
    assert!(check_coverage(&parsed).is_empty(), "coverage violated: {input:?}");
    let entry = parsed
        .root()
        .find_node(SyntaxKind::SECTION)
        .unwrap()
        .find_node(SyntaxKind::ENTRY)
        .unwrap();
    assert_eq!(entry.tokens(SyntaxKind::OPTIONAL).count(), 2);
}
