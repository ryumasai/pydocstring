//! Pattern fragments (#45): metavariable lexing, sub-grammar parsing,
//! landing-site introspection, ambiguity behaviour, and the fragment
//! byte-coverage law.

use pydocstring::model::FreeSectionKind;
use pydocstring::model::SectionKind;
use pydocstring::parse::Style;
use pydocstring::pattern::FragmentKind;
use pydocstring::pattern::Pattern;
use pydocstring::pattern::PatternContext;
use pydocstring::pattern::PatternError;
use pydocstring::pattern::PatternOptions;
use pydocstring::syntax::SyntaxElement;
use pydocstring::syntax::SyntaxKind;
use pydocstring::syntax::SyntaxNode;

/// `(name, multi, site kind, site parent kind, exact)` for every metavar.
fn var_summaries(pattern: &Pattern) -> Vec<(String, bool, SyntaxKind, SyntaxKind, bool)> {
    pattern
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

/// Shorthand: parse with a forced [`PatternContext`].
fn in_context(style: Style, context: PatternContext, text: &str) -> Result<Pattern, PatternError> {
    Pattern::new_with(style, text, &PatternOptions::default().with_context(context))
}

/// Shorthand: force the body-content reading under a section of `kind`.
fn in_section(style: Style, kind: SectionKind, text: &str) -> Result<Pattern, PatternError> {
    in_context(style, PatternContext::InSection(kind), text)
}

/// The observable polymorphism contract: `fragment_kind()` unambiguously
/// reports what `fragment()` returns.
fn assert_fragment_kind_matches(p: &Pattern) {
    let node_kind = p.fragment().kind();
    let matches = match p.fragment_kind() {
        FragmentKind::Entry => matches!(node_kind, SyntaxKind::ENTRY | SyntaxKind::CITATION),
        FragmentKind::Body => node_kind == SyntaxKind::DESCRIPTION,
        FragmentKind::Section => node_kind == SyntaxKind::SECTION,
        FragmentKind::Document => node_kind == SyntaxKind::DOCUMENT,
        other => panic!("unknown fragment kind {other:?}"),
    };
    assert!(
        matches,
        "fragment_kind() {:?} does not describe fragment() node kind {node_kind} for pattern {:?}",
        p.fragment_kind(),
        p.text()
    );
}

/// Shorthand: `Auto` context with the strict ambiguity policy.
fn strict(style: Style, text: &str) -> Result<Pattern, PatternError> {
    Pattern::new_with(style, text, &PatternOptions::default().with_strict(true))
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
// Metavariable lexing
// =============================================================================

/// `$NAME` and `$$$NAME` are recognised and inventoried in source order.
#[test]
fn metavar_lexing_single_and_multi() {
    let p = in_section(Style::Google, SectionKind::Parameters, "$NAME ($TYPE): $$$DESC").unwrap();
    let vars = var_summaries(&p);
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
    let p = in_section(Style::Google, SectionKind::Parameters, "$x ($3): a$B and $$D").unwrap();
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
    let p = in_section(Style::Google, SectionKind::Parameters, "$A: $A").unwrap();
    let vars = var_summaries(&p);
    assert_eq!(vars.len(), 2);
    assert_eq!(vars[0].0, "A");
    assert_eq!(vars[1].0, "A");
    // Two different landing sites for the two occurrences.
    assert_eq!(vars[0].2, SyntaxKind::NAME);
    assert_eq!(vars[1].2, SyntaxKind::TEXT_LINE);
    assert_ne!(p.metavars()[0].site().range(), p.metavars()[1].site().range());
}

// =============================================================================
// Entry fragments per style
// =============================================================================

/// The canonical RFC example: the Google bracket entry realises every
/// metavariable exactly, so the parameter-family roles collapse and the
/// pattern is NOT ambiguous.
#[test]
fn google_bracket_entry_pattern() {
    let p = Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap();
    assert_eq!(p.style(), Style::Google);
    assert_eq!(p.fragment_kind(), FragmentKind::Entry);
    assert_eq!(p.section_kind(), Some(&SectionKind::Parameters));
    assert_eq!(p.fragment().kind(), SyntaxKind::ENTRY);
    assert_eq!(
        var_summaries(&p),
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
}

/// `$NAME: $DESC` (Google) is genuinely ambiguous (the parameter family
/// reads NAME while Returns/Raises read TYPE — different shapes): `new`
/// resolves it to Parameters by priority; strict mode fails fast.
#[test]
fn google_colon_entry_resolves_by_priority() {
    let p = Pattern::new(Style::Google, "$NAME: $DESC").unwrap();
    assert_eq!(p.fragment_kind(), FragmentKind::Entry);
    assert_eq!(p.section_kind(), Some(&SectionKind::Parameters));
    assert_eq!(var_summaries(&p)[0].2, SyntaxKind::NAME);

    let err = strict(Style::Google, "$NAME: $DESC").unwrap_err();
    let PatternError::Ambiguous { candidates, .. } = &err else {
        panic!("expected Ambiguous, got {err:?}");
    };
    assert!(candidates.contains(&SectionKind::Parameters));
    assert!(candidates.contains(&SectionKind::Returns));
    assert!(err.to_string().contains("PatternContext::InSection"));
}

/// `$NAME : $TYPE` (NumPy) is ambiguous (parameters/returns vs raises vs
/// methods read the two slots differently): `new` resolves it to Parameters
/// by priority, strict mode reports the tie, and PatternContext::InSection forces a role
/// (the description line lands as a TEXT_LINE inside the DESCRIPTION).
#[test]
fn numpy_entry_pattern_priority_strict_and_forced() {
    let p = Pattern::new(Style::NumPy, "$NAME : $TYPE").unwrap();
    assert_eq!(p.section_kind(), Some(&SectionKind::Parameters));

    let err = strict(Style::NumPy, "$NAME : $TYPE").unwrap_err();
    let PatternError::Ambiguous { candidates, .. } = err else {
        panic!("expected Ambiguous");
    };
    assert!(candidates.contains(&SectionKind::Parameters));
    assert!(candidates.contains(&SectionKind::Raises));

    let p = in_section(Style::NumPy, SectionKind::Parameters, "$NAME : $TYPE\n    $DESC").unwrap();
    assert_eq!(p.fragment_kind(), FragmentKind::Entry);
    assert_eq!(p.fragment().kind(), SyntaxKind::ENTRY);
    assert_eq!(
        var_summaries(&p),
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
}

/// NumPy marker suffixes still parse around a `$TYPE` metavariable.
#[test]
fn numpy_entry_pattern_with_optional_marker() {
    let p = in_section(Style::NumPy, SectionKind::Parameters, "$NAME : $TYPE, optional").unwrap();
    assert_eq!(
        var_summaries(&p),
        vec![
            ("NAME".to_owned(), false, SyntaxKind::NAME, SyntaxKind::ENTRY, true),
            ("TYPE".to_owned(), false, SyntaxKind::TYPE, SyntaxKind::ENTRY, true),
        ]
    );
    assert!(p.fragment().find_token(SyntaxKind::OPTIONAL).is_some());
}

/// PatternContext::InSection forces the role: the same text lands as NAME
/// under Parameters and as TYPE under Raises.
#[test]
fn in_section_forces_the_role() {
    let param = in_section(Style::Google, SectionKind::Parameters, "$NAME: $DESC").unwrap();
    assert_eq!(var_summaries(&param)[0].2, SyntaxKind::NAME);
    assert_eq!(param.section_kind(), Some(&SectionKind::Parameters));

    let raises = in_section(Style::Google, SectionKind::Raises, "$NAME: $DESC").unwrap();
    assert_eq!(var_summaries(&raises)[0].2, SyntaxKind::TYPE);
    assert_eq!(raises.section_kind(), Some(&SectionKind::Raises));
}

/// References body content is a CITATION node; PatternContext::InSection
/// supports it.
#[test]
fn in_section_references_citation() {
    let p = in_section(Style::Google, SectionKind::References, ".. [$LABEL] $TEXT").unwrap();
    assert_eq!(p.fragment_kind(), FragmentKind::Entry);
    assert_eq!(p.fragment().kind(), SyntaxKind::CITATION);
    assert_eq!(
        var_summaries(&p),
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
// Section fragments
// =============================================================================

/// A text starting with a recognisable Google header is a section fragment.
#[test]
fn google_section_pattern() {
    let p = Pattern::new(Style::Google, "Returns:\n    $TYPE: $DESC").unwrap();
    assert_eq!(p.fragment_kind(), FragmentKind::Section);
    assert_eq!(p.fragment().kind(), SyntaxKind::SECTION);
    assert_eq!(p.section_kind(), Some(&SectionKind::Returns));
    assert_eq!(
        var_summaries(&p),
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

/// The NumPy underline form is a section fragment too.
#[test]
fn numpy_section_pattern() {
    let p = Pattern::new(Style::NumPy, "Returns\n-------\n$TYPE\n    $DESC").unwrap();
    assert_eq!(p.fragment_kind(), FragmentKind::Section);
    assert_eq!(p.section_kind(), Some(&SectionKind::Returns));
    assert_eq!(
        var_summaries(&p),
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

// =============================================================================
// Document fragments
// =============================================================================

/// Summary + section: a document fragment; the fragment root is the DOCUMENT.
#[test]
fn document_pattern() {
    let p = Pattern::new(Style::Google, "$SUMMARY\n\nArgs:\n    $NAME: $DESC").unwrap();
    assert_eq!(p.fragment_kind(), FragmentKind::Document);
    assert_eq!(p.fragment().kind(), SyntaxKind::DOCUMENT);
    assert!(p.section_kind().is_none());
    assert_eq!(
        var_summaries(&p),
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
}

/// A metavariable amid prose binds a sub-line span: recorded with
/// `is_exact() == false` inside the TEXT_LINE token.
#[test]
fn document_pattern_inexact_prose_site() {
    let p = Pattern::new(Style::NumPy, "Summary of $X.\n\nParameters\n----------\nx : int\n    d").unwrap();
    assert_eq!(p.fragment_kind(), FragmentKind::Document);
    assert_eq!(
        var_summaries(&p),
        vec![("X".to_owned(), false, SyntaxKind::TEXT_LINE, SyntaxKind::SUMMARY, false)]
    );
}

/// Plain style has no sections: patterns are always document fragments.
#[test]
fn plain_style_document_pattern() {
    let p = Pattern::new(Style::Plain, "Just a summary.").unwrap();
    assert_eq!(p.fragment_kind(), FragmentKind::Document);
    assert!(p.metavars().is_empty());
}

// =============================================================================
// `$$$X` standalone lines — the discovered mapping
// =============================================================================

/// A standalone `$$$X` line in a structured section body lands as a whole
/// ENTRY, in every entry role and both styles.
#[test]
fn multi_standalone_line_lands_as_entry() {
    for (style, kind) in [
        (Style::Google, SectionKind::Parameters),
        (Style::Google, SectionKind::Returns),
        (Style::Google, SectionKind::Raises),
        (Style::NumPy, SectionKind::Parameters),
        (Style::NumPy, SectionKind::Returns),
    ] {
        let p = in_section(style, kind.clone(), "$$$ENTRIES").unwrap();
        let vars = var_summaries(&p);
        assert_eq!(
            vars,
            vec![("ENTRIES".to_owned(), true, SyntaxKind::ENTRY, SyntaxKind::SECTION, true)],
            "role {kind:?} in {style}"
        );
        // The site is the fragment root itself: the path resolves to the
        // very node fragment() returns (pointer identity), not merely a
        // node with the same range.
        let site = p.metavars()[0].site();
        assert_eq!(site.range(), *p.fragment().range());
        match element_at(p.parsed().root(), site.path()) {
            SyntaxElement::Node(n) => assert!(std::ptr::eq::<SyntaxNode>(n, p.fragment())),
            SyntaxElement::Token(_) => panic!("fragment site must be a node"),
        }
    }
}

/// In a free-text section body, `$$$X` lands as the whole DESCRIPTION; in a
/// document, as the SUMMARY / EXTENDED_SUMMARY block it forms.
#[test]
fn multi_standalone_line_in_freetext_and_document() {
    let notes = Pattern::new(Style::Google, "Notes:\n    $$$BODY").unwrap();
    assert_eq!(notes.fragment_kind(), FragmentKind::Section);
    assert_eq!(
        var_summaries(&notes),
        vec![(
            "BODY".to_owned(),
            true,
            SyntaxKind::DESCRIPTION,
            SyntaxKind::SECTION,
            true
        )]
    );

    let doc = Pattern::new(Style::Google, "Intro.\n\n$$$REST").unwrap();
    assert_eq!(doc.fragment_kind(), FragmentKind::Document);
    assert_eq!(
        var_summaries(&doc),
        vec![(
            "REST".to_owned(),
            true,
            SyntaxKind::EXTENDED_SUMMARY,
            SyntaxKind::DOCUMENT,
            true
        )]
    );
}

/// A bare `$$$X` is a valid lone entry of several roles with different
/// shapes: `new` resolves it to Parameters by priority; strict mode keeps
/// the fail-fast Ambiguous error.
#[test]
fn multi_bare_line_resolves_to_parameters() {
    let p = Pattern::new(Style::Google, "$$$X").unwrap();
    assert_eq!(p.fragment_kind(), FragmentKind::Entry);
    assert_eq!(p.section_kind(), Some(&SectionKind::Parameters));
    assert_eq!(
        var_summaries(&p),
        vec![("X".to_owned(), true, SyntaxKind::ENTRY, SyntaxKind::SECTION, true)]
    );

    assert!(matches!(
        strict(Style::Google, "$$$X"),
        Err(PatternError::Ambiguous { .. })
    ));
}

/// `$$$X` in an entry's description slot binds the whole DESCRIPTION node
/// (where `$X` in the same spot binds the single TEXT_LINE).
#[test]
fn multi_in_description_slot_binds_description_node() {
    let multi = in_section(Style::Google, SectionKind::Parameters, "$NAME ($TYPE): $$$DESC").unwrap();
    assert_eq!(var_summaries(&multi)[2].2, SyntaxKind::DESCRIPTION);

    let single = in_section(Style::Google, SectionKind::Parameters, "$NAME ($TYPE): $DESC").unwrap();
    assert_eq!(var_summaries(&single)[2].2, SyntaxKind::TEXT_LINE);
}

// =============================================================================
// Forced contexts: Section / Document, and the strict no-op
// =============================================================================

/// PatternContext::Section accepts a section text and rejects an entry text.
#[test]
fn section_context_forces_section_reading() {
    let p = in_context(Style::Google, PatternContext::Section, "Returns:\n    $TYPE: $DESC").unwrap();
    assert_eq!(p.fragment_kind(), FragmentKind::Section);
    assert_eq!(p.section_kind(), Some(&SectionKind::Returns));

    let err = in_context(Style::Google, PatternContext::Section, "$NAME ($TYPE): $DESC").unwrap_err();
    let PatternError::Unparsable { message, .. } = err else {
        panic!("expected Unparsable");
    };
    assert!(message.contains("single section"), "{message}");
}

/// PatternContext::Document always reads (non-empty) text as a document —
/// even entry- or section-shaped text.
#[test]
fn document_context_forces_document_reading() {
    // Entry-shaped text: the whole line becomes summary prose, so every
    // metavariable is a sub-line (inexact) binding.
    let p = in_context(Style::Google, PatternContext::Document, "$NAME ($TYPE): $DESC").unwrap();
    assert_eq!(p.fragment_kind(), FragmentKind::Document);
    assert_eq!(p.fragment().kind(), SyntaxKind::DOCUMENT);
    assert!(p.metavars().iter().all(|m| !m.site().is_exact()));

    // Section-shaped text: forced Document keeps the DOCUMENT root as the
    // fragment; the sites still resolve structurally inside the section.
    let p = in_context(Style::Google, PatternContext::Document, "Returns:\n    $TYPE: $DESC").unwrap();
    assert_eq!(p.fragment_kind(), FragmentKind::Document);
    assert!(p.section_kind().is_none());
    assert_eq!(var_summaries(&p)[0].2, SyntaxKind::TYPE);

    // Empty text is still unparsable.
    assert!(matches!(
        in_context(Style::Google, PatternContext::Document, ""),
        Err(PatternError::Unparsable { .. })
    ));
}

/// The documented `x:` quirk, testable both ways: Auto reads it as a
/// section (Google header grammar accepts any `Word:` line); Entry forces
/// the entry reading.
#[test]
fn colon_quirk_resolves_both_ways() {
    let auto = Pattern::new(Style::Google, "x:").unwrap();
    assert_eq!(auto.fragment_kind(), FragmentKind::Section);

    let entry = in_section(Style::Google, SectionKind::Parameters, "x:").unwrap();
    assert_eq!(entry.fragment_kind(), FragmentKind::Entry);
    let name = entry.fragment().find_token(SyntaxKind::NAME).unwrap();
    assert_eq!(name.text(entry.parsed().source()), "x");
}

/// `strict` is a no-op for forced contexts: an ambiguous text forced into a
/// role parses fine with strict set.
#[test]
fn strict_is_a_noop_for_forced_contexts() {
    let options = PatternOptions::default()
        .with_context(PatternContext::InSection(SectionKind::Parameters))
        .with_strict(true);
    let p = Pattern::new_with(Style::Google, "$NAME: $DESC", &options).unwrap();
    assert_eq!(p.section_kind(), Some(&SectionKind::Parameters));
}

// =============================================================================
// SPEC: the ambiguity-resolution priority table (upgrade-stability pin)
// =============================================================================

/// SPEC: the full role priority table, pinned end-to-end. `$NAME: $DESC`
/// (Google) and `$NAME : $TYPE` (NumPy) are accepted by every trialled role
/// at equal rank, so strict mode's candidate list *is* the priority table —
/// changing the order is a breaking change and must fail this test.
#[test]
fn spec_priority_table_order() {
    let full_table = vec![
        SectionKind::Parameters,
        SectionKind::KeywordParameters,
        SectionKind::OtherParameters,
        SectionKind::Receives,
        SectionKind::Returns,
        SectionKind::Yields,
        SectionKind::Raises,
        SectionKind::Warns,
        SectionKind::Attributes,
        SectionKind::Methods,
        SectionKind::SeeAlso,
    ];
    for (style, text) in [(Style::Google, "$NAME: $DESC"), (Style::NumPy, "$NAME : $TYPE")] {
        let err = strict(style, text).unwrap_err();
        let PatternError::Ambiguous { candidates, .. } = err else {
            panic!("expected Ambiguous for {text:?} in {style}");
        };
        assert_eq!(candidates, full_table, "priority table changed for {text:?} in {style}");
    }
}

/// SPEC: deliberately-ambiguous patterns resolve to the documented pick.
/// This is the upgrade-stability pin: these resolutions may only change in
/// a breaking release.
#[test]
fn spec_priority_resolutions() {
    let cases: [(Style, &str, SectionKind); 5] = [
        // Every role accepts these; tier 1 (Parameters) wins.
        (Style::Google, "$NAME: $DESC", SectionKind::Parameters),
        (Style::NumPy, "$NAME : $TYPE", SectionKind::Parameters),
        (Style::Google, "$$$X", SectionKind::Parameters),
        (Style::Google, "just some literal words", SectionKind::Parameters),
        // The parameter family (and Raises/Methods/SeeAlso) lump `$X` into a
        // structural token here — invalid — so the prose-reading tier wins,
        // and within it Returns outranks Yields.
        (Style::Google, "words with $X inside", SectionKind::Returns),
    ];
    for (style, text, expected) in cases {
        let p = Pattern::new(style, text).unwrap();
        assert_eq!(
            p.fragment_kind(),
            FragmentKind::Entry,
            "expected entry pattern for {text:?} in {style}"
        );
        assert_eq!(
            p.section_kind(),
            Some(&expected),
            "resolution changed for {text:?} in {style}"
        );
    }

    // Returns and Yields read the prose case identically (same shape), so
    // even strict mode resolves it — strictness only guards *shape* ties.
    let strict = strict(Style::Google, "words with $X inside").unwrap();
    assert_eq!(strict.section_kind(), Some(&SectionKind::Returns));
}

// =============================================================================
// Errors: unparsable input, invalid sites, unsupported contexts
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

/// Garbage never panics. `new` resolves it (a bare line is a valid entry of
/// several roles, so priority picks one); strict mode reports the tie as
/// an error; a whitespace-only entry context is unparsable.
#[test]
fn garbage_is_an_error_not_a_panic() {
    let resolved = Pattern::new(Style::Google, "\u{0}\u{1}%%%???").unwrap();
    assert_eq!(resolved.section_kind(), Some(&SectionKind::Parameters));
    assert!(matches!(
        strict(Style::Google, "\u{0}\u{1}%%%???"),
        Err(PatternError::Ambiguous { .. })
    ));
    assert!(in_section(Style::NumPy, SectionKind::Parameters, "\n\n\n").is_err());
}

/// A structured section-body context demands exactly one entry.
#[test]
fn in_section_rejects_multiple_entries() {
    let err = in_section(Style::Google, SectionKind::Parameters, "a: b\nc: d").unwrap_err();
    let PatternError::Unparsable { message, .. } = err else {
        panic!("expected Unparsable");
    };
    assert!(message.contains("exactly one ENTRY"), "{message}");
}

/// A metavariable inside a structural token (here: inside a TYPE) is not a
/// bindable site.
#[test]
fn metavar_inside_structural_token_is_unparsable() {
    let err = in_section(Style::Google, SectionKind::Parameters, "x (Dict[$K, $V]): d").unwrap_err();
    let PatternError::Unparsable { message, .. } = err else {
        panic!("expected Unparsable");
    };
    assert!(message.contains("TYPE"), "{message}");
}

/// Plain style has no sections, so no section-body context exists for it.
#[test]
fn in_section_rejects_plain_style() {
    assert!(matches!(
        in_section(Style::Plain, SectionKind::Parameters, "$NAME: $DESC"),
        Err(PatternError::Unparsable { .. })
    ));
}

/// NEW with the parent framing: a free-text kind parses the text as the
/// section's prose body — the fragment is the DESCRIPTION block, and a
/// metavariable amid the prose is a sub-line (inexact) TEXT_LINE site.
#[test]
fn in_section_freetext_body() {
    let p = in_section(
        Style::Google,
        SectionKind::FreeText(FreeSectionKind::Notes),
        "prose with $X inside",
    )
    .unwrap();
    assert_eq!(p.fragment_kind(), FragmentKind::Body);
    assert_eq!(p.fragment().kind(), SyntaxKind::DESCRIPTION);
    assert_eq!(p.section_kind(), Some(&SectionKind::FreeText(FreeSectionKind::Notes)));
    assert_eq!(
        var_summaries(&p),
        vec![(
            "X".to_owned(),
            false,
            SyntaxKind::TEXT_LINE,
            SyntaxKind::DESCRIPTION,
            false
        )]
    );
}

/// A standalone `$$$X` in a free-text body binds the whole DESCRIPTION —
/// consistent with the discovered standalone-`$$$` mapping.
#[test]
fn in_section_freetext_multi_binds_whole_body() {
    for style in [Style::Google, Style::NumPy] {
        let p = in_section(style, SectionKind::FreeText(FreeSectionKind::Examples), "$$$BODY").unwrap();
        assert_eq!(p.fragment_kind(), FragmentKind::Body, "style {style}");
        assert_eq!(
            var_summaries(&p),
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
        assert_eq!(p.metavars()[0].site().range(), *p.fragment().range(), "style {style}");
    }
}

// =============================================================================
// Placeholder collision-proofing
// =============================================================================

/// Pattern text spelling out a placeholder name literally still works: the
/// stem is lengthened until it cannot collide, and the literal text survives
/// byte-for-byte.
#[test]
fn placeholder_collision_is_probed_away() {
    let p = in_section(Style::Google, SectionKind::Parameters, "PYDOCMV0X ($TYPE): $DESC").unwrap();
    let name = p.fragment().find_token(SyntaxKind::NAME).unwrap();
    assert_eq!(name.text(p.parsed().source()), "PYDOCMV0X");
    assert_eq!(
        var_summaries(&p),
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
// Laws: sites resolve, fragment byte coverage
// =============================================================================

fn pattern_test_set() -> Vec<Pattern> {
    vec![
        Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap(),
        Pattern::new(Style::Google, "Returns:\n    $TYPE: $DESC").unwrap(),
        Pattern::new(Style::Google, "Notes:\n    $$$BODY").unwrap(),
        Pattern::new(Style::Google, "$SUMMARY\n\nArgs:\n    $NAME: $DESC").unwrap(),
        Pattern::new(Style::NumPy, "Returns\n-------\n$TYPE\n    $DESC").unwrap(),
        Pattern::new(Style::NumPy, "Summary of $X.\n\nParameters\n----------\nx : int\n    d").unwrap(),
        Pattern::new(Style::Plain, "Just a summary.").unwrap(),
        in_section(Style::Google, SectionKind::Parameters, "$$$ENTRIES").unwrap(),
        in_section(Style::Google, SectionKind::Raises, "$NAME: $DESC").unwrap(),
        in_section(Style::Google, SectionKind::References, ".. [$LABEL] $TEXT").unwrap(),
        in_section(
            Style::NumPy,
            SectionKind::Parameters,
            "$NAME : $TYPE, optional\n    $DESC",
        )
        .unwrap(),
        in_section(Style::NumPy, SectionKind::KeywordParameters, "$NAME : $TYPE").unwrap(),
        in_section(Style::Google, SectionKind::Parameters, "PYDOCMV0X ($TYPE): $DESC").unwrap(),
        in_section(
            Style::Google,
            SectionKind::FreeText(FreeSectionKind::Notes),
            "prose with $X inside",
        )
        .unwrap(),
        in_section(
            Style::NumPy,
            SectionKind::FreeText(FreeSectionKind::Examples),
            "$$$BODY",
        )
        .unwrap(),
        in_context(Style::Google, PatternContext::Section, "Returns:\n    $TYPE: $DESC").unwrap(),
        in_context(Style::NumPy, PatternContext::Document, "$NAME ($TYPE): $DESC").unwrap(),
    ]
}

/// ACCEPTANCE: `fragment_kind()` unambiguously reports what `fragment()`
/// returns for EVERY `PatternContext` — the polymorphism is observable.
#[test]
fn fragment_kind_reports_fragment_for_every_context() {
    // One pattern per context (and per InSection body grammar), asserting
    // the exact kind pair.
    let cases: Vec<(Pattern, FragmentKind, SyntaxKind)> = vec![
        // Auto, each inferred reading.
        (
            Pattern::new(Style::Google, "$NAME ($TYPE): $DESC").unwrap(),
            FragmentKind::Entry,
            SyntaxKind::ENTRY,
        ),
        (
            Pattern::new(Style::Google, "Returns:\n    $TYPE: $DESC").unwrap(),
            FragmentKind::Section,
            SyntaxKind::SECTION,
        ),
        (
            Pattern::new(Style::Google, "$SUMMARY\n\nArgs:\n    $NAME: $DESC").unwrap(),
            FragmentKind::Document,
            SyntaxKind::DOCUMENT,
        ),
        // InSection, each body grammar.
        (
            in_section(Style::NumPy, SectionKind::Parameters, "$NAME : $TYPE").unwrap(),
            FragmentKind::Entry,
            SyntaxKind::ENTRY,
        ),
        (
            in_section(Style::Google, SectionKind::References, ".. [$LABEL] $TEXT").unwrap(),
            FragmentKind::Entry,
            SyntaxKind::CITATION,
        ),
        (
            in_section(Style::Google, SectionKind::FreeText(FreeSectionKind::Notes), "$$$BODY").unwrap(),
            FragmentKind::Body,
            SyntaxKind::DESCRIPTION,
        ),
        // Forced Section / Document.
        (
            in_context(Style::NumPy, PatternContext::Section, "Returns\n-------\n$TYPE").unwrap(),
            FragmentKind::Section,
            SyntaxKind::SECTION,
        ),
        (
            in_context(Style::Google, PatternContext::Document, "$NAME ($TYPE): $DESC").unwrap(),
            FragmentKind::Document,
            SyntaxKind::DOCUMENT,
        ),
    ];
    for (pattern, expected_kind, expected_node) in cases {
        assert_eq!(pattern.fragment_kind(), expected_kind, "pattern {:?}", pattern.text());
        assert_eq!(pattern.fragment().kind(), expected_node, "pattern {:?}", pattern.text());
        assert_fragment_kind_matches(&pattern);
    }
    // And the contract holds across the whole law set.
    for pattern in pattern_test_set() {
        assert_fragment_kind_matches(&pattern);
    }
}

/// LAW: every metavariable site path resolves to an element of the reported
/// kind, and exact sites cover that element's range exactly.
#[test]
fn law_sites_resolve_in_the_wrapped_tree() {
    for pattern in pattern_test_set() {
        for mv in pattern.metavars() {
            let element = element_at(pattern.parsed().root(), mv.site().path());
            assert_eq!(element.kind(), mv.site().kind(), "pattern {:?}", pattern.text());
            if mv.site().is_exact() {
                assert_eq!(*element.range(), mv.site().range(), "pattern {:?}", pattern.text());
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

/// LAW: the wrapped parse satisfies byte coverage over the fragment — the
/// tokens intersecting the fragment range tile it with no gaps.
#[test]
fn law_fragment_byte_coverage() {
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
        let frag = *pattern.fragment().range();
        let (fs, fe) = (usize::from(frag.start()), usize::from(frag.end()));
        let mut tokens = Vec::new();
        collect(pattern.parsed().root(), &mut tokens);
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
