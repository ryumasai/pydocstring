//! Pattern fragments (#45): metavariable lexing, sub-grammar parsing,
//! landing-site introspection, ambiguity behaviour, and the fragment
//! byte-coverage law.

use pydocstring::model::SectionKind;
use pydocstring::parse::Style;
use pydocstring::pattern::FragmentKind;
use pydocstring::pattern::Pattern;
use pydocstring::pattern::PatternError;
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
    let p = Pattern::in_section(Style::Google, SectionKind::Parameters, "$NAME ($TYPE): $$$DESC").unwrap();
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
    let p = Pattern::in_section(Style::Google, SectionKind::Parameters, "$x ($3): a$B and $$D").unwrap();
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
    let p = Pattern::in_section(Style::Google, SectionKind::Parameters, "$A: $A").unwrap();
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

/// `$NAME: $DESC` (Google) is genuinely ambiguous: the parameter family
/// reads NAME while Returns/Raises read TYPE — different shapes.
#[test]
fn google_colon_entry_is_ambiguous() {
    let err = Pattern::new(Style::Google, "$NAME: $DESC").unwrap_err();
    let PatternError::Ambiguous { candidates, .. } = &err else {
        panic!("expected Ambiguous, got {err:?}");
    };
    assert!(candidates.contains(&SectionKind::Parameters));
    assert!(candidates.contains(&SectionKind::Returns));
    assert!(err.to_string().contains("Pattern::in_section"));
}

/// `$NAME : $TYPE` (NumPy) is ambiguous (parameters/returns vs raises vs
/// methods read the two slots differently); in_section resolves it, and the
/// description line lands as a TEXT_LINE inside the entry's DESCRIPTION.
#[test]
fn numpy_entry_pattern_ambiguous_then_forced() {
    let err = Pattern::new(Style::NumPy, "$NAME : $TYPE").unwrap_err();
    let PatternError::Ambiguous { candidates, .. } = err else {
        panic!("expected Ambiguous");
    };
    assert!(candidates.contains(&SectionKind::Parameters));
    assert!(candidates.contains(&SectionKind::Raises));

    let p = Pattern::in_section(Style::NumPy, SectionKind::Parameters, "$NAME : $TYPE\n    $DESC").unwrap();
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
    let p = Pattern::in_section(Style::NumPy, SectionKind::Parameters, "$NAME : $TYPE, optional").unwrap();
    assert_eq!(
        var_summaries(&p),
        vec![
            ("NAME".to_owned(), false, SyntaxKind::NAME, SyntaxKind::ENTRY, true),
            ("TYPE".to_owned(), false, SyntaxKind::TYPE, SyntaxKind::ENTRY, true),
        ]
    );
    assert!(p.fragment().find_token(SyntaxKind::OPTIONAL).is_some());
}

/// in_section forces the role: the same text lands as NAME under Parameters
/// and as TYPE under Raises.
#[test]
fn in_section_forces_the_entry_role() {
    let param = Pattern::in_section(Style::Google, SectionKind::Parameters, "$NAME: $DESC").unwrap();
    assert_eq!(var_summaries(&param)[0].2, SyntaxKind::NAME);
    assert_eq!(param.section_kind(), Some(&SectionKind::Parameters));

    let raises = Pattern::in_section(Style::Google, SectionKind::Raises, "$NAME: $DESC").unwrap();
    assert_eq!(var_summaries(&raises)[0].2, SyntaxKind::TYPE);
    assert_eq!(raises.section_kind(), Some(&SectionKind::Raises));
}

/// References entries are CITATION nodes; in_section supports them.
#[test]
fn in_section_references_citation() {
    let p = Pattern::in_section(Style::Google, SectionKind::References, ".. [$LABEL] $TEXT").unwrap();
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
        let p = Pattern::in_section(style, kind.clone(), "$$$ENTRIES").unwrap();
        let vars = var_summaries(&p);
        assert_eq!(
            vars,
            vec![("ENTRIES".to_owned(), true, SyntaxKind::ENTRY, SyntaxKind::SECTION, true)],
            "role {kind:?} in {style}"
        );
        // The site is the fragment root itself.
        assert_eq!(p.metavars()[0].site().path(), {
            let frag_range = *p.fragment().range();
            assert_eq!(p.metavars()[0].site().range(), frag_range);
            p.metavars()[0].site().path()
        });
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

/// A bare `$$$X` via `new` is ambiguous (it is a valid lone entry of several
/// roles with different shapes) — the documented behaviour.
#[test]
fn multi_bare_line_new_is_ambiguous() {
    assert!(matches!(
        Pattern::new(Style::Google, "$$$X"),
        Err(PatternError::Ambiguous { .. })
    ));
}

/// `$$$X` in an entry's description slot binds the whole DESCRIPTION node
/// (where `$X` in the same spot binds the single TEXT_LINE).
#[test]
fn multi_in_description_slot_binds_description_node() {
    let multi = Pattern::in_section(Style::Google, SectionKind::Parameters, "$NAME ($TYPE): $$$DESC").unwrap();
    assert_eq!(var_summaries(&multi)[2].2, SyntaxKind::DESCRIPTION);

    let single = Pattern::in_section(Style::Google, SectionKind::Parameters, "$NAME ($TYPE): $DESC").unwrap();
    assert_eq!(var_summaries(&single)[2].2, SyntaxKind::TEXT_LINE);
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

/// Garbage never panics; it comes back as an error (here: ambiguous literal
/// text, since a bare line is a valid entry of several roles).
#[test]
fn garbage_is_an_error_not_a_panic() {
    assert!(Pattern::new(Style::Google, "\u{0}\u{1}%%%???").is_err());
    assert!(Pattern::in_section(Style::NumPy, SectionKind::Parameters, "\n\n\n").is_err());
}

/// An entry context demands exactly one entry.
#[test]
fn in_section_rejects_multiple_entries() {
    let err = Pattern::in_section(Style::Google, SectionKind::Parameters, "a: b\nc: d").unwrap_err();
    let PatternError::Unparsable { message, .. } = err else {
        panic!("expected Unparsable");
    };
    assert!(message.contains("exactly one entry"), "{message}");
}

/// A metavariable inside a structural token (here: inside a TYPE) is not a
/// bindable site.
#[test]
fn metavar_inside_structural_token_is_unparsable() {
    let err = Pattern::in_section(Style::Google, SectionKind::Parameters, "x (Dict[$K, $V]): d").unwrap_err();
    let PatternError::Unparsable { message, .. } = err else {
        panic!("expected Unparsable");
    };
    assert!(message.contains("TYPE"), "{message}");
}

/// Free-text section kinds have no entries; plain style has no sections.
#[test]
fn in_section_rejects_freetext_kind_and_plain_style() {
    use pydocstring::model::FreeSectionKind;
    assert!(matches!(
        Pattern::in_section(Style::Google, SectionKind::FreeText(FreeSectionKind::Notes), "$$$BODY"),
        Err(PatternError::Unparsable { .. })
    ));
    assert!(matches!(
        Pattern::in_section(Style::Plain, SectionKind::Parameters, "$NAME: $DESC"),
        Err(PatternError::Unparsable { .. })
    ));
}

// =============================================================================
// Placeholder collision-proofing
// =============================================================================

/// Pattern text spelling out a placeholder name literally still works: the
/// stem is lengthened until it cannot collide, and the literal text survives
/// byte-for-byte.
#[test]
fn placeholder_collision_is_probed_away() {
    let p = Pattern::in_section(Style::Google, SectionKind::Parameters, "PYDOCMV0X ($TYPE): $DESC").unwrap();
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
        Pattern::in_section(Style::Google, SectionKind::Parameters, "$$$ENTRIES").unwrap(),
        Pattern::in_section(Style::Google, SectionKind::Raises, "$NAME: $DESC").unwrap(),
        Pattern::in_section(Style::Google, SectionKind::References, ".. [$LABEL] $TEXT").unwrap(),
        Pattern::in_section(
            Style::NumPy,
            SectionKind::Parameters,
            "$NAME : $TYPE, optional\n    $DESC",
        )
        .unwrap(),
        Pattern::in_section(Style::NumPy, SectionKind::KeywordParameters, "$NAME : $TYPE").unwrap(),
        Pattern::in_section(Style::Google, SectionKind::Parameters, "PYDOCMV0X ($TYPE): $DESC").unwrap(),
    ]
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
