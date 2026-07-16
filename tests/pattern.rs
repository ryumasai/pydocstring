//! Pattern fragments (#45): metavariable lexing, reading enumeration
//! (order, merging, per-role shapes), landing-site introspection, and the
//! per-reading laws (sites resolve, fragment byte coverage).

use pydocstring::model::FreeSectionKind;
use pydocstring::model::SectionKind;
use pydocstring::parse::Style;
use pydocstring::pattern::FragmentKind;
use pydocstring::pattern::Pattern;
use pydocstring::pattern::PatternError;
use pydocstring::pattern::Reading;
use pydocstring::syntax::SyntaxElement;
use pydocstring::syntax::SyntaxKind;
use pydocstring::syntax::SyntaxNode;

/// `(name, multi, site kind, site parent kind, exact)` for every metavar of
/// one reading.
fn var_summaries(reading: &Reading) -> Vec<(String, bool, SyntaxKind, SyntaxKind, bool)> {
    reading
        .metavars()
        .iter()
        .map(|m| {
            (
                m.name().to_owned(),
                m.is_multi(),
                m.site().kind(),
                m.site().parent_kind(),
                m.site().is_exact(),
            )
        })
        .collect()
}

/// The primary reading (`readings()[0]`).
fn primary(pattern: &Pattern) -> &Reading {
    &pattern.readings()[0]
}

/// `(fragment kind, section kinds)` per reading, in enumeration order.
fn overview(pattern: &Pattern) -> Vec<(FragmentKind, Vec<SectionKind>)> {
    pattern
        .readings()
        .iter()
        .map(|r| (r.fragment_kind(), r.section_kinds().to_vec()))
        .collect()
}

/// First reading with the given fragment kind.
fn reading_of_kind(pattern: &Pattern, kind: FragmentKind) -> Option<&Reading> {
    pattern.readings().iter().find(|r| r.fragment_kind() == kind)
}

/// The known free-text kinds a Body reading applies under.
fn all_free_text() -> Vec<SectionKind> {
    [
        FreeSectionKind::Notes,
        FreeSectionKind::Examples,
        FreeSectionKind::Warnings,
        FreeSectionKind::Todo,
        FreeSectionKind::Attention,
        FreeSectionKind::Caution,
        FreeSectionKind::Danger,
        FreeSectionKind::Error,
        FreeSectionKind::Hint,
        FreeSectionKind::Important,
        FreeSectionKind::Tip,
    ]
    .into_iter()
    .map(SectionKind::FreeText)
    .collect()
}

/// The observable polymorphism contract: `fragment_kind()` unambiguously
/// reports what `fragment()` returns.
fn assert_fragment_kind_matches(reading: &Reading, context: &str) {
    let node_kind = reading.fragment().kind();
    let matches = match reading.fragment_kind() {
        FragmentKind::Entry => matches!(node_kind, SyntaxKind::ENTRY | SyntaxKind::CITATION),
        FragmentKind::Body => node_kind == SyntaxKind::DESCRIPTION,
        FragmentKind::Section => node_kind == SyntaxKind::SECTION,
        FragmentKind::Document => node_kind == SyntaxKind::DOCUMENT,
        other => panic!("unknown fragment kind {other:?}"),
    };
    assert!(
        matches,
        "fragment_kind() {:?} does not describe fragment() node kind {node_kind} ({context})",
        reading.fragment_kind()
    );
}

/// Resolve a site path to the element it denotes.
fn element_at<'a>(root: &'a SyntaxNode, path: &[usize]) -> &'a SyntaxElement {
    assert!(!path.is_empty(), "site paths always point below the root");
    let mut cur = root;
    for &i in &path[..path.len() - 1] {
        match &cur.children()[i] {
            SyntaxElement::Node(n) => cur = n,
            SyntaxElement::Token(_) => panic!("path passes through a token"),
        }
    }
    &cur.children()[path[path.len() - 1]]
}

// =============================================================================
// Metavariable lexing (asserted on primary readings)
// =============================================================================

/// `$NAME` and `$$$NAME` are recognised and inventoried in source order.
#[test]
fn metavar_lexing_single_and_multi() {
    let p = Pattern::new(Style::Google, "$NAME ($TYPE): $$$DESC").unwrap();
    let vars = var_summaries(primary(&p));
    assert_eq!(vars.len(), 3);
    assert_eq!(vars[0].0, "NAME");
    assert!(!vars[0].1);
    assert_eq!(vars[1].0, "TYPE");
    assert!(!vars[1].1);
    assert_eq!(vars[2].0, "DESC");
    assert!(vars[2].1, "$$$DESC is a sequence variable");
}

/// `$x`, `$3`, `a$B`, and `$$B` are literal text, not metavariables.
#[test]
fn metavar_lexing_literal_dollars() {
    let p = Pattern::new(Style::Google, "$x ($3): a$B and $$D").unwrap();
    assert!(p.metavars().is_empty());
    // The dollar text survives verbatim in the wrapped source.
    let name = p.fragment().find_token(SyntaxKind::NAME).unwrap();
    assert_eq!(name.text(p.parsed().source()), "$x");
    let ty = p.fragment().find_token(SyntaxKind::TYPE).unwrap();
    assert_eq!(ty.text(p.parsed().source()), "$3");
}

/// The same variable twice is recorded twice (equality semantics are #46's).
#[test]
fn metavar_same_var_twice_recorded_twice() {
    let p = Pattern::new(Style::Google, "$A: $A").unwrap();
    let vars = var_summaries(primary(&p));
    assert_eq!(vars.len(), 2);
    assert_eq!(vars[0].0, "A");
    assert_eq!(vars[1].0, "A");
    // Two different landing sites for the two occurrences.
    assert_eq!(vars[0].2, SyntaxKind::NAME);
    assert_eq!(vars[1].2, SyntaxKind::TEXT_LINE);
    assert_ne!(p.metavars()[0].site().range(), p.metavars()[1].site().range());
}

// =============================================================================
// Entry readings: role merging and shape splits
// =============================================================================

/// The canonical RFC example: the Google bracket entry realises every
/// metavariable exactly under the parameter family, which merges into ONE
/// reading; the Returns/Yields prose reading is enumerated separately.
#[test]
fn google_bracket_entry_merges_parameter_family() {
    let p = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();
    assert_eq!(p.style(), Style::Google);

    let first = primary(&p);
    assert_eq!(first.fragment_kind(), FragmentKind::Entry);
    assert_eq!(first.fragment().kind(), SyntaxKind::ENTRY);
    assert_eq!(
        first.section_kinds(),
        &[
            SectionKind::Parameters,
            SectionKind::KeywordParameters,
            SectionKind::OtherParameters,
            SectionKind::Receives,
            SectionKind::Attributes,
        ]
    );
    assert_eq!(
        var_summaries(first),
        vec![
            ("NAME".to_owned(), false, SyntaxKind::NAME, SyntaxKind::ENTRY, true),
            ("TYPE".to_owned(), false, SyntaxKind::TYPE, SyntaxKind::ENTRY, true),
            (
                "DESC".to_owned(),
                false,
                SyntaxKind::TEXT_LINE,
                SyntaxKind::DESCRIPTION,
                true
            ),
        ]
    );

    // The Returns/Yields reading folds everything into prose: distinct
    // shape, enumerated separately, all bindings inexact.
    let returns = p.reading_for(&SectionKind::Returns).unwrap();
    assert_eq!(returns.section_kinds(), &[SectionKind::Returns, SectionKind::Yields]);
    assert!(returns.metavars().iter().all(|m| !m.site().is_exact()));

    // Roles that lump the metavariables into a structural token have no
    // reading at all.
    assert!(p.reading_for(&SectionKind::Methods).is_none());
    assert!(p.reading_for(&SectionKind::Raises).is_none());
}

/// NumPy `$NAME : $TYPE` splits into distinct readings by binding shape:
/// the parameter family (incl. named Returns/Yields — NumPy shares that
/// grammar) binds `$NAME` to a NAME token; Raises/Warns bind it to a TYPE
/// token; Methods/SeeAlso read `$TYPE` as prose. All coexist.
#[test]
fn numpy_name_type_readings_split_by_shape() {
    let p = Pattern::new(Style::NumPy, "$NAME : $TYPE").unwrap();

    let first = primary(&p);
    assert_eq!(
        first.section_kinds(),
        &[
            SectionKind::Parameters,
            SectionKind::KeywordParameters,
            SectionKind::OtherParameters,
            SectionKind::Receives,
            SectionKind::Returns,
            SectionKind::Yields,
            SectionKind::Attributes,
        ]
    );
    assert_eq!(var_summaries(first)[0].2, SyntaxKind::NAME);
    assert_eq!(var_summaries(first)[1].2, SyntaxKind::TYPE);

    let raises = p.reading_for(&SectionKind::Raises).unwrap();
    assert_eq!(raises.section_kinds(), &[SectionKind::Raises, SectionKind::Warns]);
    assert_eq!(var_summaries(raises)[0].2, SyntaxKind::TYPE);
    assert_eq!(var_summaries(raises)[1].2, SyntaxKind::TEXT_LINE);

    let methods = p.reading_for(&SectionKind::Methods).unwrap();
    assert_eq!(methods.section_kinds(), &[SectionKind::Methods, SectionKind::SeeAlso]);
    assert_eq!(var_summaries(methods)[0].2, SyntaxKind::NAME);
    assert_eq!(var_summaries(methods)[1].2, SyntaxKind::TEXT_LINE);
}

/// NumPy marker suffixes still parse around a `$TYPE` metavariable — and
/// exclude the Returns/Yields merge (their grammar keeps the whole suffix
/// inside the TYPE token, which would swallow the metavariable).
#[test]
fn numpy_entry_with_optional_marker() {
    let p = Pattern::new(Style::NumPy, "$NAME : $TYPE, optional").unwrap();
    let first = primary(&p);
    assert_eq!(
        first.section_kinds(),
        &[
            SectionKind::Parameters,
            SectionKind::KeywordParameters,
            SectionKind::OtherParameters,
            SectionKind::Receives,
            SectionKind::Attributes,
        ]
    );
    assert_eq!(
        var_summaries(first),
        vec![
            ("NAME".to_owned(), false, SyntaxKind::NAME, SyntaxKind::ENTRY, true),
            ("TYPE".to_owned(), false, SyntaxKind::TYPE, SyntaxKind::ENTRY, true),
        ]
    );
    assert!(first.fragment().find_token(SyntaxKind::OPTIONAL).is_some());
    assert!(!first.section_kinds().contains(&SectionKind::Returns));
}

// =============================================================================
// SPEC: the enumeration order (upgrade-stability pin)
// =============================================================================

/// SPEC: the full enumeration order, pinned end-to-end. Both texts are
/// accepted by every trialled role, so the reading sequence + each merged
/// reading's section_kinds *are* the order table — changing it is a
/// breaking change and must fail this test.
#[test]
fn spec_enumeration_order() {
    let p = Pattern::new(Style::Google, "$NAME: $DESC").unwrap();
    assert_eq!(
        overview(&p),
        vec![
            (
                FragmentKind::Entry,
                vec![
                    SectionKind::Parameters,
                    SectionKind::KeywordParameters,
                    SectionKind::OtherParameters,
                    SectionKind::Receives,
                    SectionKind::Attributes,
                    SectionKind::Methods,
                    SectionKind::SeeAlso,
                ]
            ),
            (
                FragmentKind::Entry,
                vec![
                    SectionKind::Returns,
                    SectionKind::Yields,
                    SectionKind::Raises,
                    SectionKind::Warns,
                ]
            ),
            (FragmentKind::Entry, vec![SectionKind::References]),
            (FragmentKind::Body, all_free_text()),
            (FragmentKind::Document, vec![]),
        ]
    );

    let p = Pattern::new(Style::NumPy, "$NAME : $TYPE").unwrap();
    assert_eq!(
        overview(&p),
        vec![
            (
                FragmentKind::Entry,
                vec![
                    SectionKind::Parameters,
                    SectionKind::KeywordParameters,
                    SectionKind::OtherParameters,
                    SectionKind::Receives,
                    SectionKind::Returns,
                    SectionKind::Yields,
                    SectionKind::Attributes,
                ]
            ),
            (FragmentKind::Entry, vec![SectionKind::Raises, SectionKind::Warns]),
            (FragmentKind::Entry, vec![SectionKind::Methods, SectionKind::SeeAlso]),
            (FragmentKind::Entry, vec![SectionKind::References]),
            (FragmentKind::Body, all_free_text()),
            (FragmentKind::Document, vec![]),
        ]
    );
}

/// SPEC: the primary reading (`readings()[0]`) for characteristic texts.
/// These may only change in a breaking release.
#[test]
fn spec_primary_readings() {
    // Every role accepts these; the parameter family enumerates first.
    for (style, text) in [
        (Style::Google, "$NAME: $DESC"),
        (Style::NumPy, "$NAME : $TYPE"),
        (Style::Google, "$$$X"),
        (Style::Google, "just some literal words"),
    ] {
        let p = Pattern::new(style, text).unwrap();
        let first = primary(&p);
        assert_eq!(first.fragment_kind(), FragmentKind::Entry, "{text:?} in {style}");
        assert!(
            first.section_kinds().contains(&SectionKind::Parameters),
            "{text:?} in {style}: primary reading is {:?}",
            first.section_kinds()
        );
    }

    // The parameter family lumps `$X` into a structural NAME token here —
    // no reading — so the prose tier (Returns first) becomes primary.
    let p = Pattern::new(Style::Google, "words with $X inside").unwrap();
    let first = primary(&p);
    assert_eq!(first.section_kinds(), &[SectionKind::Returns, SectionKind::Yields]);
    assert!(p.reading_for(&SectionKind::Parameters).is_none());
}

// =============================================================================
// Section readings
// =============================================================================

/// A Google section text has a Section reading — and, honestly, entry-tier
/// readings too (the wrap grammar absorbs the header line as entry text),
/// which enumerate first per the documented order.
#[test]
fn google_section_reading() {
    let p = Pattern::new(Style::Google, "Returns:\n    $TYPE: $DESC").unwrap();
    assert_eq!(primary(&p).fragment_kind(), FragmentKind::Entry);

    let section = reading_of_kind(&p, FragmentKind::Section).unwrap();
    assert_eq!(section.fragment().kind(), SyntaxKind::SECTION);
    assert!(section.section_kinds().is_empty());
    assert_eq!(
        var_summaries(section),
        vec![
            ("TYPE".to_owned(), false, SyntaxKind::TYPE, SyntaxKind::ENTRY, true),
            (
                "DESC".to_owned(),
                false,
                SyntaxKind::TEXT_LINE,
                SyntaxKind::DESCRIPTION,
                true
            ),
        ]
    );

    // The document reading is always last.
    let last = p.readings().last().unwrap();
    assert_eq!(last.fragment_kind(), FragmentKind::Document);
}

/// The NumPy underline form kills the entry/body wraps (the embedded header
/// splits the wrapped section in two), so the Section reading is primary.
#[test]
fn numpy_section_reading_is_primary() {
    let p = Pattern::new(Style::NumPy, "Returns\n-------\n$TYPE\n    $DESC").unwrap();
    assert_eq!(
        overview(&p),
        vec![(FragmentKind::Section, vec![]), (FragmentKind::Document, vec![])]
    );
    assert_eq!(
        var_summaries(primary(&p)),
        vec![
            ("TYPE".to_owned(), false, SyntaxKind::TYPE, SyntaxKind::ENTRY, true),
            (
                "DESC".to_owned(),
                false,
                SyntaxKind::TEXT_LINE,
                SyntaxKind::DESCRIPTION,
                true
            ),
        ]
    );
}

/// The documented `x:` quirk under enumeration: Google's grammar accepts
/// any `Word:` line as a header, so the text has BOTH entry readings and a
/// Section reading — entry tier first.
#[test]
fn colon_quirk_has_both_readings() {
    // A KNOWN name with a colon reads both as an entry and as a section
    // header; entry readings enumerate first.
    let p = Pattern::new(Style::Google, "Returns:").unwrap();
    let kinds: Vec<FragmentKind> = p.readings().iter().map(|r| r.fragment_kind()).collect();
    let entry_pos = kinds.iter().position(|k| *k == FragmentKind::Entry).unwrap();
    let section_pos = kinds.iter().position(|k| *k == FragmentKind::Section).unwrap();
    assert!(entry_pos < section_pos, "entry readings enumerate before Section");

    // An UNKNOWN name has no Section reading at all since #143: the parsers
    // are napoleon-strict, so `x:` cannot be a section header — the pattern
    // grammar follows the document grammar by construction.
    let p = Pattern::new(Style::Google, "x:").unwrap();
    let kinds: Vec<FragmentKind> = p.readings().iter().map(|r| r.fragment_kind()).collect();
    assert!(
        !kinds.contains(&FragmentKind::Section),
        "unknown `x:` must not read as a section"
    );

    let entry = primary(&p);
    assert!(entry.section_kinds().contains(&SectionKind::Parameters));
    let name = entry.fragment().find_token(SyntaxKind::NAME).unwrap();
    assert_eq!(name.text(entry.parsed().source()), "x");
}

// =============================================================================
// Body readings
// =============================================================================

/// The free-text body reading: fragment = the DESCRIPTION block, applying
/// under every known free-text kind; prose metavariables are sub-line
/// (inexact) TEXT_LINE sites.
#[test]
fn body_reading_freetext() {
    let p = Pattern::new(Style::Google, "prose with $X inside").unwrap();
    let body = reading_of_kind(&p, FragmentKind::Body).unwrap();
    assert_eq!(body.fragment().kind(), SyntaxKind::DESCRIPTION);
    assert_eq!(body.section_kinds(), all_free_text());
    assert_eq!(
        var_summaries(body),
        vec![(
            "X".to_owned(),
            false,
            SyntaxKind::TEXT_LINE,
            SyntaxKind::DESCRIPTION,
            false
        )]
    );
    // reading_for resolves free-text kinds to the Body reading.
    let via_notes = p.reading_for(&SectionKind::FreeText(FreeSectionKind::Notes)).unwrap();
    let via_examples = p
        .reading_for(&SectionKind::FreeText(FreeSectionKind::Examples))
        .unwrap();
    assert_eq!(via_notes.fragment_kind(), FragmentKind::Body);
    assert_eq!(via_examples.fragment_kind(), FragmentKind::Body);
}

/// A standalone `$$$X` in a free-text body binds the whole DESCRIPTION —
/// consistent with the discovered standalone-`$$$` mapping — in both styles.
#[test]
fn body_reading_multi_binds_whole_body() {
    for style in [Style::Google, Style::NumPy] {
        let p = Pattern::new(style, "$$$BODY").unwrap();
        let body = reading_of_kind(&p, FragmentKind::Body).unwrap();
        assert_eq!(
            var_summaries(body),
            vec![(
                "BODY".to_owned(),
                true,
                SyntaxKind::DESCRIPTION,
                SyntaxKind::SECTION,
                true
            )],
            "style {style}"
        );
        // The site is the fragment root itself.
        assert_eq!(
            body.metavars()[0].site().range(),
            body.fragment().range(),
            "style {style}"
        );
    }
}

/// A free-text DESCRIPTION owns its blank lines, so — unlike the entry
/// tier, which is single-block only — the Body reading exists for
/// multi-paragraph text.
#[test]
fn body_reading_spans_blank_lines() {
    let p = Pattern::new(Style::Google, "para one.\n\npara two.").unwrap();
    assert_eq!(
        overview(&p),
        vec![(FragmentKind::Body, all_free_text()), (FragmentKind::Document, vec![])]
    );
    assert_eq!(primary(&p).fragment().kind(), SyntaxKind::DESCRIPTION);
}

// =============================================================================
// Document readings
// =============================================================================

/// Summary + section: multi-block text has no entry tier; the Document
/// reading carries the structural sites.
#[test]
fn document_reading() {
    let p = Pattern::new(Style::Google, "$SUMMARY\n\nArgs:\n    $NAME: $DESC").unwrap();
    let doc = reading_of_kind(&p, FragmentKind::Document).unwrap();
    assert_eq!(doc.fragment().kind(), SyntaxKind::DOCUMENT);
    assert!(doc.section_kinds().is_empty());
    assert_eq!(
        var_summaries(doc),
        vec![
            (
                "SUMMARY".to_owned(),
                false,
                SyntaxKind::TEXT_LINE,
                SyntaxKind::SUMMARY,
                true
            ),
            ("NAME".to_owned(), false, SyntaxKind::NAME, SyntaxKind::ENTRY, true),
            (
                "DESC".to_owned(),
                false,
                SyntaxKind::TEXT_LINE,
                SyntaxKind::DESCRIPTION,
                true
            ),
        ]
    );
    // No entry readings for multi-block text; Document is last.
    assert!(p.reading_for(&SectionKind::Parameters).is_none());
    assert_eq!(p.readings().last().unwrap().fragment_kind(), FragmentKind::Document);
}

/// A metavariable amid document prose binds a sub-line span (inexact). The
/// embedded NumPy header also kills the body wrap here, so the Document
/// reading is the only one.
#[test]
fn document_reading_inexact_prose_site() {
    let p = Pattern::new(Style::NumPy, "Summary of $X.\n\nParameters\n----------\nx : int\n    d").unwrap();
    assert_eq!(overview(&p), vec![(FragmentKind::Document, vec![])]);
    assert_eq!(
        var_summaries(primary(&p)),
        vec![("X".to_owned(), false, SyntaxKind::TEXT_LINE, SyntaxKind::SUMMARY, false)]
    );
}

/// Plain style has no sections: the document reading is the only one.
#[test]
fn plain_style_document_only() {
    let p = Pattern::new(Style::Plain, "Just a summary.").unwrap();
    assert_eq!(overview(&p), vec![(FragmentKind::Document, vec![])]);
    assert!(p.metavars().is_empty());
}

// =============================================================================
// References reading
// =============================================================================

/// References body content is a CITATION node, found via reading_for.
#[test]
fn references_reading_citation() {
    let p = Pattern::new(Style::Google, ".. [$LABEL] $TEXT").unwrap();
    let citation = p.reading_for(&SectionKind::References).unwrap();
    assert_eq!(citation.fragment_kind(), FragmentKind::Entry);
    assert_eq!(citation.fragment().kind(), SyntaxKind::CITATION);
    assert_eq!(citation.section_kinds(), &[SectionKind::References]);
    assert_eq!(
        var_summaries(citation),
        vec![
            ("LABEL".to_owned(), false, SyntaxKind::LABEL, SyntaxKind::CITATION, true),
            (
                "TEXT".to_owned(),
                false,
                SyntaxKind::TEXT_LINE,
                SyntaxKind::DESCRIPTION,
                true
            ),
        ]
    );
}

// =============================================================================
// `$$$X` standalone lines — the discovered mapping
// =============================================================================

/// A standalone `$$$X` line in a structured section body lands as a whole
/// ENTRY under every role that hosts it, in both styles — via reading_for.
#[test]
fn multi_standalone_line_lands_as_entry() {
    for (style, kind) in [
        (Style::Google, SectionKind::Parameters),
        (Style::Google, SectionKind::Returns),
        (Style::Google, SectionKind::Raises),
        (Style::NumPy, SectionKind::Parameters),
        (Style::NumPy, SectionKind::Returns),
    ] {
        let p = Pattern::new(style, "$$$ENTRIES").unwrap();
        let reading = p.reading_for(&kind).unwrap();
        let vars = var_summaries(reading);
        assert_eq!(
            vars,
            vec![("ENTRIES".to_owned(), true, SyntaxKind::ENTRY, SyntaxKind::SECTION, true)],
            "role {kind:?} in {style}"
        );
        assert_eq!(
            reading.metavars()[0].site().range(),
            reading.fragment().range(),
            "role {kind:?} in {style}: the site is the fragment root"
        );
    }
}

/// `$$$X` after a summary lands as the whole EXTENDED_SUMMARY block in the
/// document reading.
#[test]
fn multi_standalone_line_in_document() {
    let p = Pattern::new(Style::Google, "Intro.\n\n$$$REST").unwrap();
    let doc = reading_of_kind(&p, FragmentKind::Document).unwrap();
    assert_eq!(
        var_summaries(doc),
        vec![(
            "REST".to_owned(),
            true,
            SyntaxKind::EXTENDED_SUMMARY,
            SyntaxKind::DOCUMENT,
            true
        )]
    );
}

/// `$$$X` in an entry's description slot binds the whole DESCRIPTION node
/// (where `$X` in the same spot binds the single TEXT_LINE).
#[test]
fn multi_in_description_slot_binds_description_node() {
    let multi = Pattern::new(Style::Google, "$NAME ($TYPE): $$$DESC").unwrap();
    assert_eq!(var_summaries(primary(&multi))[2].2, SyntaxKind::DESCRIPTION);

    let single = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();
    assert_eq!(var_summaries(primary(&single))[2].2, SyntaxKind::TEXT_LINE);
}

// =============================================================================
// Errors: zero-reading input
// =============================================================================

#[test]
fn empty_pattern_is_unparsable() {
    assert!(matches!(
        Pattern::new(Style::Google, ""),
        Err(PatternError::Unparsable { .. })
    ));
    assert!(matches!(
        Pattern::new(Style::NumPy, "   \n  \n"),
        Err(PatternError::Unparsable { .. })
    ));
}

/// The genuinely-zero-readings case: the embedded NumPy header invalidates
/// the entry/body wraps, and the structural-token metavariable invalidates
/// the section and document parses.
#[test]
fn zero_readings_is_unparsable() {
    let err = Pattern::new(Style::NumPy, "Parameters\n----------\nx : Dict[$K]\n    d").unwrap_err();
    let PatternError::Unparsable { message, .. } = err else {
        panic!("expected Unparsable");
    };
    assert!(message.contains("no valid reading"), "{message}");
}

/// Garbage never panics — a bare line is a valid entry of several roles, so
/// it simply enumerates readings.
#[test]
fn garbage_never_panics() {
    let p = Pattern::new(Style::Google, "\u{0}\u{1}%%%???").unwrap();
    assert!(primary(&p).section_kinds().contains(&SectionKind::Parameters));
}

/// A metavariable inside a structural token is not a bindable site: the
/// roles that read it that way contribute no reading, while prose readings
/// survive.
#[test]
fn structural_token_metavar_excludes_readings() {
    let p = Pattern::new(Style::Google, "x (Dict[$K, $V]): d").unwrap();
    assert!(p.reading_for(&SectionKind::Parameters).is_none());
    let first = primary(&p);
    assert_eq!(first.section_kinds(), &[SectionKind::Returns, SectionKind::Yields]);
    assert!(first.metavars().iter().all(|m| !m.site().is_exact()));
}

// =============================================================================
// Placeholder collision-proofing
// =============================================================================

/// Pattern text spelling out a placeholder name literally still works: the
/// stem is lengthened until it cannot collide, and the literal text survives
/// byte-for-byte.
#[test]
fn placeholder_collision_is_probed_away() {
    let p = Pattern::new(Style::Google, "PYDOCMV0X ($TYPE): $DESC").unwrap();
    let name = p.fragment().find_token(SyntaxKind::NAME).unwrap();
    assert_eq!(name.text(p.parsed().source()), "PYDOCMV0X");
    assert_eq!(
        var_summaries(primary(&p)),
        vec![
            ("TYPE".to_owned(), false, SyntaxKind::TYPE, SyntaxKind::ENTRY, true),
            (
                "DESC".to_owned(),
                false,
                SyntaxKind::TEXT_LINE,
                SyntaxKind::DESCRIPTION,
                true
            ),
        ]
    );
    // The TYPE site is NOT the literal name's bytes.
    let type_text = p.metavars()[0].site().range().source_text(p.parsed().source());
    assert_ne!(type_text, "PYDOCMV0X");
    assert!(type_text.starts_with("PYDOCMVQ"), "lengthened stem, got {type_text:?}");
}

// =============================================================================
// Observability + laws over EVERY reading
// =============================================================================

fn pattern_test_set() -> Vec<Pattern> {
    vec![
        Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap(),
        Pattern::new(Style::Google, "$NAME: $DESC").unwrap(),
        Pattern::new(Style::Google, "Returns:\n    $TYPE: $DESC").unwrap(),
        Pattern::new(Style::Google, "$SUMMARY\n\nArgs:\n    $NAME: $DESC").unwrap(),
        Pattern::new(Style::Google, "$$$ENTRIES").unwrap(),
        Pattern::new(Style::Google, ".. [$LABEL] $TEXT").unwrap(),
        Pattern::new(Style::Google, "PYDOCMV0X ($TYPE): $DESC").unwrap(),
        Pattern::new(Style::Google, "para one.\n\npara two.").unwrap(),
        Pattern::new(Style::Google, "x:").unwrap(),
        Pattern::new(Style::NumPy, "$NAME : $TYPE\n    $DESC").unwrap(),
        Pattern::new(Style::NumPy, "Returns\n-------\n$TYPE\n    $DESC").unwrap(),
        Pattern::new(Style::NumPy, "Summary of $X.\n\nParameters\n----------\nx : int\n    d").unwrap(),
        Pattern::new(Style::NumPy, "$$$BODY").unwrap(),
        Pattern::new(Style::Plain, "Just a summary.").unwrap(),
    ]
}

/// ACCEPTANCE: `fragment_kind()` unambiguously reports what `fragment()`
/// returns, for every fragment kind and every reading — the polymorphism is
/// observable.
#[test]
fn fragment_kind_reports_fragment_for_every_reading() {
    // One exact (fragment kind, node kind) pair per fragment kind.
    let entry = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();
    assert_eq!(primary(&entry).fragment_kind(), FragmentKind::Entry);
    assert_eq!(primary(&entry).fragment().kind(), SyntaxKind::ENTRY);

    let citation = Pattern::new(Style::Google, ".. [$LABEL] $TEXT").unwrap();
    let citation = citation.reading_for(&SectionKind::References).unwrap();
    assert_eq!(citation.fragment_kind(), FragmentKind::Entry);
    assert_eq!(citation.fragment().kind(), SyntaxKind::CITATION);

    let body = Pattern::new(Style::Google, "prose with $X inside").unwrap();
    let body = reading_of_kind(&body, FragmentKind::Body).unwrap();
    assert_eq!(body.fragment().kind(), SyntaxKind::DESCRIPTION);

    let section = Pattern::new(Style::NumPy, "Returns\n-------\n$TYPE").unwrap();
    assert_eq!(primary(&section).fragment_kind(), FragmentKind::Section);
    assert_eq!(primary(&section).fragment().kind(), SyntaxKind::SECTION);

    let document = Pattern::new(Style::Plain, "Just a summary.").unwrap();
    assert_eq!(primary(&document).fragment_kind(), FragmentKind::Document);
    assert_eq!(primary(&document).fragment().kind(), SyntaxKind::DOCUMENT);

    // And the contract holds across every reading of the law set.
    for pattern in pattern_test_set() {
        for (i, reading) in pattern.readings().iter().enumerate() {
            assert_fragment_kind_matches(reading, &format!("reading {i} of {:?}", pattern.text()));
        }
    }
}

/// LAW: for every reading of every pattern, each metavariable site path
/// resolves to an element of the reported kind, and exact sites cover that
/// element's range exactly.
#[test]
fn law_sites_resolve_in_every_reading() {
    for pattern in pattern_test_set() {
        for reading in pattern.readings() {
            for mv in reading.metavars() {
                let element = element_at(reading.parsed().root(), mv.site().path());
                assert_eq!(element.kind(), mv.site().kind(), "pattern {:?}", pattern.text());
                if mv.site().is_exact() {
                    assert_eq!(element.range(), mv.site().range(), "pattern {:?}", pattern.text());
                } else {
                    let (r, ph) = (element.range(), mv.site().range());
                    assert!(
                        r.start() <= ph.start() && ph.end() <= r.end(),
                        "inexact site outside its token, pattern {:?}",
                        pattern.text()
                    );
                }
            }
        }
    }
}

/// LAW: every reading's wrapped parse satisfies byte coverage over its
/// fragment — the tokens intersecting the fragment range tile it with no
/// gaps.
#[test]
fn law_fragment_byte_coverage_in_every_reading() {
    fn collect(node: &SyntaxNode, out: &mut Vec<(usize, usize)>) {
        for child in node.children() {
            match child {
                SyntaxElement::Node(n) => collect(n, out),
                SyntaxElement::Token(t) => {
                    out.push((usize::from(t.range().start()), usize::from(t.range().end())));
                }
            }
        }
    }

    for pattern in pattern_test_set() {
        for reading in pattern.readings() {
            let frag = reading.fragment().range();
            let (fs, fe) = (usize::from(frag.start()), usize::from(frag.end()));
            let mut tokens = Vec::new();
            collect(reading.parsed().root(), &mut tokens);
            tokens.retain(|&(s, e)| s < fe && e > fs);
            tokens.sort_unstable();
            let mut pos = fs;
            for (s, e) in tokens {
                assert!(
                    s <= pos,
                    "coverage gap at {pos}..{s} within fragment of pattern {:?}",
                    pattern.text()
                );
                pos = pos.max(e);
            }
            assert!(
                pos >= fe,
                "coverage gap at {pos}..{fe} within fragment of pattern {:?}",
                pattern.text()
            );
        }
    }
}
