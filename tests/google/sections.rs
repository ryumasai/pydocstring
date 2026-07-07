//! Spec pins for section dispatch: header-alias→kind mapping, case-insensitivity,
//! ordering, unknown sections, plus header/section span contract.
//! Exhaustive input coverage lives in tests/corpus/google/ + tests/snapshots.rs.

use super::*;

// =============================================================================
// Section-header alias → kind mapping (spec table)
// =============================================================================

/// Every recognised header alias maps to its GoogleSectionKind, and the header
/// name token preserves the source spelling.  This is the full alias table —
/// an alias removed or remapped in the parser fails here explicitly.
#[test]
fn test_section_header_alias_kind_table() {
    use GoogleSectionKind as K;
    let cases: &[(&str, GoogleSectionKind)] = &[
        ("Args", K::Args),
        ("Arguments", K::Args),
        ("Parameters", K::Args),
        ("Params", K::Args),
        ("Keyword Args", K::KeywordArgs),
        ("Keyword Arguments", K::KeywordArgs),
        ("Other Parameters", K::OtherParameters),
        ("Receives", K::Receives),
        ("Receive", K::Receives),
        ("Returns", K::Returns),
        ("Return", K::Returns),
        ("Yields", K::Yields),
        ("Yield", K::Yields),
        ("Raises", K::Raises),
        ("Raise", K::Raises),
        ("Warns", K::Warns),
        ("Warn", K::Warns),
        ("Attributes", K::Attributes),
        ("Attribute", K::Attributes),
        ("Methods", K::Methods),
        ("See Also", K::SeeAlso),
        ("Note", K::Notes),
        ("Notes", K::Notes),
        ("Example", K::Examples),
        ("Examples", K::Examples),
        ("Todo", K::Todo),
        ("References", K::References),
        // Singular "Warning" is the free-text admonition, NOT Warns.
        ("Warning", K::Warnings),
        ("Warnings", K::Warnings),
        ("Attention", K::Attention),
        ("Caution", K::Caution),
        ("Danger", K::Danger),
        ("Error", K::Error),
        ("Hint", K::Hint),
        ("Important", K::Important),
        ("Tip", K::Tip),
        ("Custom", K::Unknown),
    ];
    for (header, expected) in cases {
        let input = format!("Summary.\n\n{header}:\n    x: d.");
        let result = parse_google(&input);
        let sections = all_sections(&result);
        assert_eq!(sections.len(), 1, "header {header:?} should produce one section");
        assert_eq!(
            sections[0].section_kind(result.source()),
            *expected,
            "header {header:?}"
        );
        assert_eq!(
            sections[0].header().name().text(result.source()),
            *header,
            "header name must preserve source spelling for {header:?}"
        );
    }
}

/// Section headers are matched case-insensitively.
#[test]
fn test_napoleon_case_insensitive() {
    let docstring = "Summary.\n\nkeyword args:\n    x (int): Value.";
    let result = parse_google(docstring);
    assert_eq!(keyword_args(&result).len(), 1);
}

// =============================================================================
// Section order preservation
// =============================================================================

#[test]
fn test_section_order() {
    let docstring = "Summary.\n\nReturns:\n    int: Value.\n\nArgs:\n    x: Input.";
    let result = parse_google(docstring);
    let sections = all_sections(&result);
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].header().name().text(result.source()), "Returns");
    assert_eq!(sections[1].header().name().text(result.source()), "Args");
}

// =============================================================================
// Section header / section spans (contract)
// =============================================================================

#[test]
fn test_section_header_span() {
    let docstring = "Summary.\n\nArgs:\n    x: Value.";
    let result = parse_google(docstring);
    let header = all_sections(&result)[0].header();
    assert_eq!(header.name().text(result.source()), "Args");
    assert_eq!(header.syntax().range().source_text(result.source()), "Args:");
}

#[test]
fn test_section_span() {
    let docstring = "Summary.\n\nArgs:\n    x: Value.";
    let result = parse_google(docstring);
    let section = &all_sections(&result)[0];
    assert_eq!(
        section.syntax().range().source_text(result.source()),
        "Args:\n    x: Value."
    );
}

// =============================================================================
// Unknown sections
// =============================================================================

#[test]
fn test_unknown_section_preserved() {
    let docstring = "Summary.\n\nCustom:\n    Some custom content.";
    let result = parse_google(docstring);
    let sections = all_sections(&result);
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].header().name().text(result.source()), "Custom");
    assert_eq!(sections[0].section_kind(result.source()), GoogleSectionKind::Unknown);
    assert_eq!(
        sections[0].body_text().unwrap().text(result.source()),
        "Some custom content."
    );
}

/// Multi-word unknown names followed by a colon are still section headers.
#[test]
fn test_multiple_unknown_sections() {
    let docstring = "Summary.\n\nCustom One:\n    First.\n\nCustom Two:\n    Second.";
    let result = parse_google(docstring);
    let sections = all_sections(&result);
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].header().name().text(result.source()), "Custom One");
    assert_eq!(sections[1].header().name().text(result.source()), "Custom Two");
}

// =============================================================================
// Full docstring smoke test (exercises many accessors together)
// =============================================================================

#[test]
fn test_napoleon_full_docstring() {
    let docstring = r#"Calculate something.

Extended description.

Args:
    x (int): First argument.

Keyword Args:
    timeout (float): Timeout value.

Returns:
    int: The result.

Raises:
    ValueError: If x is negative.

Warns:
    DeprecationWarning: If old API is used.

See Also:
    other_func: Related function.

Note:
    Implementation detail.

Example:
    >>> calculate(1)
    1"#;

    let result = parse_google(docstring);
    assert_eq!(
        doc(&result).summary().unwrap().text(result.source()),
        "Calculate something."
    );
    assert!(doc(&result).extended_summary().is_some());
    assert_eq!(args(&result).len(), 1);
    assert_eq!(keyword_args(&result).len(), 1);
    assert!(returns(&result).is_some());
    assert_eq!(raises(&result).len(), 1);
    assert_eq!(warns(&result).len(), 1);
    assert_eq!(see_also(&result).len(), 1);
    assert!(notes(&result).is_some());
    assert!(examples(&result).is_some());
}

// =============================================================================
// SPEC: entry accessors are guarded by the section's kind (#77 review)
// =============================================================================

/// SPEC: all entries share the `ENTRY` node kind, so a mismatched accessor
/// (`args()` on a Raises section) must return empty instead of wrapping the
/// foreign entries — pre-unification behavior, and calling typed accessors
/// on the results must not panic.
#[test]
fn spec_mismatched_entry_accessor_returns_empty() {
    let docstring = "Summary.\n\nRaises:\n    ValueError: If the value is bad.";
    let result = parse_google(docstring);
    let source = result.source();
    let sections = all_sections(&result);
    let section = &sections[0];
    assert_eq!(section.section_kind(source), GoogleSectionKind::Raises);

    // The matching accessor sees the entry…
    assert_eq!(section.exceptions(source).count(), 1);

    // …every mismatched accessor returns empty (collecting token accessors
    // would panic in required_token if a foreign entry leaked through).
    assert_eq!(section.args(source).count(), 0);
    assert!(section.returns(source).is_none());
    assert!(section.yields(source).is_none());
    assert_eq!(section.warnings(source).count(), 0);
    assert_eq!(section.see_also_items(source).count(), 0);
    assert_eq!(section.attributes(source).count(), 0);
    assert_eq!(section.methods(source).count(), 0);
    assert_eq!(section.references().count(), 0);

    // And the guard also separates the NAME-carrying roles from each other:
    // attributes() on an Args section is empty.
    let result = parse_google("Summary.\n\nArgs:\n    x (int): The value.");
    let source = result.source();
    let sections = all_sections(&result);
    assert_eq!(sections[0].args(source).count(), 1);
    assert_eq!(sections[0].attributes(source).count(), 0);
    assert_eq!(sections[0].methods(source).count(), 0);
}
