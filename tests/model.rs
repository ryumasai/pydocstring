//! Integration tests for the style-independent document model (IR).

use pydocstring::model::FreeSectionKind;
use pydocstring::model::Section;
use pydocstring::parse::google::parse_google;
use pydocstring::parse::google::to_model::to_model as google_to_model;
use pydocstring::parse::numpy::parse_numpy;
use pydocstring::parse::numpy::to_model::to_model as numpy_to_model;

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
    match &doc.sections[0] {
        Section::Parameters(params) => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].names, vec!["x"]);
            assert_eq!(params[0].type_annotation.as_deref(), Some("int"));
            assert_eq!(params[0].description.as_deref(), Some("The value."));
            assert!(!params[0].is_optional);
            assert_eq!(params[0].default_value, None);
        }
        other => panic!("expected Parameters, got {:?}", other),
    }
}

#[test]
fn google_args_optional() {
    let parsed = parse_google("Summary.\n\nArgs:\n    x (int, optional): The value.");
    let doc = google_to_model(&parsed).unwrap();
    match &doc.sections[0] {
        Section::Parameters(params) => {
            assert!(params[0].is_optional);
        }
        other => panic!("expected Parameters, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::Parameters(params) => {
            assert_eq!(params.len(), 3);
            assert_eq!(params[0].names, vec!["x"]);
            assert_eq!(params[0].description.as_deref(), Some("First."));

            assert_eq!(params[1].names, vec!["y"]);
            assert_eq!(params[1].description.as_deref(), Some("Second.\nMore description."));

            assert_eq!(params[2].names, vec!["z"]);
            assert_eq!(
                params[2].description.as_deref(),
                Some(
                    "Third.\nMore description.\n\n.. directive:: something\n   directive_option\n\ncontinued description."
                )
            );
        }
        other => panic!("expected Parameters, got {:?}", other),
    }
}

#[test]
fn google_args_no_type() {
    let parsed = parse_google("Summary.\n\nArgs:\n    x: The value.");
    let doc = google_to_model(&parsed).unwrap();
    match &doc.sections[0] {
        Section::Parameters(params) => {
            assert_eq!(params[0].type_annotation, None);
        }
        other => panic!("expected Parameters, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::Returns(returns) => {
            assert_eq!(returns.len(), 1);
            assert_eq!(returns[0].name, None);
            assert_eq!(returns[0].type_annotation.as_deref(), Some("int"));
            assert_eq!(
                returns[0].description.as_deref(),
                Some("The result.\nMore description.")
            );
        }
        other => panic!("expected Returns, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::Returns(returns) => {
            assert_eq!(returns.len(), 1);
            assert_eq!(returns[0].type_annotation, None);
            assert_eq!(
                returns[0].description.as_deref(),
                Some("The result of executing the command.\nExecution begins with the target.")
            );
        }
        other => panic!("expected Returns, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::Raises(entries) => {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].type_name, "ValueError");
            assert_eq!(
                entries[0].description.as_deref(),
                Some("If bad.\nMore description.\n\nEven more.")
            );
        }
        other => panic!("expected Raises, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::Warns(entries) => {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].type_name, "UserWarning");
            assert_eq!(entries[0].description.as_deref(), Some("Watch out.\nDetails."));
        }
        other => panic!("expected Warns, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::FreeText { kind, body } => {
            assert_eq!(*kind, FreeSectionKind::Notes);
            assert!(!body.is_empty());
            assert_eq!(
                body,
                "Some notes here.\nMore notes.\n\n- Unordered list.\n  same item\n- next item"
            )
        }
        other => panic!("expected FreeText, got {:?}", other),
    }
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
    assert!(matches!(&doc.sections[0], Section::Parameters(_)));
    assert!(matches!(&doc.sections[1], Section::Returns(_)));
    match &doc.sections[2] {
        Section::Raises(entries) => {
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].type_name, "ValueError");
            assert_eq!(entries[0].description.as_deref(), Some("Bad."));
            assert_eq!(entries[1].type_name, "KeyError");
            assert_eq!(
                entries[1].description.as_deref(),
                Some("Very bad.\nOnly raised when key is not found.")
            );
        }
        other => panic!("expected Raises, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::Parameters(params) => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].names, vec!["x"]);
            assert_eq!(params[0].type_annotation.as_deref(), Some("int"));
            assert_eq!(params[0].description.as_deref(), Some("The value."));
        }
        other => panic!("expected Parameters, got {:?}", other),
    }
}

#[test]
fn numpy_parameters_optional() {
    let parsed = parse_numpy("Summary.\n\nParameters\n----------\nx : int, optional\n    The value.");
    let doc = numpy_to_model(&parsed).unwrap();
    match &doc.sections[0] {
        Section::Parameters(params) => {
            assert!(params[0].is_optional);
        }
        other => panic!("expected Parameters, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::Parameters(params) => {
            assert_eq!(params.len(), 3);
            assert_eq!(params[0].names, vec!["x"]);
            assert_eq!(params[0].description.as_deref(), Some("First."));

            assert_eq!(params[1].names, vec!["y"]);
            assert_eq!(params[1].description.as_deref(), Some("Second.\nMore description."));

            assert_eq!(params[2].names, vec!["z"]);
            assert_eq!(
                params[2].description.as_deref(),
                Some(
                    "Third.\nMore description.\n\n.. directive:: something\n   directive_option\n\ncontinued description."
                )
            );
        }
        other => panic!("expected Parameters, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::Parameters(params) => {
            assert_eq!(params[0].names, vec!["x", "y"]);
        }
        other => panic!("expected Parameters, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::Attributes(attrs) => {
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0].names, vec!["jac", "hess"]);
            assert_eq!(attrs[0].type_annotation.as_deref(), Some("ndarray"));
            assert_eq!(attrs[0].description.as_deref(), Some("Derivatives."));
        }
        other => panic!("expected Attributes, got {:?}", other),
    }
}

#[test]
fn google_attributes_multiple_names() {
    let parsed = parse_google("Summary.\n\nAttributes:\n    jac, hess (ndarray): Derivatives.");
    let doc = google_to_model(&parsed).unwrap();
    match &doc.sections[0] {
        Section::Attributes(attrs) => {
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0].names, vec!["jac", "hess"]);
            assert_eq!(attrs[0].type_annotation.as_deref(), Some("ndarray"));
            assert_eq!(attrs[0].description.as_deref(), Some("Derivatives."));
        }
        other => panic!("expected Attributes, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::Parameters(params) => {
            assert_eq!(params[0].default_value.as_deref(), Some("0"));
        }
        other => panic!("expected Parameters, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::Returns(returns) => {
            assert_eq!(returns.len(), 1);
            assert_eq!(returns[0].type_annotation.as_deref(), Some("int"));
            assert_eq!(
                returns[0].description.as_deref(),
                Some("The result.\nMore description.")
            )
        }
        other => panic!("expected Returns, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::Raises(entries) => {
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].type_name, "ValueError");
            assert_eq!(entries[0].description.as_deref(), Some("If bad."));
            assert_eq!(entries[1].type_name, "KeyError");
            assert_eq!(
                entries[1].description.as_deref(),
                Some("If very bad.\n\n    blockquote\n\n1. first item\n   still first item\n2. second item")
            );
        }
        other => panic!("expected Raises, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::FreeText { kind, body } => {
            assert_eq!(*kind, FreeSectionKind::Notes);
            assert!(!body.is_empty());
        }
        other => panic!("expected FreeText, got {:?}", other),
    }
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
    assert!(matches!(&doc.sections[0], Section::Parameters(_)));
    assert!(matches!(&doc.sections[1], Section::Returns(_)));
    assert!(matches!(&doc.sections[2], Section::Raises(_)));
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
    match (&g.sections[0], &n.sections[0]) {
        (Section::Parameters(gp), Section::Parameters(np)) => {
            assert_eq!(gp[0].names, np[0].names);
            assert_eq!(gp[0].type_annotation, np[0].type_annotation);
            assert_eq!(gp[0].description, np[0].description);
        }
        _ => panic!("both should be Parameters sections"),
    }
}

// =============================================================================
// NumPy IR: Google-style entries in NumPy sections
// =============================================================================

#[test]
fn numpy_google_style_entry_to_model() {
    let parsed = parse_numpy("Summary.\n\nParameters\n----------\nname (str): The name.\n");
    let doc = numpy_to_model(&parsed).unwrap();
    let params = match &doc.sections[0] {
        Section::Parameters(p) => p,
        _ => panic!("expected Parameters"),
    };
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].names, vec!["name"]);
    assert_eq!(params[0].type_annotation.as_deref(), Some("str"));
    assert_eq!(params[0].description.as_deref(), Some("The name."));
}

#[test]
fn numpy_google_style_optional_to_model() {
    let parsed = parse_numpy("Summary.\n\nParameters\n----------\nname (str, optional): The name.\n");
    let doc = numpy_to_model(&parsed).unwrap();
    let params = match &doc.sections[0] {
        Section::Parameters(p) => p,
        _ => panic!("expected Parameters"),
    };
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
    match &doc.sections[0] {
        Section::Parameters(params) => {
            assert_eq!(params[0].type_annotation.as_deref(), Some("int"));
            assert_eq!(params[0].default_value.as_deref(), Some("1"));
        }
        other => panic!("expected Parameters, got {:?}", other),
    }

    let parsed = parse_google("Summary.\n\nArgs:\n    x (int, optional, default 1, default 2): Desc.\n");
    let doc = google_to_model(&parsed).unwrap();
    match &doc.sections[0] {
        Section::Parameters(params) => {
            assert_eq!(params[0].type_annotation.as_deref(), Some("int"));
            assert!(params[0].is_optional);
            assert_eq!(params[0].default_value.as_deref(), Some("1"));
        }
        other => panic!("expected Parameters, got {:?}", other),
    }
}

/// SPEC: a repeated `optional` marker still reads as one `is_optional` flag
/// (the first occurrence wins; repetition adds no information).
#[test]
fn repeated_optional_markers_first_occurrence_wins() {
    let parsed = parse_numpy("Summary.\n\nParameters\n----------\nx : int, optional, optional\n    Desc.\n");
    let doc = numpy_to_model(&parsed).unwrap();
    match &doc.sections[0] {
        Section::Parameters(params) => {
            assert_eq!(params[0].type_annotation.as_deref(), Some("int"));
            assert!(params[0].is_optional);
        }
        other => panic!("expected Parameters, got {:?}", other),
    }
}

#[test]
fn repeated_optional_markers_first_occurrence_wins_google() {
    let parsed = parse_google("Summary.\n\nArgs:\n    x (int, optional, optional): Desc.\n");
    let doc = google_to_model(&parsed).unwrap();
    match &doc.sections[0] {
        Section::Parameters(params) => {
            assert_eq!(params[0].type_annotation.as_deref(), Some("int"));
            assert!(params[0].is_optional);
        }
        other => panic!("expected Parameters, got {:?}", other),
    }
}

/// SPEC: marker-like segments count only in the trailing suffix — a
/// non-marker segment after them makes the whole thing the type
/// (`int, optional, str` is a type, not an optional `int`).
#[test]
fn marker_like_segment_mid_type_is_part_of_the_type() {
    let parsed = parse_numpy("Summary.\n\nParameters\n----------\nx : int, optional, str\n    Desc.\n");
    let doc = numpy_to_model(&parsed).unwrap();
    match &doc.sections[0] {
        Section::Parameters(params) => {
            assert_eq!(params[0].type_annotation.as_deref(), Some("int, optional, str"));
            assert!(!params[0].is_optional);
            assert!(params[0].default_value.is_none());
        }
        other => panic!("expected Parameters, got {:?}", other),
    }
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
    match &doc.sections[0] {
        Section::SeeAlso(items) => {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].names, vec![":func:`csd`"]);
            assert_eq!(
                items[0].description.as_deref(),
                Some("Cross power spectral density\nusing Welch's method")
            );
            assert_eq!(items[1].names, vec!["periodogram", "lombscargle"]);
            assert_eq!(items[1].description, None);
        }
        other => panic!("expected SeeAlso, got {:?}", other),
    }
    let emitted = pydocstring::emit::numpy::emit_numpy(&doc, &pydocstring::emit::EmitOptions::default());
    let reparsed = numpy_to_model(&parse_numpy(&emitted)).unwrap();
    assert_eq!(reparsed, doc, "numpy see-also round trip diverged:\n{emitted}");

    // Google: the equivalent normal form.
    let parsed = parse_google(
        "Summary.\n\nSee Also:\n    :func:`csd`\n        Cross power spectral density\n        using Welch's method\n    periodogram, lombscargle\n",
    );
    let doc = google_to_model(&parsed).unwrap();
    match &doc.sections[0] {
        Section::SeeAlso(items) => {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].names, vec![":func:`csd`"]);
            assert_eq!(
                items[0].description.as_deref(),
                Some("Cross power spectral density\nusing Welch's method")
            );
            assert_eq!(items[1].names, vec!["periodogram", "lombscargle"]);
        }
        other => panic!("expected SeeAlso, got {:?}", other),
    }
    let emitted = pydocstring::emit::google::emit_google(&doc, &pydocstring::emit::EmitOptions::default());
    let reparsed = google_to_model(&parse_google(&emitted)).unwrap();
    assert_eq!(reparsed, doc, "google see-also round trip diverged:\n{emitted}");
}
