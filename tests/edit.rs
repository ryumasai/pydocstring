//! Spec and property tests for the anchored splice edit list (#44).
//!
//! The corpus-wide laws:
//!
//! 1. **Zero-edit identity** — for every corpus input, applying an empty
//!    edit list reproduces the source byte-for-byte.
//! 2. **Self-replace identity** — replacing elements with their own source
//!    text is the identity: one pass replaces every top-level child of
//!    `DOCUMENT`, a second replaces one deepest node per input.
//!
//! Plus hand-written specs: overlap/out-of-bounds rejection, same-position
//! insert ordering, boundary-insert ordering, missing-placeholder insertion
//! anchors, `remove_lines` line-extent and blank-line consumption (both
//! styles), and `apply_reparsed` style preservation.

mod common;

use std::fs;

use common::corpus_cases;
use common::corpus_name;
use pydocstring::edit::EditError;
use pydocstring::parse::Document;
use pydocstring::parse::parse;
use pydocstring::syntax::Parsed;
use pydocstring::syntax::SyntaxElement;
use pydocstring::syntax::SyntaxKind;
use pydocstring::syntax::SyntaxNode;
use pydocstring::text::TextRange;
use pydocstring::text::TextSize;

fn parse_for_style(style: &str, input: &str) -> Parsed {
    match style {
        "google" => pydocstring::parse::parse_google(input),
        "numpy" => pydocstring::parse::parse_numpy(input),
        "plain" => pydocstring::parse::parse_plain(input),
        other => panic!("unknown corpus style directory: {other}"),
    }
}

fn range(start: u32, end: u32) -> TextRange {
    TextRange::new(TextSize::new(start), TextSize::new(end))
}

// =============================================================================
// Law 1: zero edits → byte-identical output
// =============================================================================

#[test]
fn law_zero_edit_identity() {
    let cases = corpus_cases();
    assert!(!cases.is_empty(), "corpus is empty");
    for (style, path) in cases {
        let source = fs::read_to_string(&path).unwrap();
        let parsed = parse_for_style(&style, &source);
        let out = parsed.edit().apply().unwrap();
        assert_eq!(out, source, "zero-edit identity violated for {}", corpus_name(&path));
    }
}

// =============================================================================
// Law 2: replacing elements with their own text → byte-identical output
// =============================================================================

#[test]
fn law_self_replace_top_level_children_identity() {
    for (style, path) in corpus_cases() {
        let source = fs::read_to_string(&path).unwrap();
        let parsed = parse_for_style(&style, &source);
        let mut edits = parsed.edit();
        for child in parsed.root().children() {
            match child {
                SyntaxElement::Node(n) => {
                    edits.replace_node(n, n.range().source_text(&source));
                }
                SyntaxElement::Token(t) => {
                    edits.replace_token(t, t.text(&source));
                }
            }
        }
        let out = edits.apply().unwrap();
        assert_eq!(out, source, "self-replace identity violated for {}", corpus_name(&path));
    }
}

/// The first node found at the maximum depth of the tree.
fn deepest_node(node: &SyntaxNode) -> (&SyntaxNode, usize) {
    let mut best = (node, 0);
    for child in node.children() {
        if let SyntaxElement::Node(n) = child {
            let (deep, depth) = deepest_node(n);
            if depth + 1 > best.1 {
                best = (deep, depth + 1);
            }
        }
    }
    best
}

#[test]
fn law_self_replace_deep_node_identity() {
    for (style, path) in corpus_cases() {
        let source = fs::read_to_string(&path).unwrap();
        let parsed = parse_for_style(&style, &source);
        let (node, _) = deepest_node(parsed.root());
        let out = parsed
            .edit()
            .replace_node(node, node.range().source_text(&source))
            .apply()
            .unwrap();
        assert_eq!(
            out,
            source,
            "deep self-replace identity violated for {}",
            corpus_name(&path)
        );
    }
}

// =============================================================================
// Validation: overlap and out-of-bounds rejection
// =============================================================================

#[test]
fn overlapping_edits_rejected() {
    let parsed = parse("Summary text.\n");
    let err = parsed
        .edit()
        .replace(range(0, 7), "A")
        .replace(range(5, 10), "B")
        .apply()
        .unwrap_err();
    assert_eq!(
        err,
        EditError::Overlap {
            a: range(0, 7),
            b: range(5, 10),
        }
    );

    // A zero-length insert strictly inside a replaced range is ambiguous
    // and also rejected.
    let err = parsed
        .edit()
        .delete(range(0, 7))
        .insert(TextSize::new(3), "X")
        .apply()
        .unwrap_err();
    assert_eq!(
        err,
        EditError::Overlap {
            a: range(0, 7),
            b: range(3, 3),
        }
    );

    // Identical non-empty ranges overlap too.
    assert!(matches!(
        parsed
            .edit()
            .replace(range(0, 7), "A")
            .replace(range(0, 7), "B")
            .apply(),
        Err(EditError::Overlap { .. })
    ));

    // Touching ranges do not overlap.
    let out = parsed
        .edit()
        .replace(range(0, 7), "Longer")
        .replace(range(7, 8), "-")
        .apply()
        .unwrap();
    assert_eq!(out, "Longer-text.\n");
}

#[test]
fn out_of_bounds_edits_rejected() {
    let parsed = parse("Summary.\n"); // 9 bytes

    // Past the end of the source.
    let err = parsed.edit().replace(range(5, 100), "x").apply().unwrap_err();
    assert_eq!(err, EditError::OutOfBounds { range: range(5, 100) });

    // Inverted range.
    let err = parsed.edit().delete(range(5, 3)).apply().unwrap_err();
    assert_eq!(err, EditError::OutOfBounds { range: range(5, 3) });

    // Insert past the end.
    let err = parsed.edit().insert(TextSize::new(10), "x").apply().unwrap_err();
    assert_eq!(err, EditError::OutOfBounds { range: range(10, 10) });

    // Offsets inside a multi-byte character ("é" is 2 bytes at offset 1).
    let parsed = parse("héllo.\n");
    let err = parsed.edit().delete(range(2, 4)).apply().unwrap_err();
    assert_eq!(err, EditError::OutOfBounds { range: range(2, 4) });
}

// =============================================================================
// Ordering: same-position inserts and boundary inserts
// =============================================================================

#[test]
fn same_position_inserts_apply_in_call_order() {
    let parsed = parse("Summary.\n");
    let out = parsed
        .edit()
        .insert(TextSize::new(7), "A")
        .insert(TextSize::new(7), "B")
        .insert(TextSize::new(7), "C")
        .apply()
        .unwrap();
    assert_eq!(out, "SummaryABC.\n");
}

#[test]
fn boundary_inserts_order_around_replacement() {
    // An insert at the START of a replaced range lands before the
    // replacement text; an insert at the END lands after it — regardless of
    // call order.
    let parsed = parse("Summary.\n");
    let out = parsed
        .edit()
        .replace(range(0, 7), "REPL")
        .insert(TextSize::new(0), "<")
        .insert(TextSize::new(7), ">")
        .apply()
        .unwrap();
    assert_eq!(out, "<REPL>.\n");

    // Same result with the inserts registered first.
    let out = parsed
        .edit()
        .insert(TextSize::new(7), ">")
        .insert(TextSize::new(0), "<")
        .replace(range(0, 7), "REPL")
        .apply()
        .unwrap();
    assert_eq!(out, "<REPL>.\n");
}

// =============================================================================
// Missing placeholders are insertion anchors
// =============================================================================

#[test]
fn missing_placeholder_replacement_inserts_at_anchor() {
    // `x ()` has a zero-length TYPE placeholder between the brackets.
    let src = "Args:\n    x (): The x.\n";
    let parsed = pydocstring::parse::parse_google(src);

    let entry = find_first(parsed.root(), SyntaxKind::ENTRY).unwrap();
    let placeholder = entry.find_missing(SyntaxKind::TYPE).unwrap();
    assert!(placeholder.is_missing());

    let reparsed = parsed
        .edit()
        .replace_token(placeholder, "int")
        .apply_reparsed()
        .unwrap();
    assert_eq!(reparsed.source(), "Args:\n    x (int): The x.\n");

    // The type token now reads "int" ...
    let entry = find_first(reparsed.root(), SyntaxKind::ENTRY).unwrap();
    let ty = entry.find_token(SyntaxKind::TYPE).unwrap();
    assert!(!ty.is_missing());
    assert_eq!(ty.text(reparsed.source()), "int");

    // ... and the reparse is exactly what parsing the target text yields.
    assert_eq!(
        reparsed,
        pydocstring::parse::parse_google("Args:\n    x (int): The x.\n")
    );
}

/// Depth-first search for the first node of `kind`.
fn find_first(node: &SyntaxNode, kind: SyntaxKind) -> Option<&SyntaxNode> {
    if node.kind() == kind {
        return Some(node);
    }
    node.children().iter().find_map(|c| match c {
        SyntaxElement::Node(n) => find_first(n, kind),
        SyntaxElement::Token(_) => None,
    })
}

// =============================================================================
// remove_lines
// =============================================================================

#[test]
fn remove_lines_entry_google() {
    let src = "Summary.\n\nArgs:\n    x: First.\n    y: Second.\n\nReturns:\n    int: Result.\n";
    let parsed = pydocstring::parse::parse_google(src);

    let entry = find_first(parsed.root(), SyntaxKind::ENTRY).unwrap();
    let mut edits = parsed.edit();
    edits.remove_lines(entry);
    let out = edits.apply().unwrap();
    // The whole line goes: leading indentation, content, trailing newline.
    assert_eq!(out, "Summary.\n\nArgs:\n    y: Second.\n\nReturns:\n    int: Result.\n");

    let reparsed = edits.apply_reparsed().unwrap();
    let doc = Document::new(&reparsed);
    let section = doc.sections().next().unwrap();
    assert_eq!(section.entries().count(), 1);
    assert_eq!(section.entries().next().unwrap().name().unwrap().text(), "y");
}

#[test]
fn remove_lines_entry_numpy() {
    let src = "Summary.\n\nParameters\n----------\nx : int\n    First.\ny : str\n    Second.\n\nReturns\n-------\nint\n    Result.\n";
    let parsed = pydocstring::parse::parse_numpy(src);

    // A multi-line entry: both its lines are removed, no debris remains.
    let entry = find_first(parsed.root(), SyntaxKind::ENTRY).unwrap();
    let mut edits = parsed.edit();
    edits.remove_lines(entry);
    let out = edits.apply().unwrap();
    assert_eq!(
        out,
        "Summary.\n\nParameters\n----------\ny : str\n    Second.\n\nReturns\n-------\nint\n    Result.\n"
    );

    let reparsed = edits.apply_reparsed().unwrap();
    let doc = Document::new(&reparsed);
    let section = doc.sections().next().unwrap();
    assert_eq!(section.entries().count(), 1);
    assert_eq!(section.entries().next().unwrap().name().unwrap().text(), "y");
}

#[test]
fn remove_lines_range_mid_line_node_is_a_plain_delete() {
    let src = "Summary.\n\nArgs:\n    x (int): Old description.\n    y: Stays.\n";
    let parsed = pydocstring::parse::parse_google(src);
    let doc = Document::new(&parsed);
    let entry = doc.sections().next().unwrap().entries().next().unwrap();
    let desc = entry.description().unwrap();

    let mut edits = parsed.edit();
    edits.remove_lines_range(desc.range());
    let out = edits.apply().unwrap();
    // The description does not own its line start, so it gets no line
    // semantics: the newline survives and `y` keeps its own line. Consuming
    // it would splice `y: Stays.` onto `x`'s header, and a reparse would
    // silently read it as `x`'s description (#144).
    assert_eq!(out, "Summary.\n\nArgs:\n    x (int): \n    y: Stays.\n");

    let reparsed = edits.apply_reparsed().unwrap();
    let doc = Document::new(&reparsed);
    let section = doc.sections().next().unwrap();
    assert_eq!(section.entries().count(), 2);
    let names: Vec<_> = section.entries().map(|e| e.name().unwrap().text().to_owned()).collect();
    assert_eq!(names, ["x", "y"]);
}

#[test]
fn remove_lines_section_consumes_trailing_blank_line_google() {
    let src = "Summary.\n\nArgs:\n    x: Desc.\n\nReturns:\n    int: Result.\n";
    let parsed = pydocstring::parse::parse_google(src);

    let section = find_first(parsed.root(), SyntaxKind::SECTION).unwrap();
    let out = parsed.edit().remove_lines(section).apply().unwrap();
    // The blank line separating Args from Returns is consumed with it.
    assert_eq!(out, "Summary.\n\nReturns:\n    int: Result.\n");

    let reparsed = parsed.edit().remove_lines(section).apply_reparsed().unwrap();
    let doc = Document::new(&reparsed);
    assert_eq!(doc.sections().count(), 1);
    assert_eq!(doc.sections().next().unwrap().header_name(), "Returns");
}

#[test]
fn remove_lines_section_consumes_trailing_blank_line_numpy() {
    let src = "Summary.\n\nParameters\n----------\nx : int\n    Desc.\n\nReturns\n-------\nint\n    Result.\n";
    let parsed = pydocstring::parse::parse_numpy(src);

    let section = find_first(parsed.root(), SyntaxKind::SECTION).unwrap();
    let out = parsed.edit().remove_lines(section).apply().unwrap();
    assert_eq!(out, "Summary.\n\nReturns\n-------\nint\n    Result.\n");

    let reparsed = parsed.edit().remove_lines(section).apply_reparsed().unwrap();
    let doc = Document::new(&reparsed);
    assert_eq!(doc.sections().count(), 1);
    assert_eq!(doc.sections().next().unwrap().header_name(), "Returns");
}

#[test]
fn remove_lines_consumes_exactly_one_blank_line() {
    // Two consecutive blank lines: only the first is consumed.
    let src = "Summary.\n\nArgs:\n    x: Desc.\n\n\nReturns:\n    int: Result.\n";
    let parsed = pydocstring::parse::parse_google(src);

    let section = find_first(parsed.root(), SyntaxKind::SECTION).unwrap();
    let out = parsed.edit().remove_lines(section).apply().unwrap();
    assert_eq!(out, "Summary.\n\n\nReturns:\n    int: Result.\n");
}

#[test]
fn remove_lines_at_end_of_source_without_newline() {
    let src = "Summary.\n\nArgs:\n    x: Desc.";
    let parsed = pydocstring::parse::parse_google(src);

    let entry = find_first(parsed.root(), SyntaxKind::ENTRY).unwrap();
    let out = parsed.edit().remove_lines(entry).apply().unwrap();
    assert_eq!(out, "Summary.\n\nArgs:\n");
}

// =============================================================================
// apply_reparsed style preservation
// =============================================================================

#[test]
fn apply_reparsed_preserves_style() {
    use pydocstring::parse::Style;

    let src = "Summary.\n\nArgs:\n    x: Old.\n";
    let parsed = parse(src);
    assert_eq!(parsed.style(), Style::Google);

    // An ordinary edit keeps the style.
    let doc = Document::new(&parsed);
    let entry = doc.sections().next().unwrap().entries().next().unwrap();
    let desc = entry.description().unwrap();
    let reparsed = parsed
        .edit()
        .replace_node(desc.syntax(), "New.")
        .apply_reparsed()
        .unwrap();
    assert_eq!(reparsed.style(), Style::Google);
    assert_eq!(reparsed.source(), "Summary.\n\nArgs:\n    x: New.\n");

    // Even an edit that removes every style marker reparses with the same
    // style parser — the style is not re-detected.
    let section = doc.sections().next().unwrap();
    let reparsed = parsed.edit().remove_lines(section.syntax()).apply_reparsed().unwrap();
    assert_eq!(reparsed.source(), "Summary.\n\n");
    assert_eq!(reparsed.style(), Style::Google);
}

/// SPEC: a user-built node whose range splits a multi-byte character must
/// surface as EditError::OutOfBounds from apply(), never a panic during
/// extent computation (SyntaxNode::new / TextRange::new are public).
#[test]
fn remove_lines_non_char_boundary_range_is_rejected_not_panicking() {
    use pydocstring::syntax::SyntaxKind;
    use pydocstring::syntax::SyntaxNode;
    use pydocstring::text::TextRange;
    use pydocstring::text::TextSize;

    let parsed = pydocstring::parse::parse_google("Summary é.\n\nArgs:\n    x: D.\n");
    // Offset 9 lands inside the two-byte 'é' (bytes 8..10).
    let bogus = SyntaxNode::new(
        SyntaxKind::ENTRY,
        TextRange::new(TextSize::from(9usize), TextSize::from(12usize)),
        Vec::new(),
    );
    let mut edits = parsed.edit();
    edits.remove_lines(&bogus);
    let err = edits.apply().unwrap_err();
    assert!(
        matches!(err, pydocstring::edit::EditError::OutOfBounds { .. }),
        "got {err:?}"
    );
}

// =============================================================================
// remove_lines_range — the range-anchored form
// =============================================================================

/// Collect every node of the tree, in document order.
fn all_nodes<'a>(node: &'a SyntaxNode, out: &mut Vec<&'a SyntaxNode>) {
    out.push(node);
    for child in node.children() {
        if let SyntaxElement::Node(n) = child {
            all_nodes(n, out);
        }
    }
}

/// `remove_lines(node)` is exactly `remove_lines_range(node.range())` — the
/// expansion only ever reads the node's range, and the blank-line step
/// resolves against the whole tree. Pinned over the corpus, node by node, so
/// the FFI surface (which can only hold a range) cannot drift from the
/// node-anchored API.
#[test]
fn remove_lines_range_matches_remove_lines_for_every_node() {
    for (style, path) in corpus_cases() {
        let source = fs::read_to_string(&path).unwrap();
        let parsed = parse_for_style(&style, &source);

        let mut nodes = Vec::new();
        all_nodes(parsed.root(), &mut nodes);

        for node in nodes {
            let by_node = parsed.edit().remove_lines(node).apply().unwrap();
            let by_range = parsed.edit().remove_lines_range(node.range()).apply().unwrap();
            assert_eq!(
                by_node,
                by_range,
                "remove_lines/remove_lines_range diverged on {:?} in {}",
                node.kind(),
                corpus_name(&path)
            );
        }
    }
}

#[test]
fn source_text_never_panics_on_a_hand_built_range() {
    use pydocstring::text::TextRange;
    use pydocstring::text::TextSize;

    // A `TextRange` is two numbers, and since #133 the Python binding can build
    // one, so every case below is reachable from user code. `&source[start..end]`
    // panics on the last two — and a panic across the FFI boundary is an abort.
    let src = "café";
    let r = |a: u32, b: u32| TextRange::new(TextSize::new(a), TextSize::new(b));

    assert_eq!(r(0, 2).source_text(src), "ca");
    assert_eq!(r(0, 99).source_text(src), "", "out of bounds");
    assert_eq!(r(3, 1).source_text(src), "", "inverted");
    // Bytes 3..5 are the `é`, so 4 is inside it.
    assert_eq!(r(0, 4).source_text(src), "", "end splits a character");
    assert_eq!(r(4, 5).source_text(src), "", "start splits a character");
}
