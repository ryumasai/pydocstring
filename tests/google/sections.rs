//! Spec pins for section dispatch: header-alias→kind mapping, case-insensitivity,
//! ordering, unknown sections, plus header/section span contract.
//! Exhaustive input coverage lives in tests/corpus/google/ + tests/snapshots.rs.

use super::*;

// =============================================================================
// Section-header alias → kind mapping (spec table)
// =============================================================================

/// Every recognised header alias maps to its SectionKind, and the header
/// name token preserves the source spelling.  This is the full alias table —
/// an alias removed or remapped in the parser fails here explicitly.
#[test]
fn test_section_header_alias_kind_table() {
    use FreeSectionKind as F;
    use SectionKind as K;
    let cases: &[(&str, SectionKind)] = &[
        ("Args", K::Parameters),
        ("Arguments", K::Parameters),
        ("Parameters", K::Parameters),
        ("Params", K::Parameters),
        ("Keyword Args", K::KeywordParameters),
        ("Keyword Arguments", K::KeywordParameters),
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
        ("Note", K::FreeText(F::Notes)),
        ("Notes", K::FreeText(F::Notes)),
        ("Example", K::FreeText(F::Examples)),
        ("Examples", K::FreeText(F::Examples)),
        ("Todo", K::FreeText(F::Todo)),
        ("References", K::References),
        // Singular "Warning" is the free-text admonition, NOT Warns.
        ("Warning", K::FreeText(F::Warnings)),
        ("Warnings", K::FreeText(F::Warnings)),
        ("Attention", K::FreeText(F::Attention)),
        ("Caution", K::FreeText(F::Caution)),
        ("Danger", K::FreeText(F::Danger)),
        ("Error", K::FreeText(F::Error)),
        ("Hint", K::FreeText(F::Hint)),
        ("Important", K::FreeText(F::Important)),
        ("Tip", K::FreeText(F::Tip)),
    ];
    for (header, expected) in cases {
        let input = format!("Summary.\n\n{header}:\n    x: d.");
        let result = parse_google(&input);
        let sections = all_sections(&result);
        assert_eq!(sections.len(), 1, "header {header:?} should produce one section");
        assert_eq!(sections[0].kind(), *expected, "header {header:?}");
        assert_eq!(
            sections[0].header_name(),
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
    assert_eq!(sections[0].header_name(), "Returns");
    assert_eq!(sections[1].header_name(), "Args");
}

// =============================================================================
// Section header / section spans (contract)
// =============================================================================

#[test]
fn test_section_header_span() {
    let docstring = "Summary.\n\nArgs:\n    x: Value.";
    let result = parse_google(docstring);
    let sections = all_sections(&result);
    assert_eq!(sections[0].header_name(), "Args");
    assert_eq!(header(&sections[0]).range().source_text(result.source()), "Args:");
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

/// napoleon's line (#143): an unknown name — even colon-terminated — is
/// prose by default; it becomes a section only when registered.
#[test]
fn test_unknown_section_requires_registration() {
    let docstring = "Summary.\n\nCustom:\n    Some custom content.";

    // Strict default: prose, not a section (napoleon reads it the same way).
    let strict = parse_google(docstring);
    assert_eq!(all_sections(&strict).len(), 0);

    // Registered: parses exactly as unknown sections always did.
    let opts = ParseOptions::new().with_custom_sections(["Custom"]);
    let result = parse_google_with(docstring, &opts);
    let sections = all_sections(&result);
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].header_name(), "Custom");
    assert_eq!(
        sections[0].kind(),
        SectionKind::FreeText(FreeSectionKind::Unknown("Custom".to_owned()))
    );
    assert_eq!(sections[0].body().unwrap().text(), "Some custom content.");
}

/// Multi-word registered names followed by a colon are section headers.
#[test]
fn test_multiple_custom_sections() {
    let docstring = "Summary.\n\nCustom One:\n    First.\n\nCustom Two:\n    Second.";
    let opts = ParseOptions::new().with_custom_sections(["Custom One", "Custom Two"]);
    let result = parse_google_with(docstring, &opts);
    let sections = all_sections(&result);
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].header_name(), "Custom One");
    assert_eq!(sections[1].header_name(), "Custom Two");
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
    assert_eq!(doc(&result).summary().unwrap().text(), "Calculate something.");
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

/// SPEC: all entries share the `ENTRY` node kind, so an entry's role is its
/// parent section's kind: a Raises entry is only ever reachable through the
/// Raises lens, never as an arg / return / warning / … (pre-unification the
/// per-role accessors were guarded by the section kind for the same reason),
/// and reading it through a foreign role's accessors must not panic.
#[test]
fn spec_mismatched_entry_accessor_returns_empty() {
    let docstring = "Summary.\n\nRaises:\n    ValueError: If the value is bad.";
    let result = parse_google(docstring);
    let sections = all_sections(&result);
    let section = &sections[0];
    assert_eq!(section.kind(), SectionKind::Raises);

    // The matching lens sees the entry…
    assert_eq!(section.entries().count(), 1);
    assert_eq!(raises(&result).len(), 1);

    // …and every other role's lens is empty (pre-unification, a token
    // accessor would have panicked in required_token if a foreign entry
    // leaked through).
    assert_eq!(args(&result).len(), 0);
    assert!(returns(&result).is_none());
    assert!(yields(&result).is_none());
    assert_eq!(warns(&result).len(), 0);
    assert_eq!(see_also(&result).len(), 0);
    assert_eq!(attributes(&result).len(), 0);
    assert_eq!(methods(&result).len(), 0);
    assert_eq!(references(&result).len(), 0);

    // Reading the exception entry through the NAME-carrying accessors is
    // total (no panic): a Raises entry carries a TYPE, not a NAME.
    let entry = section.entries().next().unwrap();
    assert!(entry.name().is_none());
    assert_eq!(entry.type_annotation().unwrap().text(), "ValueError");

    // And the same rule separates the NAME-carrying roles from each other:
    // an Args entry never surfaces as an attribute or a method.
    let result = parse_google("Summary.\n\nArgs:\n    x (int): The value.");
    assert_eq!(args(&result).len(), 1);
    assert_eq!(attributes(&result).len(), 0);
    assert_eq!(methods(&result).len(), 0);
}

// =============================================================================
// #146: zero-indent sections close at a blank line + non-entry line
// =============================================================================

/// Trailing prose after a blank line is not a parameter: it ends the
/// zero-indent body instead of being swallowed (#146).
#[test]
fn test_zero_indent_section_closes_at_blank_plus_prose() {
    let result = parse_google("Args:\nx: d.\n\nThis is trailing prose.\n");
    let sections = all_sections(&result);
    assert_eq!(sections.len(), 1);
    let entries: Vec<_> = sections[0].entries().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name().unwrap().text(), "x");

    // The prose is a document-level paragraph, not a Parameter.
    let doc = Document::new(&result);
    let paragraphs: Vec<_> = doc.paragraphs().collect();
    assert_eq!(paragraphs.len(), 1);
    assert_eq!(paragraphs[0].text(), "This is trailing prose.");
}

/// A blank line followed by an entry-shaped line continues the body …
#[test]
fn test_zero_indent_section_continues_over_blank_plus_entry() {
    let result = parse_google("Args:\nx: d.\n\ny: e.\n");
    let sections = all_sections(&result);
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].entries().count(), 2);
}

/// … and so does a bare name that owns an indented definition.
#[test]
fn test_zero_indent_section_continues_over_blank_plus_defined_name() {
    let result = parse_google("Methods:\nfoo()\n    Does bar.\n\nbaz()\n    Does qux.\n");
    let sections = all_sections(&result);
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].entries().count(), 2);
}

/// Free-text bodies are exempt: a zero-indent Notes section spans its
/// paragraphs by design.
#[test]
fn test_zero_indent_free_text_spans_paragraphs() {
    let result = parse_google("Notes:\nFirst paragraph.\n\nSecond paragraph.\n");
    let sections = all_sections(&result);
    assert_eq!(sections.len(), 1);
    let body = sections[0].body().unwrap();
    assert!(body.text().contains("Second paragraph."));
}
