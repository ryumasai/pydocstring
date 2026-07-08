//! Integration tests for the style-independent typed layer
//! (`pydocstring::parse::unified`) — the single code path over the unified
//! node kinds promised in #26/#41: one generic traversal works for every
//! docstring style.

use pydocstring::model::FreeSectionKind;
use pydocstring::model::SectionKind;
use pydocstring::parse::Style;
use pydocstring::parse::parse;
use pydocstring::parse::unified::Document;

// =============================================================================
// The single code path: one generic function, every style
// =============================================================================

/// Extracted entry data: `(names, type_annotation, description)` per entry,
/// grouped by section kind — the same shape whatever the source style.
type Extracted = Vec<(SectionKind, Vec<(Vec<String>, Option<String>, Option<String>)>)>;

/// ONE style-independent extraction function. It never looks at the style:
/// it parses with auto-detection and walks Document → Section → Entry.
fn extract(source: &str) -> Extracted {
    let parsed = parse(source);
    let doc = Document::new(&parsed);
    doc.sections()
        .map(|section| {
            let entries = section
                .entries()
                .map(|entry| {
                    (
                        entry.names().map(|n| n.text().to_owned()).collect(),
                        entry.type_annotation().map(|t| t.text().to_owned()),
                        entry.description().map(|d| d.logical_text()),
                    )
                })
                .collect();
            (section.kind(), entries)
        })
        .collect()
}

/// SPEC: the same docstring content written in Google style and in NumPy
/// style, traversed with ONE generic function, produces identical extracted
/// (names, types, descriptions) lists.
#[test]
fn one_code_path_extracts_identical_data_from_both_styles() {
    let google = "Summary.\n\n\
        Args:\n\
        \x20   x (int): The value.\n\
        \x20   y (str, optional): A name.\n\
        \n\
        Raises:\n\
        \x20   ValueError: If x is negative.\n";
    let numpy = "Summary.\n\n\
        Parameters\n\
        ----------\n\
        x : int\n\
        \x20   The value.\n\
        y : str, optional\n\
        \x20   A name.\n\
        \n\
        Raises\n\
        ------\n\
        ValueError\n\
        \x20   If x is negative.\n";

    let from_google = extract(google);
    let from_numpy = extract(numpy);
    assert_eq!(from_google, from_numpy);

    // And the data itself is what was written, not merely equal.
    assert_eq!(
        from_google,
        vec![
            (
                SectionKind::Parameters,
                vec![
                    (
                        vec!["x".to_owned()],
                        Some("int".to_owned()),
                        Some("The value.".to_owned())
                    ),
                    (vec!["y".to_owned()], Some("str".to_owned()), Some("A name.".to_owned())),
                ]
            ),
            (
                SectionKind::Raises,
                // Exception entries carry their type, not a name.
                vec![(
                    vec![],
                    Some("ValueError".to_owned()),
                    Some("If x is negative.".to_owned())
                )]
            ),
        ]
    );
}

/// LAW (cross-style slot-kind parity): for equivalent content, EVERY entry
/// role must expose identical slots through the unified layer — the same
/// name / type_annotation / description presence AND the same extracted
/// text — whatever the source style. Entry roles derive from the enclosing
/// section, never from per-role token kinds.
///
/// Warns is where this once broke: Google warns entries emitted a
/// `WARNING_TYPE` token while NumPy warns entries emitted `TYPE`, so
/// `Entry::type_annotation()` returned `None` for Google warns only. Every
/// entry role is pinned here so that class of divergence cannot recur.
#[test]
fn cross_style_slot_kind_parity_covers_every_entry_role() {
    // (role, google source, numpy source, expected extraction)
    let cases: Vec<(&str, &str, &str, Extracted)> = vec![
        (
            "params",
            "Summary.\n\nArgs:\n    x (int): The value.\n",
            "Summary.\n\nParameters\n----------\nx : int\n    The value.\n",
            vec![(
                SectionKind::Parameters,
                vec![(
                    vec!["x".to_owned()],
                    Some("int".to_owned()),
                    Some("The value.".to_owned()),
                )],
            )],
        ),
        (
            "returns with type",
            "Summary.\n\nReturns:\n    int: The result.\n",
            "Summary.\n\nReturns\n-------\nint\n    The result.\n",
            vec![(
                SectionKind::Returns,
                vec![(vec![], Some("int".to_owned()), Some("The result.".to_owned()))],
            )],
        ),
        (
            "yields",
            "Summary.\n\nYields:\n    str: The next chunk.\n",
            "Summary.\n\nYields\n------\nstr\n    The next chunk.\n",
            vec![(
                SectionKind::Yields,
                vec![(vec![], Some("str".to_owned()), Some("The next chunk.".to_owned()))],
            )],
        ),
        (
            "raises",
            "Summary.\n\nRaises:\n    ValueError: If x is negative.\n",
            "Summary.\n\nRaises\n------\nValueError\n    If x is negative.\n",
            vec![(
                SectionKind::Raises,
                vec![(
                    vec![],
                    Some("ValueError".to_owned()),
                    Some("If x is negative.".to_owned()),
                )],
            )],
        ),
        (
            "warns",
            "Summary.\n\nWarns:\n    DeprecationWarning: If using the old API.\n",
            "Summary.\n\nWarns\n-----\nDeprecationWarning\n    If using the old API.\n",
            vec![(
                SectionKind::Warns,
                vec![(
                    vec![],
                    Some("DeprecationWarning".to_owned()),
                    Some("If using the old API.".to_owned()),
                )],
            )],
        ),
        (
            "attributes",
            "Summary.\n\nAttributes:\n    attr (bool): Whether enabled.\n",
            "Summary.\n\nAttributes\n----------\nattr : bool\n    Whether enabled.\n",
            vec![(
                SectionKind::Attributes,
                vec![(
                    vec!["attr".to_owned()],
                    Some("bool".to_owned()),
                    Some("Whether enabled.".to_owned()),
                )],
            )],
        ),
    ];

    for (role, google, numpy, expected) in &cases {
        // The comparison is only meaningful if the two inputs really parse
        // as different styles.
        assert_eq!(parse(google).style(), Style::Google, "{role}: google input misdetected");
        assert_eq!(parse(numpy).style(), Style::NumPy, "{role}: numpy input misdetected");

        let from_google = extract(google);
        let from_numpy = extract(numpy);
        assert_eq!(from_google, from_numpy, "{role}: cross-style slot parity broken");
        assert_eq!(&from_google, expected, "{role}: extracted data mismatch");
    }
}

// =============================================================================
// Document
// =============================================================================

#[test]
fn document_summary_style_and_sections() {
    let parsed = parse("Summary.\n\nExtended text.\n\nArgs:\n    x: Desc.\n\nNotes:\n    A note.\n");
    let doc = Document::new(&parsed);
    assert_eq!(doc.style(), Style::Google);
    assert_eq!(doc.summary().unwrap().text(), "Summary.");
    assert_eq!(doc.extended_summary().unwrap().text(), "Extended text.");

    let sections: Vec<_> = doc.sections().collect();
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].header_name(), "Args");
    assert_eq!(sections[0].kind(), SectionKind::Parameters);
    assert_eq!(sections[1].kind(), SectionKind::FreeText(FreeSectionKind::Notes));
    // Free-text sections expose their body and have no entries.
    assert_eq!(sections[1].body().unwrap().text(), "A note.");
    assert_eq!(sections[1].entries().count(), 0);
}

#[test]
fn document_directives() {
    let parsed = parse(
        "Summary.\n\n.. deprecated:: 1.6.0\n    Use `other` instead.\n\nParameters\n----------\nx : int\n    Desc.\n",
    );
    assert_eq!(parsed.style(), Style::NumPy);
    let doc = Document::new(&parsed);
    let directives: Vec<_> = doc.directives().collect();
    assert_eq!(directives.len(), 1);
    let dep = &directives[0];
    assert_eq!(dep.name().text(), "deprecated");
    assert_eq!(dep.argument().unwrap().text(), "1.6.0");
    assert_eq!(dep.description().unwrap().text(), "Use `other` instead.");
}

// =============================================================================
// Section: citations
// =============================================================================

#[test]
fn section_citations() {
    let parsed = parse("Summary.\n\nReferences\n----------\n.. [1] First reference.\n.. [CIT2002] Second one.\n");
    let doc = Document::new(&parsed);
    let section = doc.sections().next().unwrap();
    assert_eq!(section.kind(), SectionKind::References);

    let citations: Vec<_> = section.citations().collect();
    assert_eq!(citations.len(), 2);
    assert_eq!(citations[0].label().unwrap().text(), "1");
    assert_eq!(citations[0].description().unwrap().text(), "First reference.");
    assert_eq!(citations[1].label().unwrap().text(), "CIT2002");
}

// =============================================================================
// Entry: markers
// =============================================================================

#[test]
fn entry_markers_every_occurrence_first_value_shorthand() {
    let parsed = parse("Summary.\n\nParameters\n----------\nx : int, optional, default 1, default 2\n    Desc.\n");
    let doc = Document::new(&parsed);
    let entry = doc.sections().next().unwrap().entries().next().unwrap();

    assert!(entry.is_optional());
    assert_eq!(entry.optionals().count(), 1);

    let defaults: Vec<_> = entry.defaults().collect();
    assert_eq!(defaults.len(), 2);
    assert_eq!(defaults[0].keyword().text(), "default");
    assert_eq!(defaults[0].value().unwrap().text(), "1");
    assert_eq!(defaults[1].value().unwrap().text(), "2");
    assert!(defaults[0].separator().is_none());

    // First occurrence wins for the shorthand, mirroring the model rule.
    assert_eq!(entry.default_value().unwrap().text(), "1");
}

#[test]
fn entry_returns_use_return_type() {
    let parsed = parse("Summary.\n\nReturns:\n    int: The result.\n");
    let doc = Document::new(&parsed);
    let section = doc.sections().next().unwrap();
    assert_eq!(section.kind(), SectionKind::Returns);
    let entry = section.entries().next().unwrap();
    assert_eq!(entry.type_annotation().unwrap().text(), "int");
    assert_eq!(entry.description().unwrap().text(), "The result.");
    assert!(entry.name().is_none());
    assert!(!entry.is_optional());
    assert_eq!(entry.defaults().count(), 0);
}
