//! Spec + contract tests for section recognition: header aliases, case
//! insensitivity, ordering, deprecation directive, stray lines, spans.
//! Exhaustive input coverage lives in tests/corpus/numpy/ + tests/snapshots.rs;
//! these tests pin deliberate spec decisions and the typed-accessor contract.

use super::*;

// =============================================================================
// Section header alias → kind mapping (spec table)
// =============================================================================

/// SPEC: full table of accepted section header spellings and the
/// NumPySectionKind each maps to (parsed end-to-end through the parser).
#[test]
fn test_section_header_alias_table() {
    #[rustfmt::skip]
    let cases: &[(&str, NumPySectionKind)] = &[
        // Parameters and aliases (incl. Arguments family)
        ("Parameters", NumPySectionKind::Parameters),
        ("Parameter", NumPySectionKind::Parameters),
        ("Params", NumPySectionKind::Parameters),
        ("Param", NumPySectionKind::Parameters),
        ("Arguments", NumPySectionKind::Parameters),
        ("Argument", NumPySectionKind::Parameters),
        ("Args", NumPySectionKind::Parameters),
        ("Arg", NumPySectionKind::Parameters),
        // Other Parameters and aliases
        ("Other Parameters", NumPySectionKind::OtherParameters),
        ("Other Parameter", NumPySectionKind::OtherParameters),
        ("Other Params", NumPySectionKind::OtherParameters),
        ("Other Param", NumPySectionKind::OtherParameters),
        ("Other Arguments", NumPySectionKind::OtherParameters),
        ("Other Argument", NumPySectionKind::OtherParameters),
        ("Other Args", NumPySectionKind::OtherParameters),
        ("Other Arg", NumPySectionKind::OtherParameters),
        // Returns / Yields / Receives
        ("Returns", NumPySectionKind::Returns),
        ("Return", NumPySectionKind::Returns),
        ("Yields", NumPySectionKind::Yields),
        ("Yield", NumPySectionKind::Yields),
        ("Receives", NumPySectionKind::Receives),
        ("Receive", NumPySectionKind::Receives),
        // Raises / Warns / Warnings
        ("Raises", NumPySectionKind::Raises),
        ("Raise", NumPySectionKind::Raises),
        ("Warns", NumPySectionKind::Warns),
        ("Warn", NumPySectionKind::Warns),
        ("Warnings", NumPySectionKind::Warnings),
        ("Warning", NumPySectionKind::Warnings),
        // Free-text and item sections
        ("See Also", NumPySectionKind::SeeAlso),
        ("Notes", NumPySectionKind::Notes),
        ("Note", NumPySectionKind::Notes),
        ("References", NumPySectionKind::References),
        ("Reference", NumPySectionKind::References),
        ("Examples", NumPySectionKind::Examples),
        ("Example", NumPySectionKind::Examples),
        // Class sections
        ("Attributes", NumPySectionKind::Attributes),
        ("Attribute", NumPySectionKind::Attributes),
        ("Methods", NumPySectionKind::Methods),
        ("Method", NumPySectionKind::Methods),
        // Keyword parameters family (recognized for cross-style round trips, #53)
        ("Keyword Parameters", NumPySectionKind::KeywordParameters),
        ("Keyword Parameter", NumPySectionKind::KeywordParameters),
        ("Keyword Params", NumPySectionKind::KeywordParameters),
        ("Keyword Param", NumPySectionKind::KeywordParameters),
        ("Keyword Arguments", NumPySectionKind::KeywordParameters),
        ("Keyword Argument", NumPySectionKind::KeywordParameters),
        ("Keyword Args", NumPySectionKind::KeywordParameters),
        ("Keyword Arg", NumPySectionKind::KeywordParameters),
        // Admonition free-text sections (recognized for cross-style round trips, #52)
        ("Todo", NumPySectionKind::Todo),
        ("Attention", NumPySectionKind::Attention),
        ("Caution", NumPySectionKind::Caution),
        ("Danger", NumPySectionKind::Danger),
        ("Error", NumPySectionKind::Error),
        ("Hint", NumPySectionKind::Hint),
        ("Important", NumPySectionKind::Important),
        ("Tip", NumPySectionKind::Tip),
    ];

    for (header, expected) in cases {
        let underline = "-".repeat(header.len());
        let docstring = format!("Summary.\n\n{header}\n{underline}\nx : int\n    d.\n");
        let result = parse_numpy(&docstring);
        let sections = all_sections(&result);
        assert_eq!(sections.len(), 1, "header {header:?} should start a section");
        assert_eq!(sections[0].header().name().text(result.source()), *header);
        assert_eq!(
            sections[0].section_kind(result.source()),
            *expected,
            "header {header:?}"
        );
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
    assert_eq!(names[0].text(result.source()), "x");
    assert_eq!(returns(&result).len(), 1);
    assert!(notes(&result).is_some());
    assert_eq!(
        all_sections(&result)[0].header().name().text(result.source()),
        "parameters"
    );
    assert_eq!(all_sections(&result)[2].header().name().text(result.source()), "NOTES");
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
    let src = result.source();

    assert_eq!(doc(&result).summary().unwrap().text(src), "Summary line.");
    assert_eq!(all_sections(&result)[0].header().name().text(src), "Parameters");
    let underline = all_sections(&result)[0].header().underline().text(result.source());
    assert!(underline.chars().all(|c| c == '-'));

    let p = &parameters(&result)[0];
    let names: Vec<_> = p.names().collect();
    assert_eq!(names[0].text(src), "x");
    assert_eq!(p.r#type().unwrap().text(src), "int");
    assert_eq!(p.description().unwrap().text(src), "Description of x.");
}

// =============================================================================
// Deprecation
// =============================================================================

/// SPEC: `.. deprecated:: <version>` directive is recognized before sections.
/// Also CONTRACT for NumPyDeprecation accessors (version / description).
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
    let dep = doc(&result).deprecation().expect("deprecation should be parsed");
    assert_eq!(dep.version().text(result.source()), "1.6.0");
    assert_eq!(
        dep.description().unwrap().text(result.source()),
        "Use `new_func` instead."
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
    assert_eq!(s[0].section_kind(result.source()), NumPySectionKind::Parameters);
    assert_eq!(s[1].section_kind(result.source()), NumPySectionKind::Returns);
    assert_eq!(s[2].section_kind(result.source()), NumPySectionKind::Raises);
    assert_eq!(s[3].section_kind(result.source()), NumPySectionKind::Notes);
}

// =============================================================================
// NumPySectionKind API
// =============================================================================

/// CONTRACT: ALL contains every known kind and never Unknown.
#[test]
fn test_all_section_kinds_exist() {
    assert_eq!(NumPySectionKind::ALL.len(), 23);
    for kind in NumPySectionKind::ALL {
        assert_ne!(*kind, NumPySectionKind::Unknown);
    }
}

/// CONTRACT: from_name / is_known behavior for unknown and known names.
#[test]
fn test_section_kind_from_name_unknown() {
    assert_eq!(NumPySectionKind::from_name("nonexistent"), NumPySectionKind::Unknown);
    assert!(!NumPySectionKind::is_known("nonexistent"));
    assert!(NumPySectionKind::is_known("parameters"));
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
    assert_eq!(s[0].section_kind(result.source()), NumPySectionKind::Parameters);
    assert_eq!(s[1].section_kind(result.source()), NumPySectionKind::Returns);
}

// =============================================================================
// Display impl
// =============================================================================

/// CONTRACT: Display impl for NumPySectionKind.
#[test]
fn test_section_kind_display() {
    assert_eq!(format!("{}", NumPySectionKind::Parameters), "Parameters");
    assert_eq!(format!("{}", NumPySectionKind::Returns), "Returns");
    assert_eq!(format!("{}", NumPySectionKind::Yields), "Yields");
    assert_eq!(format!("{}", NumPySectionKind::Receives), "Receives");
    assert_eq!(format!("{}", NumPySectionKind::OtherParameters), "Other Parameters");
    assert_eq!(format!("{}", NumPySectionKind::Raises), "Raises");
    assert_eq!(format!("{}", NumPySectionKind::Warns), "Warns");
    assert_eq!(format!("{}", NumPySectionKind::Warnings), "Warnings");
    assert_eq!(format!("{}", NumPySectionKind::SeeAlso), "See Also");
    assert_eq!(format!("{}", NumPySectionKind::Notes), "Notes");
    assert_eq!(format!("{}", NumPySectionKind::References), "References");
    assert_eq!(format!("{}", NumPySectionKind::Examples), "Examples");
    assert_eq!(format!("{}", NumPySectionKind::Attributes), "Attributes");
    assert_eq!(format!("{}", NumPySectionKind::Methods), "Methods");
    assert_eq!(format!("{}", NumPySectionKind::Unknown), "Unknown");
}

// =============================================================================
// SPEC: entry accessors are guarded by the section's kind (#77 review)
// =============================================================================

/// SPEC: all entries share the `ENTRY` node kind, so a mismatched accessor
/// (`parameters()` on a Raises section) must return empty instead of wrapping
/// the foreign entries — pre-unification behavior, and calling typed
/// accessors on the results must not panic.
#[test]
fn spec_mismatched_entry_accessor_returns_empty() {
    let docstring = "Summary.\n\nRaises\n------\nValueError\n    If the value is bad.\n";
    let result = parse_numpy(docstring);
    let source = result.source();
    let sections = all_sections(&result);
    let section = &sections[0];
    assert_eq!(section.section_kind(source), NumPySectionKind::Raises);

    // The matching accessor sees the entry…
    assert_eq!(section.exceptions(source).count(), 1);

    // …every mismatched accessor returns empty (collecting token accessors
    // would panic in required_token if a foreign entry leaked through).
    assert_eq!(section.parameters(source).count(), 0);
    assert_eq!(section.returns(source).count(), 0);
    assert_eq!(section.yields(source).count(), 0);
    assert_eq!(section.warnings(source).count(), 0);
    assert_eq!(section.see_also_items(source).count(), 0);
    assert_eq!(section.attributes(source).count(), 0);
    assert_eq!(section.methods(source).count(), 0);
    assert_eq!(section.references().count(), 0);

    // And the guard also separates the NAME-carrying roles from each other:
    // attributes() on a Parameters section is empty.
    let result = parse_numpy("Summary.\n\nParameters\n----------\nx : int\n    The value.\n");
    let source = result.source();
    let sections = all_sections(&result);
    assert_eq!(sections[0].parameters(source).count(), 1);
    assert_eq!(sections[0].attributes(source).count(), 0);
    assert_eq!(sections[0].methods(source).count(), 0);
}
