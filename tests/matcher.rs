//! Matching engine (#46): structural unification, metavariable captures,
//! anchor-derived grammar binding, ordering/overlap rules, and the corpus
//! parity law against the unified layer.

mod common;

use pydocstring::matcher::Match;
use pydocstring::model::SectionKind;
use pydocstring::parse::Style;
use pydocstring::parse::google::parse_google;
use pydocstring::parse::parse;
use pydocstring::parse::unified::Document;
use pydocstring::parse::unified::Section;
use pydocstring::pattern::Pattern;
use pydocstring::syntax::Parsed;
use pydocstring::syntax::SyntaxKind;

/// `(capture name, capture text)` pairs of a match, in reported order.
fn capture_texts<'t>(m: &Match<'t>) -> Vec<(String, &'t str)> {
    m.captures().map(|(name, c)| (name.to_owned(), c.text())).collect()
}

/// The section of `parsed` with the given kind (must exist exactly once).
fn section_of<'a>(parsed: &'a Parsed, kind: &SectionKind) -> Section<'a> {
    let doc = Document::new(parsed);
    let sections: Vec<Section<'a>> = doc.sections().filter(|s| s.kind() == *kind).collect();
    let [section] = sections[..] else {
        panic!("expected exactly one section of the requested kind");
    };
    section
}

// =============================================================================
// Role gating
// =============================================================================

/// An entry reading matches only inside sections whose role is in its
/// `section_kinds`: `$NAME ($TYPE): $DESC` (parameter-family reading)
/// matches the Args entries and nothing in the identically-entry-shaped
/// Raises section.
#[test]
fn role_gating_entry_readings() {
    let src = "Summary.\n\nArgs:\n    x (int): The value.\n    y (str): Another.\n\nRaises:\n    ValueError: bad.\n";
    let target = parse(src);
    let p = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();

    let matches = p.matches(&target);
    assert_eq!(matches.len(), 2);
    for m in &matches {
        assert!(m.reading().section_kinds().contains(&SectionKind::Parameters));
    }
    assert_eq!(
        capture_texts(&matches[0]),
        vec![
            ("NAME".to_owned(), "x"),
            ("TYPE".to_owned(), "int"),
            ("DESC".to_owned(), "The value."),
        ]
    );
    assert_eq!(capture_texts(&matches[1])[0], ("NAME".to_owned(), "y"));

    // Nothing matched inside the Raises section.
    let raises = section_of(&target, &SectionKind::Raises);
    assert!(
        matches.iter().all(|m| m.range().end() <= raises.range().start()),
        "no match may fall inside Raises"
    );
    assert!(p.matches_in(&target, raises.syntax()).is_empty());
}

// =============================================================================
// Anchor-derived grammar binding
// =============================================================================

/// The maintainer's parent-node design: the same pattern text binds
/// differently depending on the anchor's role — `$TYPE` is a NAME token
/// under an Args anchor and a TYPE token under a Raises anchor.
#[test]
fn anchored_matching_derives_reading_from_section_role() {
    let src = "Args:\n    x: The value.\n\nRaises:\n    ValueError: bad value.\n";
    let target = parse(src);
    let p = Pattern::new(Style::Google, "$TYPE: $DESC").unwrap();

    let args = section_of(&target, &SectionKind::Parameters);
    let in_args = p.matches_in(&target, args.syntax());
    assert_eq!(in_args.len(), 1);
    assert!(in_args[0].reading().section_kinds().contains(&SectionKind::Parameters));
    let cap = in_args[0].capture("TYPE").unwrap();
    assert_eq!(cap.text(), "x");
    assert_eq!(cap.element().unwrap().kind(), SyntaxKind::NAME);

    let raises = section_of(&target, &SectionKind::Raises);
    let in_raises = p.matches_in(&target, raises.syntax());
    assert_eq!(in_raises.len(), 1);
    assert!(in_raises[0].reading().section_kinds().contains(&SectionKind::Raises));
    let cap = in_raises[0].capture("TYPE").unwrap();
    assert_eq!(cap.text(), "ValueError");
    assert_eq!(cap.element().unwrap().kind(), SyntaxKind::TYPE);

    // Global matching finds both, in document order, each under the
    // reading its section admits.
    let all = p.matches(&target);
    assert_eq!(all.len(), 2);
    assert!(all[0].range().start() < all[1].range().start());
    assert_eq!(all[0].range(), in_args[0].range());
    assert_eq!(all[1].range(), in_raises[0].range());
}

/// An anchor that is not a node of the target's tree yields no matches.
#[test]
fn foreign_anchor_yields_no_matches() {
    let target = parse("Args:\n    x: d\n");
    let other = parse("Args:\n    x: d\n");
    let p = Pattern::new(Style::Google, "$NAME: $DESC").unwrap();
    assert_eq!(p.matches(&target).len(), 1);
    assert!(p.matches_in(&target, other.root()).is_empty());
}

// =============================================================================
// Captures: target coordinates, byte-for-byte
// =============================================================================

/// Capture ranges index the ORIGINAL target source and `text()` is exactly
/// that slice (the RFC preservation guarantee).
#[test]
fn captures_are_target_bytes() {
    let src = "Parameters\n----------\nn_comps : int, optional\n    Number of PCs.\n    More text.\n";
    let target = parse(src);
    let p = Pattern::new(Style::NumPy, "$NAME : $TYPE\n    $$$REST").unwrap();

    let matches = p.matches(&target);
    assert_eq!(matches.len(), 1);
    let m = &matches[0];
    assert_eq!(
        m.text(),
        &src[usize::from(m.range().start())..usize::from(m.range().end())]
    );
    for (_, capture) in m.captures() {
        let (s, e) = (usize::from(capture.range().start()), usize::from(capture.range().end()));
        assert_eq!(capture.text(), &src[s..e]);
    }
    assert_eq!(m.capture("NAME").unwrap().text(), "n_comps");
    assert_eq!(m.capture("TYPE").unwrap().text(), "int");
    // The `$$$REST` hole swallowed the optional marker and the multi-line
    // description — original bytes, layout included.
    assert_eq!(
        m.capture("REST").unwrap().text(),
        ", optional\n    Number of PCs.\n    More text."
    );
    assert!(m.capture("REST").unwrap().is_multi());
    assert!(m.capture("NOSUCH").is_none());
}

// =============================================================================
// `$$$` sequences
// =============================================================================

/// A `$$$` hole between literal entries binds exactly the middle siblings.
#[test]
fn multi_hole_binds_middle_between_literal_entries() {
    let src = "Args:\n    x: a\n    m: b\n    z: c\n";
    let target = parse(src);
    let p = Pattern::new(Style::Google, "Args:\n    x: a\n    $$$REST\n    z: c").unwrap();

    let matches = p.matches(&target);
    assert_eq!(matches.len(), 1);
    let rest = matches[0].capture("REST").unwrap();
    assert_eq!(rest.text(), "m: b");
    assert_eq!(rest.elements().len(), 1);
    assert_eq!(rest.elements()[0].kind(), SyntaxKind::ENTRY);

    // The same pattern still matches when the middle is empty …
    let two = parse("Args:\n    x: a\n    z: c\n");
    let matches = p.matches(&two);
    assert_eq!(matches.len(), 1);
    assert!(matches[0].capture("REST").unwrap().text().is_empty());

    // … but not when the literal frame is broken.
    let broken = parse("Args:\n    x: a\n    m: b\n");
    assert!(p.matches(&broken).is_empty());
}

/// An empty `$$$` capture has a zero-length range at the position where
/// the sequence would be inserted (after the last sibling matched before
/// the hole).
#[test]
fn empty_multi_capture_position() {
    let src = "Args:\n    x: a\n";
    let target = parse(src);
    let p = Pattern::new(Style::Google, "Args:\n    x: a\n    $$$REST").unwrap();

    let matches = p.matches(&target);
    assert_eq!(matches.len(), 1);
    let rest = matches[0].capture("REST").unwrap();
    assert!(rest.is_multi());
    assert!(rest.range().is_empty());
    assert!(rest.text().is_empty());
    assert!(rest.elements().is_empty());
    assert!(rest.element().is_none());
    // Right after the `x: a` entry.
    let expected = src.find("x: a").unwrap() + "x: a".len();
    assert_eq!(usize::from(rest.range().start()), expected);
}

/// Two `$$$` holes in one sibling list are ambiguous: the affected reading
/// contributes no matches (panic-free), per the documented v1 limit.
#[test]
fn two_holes_in_one_sibling_list_never_match() {
    let p = Pattern::new(Style::Google, "$$$A\n$$$B").unwrap();
    let target = parse("Notes:\n    line one.\n    line two.\n    line three.\n");
    assert!(p.matches(&target).is_empty());
    assert!(p.matches_in(&target, target.root()).is_empty());
}

// =============================================================================
// Indentation-relative matching
// =============================================================================

/// The same content matches at 4-space, 8-space, and tab indentation —
/// trivia skipping plus per-line TEXT_LINE tokens make matching
/// indentation-relative.
#[test]
fn indent_insensitive_matching() {
    let p = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();
    for src in [
        "Args:\n    x (int): The value.\n",
        "Args:\n        x (int): The value.\n",
        "Args:\n\tx (int): The value.\n",
    ] {
        let target = parse(src);
        assert_eq!(target.style(), Style::Google, "{src:?}");
        let matches = p.matches(&target);
        assert_eq!(matches.len(), 1, "{src:?}");
        assert_eq!(
            capture_texts(&matches[0]),
            vec![
                ("NAME".to_owned(), "x"),
                ("TYPE".to_owned(), "int"),
                ("DESC".to_owned(), "The value."),
            ],
            "{src:?}"
        );
    }

    // NumPy: description lines at different depths match the same
    // 4-space-indented pattern.
    let p = Pattern::new(Style::NumPy, "$NAME : $TYPE\n    $DESC").unwrap();
    for src in [
        "Parameters\n----------\nx : int\n    The value.\n",
        "Parameters\n----------\nx : int\n        The value.\n",
        "Parameters\n----------\nx : int\n\tThe value.\n",
    ] {
        let target = parse(src);
        let matches = p.matches(&target);
        assert_eq!(matches.len(), 1, "{src:?}");
        assert_eq!(matches[0].capture("DESC").unwrap().text(), "The value.", "{src:?}");
    }
}

// =============================================================================
// Repeated metavariables
// =============================================================================

/// Every occurrence of a repeated metavariable must bind byte-identical
/// text.
#[test]
fn repeated_metavar_requires_identical_text() {
    // Google: `$A: $A` — NAME and description line must be identical.
    let p = Pattern::new(Style::Google, "$A: $A").unwrap();
    let target = parse("Args:\n    x: x\n    y: z\n");
    let matches = p.matches(&target);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].capture("A").unwrap().text(), "x");

    // NumPy: `$A, $A : int` — both comma-separated names must be identical.
    let p = Pattern::new(Style::NumPy, "$A, $A : int").unwrap();
    let target = parse("Parameters\n----------\nfoo, foo : int\nfoo, bar : int\n");
    let matches = p.matches(&target);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].capture("A").unwrap().text(), "foo");
}

// =============================================================================
// BLANK_LINE is structure
// =============================================================================

/// A multi-paragraph body pattern requires the paragraph break: one
/// BLANK_LINE pairs with exactly one BLANK_LINE.
#[test]
fn blank_line_is_structure() {
    let p = Pattern::new(Style::Google, "para one.\n\npara two.").unwrap();

    let two_paragraphs = parse("Notes:\n    para one.\n\n    para two.\n");
    assert_eq!(p.matches(&two_paragraphs).len(), 1);

    // No paragraph break: no match.
    let one_paragraph = parse("Notes:\n    para one.\n    para two.\n");
    assert!(p.matches(&one_paragraph).is_empty());

    // A double blank is two BLANK_LINE tokens: still no match.
    let double_blank = parse("Notes:\n    para one.\n\n\n    para two.\n");
    assert!(p.matches(&double_blank).is_empty());
}

// =============================================================================
// Style strictness
// =============================================================================

/// A pattern only matches targets of its own style.
#[test]
fn style_strict_matching() {
    let google = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();
    let numpy = Pattern::new(Style::NumPy, "$NAME : $TYPE").unwrap();

    let numpy_doc = parse("Parameters\n----------\nx : int\n    The value.\n");
    let google_doc = parse("Args:\n    x (int): The value.\n");
    assert_eq!(numpy_doc.style(), Style::NumPy);
    assert_eq!(google_doc.style(), Style::Google);

    assert!(google.matches(&numpy_doc).is_empty());
    assert!(google.matches_in(&numpy_doc, numpy_doc.root()).is_empty());
    assert!(numpy.matches(&google_doc).is_empty());
    assert!(numpy.matches_in(&google_doc, google_doc.root()).is_empty());

    // Sanity: each matches its own style (a description-less target for
    // the description-less NumPy pattern).
    assert_eq!(google.matches(&google_doc).len(), 1);
    let bare_numpy_doc = parse("Parameters\n----------\nx : int\n");
    assert_eq!(bare_numpy_doc.style(), Style::NumPy);
    assert_eq!(numpy.matches(&bare_numpy_doc).len(), 1);
}

// =============================================================================
// Missing placeholders
// =============================================================================

/// A zero-length (missing) pattern token matches only a zero-length target
/// token: missing matches missing.
#[test]
fn missing_placeholder_matches_only_missing() {
    let missing_type = Pattern::new(Style::Google, "x ():").unwrap();
    let real_type = Pattern::new(Style::Google, "x (int):").unwrap();

    let target_missing = parse("Args:\n    x ():\n");
    let target_real = parse("Args:\n    x (int):\n");

    assert_eq!(missing_type.matches(&target_missing).len(), 1);
    assert!(missing_type.matches(&target_real).is_empty());
    assert!(real_type.matches(&target_missing).is_empty());
    assert_eq!(real_type.matches(&target_real).len(), 1);

    // `$X` never binds a missing element: `$NAME ($TYPE):` requires a
    // present TYPE.
    let metavar_type = Pattern::new(Style::Google, "$NAME ($TYPE):").unwrap();
    assert!(metavar_type.matches(&target_missing).is_empty());
    assert_eq!(metavar_type.matches(&target_real).len(), 1);
}

// =============================================================================
// Inexact (sub-line prose) sites: documented deferral
// =============================================================================

/// A reading whose metavariable sits amid literal prose inside one
/// TEXT_LINE is not matchable in v1 — no matches, no panic.
#[test]
fn inexact_prose_sites_do_not_match() {
    let p = Pattern::new(Style::Google, "prose with $X inside").unwrap();
    let target = parse("Notes:\n    prose with something inside\n");
    assert!(p.matches(&target).is_empty());
    assert!(p.matches_in(&target, target.root()).is_empty());
}

// =============================================================================
// Document-reading policy
// =============================================================================

/// `matches()` skips Document readings when other readings exist;
/// anchoring at the root opts in.
#[test]
fn document_reading_policy() {
    // "hello" has entry/body readings, so its Document reading is skipped
    // globally — a summary-only document yields no global match …
    let p = Pattern::new(Style::Google, "hello").unwrap();
    let target = parse_google("hello");
    assert!(p.matches(&target).is_empty());
    // … but anchoring at the root explicitly uses the Document reading.
    let anchored = p.matches_in(&target, target.root());
    assert_eq!(anchored.len(), 1);
    assert_eq!(anchored[0].range(), *target.root().range());

    // A Document-only pattern participates in matches() (its only reading).
    let p = Pattern::new(Style::Plain, "Just a summary.").unwrap();
    let target = parse("Just a summary.");
    assert_eq!(target.style(), Style::Plain);
    let matches = p.matches(&target);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].range(), *target.root().range());
    assert!(p.matches(&parse("A different summary.")).is_empty());
}

// =============================================================================
// Order and overlap
// =============================================================================

/// First match wins: an outer (section) match suppresses overlapping inner
/// (entry) candidates.
#[test]
fn first_match_wins_on_overlap() {
    // "Args:\n    x: a" has both entry readings (reading the whole text as
    // one entry) and a Section reading; on this target the Section reading
    // matches the SECTION node, which is visited before its entries.
    let p = Pattern::new(Style::Google, "Args:\n    x: a").unwrap();
    let target = parse("Args:\n    x: a\n");
    let matches = p.matches(&target);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].reading().fragment().kind(), SyntaxKind::SECTION);
}

/// Global matches come back in document order.
#[test]
fn matches_are_in_document_order() {
    let p = Pattern::new(Style::NumPy, "$NAME : $TYPE").unwrap();
    let target = parse("Parameters\n----------\na : int\nb : str\n\nReturns\n-------\nout : bool\n");
    let matches = p.matches(&target);
    assert_eq!(matches.len(), 3);
    let names: Vec<&str> = matches.iter().map(|m| m.capture("NAME").unwrap().text()).collect();
    assert_eq!(names, vec!["a", "b", "out"]);
    assert!(matches.windows(2).all(|w| w[0].range().end() <= w[1].range().start()));
}

// =============================================================================
// Corpus laws
// =============================================================================

/// The NumPy corpus targets (first-party + third_party), parsed.
fn numpy_corpus() -> Vec<(String, Parsed)> {
    let docs: Vec<(String, Parsed)> = common::corpus_cases()
        .into_iter()
        .filter(|(style, _)| style == "numpy")
        .map(|(_, path)| {
            let text = std::fs::read_to_string(&path).unwrap();
            (common::corpus_name(&path), parse(&text))
        })
        .filter(|(_, parsed)| parsed.style() == Style::NumPy)
        .collect();
    assert!(docs.len() >= 20, "unexpectedly small numpy corpus: {}", docs.len());
    docs
}

/// PARITY LAW: over the whole NumPy corpus, matches of the generic entry
/// pattern `$NAME : $TYPE\n    $$$REST` under its parameter-family reading
/// correspond 1:1 — same count, same NAME/TYPE texts, same order — with
/// the unified layer's entries that have the pattern's exact spelling:
/// exactly one (present) name, a present type, and no bracket tokens (a
/// Google-spelled `name (type):` entry inside a NumPy section carries the
/// same accessor-level name and type but a different concrete shape, and
/// concrete-syntax patterns are spelling-exact by design). The scope is
/// every section whose kind is in the reading's `section_kinds`; the
/// `$$$REST` hole absorbs everything after the type — markers, defaults,
/// descriptions — so name-colon-type is precisely the pattern's shape.
/// Global and section-anchored matching agree.
#[test]
fn law_numpy_corpus_generic_entry_parity() {
    let pattern = Pattern::new(Style::NumPy, "$NAME : $TYPE\n    $$$REST").unwrap();
    let primary = &pattern.readings()[0];
    assert!(primary.section_kinds().contains(&SectionKind::Parameters));

    let mut total = 0usize;
    for (name, parsed) in numpy_corpus() {
        let doc = Document::new(&parsed);

        // Expected, via the unified layer: `(name, type)` per admitted
        // section, in document order.
        let mut expected: Vec<(String, String)> = Vec::new();
        for section in doc.sections() {
            if !primary.section_kinds().contains(&section.kind()) {
                continue;
            }
            for entry in section.entries() {
                let names: Vec<_> = entry.names().collect();
                let [entry_name] = names[..] else { continue };
                if entry_name.is_missing() {
                    continue;
                }
                let Some(ty) = entry.type_annotation() else { continue };
                if entry.syntax().tokens(SyntaxKind::OPEN_BRACKET).next().is_some() {
                    continue; // Google-spelled `name (type):` entry.
                }
                expected.push((entry_name.text().to_owned(), ty.text().to_owned()));
            }
        }

        // Global matching, restricted to the parameter-family reading.
        let global: Vec<(String, String)> = pattern
            .matches(&parsed)
            .iter()
            .filter(|m| std::ptr::eq(m.reading(), primary))
            .map(|m| {
                (
                    m.capture("NAME").unwrap().text().to_owned(),
                    m.capture("TYPE").unwrap().text().to_owned(),
                )
            })
            .collect();
        assert_eq!(global, expected, "global parity failed for {name}");

        // Anchored matching agrees section by section.
        let mut anchored: Vec<(String, String)> = Vec::new();
        for section in doc.sections() {
            if !primary.section_kinds().contains(&section.kind()) {
                continue;
            }
            for m in pattern.matches_in(&parsed, section.syntax()) {
                anchored.push((
                    m.capture("NAME").unwrap().text().to_owned(),
                    m.capture("TYPE").unwrap().text().to_owned(),
                ));
            }
        }
        assert_eq!(anchored, expected, "anchored parity failed for {name}");

        total += expected.len();
    }
    assert!(total >= 100, "law is near-vacuous: only {total} entries");
}

/// COROLLARY (indent insensitivity over real files): rewriting the
/// pattern's own indentation (4 spaces → 8 spaces → tab) changes nothing —
/// identical matches and captures across the whole NumPy corpus.
#[test]
fn law_numpy_corpus_pattern_indent_is_irrelevant() {
    let four = Pattern::new(Style::NumPy, "$NAME : $TYPE\n    $$$REST").unwrap();
    let eight = Pattern::new(Style::NumPy, "$NAME : $TYPE\n        $$$REST").unwrap();
    let tab = Pattern::new(Style::NumPy, "$NAME : $TYPE\n\t$$$REST").unwrap();

    /// `(start, end, capture name/text pairs)` per match.
    type MatchSummary = (u32, u32, Vec<(String, String)>);

    for (name, parsed) in numpy_corpus() {
        let summarize = |p: &Pattern| -> Vec<MatchSummary> {
            p.matches(&parsed)
                .iter()
                .map(|m| {
                    (
                        m.range().start().raw(),
                        m.range().end().raw(),
                        m.captures().map(|(n, c)| (n.to_owned(), c.text().to_owned())).collect(),
                    )
                })
                .collect()
        };
        let base = summarize(&four);
        assert_eq!(summarize(&eight), base, "8-space pattern diverges on {name}");
        assert_eq!(summarize(&tab), base, "tab pattern diverges on {name}");
    }
}

/// NO-MATCH SANITY: a Document-only pattern matches only a document of
/// exactly its shape — across the entire corpus (every style), zero
/// matches, no panics.
#[test]
fn law_document_only_pattern_no_spurious_corpus_matches() {
    // Summary + section make it multi-block (no entry/body readings), and
    // the summary excludes the Section reading: Document reading only.
    let text = "Zzz qqq unique summary.\n\nWww\n---\nqq : zz\n";
    let p = Pattern::new(Style::NumPy, text).unwrap();
    assert_eq!(p.readings().len(), 1, "test premise: Document-only pattern");

    // It does match itself …
    let this = parse(text);
    assert_eq!(p.matches(&this).len(), 1);

    // … and nothing anywhere in the corpus.
    for (_, path) in common::corpus_cases() {
        let text = std::fs::read_to_string(&path).unwrap();
        let parsed = parse(&text);
        assert!(
            p.matches(&parsed).is_empty(),
            "spurious document match in {}",
            common::corpus_name(&path)
        );
    }
}
