//! Spec + contract tests for section recognition: header aliases, case
//! insensitivity, ordering, deprecation directive, stray lines, spans.
//! Exhaustive input coverage lives in tests/corpus/numpy/ + tests/snapshots.rs;
//! these tests pin deliberate spec decisions and the typed-accessor contract.

use super::*;

// =============================================================================
// Section header alias → kind mapping (spec table)
// =============================================================================

/// SPEC: full table of accepted section header spellings and the
/// SectionKind each maps to (parsed end-to-end through the parser).
#[test]
fn test_section_header_alias_table() {
    #[rustfmt::skip]
    let cases: &[(&str, SectionKind)] = &[
        // Parameters and aliases (incl. Arguments family)
        ("Parameters", SectionKind::Parameters),
        ("Parameter", SectionKind::Parameters),
        ("Params", SectionKind::Parameters),
        ("Param", SectionKind::Parameters),
        ("Arguments", SectionKind::Parameters),
        ("Argument", SectionKind::Parameters),
        ("Args", SectionKind::Parameters),
        ("Arg", SectionKind::Parameters),
        // Other Parameters and aliases
        ("Other Parameters", SectionKind::OtherParameters),
        ("Other Parameter", SectionKind::OtherParameters),
        ("Other Params", SectionKind::OtherParameters),
        ("Other Param", SectionKind::OtherParameters),
        ("Other Arguments", SectionKind::OtherParameters),
        ("Other Argument", SectionKind::OtherParameters),
        ("Other Args", SectionKind::OtherParameters),
        ("Other Arg", SectionKind::OtherParameters),
        // Returns / Yields / Receives
        ("Returns", SectionKind::Returns),
        ("Return", SectionKind::Returns),
        ("Yields", SectionKind::Yields),
        ("Yield", SectionKind::Yields),
        ("Receives", SectionKind::Receives),
        ("Receive", SectionKind::Receives),
        // Raises / Warns / Warnings
        ("Raises", SectionKind::Raises),
        ("Raise", SectionKind::Raises),
        ("Warns", SectionKind::Warns),
        ("Warn", SectionKind::Warns),
        ("Warnings", SectionKind::FreeText(FreeSectionKind::Warnings)),
        ("Warning", SectionKind::FreeText(FreeSectionKind::Warnings)),
        // Free-text and item sections
        ("See Also", SectionKind::SeeAlso),
        ("Notes", SectionKind::FreeText(FreeSectionKind::Notes)),
        ("Note", SectionKind::FreeText(FreeSectionKind::Notes)),
        ("References", SectionKind::References),
        ("Reference", SectionKind::References),
        ("Examples", SectionKind::FreeText(FreeSectionKind::Examples)),
        ("Example", SectionKind::FreeText(FreeSectionKind::Examples)),
        // Class sections
        ("Attributes", SectionKind::Attributes),
        ("Attribute", SectionKind::Attributes),
        ("Methods", SectionKind::Methods),
        ("Method", SectionKind::Methods),
        // Keyword parameters family (recognized for cross-style round trips, #53)
        ("Keyword Parameters", SectionKind::KeywordParameters),
        ("Keyword Parameter", SectionKind::KeywordParameters),
        ("Keyword Params", SectionKind::KeywordParameters),
        ("Keyword Param", SectionKind::KeywordParameters),
        ("Keyword Arguments", SectionKind::KeywordParameters),
        ("Keyword Argument", SectionKind::KeywordParameters),
        ("Keyword Args", SectionKind::KeywordParameters),
        ("Keyword Arg", SectionKind::KeywordParameters),
        // Admonition free-text sections (recognized for cross-style round trips, #52)
        ("Todo", SectionKind::FreeText(FreeSectionKind::Todo)),
        ("Attention", SectionKind::FreeText(FreeSectionKind::Attention)),
        ("Caution", SectionKind::FreeText(FreeSectionKind::Caution)),
        ("Danger", SectionKind::FreeText(FreeSectionKind::Danger)),
        ("Error", SectionKind::FreeText(FreeSectionKind::Error)),
        ("Hint", SectionKind::FreeText(FreeSectionKind::Hint)),
        ("Important", SectionKind::FreeText(FreeSectionKind::Important)),
        ("Tip", SectionKind::FreeText(FreeSectionKind::Tip)),
    ];

    for (header, expected) in cases {
        let underline = "-".repeat(header.len());
        let docstring = format!("Summary.\n\n{header}\n{underline}\nx : int\n    d.\n");
        let result = parse_numpy(&docstring);
        let sections = all_sections(&result);
        assert_eq!(sections.len(), 1, "header {header:?} should start a section");
        assert_eq!(sections[0].header_name(), *header);
        assert_eq!(&sections[0].kind(), expected, "header {header:?}");
    }
}

// =============================================================================
// Case insensitive sections
// =============================================================================

/// SPEC: section headers are matched case-insensitively; the header token
/// preserves the original spelling.
#[test]
fn test_case_insensitive_sections() {
    let docstring = r#"Brief summary.

parameters
----------
x : int
    First param.

returns
-------
int
    The result.

NOTES
-----
Some notes here.
"#;
    let result = parse_numpy(docstring);
    assert_eq!(parameters(&result).len(), 1);
    let names: Vec<_> = parameters(&result)[0].names().collect();
    assert_eq!(names[0].text(), "x");
    assert_eq!(returns(&result).len(), 1);
    assert!(notes(&result).is_some());
    assert_eq!(all_sections(&result)[0].header_name(), "parameters");
    assert_eq!(all_sections(&result)[2].header_name(), "NOTES");
}

// =============================================================================
// Span round-trip
// =============================================================================

/// CONTRACT: token spans (summary, header name/underline, parameter fields)
/// slice the original source back out.
#[test]
fn test_span_source_text_round_trip() {
    let docstring = r#"Summary line.

Parameters
----------
x : int
    Description of x.
"#;
    let result = parse_numpy(docstring);

    assert_eq!(doc(&result).summary().unwrap().text(), "Summary line.");
    assert_eq!(all_sections(&result)[0].header_name(), "Parameters");
    let underline = header_underline(&result, &all_sections(&result)[0]);
    assert!(underline.chars().all(|c| c == '-'));

    let p = &parameters(&result)[0];
    let names: Vec<_> = p.names().collect();
    assert_eq!(names[0].text(), "x");
    assert_eq!(p.type_annotation().unwrap().text(), "int");
    assert_eq!(p.description().unwrap().text(), "Description of x.");
}

// =============================================================================
// Deprecation
// =============================================================================

/// SPEC: `.. deprecated:: <version>` directive is recognized before sections.
/// Also CONTRACT for Directive accessors (name / argument / description).
#[test]
fn test_deprecation_directive() {
    let docstring = r#"Summary.

.. deprecated:: 1.6.0
    Use `new_func` instead.

Parameters
----------
x : int
    Desc.
"#;
    let result = parse_numpy(docstring);
    let dep: Directive = doc(&result)
        .directives()
        .find(|d| d.name().text() == "deprecated")
        .expect("deprecation should be parsed");
    assert_eq!(dep.argument().unwrap().text(), "1.6.0");
    assert_eq!(dep.description().unwrap().text(), "Use `new_func` instead.");
    // The `..` directive marker is raw-tree punctuation.
    assert_eq!(
        dep.syntax()
            .find_token(SyntaxKind::DIRECTIVE_MARKER)
            .map(|t| t.text(result.source())),
        Some("..")
    );
}

// =============================================================================
// Section ordering
// =============================================================================

/// CONTRACT: `sections()` yields sections in source order.
#[test]
fn test_section_order_preserved() {
    let docstring = r#"Summary.

Parameters
----------
x : int
    Desc.

Returns
-------
int
    Result.

Raises
------
ValueError
    Bad input.

Notes
-----
Some notes.
"#;
    let result = parse_numpy(docstring);
    let s = all_sections(&result);
    assert_eq!(s.len(), 4);
    assert_eq!(s[0].kind(), SectionKind::Parameters);
    assert_eq!(s[1].kind(), SectionKind::Returns);
    assert_eq!(s[2].kind(), SectionKind::Raises);
    assert_eq!(s[3].kind(), SectionKind::FreeText(FreeSectionKind::Notes));
}

// =============================================================================
// Section-kind table (the NumPy name↔kind mapping, read through the public API)
// =============================================================================

/// CONTRACT: the NumPy style knows exactly 23 section kinds, and every one of
/// them resolves to a distinct kind that is never `FreeText(Unknown(_))`
/// (the old `NumPySectionKind::ALL` invariant, observed end-to-end).
#[test]
fn test_all_section_kinds_exist() {
    /// The canonical spelling of each of the 23 known NumPy section kinds.
    #[rustfmt::skip]
    const CANONICAL: &[&str] = &[
        "Parameters", "Returns", "Yields", "Receives", "Other Parameters",
        "Keyword Parameters", "Raises", "Warns", "Warnings", "See Also",
        "Notes", "References", "Examples", "Attributes", "Methods", "Todo",
        "Attention", "Caution", "Danger", "Error", "Hint", "Important", "Tip",
    ];
    assert_eq!(CANONICAL.len(), 23);

    let mut kinds = std::collections::HashSet::new();
    for header in CANONICAL {
        let underline = "-".repeat(header.len());
        let docstring = format!("Summary.\n\n{header}\n{underline}\nx : int\n    d.\n");
        let result = parse_numpy(&docstring);
        let kind = all_sections(&result)[0].kind();
        assert!(
            !matches!(kind, SectionKind::FreeText(FreeSectionKind::Unknown(_))),
            "header {header:?} must map to a known kind"
        );
        assert!(kinds.insert(kind), "header {header:?} must map to a distinct kind");
    }
    assert_eq!(kinds.len(), 23);
}

/// CONTRACT: a *registered* custom name yields `FreeText(Unknown(name))`,
/// carrying the header text as written; unregistered unknown names are not
/// headers at all (napoleon's line, #147); a known name (matched
/// case-insensitively) yields its kind.
#[test]
fn test_section_kind_from_name_unknown() {
    let src = "Summary.\n\nnonexistent\n-----------\nBody.\n";

    // Strict default: no section — the dash run underlines nothing known.
    assert_eq!(all_sections(&parse_numpy(src)).len(), 0);

    let opts = ParseOptions::new().with_custom_sections(["nonexistent"]);
    let result = parse_numpy_with(src, &opts);
    assert_eq!(
        all_sections(&result)[0].kind(),
        SectionKind::FreeText(FreeSectionKind::Unknown("nonexistent".to_owned()))
    );

    let result = parse_numpy("Summary.\n\nparameters\n----------\nx : int\n    d.\n");
    assert_eq!(all_sections(&result)[0].kind(), SectionKind::Parameters);
}

// =============================================================================
// Stray lines
// =============================================================================

/// SPEC: a non-section line before the first section does not prevent later
/// sections from being parsed.
#[test]
fn test_stray_lines() {
    let docstring = "Summary.\n\nThis line is not a section.\n\nParameters\n----------\nx : int\n    Desc.\n";
    let result = parse_numpy(docstring);
    // The non-section line might be treated as extended summary or stray line
    // depending on parser behavior. Just verify parameters are still parsed.
    assert_eq!(parameters(&result).len(), 1);
}

/// SPEC (documented limitation): in NumPy style, entries and stray lines sit at
/// the same indentation level, so a stray line between sections is absorbed
/// into the preceding section. Sections end only at the next header+underline.
#[test]
fn test_stray_line_between_sections() {
    let input = "Summary.\n\nParameters\n----------\na : int\n    desc.\n\nstray line 1\n\nReturns\n-------\nbool\n    desc\n\nstray line 2\n";
    let result = parse_numpy(input);
    // Returns is still parsed (it has a proper header+underline).
    let r = returns(&result);
    assert!(!r.is_empty(), "Returns section must be parsed");
    // Stray lines are absorbed, never dropped into new sections.
    let s = all_sections(&result);
    assert_eq!(s.len(), 2, "stray lines must not start new sections");
    assert_eq!(s[0].kind(), SectionKind::Parameters);
    assert_eq!(s[1].kind(), SectionKind::Returns);
}

// =============================================================================
// Canonical header spelling of each kind
// =============================================================================

/// CONTRACT: the canonical NumPy header spelling of each section kind — the
/// name the (now crate-private) `NumPySectionKind: Display` impl produced.
/// The same name table is observable through the public NumPy emitter, so the
/// spelling of every kind is pinned there.
#[test]
fn test_section_kind_display() {
    let cases: &[(SectionKind, &str)] = &[
        (SectionKind::Parameters, "Parameters"),
        (SectionKind::Returns, "Returns"),
        (SectionKind::Yields, "Yields"),
        (SectionKind::Receives, "Receives"),
        (SectionKind::OtherParameters, "Other Parameters"),
        (SectionKind::Raises, "Raises"),
        (SectionKind::Warns, "Warns"),
        (SectionKind::FreeText(FreeSectionKind::Warnings), "Warnings"),
        (SectionKind::SeeAlso, "See Also"),
        (SectionKind::FreeText(FreeSectionKind::Notes), "Notes"),
        (SectionKind::References, "References"),
        (SectionKind::FreeText(FreeSectionKind::Examples), "Examples"),
        (SectionKind::Attributes, "Attributes"),
        (SectionKind::Methods, "Methods"),
        // `NumPySectionKind::Unknown` displayed as the literal "Unknown"; the
        // model's Unknown carries the header text as written, so its canonical
        // spelling is that text.
        (
            SectionKind::FreeText(FreeSectionKind::Unknown("Unknown".to_owned())),
            "Unknown",
        ),
    ];

    for (kind, name) in cases {
        let docstring = Docstring {
            sections: vec![ModelSection::new(kind.clone(), vec![])],
            ..Default::default()
        };
        let emitted = emit_numpy(&docstring, &EmitOptions::default());
        let underline = "-".repeat(name.chars().count());
        assert!(
            emitted.contains(&format!("{name}\n{underline}\n")),
            "kind {kind:?} should emit header {name:?}, got {emitted:?}"
        );
    }
}

// =============================================================================
// SPEC: entry accessors are guarded by the section's kind (#77 review)
// =============================================================================

/// SPEC: all entries share the `ENTRY` node kind, so an entry's role comes
/// solely from its section's kind: selecting entries by any other role's
/// section kind (`parameters()` on a Raises-only docstring) must return empty
/// instead of yielding the foreign entries — pre-unification behavior — and
/// reading those entries through the typed accessors must not panic.
#[test]
fn spec_mismatched_entry_accessor_returns_empty() {
    let docstring = "Summary.\n\nRaises\n------\nValueError\n    If the value is bad.\n";
    let result = parse_numpy(docstring);
    let sections = all_sections(&result);
    let section = &sections[0];
    assert_eq!(section.kind(), SectionKind::Raises);

    // The matching role sees the entry…
    assert_eq!(section.entries().count(), 1);
    assert_eq!(raises(&result).len(), 1);

    // …every mismatched role selects nothing (a foreign entry leaking through
    // would show up as a type/name-less entry in the wrong role).
    assert_eq!(parameters(&result).len(), 0);
    assert_eq!(returns(&result).len(), 0);
    assert_eq!(yields(&result).len(), 0);
    assert_eq!(warns(&result).len(), 0);
    assert_eq!(see_also(&result).len(), 0);
    assert_eq!(attributes(&result).len(), 0);
    assert_eq!(methods(&result).len(), 0);
    assert_eq!(references(&result).len(), 0);

    // Reading the exception entry through the unified accessors does not panic:
    // an exception carries a TYPE and no NAME.
    let exc = &raises(&result)[0];
    assert!(exc.name().is_none());
    assert_eq!(exc.type_annotation().unwrap().text(), "ValueError");
    assert_eq!(exc.description().unwrap().text(), "If the value is bad.");

    // And the kind also separates the NAME-carrying roles from each other:
    // attributes on a Parameters-only docstring is empty.
    let result = parse_numpy("Summary.\n\nParameters\n----------\nx : int\n    The value.\n");
    assert_eq!(parameters(&result).len(), 1);
    assert_eq!(attributes(&result).len(), 0);
    assert_eq!(methods(&result).len(), 0);
}
