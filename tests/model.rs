//! Integration tests for the style-independent document model (IR).

use pydocstring::model::FreeSectionKind;
use pydocstring::model::Section;
use pydocstring::parse::google::parse_google;
use pydocstring::parse::google::to_model::to_model as google_to_model;
use pydocstring::parse::numpy::parse_numpy;
use pydocstring::parse::numpy::to_model::to_model as numpy_to_model;

use pydocstring::model::Attribute;
use pydocstring::model::Block;
use pydocstring::model::ExceptionEntry;
use pydocstring::model::Parameter;
use pydocstring::model::Return;
use pydocstring::model::SectionKind;
use pydocstring::model::SeeAlsoEntry;

fn params_of(s: &Section) -> Vec<&Parameter> {
    assert_eq!(s.kind, SectionKind::Parameters, "expected Parameters, got {:?}", s.kind);
    s.blocks.iter().filter_map(Block::as_parameter).collect()
}
fn returns_of(s: &Section) -> Vec<&Return> {
    assert_eq!(s.kind, SectionKind::Returns, "expected Returns, got {:?}", s.kind);
    s.blocks.iter().filter_map(Block::as_return).collect()
}
fn raises_of(s: &Section) -> Vec<&ExceptionEntry> {
    assert_eq!(s.kind, SectionKind::Raises, "expected Raises, got {:?}", s.kind);
    s.blocks.iter().filter_map(Block::as_exception).collect()
}
fn warns_of(s: &Section) -> Vec<&ExceptionEntry> {
    assert_eq!(s.kind, SectionKind::Warns, "expected Warns, got {:?}", s.kind);
    s.blocks.iter().filter_map(Block::as_exception).collect()
}
fn attrs_of(s: &Section) -> Vec<&Attribute> {
    assert_eq!(s.kind, SectionKind::Attributes, "expected Attributes, got {:?}", s.kind);
    s.blocks.iter().filter_map(Block::as_attribute).collect()
}
fn see_also_of(s: &Section) -> Vec<&SeeAlsoEntry> {
    assert_eq!(s.kind, SectionKind::SeeAlso, "expected SeeAlso, got {:?}", s.kind);
    s.blocks.iter().filter_map(Block::as_see_also).collect()
}
fn free_text_of(s: &Section) -> (FreeSectionKind, String) {
    let kind = match &s.kind {
        SectionKind::FreeText(k) => k.clone(),
        other => panic!("expected FreeText, got {:?}", other),
    };
    let body = s
        .blocks
        .iter()
        .filter_map(Block::as_paragraph)
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    (kind, body)
}

// =============================================================================
// Google → IR: Summary & Extended Summary
// =============================================================================

#[test]
fn google_summary_only() {
    let parsed = parse_google("Summary line.");
    let doc = google_to_model(&parsed).unwrap();
    assert_eq!(doc.summary.as_deref(), Some("Summary line."));
    assert_eq!(doc.extended_summary, None);
    assert!(doc.sections.is_empty());
}

#[test]
fn google_summary_and_extended() {
    let parsed = parse_google(
        "Summary.

    Extended description
    spanning lines.",
    );
    let doc = google_to_model(&parsed).unwrap();
    assert_eq!(doc.summary.as_deref(), Some("Summary."));
    assert_eq!(
        doc.extended_summary.as_deref(),
        Some("Extended description\nspanning lines.")
    );
}

// =============================================================================
// Google → IR: Parameters
// =============================================================================

#[test]
fn google_args_basic() {
    let parsed = parse_google("Summary.\n\nArgs:\n    x (int): The value.");
    let doc = google_to_model(&parsed).unwrap();
    assert_eq!(doc.sections.len(), 1);
    let params = params_of(&doc.sections[0]);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].names, vec!["x"]);
    assert_eq!(params[0].type_annotation.as_deref(), Some("int"));
    assert_eq!(params[0].description.as_deref(), Some("The value."));
    assert!(!params[0].is_optional);
    assert_eq!(params[0].default_value, None);
}

#[test]
fn google_args_optional() {
    let parsed = parse_google("Summary.\n\nArgs:\n    x (int, optional): The value.");
    let doc = google_to_model(&parsed).unwrap();
    let params = params_of(&doc.sections[0]);
    assert!(params[0].is_optional);
}

#[test]
fn google_args_multiple() {
    let parsed = parse_google(
        "Summary.

        Args:
            x (int): First.
            y (str): Second.
                More description.
            z: Third.
                More description.

                .. directive:: something
                   directive_option

                continued description.",
    );
    let doc = google_to_model(&parsed).unwrap();
    let params = params_of(&doc.sections[0]);
    assert_eq!(params.len(), 3);
    assert_eq!(params[0].names, vec!["x"]);
    assert_eq!(params[0].description.as_deref(), Some("First."));

    assert_eq!(params[1].names, vec!["y"]);
    assert_eq!(params[1].description.as_deref(), Some("Second.\nMore description."));

    assert_eq!(params[2].names, vec!["z"]);
    assert_eq!(
        params[2].description.as_deref(),
        Some("Third.\nMore description.\n\n.. directive:: something\n   directive_option\n\ncontinued description.")
    );
}

#[test]
fn google_args_no_type() {
    let parsed = parse_google("Summary.\n\nArgs:\n    x: The value.");
    let doc = google_to_model(&parsed).unwrap();
    let params = params_of(&doc.sections[0]);
    assert_eq!(params[0].type_annotation, None);
}

// =============================================================================
// Google → IR: Returns
// =============================================================================

#[test]
fn google_returns() {
    let parsed = parse_google(
        "Summary.

    Returns:
        int: The result.
        More description.",
    );
    let doc = google_to_model(&parsed).unwrap();
    let returns = returns_of(&doc.sections[0]);
    assert_eq!(returns.len(), 1);
    assert_eq!(returns[0].name, None);
    assert_eq!(returns[0].type_annotation.as_deref(), Some("int"));
    assert_eq!(
        returns[0].description.as_deref(),
        Some("The result.\nMore description.")
    );
}

/// A description-only Returns entry with a multi-line description survives
/// the google emit/parse round trip: the continuation lines are emitted
/// inside the section body, not at column 0 (#93).
#[test]
fn google_returns_description_only_multiline_round_trips() {
    let parsed = parse_google(
        "Summary.\n\nReturns:\n    The result of executing the command.\n    Execution begins with the target.\n",
    );
    let doc = google_to_model(&parsed).unwrap();
    let returns = returns_of(&doc.sections[0]);
    assert_eq!(returns.len(), 1);
    assert_eq!(returns[0].type_annotation, None);
    assert_eq!(
        returns[0].description.as_deref(),
        Some("The result of executing the command.\nExecution begins with the target.")
    );
    let emitted = pydocstring::emit::google::emit_google(&doc, &pydocstring::emit::EmitOptions::default());
    let reparsed = google_to_model(&parse_google(&emitted)).unwrap();
    assert_eq!(
        reparsed, doc,
        "google desc-only returns round trip diverged:\n{emitted}"
    );
}

// =============================================================================
// Google → IR: Raises
// =============================================================================

#[test]
fn google_raises() {
    let parsed = parse_google(
        "Summary.

    Raises:
        ValueError: If bad.
            More description.

            Even more.",
    );
    let doc = google_to_model(&parsed).unwrap();
    let entries = raises_of(&doc.sections[0]);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].type_name, "ValueError");
    assert_eq!(
        entries[0].description.as_deref(),
        Some("If bad.\nMore description.\n\nEven more.")
    );
}

// =============================================================================
// Google → IR: Warns
// =============================================================================

#[test]
fn google_warns() {
    let parsed = parse_google(
        "Summary.

    Warns:
        UserWarning: Watch out.
            Details.",
    );
    let doc = google_to_model(&parsed).unwrap();
    let entries = warns_of(&doc.sections[0]);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].type_name, "UserWarning");
    assert_eq!(entries[0].description.as_deref(), Some("Watch out.\nDetails."));
}

// =============================================================================
// Google → IR: Free text sections
// =============================================================================

#[test]
fn google_notes_section() {
    let parsed = parse_google(
        "Summary.

    Notes:
        Some notes here.
        More notes.

        - Unordered list.
          same item
        - next item",
    );
    let doc = google_to_model(&parsed).unwrap();
    let (kind, body) = free_text_of(&doc.sections[0]);
    let (kind, body) = (&kind, body.as_str());
    assert_eq!(*kind, FreeSectionKind::Notes);
    assert!(!body.is_empty());
    assert_eq!(
        body,
        "Some notes here.\nMore notes.\n\n- Unordered list.\n  same item\n- next item"
    )
}

// =============================================================================
// Google → IR: Multiple sections
// =============================================================================

#[test]
fn google_multiple_sections() {
    let parsed = parse_google(
        "Summary.

        Args:
            x (int): Val.

        Returns:
            str: Result.
        Raises:
            ValueError: Bad.
            KeyError: Very bad.
                Only raised when key is not found.",
    );
    let doc = google_to_model(&parsed).unwrap();
    assert_eq!(doc.sections.len(), 3);
    assert!(doc.sections[0].kind == SectionKind::Parameters);
    assert!(doc.sections[1].kind == SectionKind::Returns);
    let entries = raises_of(&doc.sections[2]);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].type_name, "ValueError");
    assert_eq!(entries[0].description.as_deref(), Some("Bad."));
    assert_eq!(entries[1].type_name, "KeyError");
    assert_eq!(
        entries[1].description.as_deref(),
        Some("Very bad.\nOnly raised when key is not found.")
    );
}

// =============================================================================
// Google → IR: No deprecation
// =============================================================================

#[test]
fn google_no_deprecation() {
    let parsed = parse_google("Summary.\n\nArgs:\n    x: Val.");
    let doc = google_to_model(&parsed).unwrap();
    assert_eq!(doc.deprecation(), None);
    assert!(doc.directives.is_empty());
}

/// A non-deprecated directive flows to `model.directives` (name/argument/
/// description), while `deprecation()` — the `deprecated`-name filter — stays
/// `None`.
#[test]
fn numpy_non_deprecated_directive_flows_to_model() {
    let parsed = parse_numpy("Summary.\n\n.. versionadded:: 2.0\n\nParameters\n----------\nx : int\n    Desc.\n");
    let doc = numpy_to_model(&parsed).unwrap();
    assert_eq!(doc.deprecation(), None);
    assert_eq!(doc.directives.len(), 1);
    assert_eq!(doc.directives[0].name, "versionadded");
    assert_eq!(doc.directives[0].argument.as_deref(), Some("2.0"));
    assert_eq!(doc.directives[0].description, None);
}

/// Consecutive directives all land in `model.directives`, in source order,
/// and the `deprecated` one is still surfaced by `deprecation()`.
#[test]
fn numpy_multiple_directives_flow_to_model() {
    let parsed = parse_numpy(
        "Summary.\n\n.. deprecated:: 1.6.0\n    Use `other`.\n.. versionadded:: 2.0\n\nParameters\n----------\nx : int\n    Desc.\n",
    );
    let doc = numpy_to_model(&parsed).unwrap();
    let names: Vec<&str> = doc.directives.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, vec!["deprecated", "versionadded"]);
    assert_eq!(doc.deprecation().map(|d| d.argument.as_deref()), Some(Some("1.6.0")));
}

// =============================================================================
// NumPy → IR: Summary & Extended Summary
// =============================================================================

#[test]
fn numpy_summary_only() {
    let parsed = parse_numpy("Summary line.");
    let doc = numpy_to_model(&parsed).unwrap();
    assert_eq!(doc.summary.as_deref(), Some("Summary line."));
    assert_eq!(doc.extended_summary, None);
    assert!(doc.sections.is_empty());
}

#[test]
fn numpy_summary_and_extended() {
    let parsed = parse_numpy("Summary.\n\nExtended description.");
    let doc = numpy_to_model(&parsed).unwrap();
    assert_eq!(doc.summary.as_deref(), Some("Summary."));
    assert!(doc.extended_summary.is_some());
}

// =============================================================================
// NumPy → IR: Parameters
// =============================================================================

#[test]
fn numpy_parameters_basic() {
    let parsed = parse_numpy(
        "Summary.

    Parameters
    ----------
    x : int
        The value.",
    );
    let doc = numpy_to_model(&parsed).unwrap();
    assert_eq!(doc.sections.len(), 1);
    let params = params_of(&doc.sections[0]);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].names, vec!["x"]);
    assert_eq!(params[0].type_annotation.as_deref(), Some("int"));
    assert_eq!(params[0].description.as_deref(), Some("The value."));
}

#[test]
fn numpy_parameters_optional() {
    let parsed = parse_numpy("Summary.\n\nParameters\n----------\nx : int, optional\n    The value.");
    let doc = numpy_to_model(&parsed).unwrap();
    let params = params_of(&doc.sections[0]);
    assert!(params[0].is_optional);
}

#[test]
fn numpy_parameters_multiple() {
    let parsed = parse_numpy(
        "Summary.

        Parameters
        ----------
        x: int
            First.
        y: str
            Second.
            More description.
        z
            Third.
            More description.

            .. directive:: something
               directive_option

            continued description.",
    );
    let doc = numpy_to_model(&parsed).unwrap();
    let params = params_of(&doc.sections[0]);
    assert_eq!(params.len(), 3);
    assert_eq!(params[0].names, vec!["x"]);
    assert_eq!(params[0].description.as_deref(), Some("First."));

    assert_eq!(params[1].names, vec!["y"]);
    assert_eq!(params[1].description.as_deref(), Some("Second.\nMore description."));

    assert_eq!(params[2].names, vec!["z"]);
    assert_eq!(
        params[2].description.as_deref(),
        Some("Third.\nMore description.\n\n.. directive:: something\n   directive_option\n\ncontinued description.")
    );
}

#[test]
fn numpy_parameters_multiple_names() {
    let parsed = parse_numpy(
        "Summary.

    Parameters
    ----------
    x, y : float
        Values.",
    );
    let doc = numpy_to_model(&parsed).unwrap();
    let params = params_of(&doc.sections[0]);
    assert_eq!(params[0].names, vec!["x", "y"]);
}

// =============================================================================
// Attributes: multi-name entries keep every name (#89)
// =============================================================================

#[test]
fn numpy_attributes_multiple_names() {
    let parsed = parse_numpy(
        "Summary.

    Attributes
    ----------
    jac, hess : ndarray
        Derivatives.",
    );
    let doc = numpy_to_model(&parsed).unwrap();
    let attrs = attrs_of(&doc.sections[0]);
    assert_eq!(attrs.len(), 1);
    assert_eq!(attrs[0].names, vec!["jac", "hess"]);
    assert_eq!(attrs[0].type_annotation.as_deref(), Some("ndarray"));
    assert_eq!(attrs[0].description.as_deref(), Some("Derivatives."));
}

#[test]
fn google_attributes_multiple_names() {
    let parsed = parse_google("Summary.\n\nAttributes:\n    jac, hess (ndarray): Derivatives.");
    let doc = google_to_model(&parsed).unwrap();
    let attrs = attrs_of(&doc.sections[0]);
    assert_eq!(attrs.len(), 1);
    assert_eq!(attrs[0].names, vec!["jac", "hess"]);
    assert_eq!(attrs[0].type_annotation.as_deref(), Some("ndarray"));
    assert_eq!(attrs[0].description.as_deref(), Some("Derivatives."));
}

#[test]
fn numpy_parameters_default_value() {
    let parsed = parse_numpy(
        "Summary.

    Parameters
    ----------
    x : int, default: 0
        The value.",
    );
    let doc = numpy_to_model(&parsed).unwrap();
    let params = params_of(&doc.sections[0]);
    assert_eq!(params[0].default_value.as_deref(), Some("0"));
}

// =============================================================================
// NumPy → IR: Returns
// =============================================================================

#[test]
fn numpy_returns() {
    let parsed = parse_numpy(
        "Summary.

    Returns
    -------
    result : int
        The result.
        More description.",
    );
    let doc = numpy_to_model(&parsed).unwrap();
    let returns = returns_of(&doc.sections[0]);
    assert_eq!(returns.len(), 1);
    assert_eq!(returns[0].type_annotation.as_deref(), Some("int"));
    assert_eq!(
        returns[0].description.as_deref(),
        Some("The result.\nMore description.")
    )
}

// =============================================================================
// NumPy → IR: Raises
// =============================================================================

#[test]
fn numpy_raises() {
    let parsed = parse_numpy(
        "Summary.

    Raises
    ------
    ValueError
        If bad.
    KeyError
        If very bad.

            blockquote

        1. first item
           still first item
        2. second item",
    );
    let doc = numpy_to_model(&parsed).unwrap();
    let entries = raises_of(&doc.sections[0]);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].type_name, "ValueError");
    assert_eq!(entries[0].description.as_deref(), Some("If bad."));
    assert_eq!(entries[1].type_name, "KeyError");
    assert_eq!(
        entries[1].description.as_deref(),
        Some("If very bad.\n\n    blockquote\n\n1. first item\n   still first item\n2. second item")
    );
}

// =============================================================================
// NumPy → IR: Deprecation
// =============================================================================

#[test]
fn numpy_deprecation() {
    let parsed = parse_numpy(
        "Summary.

    .. deprecated:: 1.6.0
       Use `other` instead.",
    );
    let doc = numpy_to_model(&parsed).unwrap();
    let dep = doc.deprecation().expect("should have deprecation");
    assert_eq!(dep.name, "deprecated");
    assert_eq!(dep.argument.as_deref(), Some("1.6.0"));
    assert_eq!(dep.description.as_deref(), Some("Use `other` instead."));
}

/// A multi-line directive body is DEDENTED in the model (logical text; the
/// emitter re-indents exactly once), and the emit/parse round trip is a
/// fixed point — the indent must not grow per cycle (#92).
#[test]
fn directive_body_is_dedented_and_round_trips() {
    let input =
        "Summary.\n\n.. deprecated:: 1.18.0\n    This function is deprecated. Use\n    `mpmath.pade` instead.\n";
    for style in ["numpy", "google"] {
        let (doc, emitted) = match style {
            "numpy" => {
                let doc = numpy_to_model(&parse_numpy(input)).unwrap();
                let emitted = pydocstring::emit::numpy::emit_numpy(&doc, &pydocstring::emit::EmitOptions::default());
                (doc, emitted)
            }
            _ => {
                let doc = google_to_model(&parse_google(input)).unwrap();
                let emitted = pydocstring::emit::google::emit_google(&doc, &pydocstring::emit::EmitOptions::default());
                (doc, emitted)
            }
        };
        let dep = doc.deprecation().expect("should have deprecation");
        assert_eq!(
            dep.description.as_deref(),
            Some("This function is deprecated. Use\n`mpmath.pade` instead."),
            "{style}: model must hold the dedented body"
        );
        assert!(
            emitted
                .contains(".. deprecated:: 1.18.0\n    This function is deprecated. Use\n    `mpmath.pade` instead.\n"),
            "{style}: emit must re-indent the body exactly once:\n{emitted}"
        );
        let reparsed = match style {
            "numpy" => numpy_to_model(&parse_numpy(&emitted)).unwrap(),
            _ => google_to_model(&parse_google(&emitted)).unwrap(),
        };
        assert_eq!(reparsed, doc, "{style}: directive round trip diverged:\n{emitted}");
    }
}

// =============================================================================
// NumPy → IR: Free text sections
// =============================================================================

#[test]
fn numpy_notes_section() {
    let parsed = parse_numpy(
        "Summary.

    Notes
    -----
    Some notes here.",
    );
    let doc = numpy_to_model(&parsed).unwrap();
    let (kind, body) = free_text_of(&doc.sections[0]);
    let (kind, body) = (&kind, body.as_str());
    assert_eq!(*kind, FreeSectionKind::Notes);
    assert!(!body.is_empty());
}

// =============================================================================
// NumPy → IR: Multiple sections
// =============================================================================

#[test]
fn numpy_multiple_sections() {
    let parsed = parse_numpy(
        "Summary.

        Parameters
        ----------
        x : int
            Val.
        Returns
        -------
        int
            Result.

        Raises
        ------
        ValueError
            Bad.",
    );
    let doc = numpy_to_model(&parsed).unwrap();
    assert_eq!(doc.sections.len(), 3);
    assert!(doc.sections[0].kind == SectionKind::Parameters);
    assert!(doc.sections[1].kind == SectionKind::Returns);
    assert!(doc.sections[2].kind == SectionKind::Raises);
}

// =============================================================================
// IR equality: same content from different styles
// =============================================================================

#[test]
fn same_ir_from_both_styles() {
    let google = parse_google(
        "Summary.

    Args:
        x (int): The value.",
    );
    let numpy = parse_numpy(
        "Summary.

    Parameters
    ----------
    x : int
        The value.",
    );

    let g = google_to_model(&google).unwrap();
    let n = numpy_to_model(&numpy).unwrap();

    assert_eq!(g.summary, n.summary);

    // Both should produce Parameters with the same content
    let gp = params_of(&g.sections[0]);
    let np = params_of(&n.sections[0]);
    assert_eq!(gp[0].names, np[0].names);
    assert_eq!(gp[0].type_annotation, np[0].type_annotation);
    assert_eq!(gp[0].description, np[0].description);
}

// =============================================================================
// NumPy IR: Google-style entries in NumPy sections
// =============================================================================

#[test]
fn numpy_google_style_entry_to_model() {
    let parsed = parse_numpy("Summary.\n\nParameters\n----------\nname (str): The name.\n");
    let doc = numpy_to_model(&parsed).unwrap();
    let params = params_of(&doc.sections[0]);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].names, vec!["name"]);
    assert_eq!(params[0].type_annotation.as_deref(), Some("str"));
    assert_eq!(params[0].description.as_deref(), Some("The name."));
}

#[test]
fn numpy_google_style_optional_to_model() {
    let parsed = parse_numpy("Summary.\n\nParameters\n----------\nname (str, optional): The name.\n");
    let doc = numpy_to_model(&parsed).unwrap();
    let params = params_of(&doc.sections[0]);
    assert_eq!(params[0].type_annotation.as_deref(), Some("str"));
    assert!(params[0].is_optional);
}

// =============================================================================
// Repeated markers: model normalization takes the FIRST occurrence (#41/#76)
// =============================================================================

/// SPEC: when a `default …` marker is repeated, the model's `default_value`
/// is the FIRST occurrence, in both styles. The CST keeps every occurrence
/// (pinned in tests/coverage.rs); which one wins is this model-layer rule.
#[test]
fn repeated_default_markers_first_occurrence_wins() {
    let parsed = parse_numpy("Summary.\n\nParameters\n----------\nx : int, default 1, default 2\n    Desc.\n");
    let doc = numpy_to_model(&parsed).unwrap();
    let params = params_of(&doc.sections[0]);
    assert_eq!(params[0].type_annotation.as_deref(), Some("int"));
    assert_eq!(params[0].default_value.as_deref(), Some("1"));

    let parsed = parse_google("Summary.\n\nArgs:\n    x (int, optional, default 1, default 2): Desc.\n");
    let doc = google_to_model(&parsed).unwrap();
    let params = params_of(&doc.sections[0]);
    assert_eq!(params[0].type_annotation.as_deref(), Some("int"));
    assert!(params[0].is_optional);
    assert_eq!(params[0].default_value.as_deref(), Some("1"));
}

/// SPEC: a repeated `optional` marker still reads as one `is_optional` flag
/// (the first occurrence wins; repetition adds no information).
#[test]
fn repeated_optional_markers_first_occurrence_wins() {
    let parsed = parse_numpy("Summary.\n\nParameters\n----------\nx : int, optional, optional\n    Desc.\n");
    let doc = numpy_to_model(&parsed).unwrap();
    let params = params_of(&doc.sections[0]);
    assert_eq!(params[0].type_annotation.as_deref(), Some("int"));
    assert!(params[0].is_optional);
}

#[test]
fn repeated_optional_markers_first_occurrence_wins_google() {
    let parsed = parse_google("Summary.\n\nArgs:\n    x (int, optional, optional): Desc.\n");
    let doc = google_to_model(&parsed).unwrap();
    let params = params_of(&doc.sections[0]);
    assert_eq!(params[0].type_annotation.as_deref(), Some("int"));
    assert!(params[0].is_optional);
}

/// SPEC: marker-like segments count only in the trailing suffix — a
/// non-marker segment after them makes the whole thing the type
/// (`int, optional, str` is a type, not an optional `int`).
#[test]
fn marker_like_segment_mid_type_is_part_of_the_type() {
    let parsed = parse_numpy("Summary.\n\nParameters\n----------\nx : int, optional, str\n    Desc.\n");
    let doc = numpy_to_model(&parsed).unwrap();
    let params = params_of(&doc.sections[0]);
    assert_eq!(params[0].type_annotation.as_deref(), Some("int, optional, str"));
    assert!(!params[0].is_optional);
    assert!(params[0].default_value.is_none());
}

// =============================================================================
// See Also round trips: multi-line + rST-role names (#90/#91)
// =============================================================================

/// The see-also normal form (description on the following indented line)
/// round-trips multi-line descriptions and rST-role names: the model
/// survives emit → parse unchanged in both styles.
#[test]
fn see_also_role_names_and_multiline_descriptions_round_trip() {
    // NumPy: an rST-role name plus a multi-line description.
    let parsed = parse_numpy(
        "Summary.\n\nSee Also\n--------\n:func:`csd`\n    Cross power spectral density\n    using Welch's method\nperiodogram, lombscargle\n",
    );
    let doc = numpy_to_model(&parsed).unwrap();
    let items = see_also_of(&doc.sections[0]);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].names, vec![":func:`csd`"]);
    assert_eq!(
        items[0].description.as_deref(),
        Some("Cross power spectral density\nusing Welch's method")
    );
    assert_eq!(items[1].names, vec!["periodogram", "lombscargle"]);
    assert_eq!(items[1].description, None);
    let emitted = pydocstring::emit::numpy::emit_numpy(&doc, &pydocstring::emit::EmitOptions::default());
    let reparsed = numpy_to_model(&parse_numpy(&emitted)).unwrap();
    assert_eq!(reparsed, doc, "numpy see-also round trip diverged:\n{emitted}");

    // Google: the equivalent normal form.
    let parsed = parse_google(
        "Summary.\n\nSee Also:\n    :func:`csd`\n        Cross power spectral density\n        using Welch's method\n    periodogram, lombscargle\n",
    );
    let doc = google_to_model(&parsed).unwrap();
    let items = see_also_of(&doc.sections[0]);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].names, vec![":func:`csd`"]);
    assert_eq!(
        items[0].description.as_deref(),
        Some("Cross power spectral density\nusing Welch's method")
    );
    assert_eq!(items[1].names, vec!["periodogram", "lombscargle"]);
    let emitted = pydocstring::emit::google::emit_google(&doc, &pydocstring::emit::EmitOptions::default());
    let reparsed = google_to_model(&parse_google(&emitted)).unwrap();
    assert_eq!(reparsed, doc, "google see-also round trip diverged:\n{emitted}");
}

// =============================================================================
// SPEC: the paragraph rule — bare Returns entries vs prose (#104, napoleon)
// =============================================================================
//
// The CST keeps every base-indent line of a structured section body as an
// ENTRY (local, predictable line grammar). Which bare entries *mean* prose is
// decided here, in to_model, mirroring napoleon:
//   - a LONE bare line in an entry-less Returns body is a type (:rtype:)
//   - a run of >=2 consecutive bare lines is one prose paragraph
//   - a blank line splits runs into separate paragraphs
//   - any bare line coexisting with a genuine entry is prose
// The rule is byte-neutral: paragraphs and type-only entries emit the same
// bare lines, so emit∘parse stays a fixed point either way.

#[test]
fn numpy_returns_lone_bare_line_is_a_type() {
    let doc = numpy_to_model(&parse_numpy("Summary.\n\nReturns\n-------\nint\n")).unwrap();
    let rets = returns_of(&doc.sections[0]);
    assert_eq!(rets.len(), 1);
    assert_eq!(rets[0].name, None);
    assert_eq!(rets[0].type_annotation.as_deref(), Some("int"));
    assert!(doc.sections[0].blocks.iter().all(|b| b.as_paragraph().is_none()));
}

#[test]
fn numpy_returns_prose_run_is_one_paragraph() {
    let doc = numpy_to_model(&parse_numpy(
        "Summary.\n\nReturns\n-------\nReturns a dict with normalized copies\nor updates `adata` in place.\n",
    ))
    .unwrap();
    let paras: Vec<&str> = doc.sections[0].blocks.iter().filter_map(Block::as_paragraph).collect();
    assert_eq!(
        paras,
        vec!["Returns a dict with normalized copies\nor updates `adata` in place."]
    );
    assert!(returns_of(&doc.sections[0]).is_empty());
}

#[test]
fn numpy_returns_blank_line_splits_paragraphs() {
    let doc = numpy_to_model(&parse_numpy(
        "Summary.\n\nReturns\n-------\nFirst paragraph line one\nand line two.\n\nSecond paragraph.\nIt has two lines.\n",
    ))
    .unwrap();
    let paras: Vec<&str> = doc.sections[0].blocks.iter().filter_map(Block::as_paragraph).collect();
    assert_eq!(
        paras,
        vec![
            "First paragraph line one\nand line two.",
            "Second paragraph.\nIt has two lines.",
        ]
    );
}

#[test]
fn numpy_returns_prose_intro_before_definition_list() {
    // The scverse shape (#104): a prose intro followed by named entries.
    let doc = numpy_to_model(&parse_numpy(
        "Summary.\n\nReturns\n-------\nSets the following fields:\n\n`.obsm['X_pca']` : ndarray\n    PCA representation.\n",
    ))
    .unwrap();
    let blocks = &doc.sections[0].blocks;
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].as_paragraph(), Some("Sets the following fields:"));
    let ret = blocks[1].as_return().expect("second block is the named return");
    assert_eq!(ret.name.as_deref(), Some("`.obsm['X_pca']`"));
    assert_eq!(ret.type_annotation.as_deref(), Some("ndarray"));

    // Byte-level round trip: the paragraph re-emits as the same bare line.
    let emitted = pydocstring::emit::numpy::emit_numpy(&doc, &pydocstring::emit::EmitOptions::default());
    let reparsed = numpy_to_model(&parse_numpy(&emitted)).unwrap();
    assert_eq!(reparsed, doc, "paragraph-rule round trip diverged:\n{emitted}");
}

#[test]
fn numpy_parameters_bare_line_stays_an_entry() {
    // Parameters bodies get NO paragraph rule: napoleon reads a bare line in
    // Parameters as a (type-less) parameter name, and so do we.
    let doc = numpy_to_model(&parse_numpy(
        "Summary.\n\nParameters\n----------\n(see Notes below)\n\nshape : tuple of ints\n    Shape of created array.\n",
    ))
    .unwrap();
    let params = params_of(&doc.sections[0]);
    assert_eq!(params.len(), 2);
    assert_eq!(params[0].names, vec!["(see Notes below)"]);
    assert_eq!(params[0].type_annotation, None);
    assert_eq!(params[1].names, vec!["shape"]);
}
