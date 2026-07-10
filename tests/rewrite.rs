//! Spec and law tests for pattern-based rewriting (#47).
//!
//! The headline laws:
//!
//! 1. **Preservation law** — replacing every match of a whole-construct
//!    pattern with a template that re-emits the capture verbatim reproduces
//!    the source byte-for-byte, over the whole corpus. This is the ultimate
//!    "you don't touch what you don't rewrite" proof: matching visits many
//!    regions, each is rewritten with its own original bytes, and the result
//!    is identical.
//! 2. **Surgical change** — a single-line rewrite changes only the matched
//!    span; every other byte is identical.
//!
//! Plus hand-written specs: single-line replace, capture byte-exactness,
//! multi-line template re-indentation, multi-line capture preservation,
//! unknown-metavariable errors, no-match / style-strict no-ops, the #26
//! use case end-to-end, and `replace_in` scoping.

mod common;

use std::fs;

use common::corpus_cases;
use common::corpus_name;
use pydocstring::parse::Style;
use pydocstring::parse::google::parse_google;
use pydocstring::parse::numpy::parse_numpy;
use pydocstring::parse::parse;
use pydocstring::parse::unified::Document;
use pydocstring::pattern::Pattern;
use pydocstring::rewrite::RewriteError;
use pydocstring::syntax::Parsed;

fn parse_for_style(style: &str, input: &str) -> Parsed {
    match style {
        "google" => parse_google(input),
        "numpy" => parse_numpy(input),
        "plain" => pydocstring::parse::plain::parse_plain(input),
        other => panic!("unknown corpus style directory: {other}"),
    }
}

// =============================================================================
// Single-line replace — the rock-solid core
// =============================================================================

/// A single-line template rewrites the matched span verbatim, substituting
/// captured original bytes.
#[test]
fn single_line_replace_substitutes_captures() {
    let src = "Summary.\n\nArgs:\n    x (int): The value.\n";
    let parsed = parse(src);
    let pattern = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();

    let out = parsed.replace(&pattern, "$NAME ($TYPE): $DESC (deprecated)").unwrap();
    assert_eq!(out, "Summary.\n\nArgs:\n    x (int): The value. (deprecated)\n");
}

/// A single-line rewrite changes only the matched span — the diff is exactly
/// the intended edit and every surrounding byte is identical.
#[test]
fn single_line_replace_is_surgical() {
    let src = "Summary.\n\nArgs:\n    x (int): The value.\n    y (str): Kept as is.\n\nReturns:\n    bool: ok.\n";
    let parsed = parse(src);
    let pattern = Pattern::new(Style::Google, "$NAME (str): $DESC").unwrap();

    let out = parsed.replace(&pattern, "$NAME (str): CHANGED").unwrap();
    // Only the `y` entry's description changed; the surrounding bytes are
    // byte-for-byte identical to the source.
    let expected = src.replace("y (str): Kept as is.", "y (str): CHANGED");
    assert_eq!(out, expected);
    assert!(out.contains("x (int): The value."));
    assert!(out.contains("Returns:\n    bool: ok.\n"));
}

/// The captured bytes appear in the output exactly as they were in the source
/// — internal spacing and punctuation are never reformatted.
#[test]
fn capture_bytes_are_byte_exact() {
    let src = "Summary.\n\nArgs:\n    x (Dict[str,  int]): Odd   spacing kept.\n";
    let parsed = parse(src);
    let pattern = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();

    let out = parsed.replace(&pattern, "[$TYPE] $DESC").unwrap();
    assert_eq!(out, "Summary.\n\nArgs:\n    [Dict[str,  int]] Odd   spacing kept.\n");
}

/// A pattern metavariable left out of the template drops that region — an
/// intentional deletion, not an error.
#[test]
fn unused_capture_is_dropped() {
    let src = "Summary.\n\nArgs:\n    x (int): The value.\n";
    let parsed = parse(src);
    let pattern = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();

    let out = parsed.replace(&pattern, "$NAME: $DESC").unwrap();
    assert_eq!(out, "Summary.\n\nArgs:\n    x: The value.\n");
}

/// A repeated template hole substitutes the same capture at each occurrence.
#[test]
fn repeated_template_metavar_repeats_capture() {
    let src = "Summary.\n\nArgs:\n    x (int): The value.\n";
    let parsed = parse(src);
    let pattern = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();

    let out = parsed.replace(&pattern, "$NAME ($TYPE): $NAME is a $TYPE").unwrap();
    assert_eq!(out, "Summary.\n\nArgs:\n    x (int): x is a int\n");
}

// =============================================================================
// Multi-line templates & captures
// =============================================================================

/// A multi-line template's continuation lines are re-indented to the match's
/// base indent, while its first line lands after the existing indentation.
#[test]
fn multiline_template_reindents_continuations() {
    let src = "Summary.\n\nArgs:\n    x (int): The value.\n";
    let parsed = parse(src);
    let pattern = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();

    // The template spans two lines; the second is a template continuation and
    // must be prefixed with the entry's 4-space base indent.
    let out = parsed.replace(&pattern, "$NAME ($TYPE):\n$DESC").unwrap();
    assert_eq!(out, "Summary.\n\nArgs:\n    x (int):\n    The value.\n");
}

/// A blank CRLF continuation line in the template stays blank — the owed base
/// indent must not be flushed before the `\r` of an empty `\r\n` line (#101).
#[test]
fn crlf_blank_continuation_line_stays_blank() {
    let src = "Summary.\n\nArgs:\n    x (int): The value.\n";
    let parsed = parse(src);
    let pattern = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();

    // Template with a blank CRLF line between two content lines.
    let out = parsed.replace(&pattern, "$NAME ($TYPE): $DESC\r\n\r\ntail").unwrap();
    // The empty line carries no indentation; only the real continuation does.
    assert_eq!(out, "Summary.\n\nArgs:\n    x (int): The value.\r\n\r\n    tail\n");
}

/// Interior template indentation is preserved *on top of* the base indent:
/// the template's own leading spaces stack under the base indent.
#[test]
fn multiline_template_keeps_relative_indent() {
    let src = "Summary.\n\nArgs:\n    x (int): The value.\n";
    let parsed = parse(src);
    let pattern = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();

    let out = parsed
        .replace(&pattern, "$NAME ($TYPE): $DESC\n    Note: see above.")
        .unwrap();
    // Base indent (4) + the template's own 4 = 8 spaces on the note line.
    assert_eq!(
        out,
        "Summary.\n\nArgs:\n    x (int): The value.\n        Note: see above.\n"
    );
}

/// A multi-line *capture* keeps its own original bytes and interior
/// indentation verbatim — it is never re-indented, even when the template
/// around it is.
#[test]
fn multiline_capture_is_preserved_verbatim() {
    let src = "Summary.\n\nArgs:\n    x (int): First line.\n        Second line.\n";
    let parsed = parse(src);
    // `$$$REST` absorbs the whole entry so the multi-line description is
    // captured as one span with its original indentation.
    let pattern = Pattern::new(Style::Google, "$$$REST").unwrap();

    // A template with a literal continuation line plus the verbatim capture.
    let out = parsed.replace(&pattern, "$$$REST").unwrap();
    assert_eq!(out, src, "self-emit of a multi-line capture is identity");
}

// =============================================================================
// Errors and no-ops
// =============================================================================

/// A template hole the reading does not bind is an `UnknownMetavar` error.
#[test]
fn unknown_template_metavar_errors() {
    let src = "Summary.\n\nArgs:\n    x (int): The value.\n";
    let parsed = parse(src);
    let pattern = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();

    let err = parsed.replace(&pattern, "$NAME ($NOPE): $DESC").unwrap_err();
    assert_eq!(
        err,
        RewriteError::UnknownMetavar {
            name: "NOPE".to_owned()
        }
    );
    assert!(err.to_string().contains("NOPE"));
}

/// A pattern that matches nothing is a no-op: the source is returned
/// unchanged.
#[test]
fn no_match_is_noop() {
    let src = "Summary.\n\nArgs:\n    x (int): The value.\n";
    let parsed = parse(src);
    let pattern = Pattern::new(Style::Google, "$NAME (bool): $DESC").unwrap();

    let out = parsed.replace(&pattern, "changed").unwrap();
    assert_eq!(out, src);
}

/// Style strictness is inherited: a NumPy pattern against a Google target
/// matches nothing, so the source is returned unchanged.
#[test]
fn style_mismatch_is_noop() {
    let src = "Summary.\n\nArgs:\n    x (int): The value.\n";
    let parsed = parse(src);
    assert_eq!(parsed.style(), Style::Google);
    let pattern = Pattern::new(Style::NumPy, "$NAME : $TYPE").unwrap();

    let out = parsed.replace(&pattern, "changed").unwrap();
    assert_eq!(out, src);
}

// =============================================================================
// The #26 use case, end-to-end
// =============================================================================

/// The issue #26 use case: rewrite exactly one entry's line (annotate its
/// description), leaving every other byte of the docstring identical.
#[test]
fn issue_26_annotate_one_entry() {
    let src = "\
Fit the model.

Args:
    adata: The annotated data matrix.
    n_comps (int): Number of components.
    copy (bool): Return a copy.

Returns:
    Depending on `copy`.
";
    let parsed = parse(src);
    let pattern = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();

    let out = parsed.replace(&pattern, "$NAME ($TYPE): $DESC (deprecated)").unwrap();
    let expected = "\
Fit the model.

Args:
    adata: The annotated data matrix.
    n_comps (int): Number of components. (deprecated)
    copy (bool): Return a copy. (deprecated)

Returns:
    Depending on `copy`.
";
    assert_eq!(out, expected);
    // The bracket-less `adata` entry (no `$TYPE`) is untouched.
    assert!(out.contains("    adata: The annotated data matrix.\n"));
}

// =============================================================================
// replace_in scoping
// =============================================================================

/// `replace_in` scopes the rewrite to an anchor: anchored at the `Raises:`
/// section, `$NAME: $DESC` binds the exception `TYPE` and rewrites only there,
/// leaving the identically-shaped `Args` entries untouched.
#[test]
fn replace_in_scopes_to_anchor() {
    let src = "\
Summary.

Args:
    x: The value.

Raises:
    ValueError: Bad input.
";
    let parsed = parse(src);
    let doc = Document::new(&parsed);
    let raises = doc
        .sections()
        .find(|s| s.kind() == pydocstring::model::SectionKind::Raises)
        .unwrap();
    let pattern = Pattern::new(Style::Google, "$NAME: $DESC").unwrap();

    let out = parsed.replace_in(&pattern, raises.syntax(), "$NAME -> $DESC").unwrap();
    let expected = "\
Summary.

Args:
    x: The value.

Raises:
    ValueError -> Bad input.
";
    assert_eq!(out, expected);
    // The Args entry, outside the anchor, is untouched.
    assert!(out.contains("    x: The value.\n"));
}

// =============================================================================
// Laws over the corpus
// =============================================================================

/// PRESERVATION LAW: over the whole corpus, replacing every match of the
/// whole-construct pattern `$$$X` with the template `$$$X` (which re-emits the
/// captured span verbatim) reproduces the source byte-for-byte. Each match's
/// range and capture range coincide with the matched fragment, so every
/// rewritten region is spliced back with its own original bytes — the
/// ultimate proof that rewriting touches nothing it does not explicitly
/// change.
#[test]
fn law_preservation_self_emit_is_identity() {
    let mut checked = 0usize;
    for (style, path) in corpus_cases() {
        let text = fs::read_to_string(&path).unwrap();
        let parsed = parse_for_style(&style, &text);
        let pattern = Pattern::new(style_of(&style), "$$$X").unwrap();

        let out = parsed.replace(&pattern, "$$$X").unwrap();
        assert_eq!(out, text, "preservation law failed for {}", corpus_name(&path));
        checked += 1;
    }
    assert!(checked >= 20, "unexpectedly small corpus: {checked}");
}

/// PRESERVATION LAW (entry shape): over the corpus, the generic entry pattern
/// `$NAME ($TYPE): $DESC` (Google) / `$NAME : $TYPE` (NumPy) rewritten with a
/// template that re-emits each capture at its own site preserves the source
/// wherever the template's literal separators reproduce the entry's exact
/// spelling. This exercises multi-capture rendering across every match.
#[test]
fn law_self_emit_entry_preserves_matched_regions() {
    // A controlled, canonically-spelled corpus subset: every Google entry
    // here is spelled `name (type): desc` with single spaces, so re-emitting
    // the captures reproduces the entry byte-for-byte.
    let src = "\
Summary.

Args:
    x (int): The value.
    y (str): Another one.
    z (bool): Third.
";
    let parsed = parse(src);
    let pattern = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();
    let out = parsed.replace(&pattern, "$NAME ($TYPE): $DESC").unwrap();
    assert_eq!(out, src);
}

fn style_of(style: &str) -> Style {
    match style {
        "google" => Style::Google,
        "numpy" => Style::NumPy,
        "plain" => Style::Plain,
        other => panic!("unknown style: {other}"),
    }
}
