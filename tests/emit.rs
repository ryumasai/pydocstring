//! Integration tests for emit (Model → docstring text).

use pydocstring::emit::google::emit_google;
use pydocstring::emit::numpy::emit_numpy;
use pydocstring::model::*;

// =============================================================================
// Google emit
// =============================================================================

#[test]
fn google_emit_summary_only() {
    let doc = Docstring {
        summary: Some("Brief summary.".into()),
        ..Default::default()
    };
    assert_eq!(emit_google(&doc, 0), "Brief summary.\n");
}

#[test]
fn google_emit_summary_and_extended() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        extended_summary: Some("Extended description\nspanning lines.".into()),
        ..Default::default()
    };
    let text = emit_google(&doc, 0);
    assert_eq!(text, "Summary.\n\nExtended description\nspanning lines.\n");
}

#[test]
fn google_emit_args() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Parameters(vec![
            Parameter {
                names: vec!["x".into()],
                type_annotation: Some("int".into()),
                description: Some("The value.\nMore description.\n\n    blockquote\n\nlast line.".into()),
                is_optional: false,
                default_value: None,
                ..Default::default()
            },
            Parameter {
                names: vec!["y".into()],
                type_annotation: Some("str".into()),
                description: Some("The name.".into()),
                is_optional: false,
                default_value: None,
                ..Default::default()
            },
        ])],
        ..Default::default()
    };
    let text = emit_google(&doc, 0);
    assert!(text.contains("Args:\n"));
    assert!(text.contains(
        "    x (int): The value.\n        More description.\n\n            blockquote\n\n        last line."
    ));
    assert!(text.contains("    y (str): The name.\n"));
}

#[test]
fn google_emit_args_optional() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("The value.".into()),
            is_optional: true,
            default_value: None,
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, 0);
    assert!(text.contains("    x (int, optional): The value.\n"));
}

#[test]
fn google_emit_args_no_type() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: None,
            description: Some("The value.".into()),
            is_optional: false,
            default_value: None,
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, 0);
    assert!(text.contains("    x: The value.\n"));
}

#[test]
fn google_emit_returns() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Returns(vec![Return {
            name: None,
            type_annotation: Some("int".into()),
            description: Some("The result.".into()),
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, 0);
    assert!(text.contains("Returns:\n"));
    assert!(text.contains("    int: The result.\n"));
}

#[test]
fn google_emit_returns_no_type() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Returns(vec![Return {
            name: None,
            type_annotation: None,
            description: Some("The computed result.".into()),
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, 0);
    assert!(text.contains("    The computed result.\n"));
}

#[test]
fn google_emit_raises() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Raises(vec![
            ExceptionEntry {
                type_name: "ValueError".into(),
                description: Some("If the input is invalid.".into()),
                ..Default::default()
            },
            ExceptionEntry {
                type_name: "TypeError".into(),
                description: Some("If wrong type.".into()),
                ..Default::default()
            },
        ])],
        ..Default::default()
    };
    let text = emit_google(&doc, 0);
    assert!(text.contains("Raises:\n"));
    assert!(text.contains("    ValueError: If the input is invalid.\n"));
    assert!(text.contains("    TypeError: If wrong type.\n"));
}

#[test]
fn google_emit_notes() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::FreeText {
            kind: FreeSectionKind::Notes,
            body: "Some notes here.".into(),
        }],
        ..Default::default()
    };
    let text = emit_google(&doc, 0);
    assert!(text.contains("Notes:\n"));
    assert!(text.contains("    Some notes here.\n"));
}

#[test]
fn google_emit_multiple_sections() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![
            Section::Parameters(vec![Parameter {
                names: vec!["x".into()],
                type_annotation: Some("int".into()),
                description: Some("Val.".into()),
                is_optional: false,
                default_value: None,
                ..Default::default()
            }]),
            Section::Returns(vec![Return {
                name: None,
                type_annotation: Some("str".into()),
                description: Some("Result.".into()),
                ..Default::default()
            }]),
            Section::Raises(vec![ExceptionEntry {
                type_name: "ValueError".into(),
                description: Some("Bad.".into()),
                ..Default::default()
            }]),
        ],
        ..Default::default()
    };
    let text = emit_google(&doc, 0);
    assert!(text.contains("Args:\n"));
    assert!(text.contains("Returns:\n"));
    assert!(text.contains("Raises:\n"));
}

#[test]
fn google_emit_attributes() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Attributes(vec![Attribute {
            name: "name".into(),
            type_annotation: Some("str".into()),
            description: Some("The name.".into()),
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, 0);
    assert!(text.contains("Attributes:\n"));
    assert!(text.contains("    name (str): The name.\n"));
}

#[test]
fn google_emit_see_also() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::SeeAlso(vec![SeeAlsoEntry {
            names: vec!["func1".into(), "func2".into()],
            description: Some("Related functions.".into()),
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, 0);
    assert!(text.contains("See Also:\n"));
    assert!(text.contains("    func1, func2: Related functions.\n"));
}

// =============================================================================
// NumPy emit
// =============================================================================

#[test]
fn numpy_emit_summary_only() {
    let doc = Docstring {
        summary: Some("Brief summary.".into()),
        ..Default::default()
    };
    assert_eq!(emit_numpy(&doc, 0), "Brief summary.\n");
}

#[test]
fn numpy_emit_summary_and_extended() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        extended_summary: Some("Extended description.".into()),
        ..Default::default()
    };
    let text = emit_numpy(&doc, 0);
    assert_eq!(text, "Summary.\n\nExtended description.\n");
}

#[test]
fn numpy_emit_parameters() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Parameters(vec![
            Parameter {
                names: vec!["x".into()],
                type_annotation: Some("int".into()),
                description: Some("The first number.\nMore description\n\n    blockquote\n\nlast line.".into()),
                is_optional: false,
                default_value: None,
                ..Default::default()
            },
            Parameter {
                names: vec!["y".into()],
                type_annotation: Some("int".into()),
                description: Some("The second number.".into()),
                is_optional: false,
                default_value: None,
                ..Default::default()
            },
        ])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, 0);
    assert!(text.contains("Parameters\n----------\n"));
    assert!(
        text.contains("x : int\n    The first number.\n    More description\n\n        blockquote\n\n    last line.")
    );
    assert!(text.contains("y : int\n    The second number.\n"));
}

#[test]
fn numpy_emit_parameters_optional() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("The value.".into()),
            is_optional: true,
            default_value: None,
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, 0);
    assert!(text.contains("x : int, optional\n"));
}

#[test]
fn numpy_emit_parameters_multiple_names() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Parameters(vec![Parameter {
            names: vec!["x".into(), "y".into()],
            type_annotation: Some("float".into()),
            description: Some("Values.".into()),
            is_optional: false,
            default_value: None,
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, 0);
    assert!(text.contains("x, y : float\n"));
}

#[test]
fn numpy_emit_parameters_default() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("The value.".into()),
            is_optional: false,
            default_value: Some("0".into()),
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, 0);
    assert!(text.contains("x : int, default: 0\n"));
}

#[test]
fn numpy_emit_returns_named() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Returns(vec![Return {
            name: Some("result".into()),
            type_annotation: Some("int".into()),
            description: Some("The result.".into()),
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, 0);
    assert!(text.contains("Returns\n-------\n"));
    assert!(text.contains("result : int\n    The result.\n"));
}

#[test]
fn numpy_emit_returns_type_only() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Returns(vec![Return {
            name: None,
            type_annotation: Some("int".into()),
            description: Some("The result.".into()),
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, 0);
    assert!(text.contains("int\n    The result.\n"));
}

#[test]
fn numpy_emit_raises() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Raises(vec![
            ExceptionEntry {
                type_name: "ValueError".into(),
                description: Some("If the input is invalid.".into()),
                ..Default::default()
            },
            ExceptionEntry {
                type_name: "TypeError".into(),
                description: Some("If the type is wrong.".into()),
                ..Default::default()
            },
        ])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, 0);
    assert!(text.contains("Raises\n------\n"));
    assert!(text.contains("ValueError\n    If the input is invalid.\n"));
    assert!(text.contains("TypeError\n    If the type is wrong.\n"));
}

#[test]
fn numpy_emit_deprecation() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        deprecation: Some(Deprecation {
            version: "1.6.0".into(),
            description: Some("Use `other` instead.".into()),
        }),
        ..Default::default()
    };
    let text = emit_numpy(&doc, 0);
    assert!(text.contains(".. deprecated:: 1.6.0\n"));
    assert!(text.contains("    Use `other` instead.\n"));
}

#[test]
fn numpy_emit_notes() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::FreeText {
            kind: FreeSectionKind::Notes,
            body: "Some notes here.".into(),
        }],
        ..Default::default()
    };
    let text = emit_numpy(&doc, 0);
    assert!(text.contains("Notes\n-----\n"));
    assert!(text.contains("Some notes here.\n"));
}

#[test]
fn numpy_emit_references() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::References(vec![
            Reference {
                number: Some("1".into()),
                content: Some("Author, Title, Journal.".into()),
                ..Default::default()
            },
            Reference {
                number: Some("2".into()),
                content: Some("Another reference.".into()),
                ..Default::default()
            },
        ])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, 0);
    assert!(text.contains("References\n----------\n"));
    assert!(text.contains(".. [1] Author, Title, Journal.\n"));
    assert!(text.contains(".. [2] Another reference.\n"));
}

#[test]
fn numpy_emit_attributes() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Attributes(vec![Attribute {
            name: "name".into(),
            type_annotation: Some("str".into()),
            description: Some("The name.".into()),
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, 0);
    assert!(text.contains("Attributes\n----------\n"));
    assert!(text.contains("name : str\n    The name.\n"));
}

#[test]
fn numpy_emit_see_also() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::SeeAlso(vec![SeeAlsoEntry {
            names: vec!["func1".into(), "func2".into()],
            description: Some("Related.".into()),
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, 0);
    assert!(text.contains("See Also\n--------\n"));
    assert!(text.contains("func1, func2 : Related.\n"));
}

#[test]
fn numpy_emit_multiple_sections() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![
            Section::Parameters(vec![Parameter {
                names: vec!["x".into()],
                type_annotation: Some("int".into()),
                description: Some("Val.".into()),
                is_optional: false,
                default_value: None,
                ..Default::default()
            }]),
            Section::Returns(vec![Return {
                name: None,
                type_annotation: Some("int".into()),
                description: Some("Result.".into()),
                ..Default::default()
            }]),
        ],
        ..Default::default()
    };
    let text = emit_numpy(&doc, 0);
    assert!(text.contains("Parameters\n----------\n"));
    assert!(text.contains("Returns\n-------\n"));
}

// =============================================================================
// Round-trip: parse → to_model → emit
// =============================================================================

#[test]
fn google_roundtrip_summary() {
    use pydocstring::parse::google::{parse_google, to_model::to_model};

    let input = "Summary line.";
    let doc = to_model(&parse_google(input)).unwrap();
    let output = emit_google(&doc, 0);
    assert_eq!(output.trim(), input);
}

#[test]
fn numpy_roundtrip_summary() {
    use pydocstring::parse::numpy::{parse_numpy, to_model::to_model};

    let input = "Summary line.";
    let doc = to_model(&parse_numpy(input)).unwrap();
    let output = emit_numpy(&doc, 0);
    assert_eq!(output.trim(), input);
}

// =============================================================================
// Regression: Issue #26 — rST role colons must not be eaten
// =============================================================================

/// NumPy Returns section that mixes prose with rST `:attr:` role references.
/// The trailing colon of "Description with attributes:" and the leading colons
/// of the `:attr:` lines must survive a parse → emit round-trip.
#[test]
fn numpy_roundtrip_rst_role_colons() {
    use pydocstring::parse::numpy::{parse_numpy, to_model::to_model};

    let input = "Returns\n-------\nDescription with attributes:\n:attr:`~module.ClassName.attr1`\n    First attribute\n:attr:`~module.ClassName.attr2`\n    Second attribute\n";
    let output = emit_numpy(&to_model(&parse_numpy(input)).unwrap(), 0);
    assert!(
        output.contains("Description with attributes:"),
        "trailing colon eaten:\n{output}"
    );
    assert!(
        output.contains(":attr:`~module.ClassName.attr1`"),
        "leading colon eaten:\n{output}"
    );
    assert!(
        output.contains(":attr:`~module.ClassName.attr2`"),
        "leading colon eaten:\n{output}"
    );
}

/// `to_model` normalizes by default: blank lines between entries are dropped.
/// `to_model_with_options(preserve_blank_lines)` opts into preserving them.
#[test]
fn numpy_blank_lines_between_returns_are_opt_in() {
    use pydocstring::model::Section;
    use pydocstring::parse::ToModelOptions;
    use pydocstring::parse::numpy::parse_numpy;
    use pydocstring::parse::numpy::to_model::{to_model, to_model_with_options};

    let input = "Summary.\n\nReturns\n-------\nDescription with attributes:\n\n:attr:`~module.ClassName.attr1`\n    First attribute\n";
    let parsed = parse_numpy(input);

    // Default: normalized — blank line dropped.
    let normalized = to_model(&parsed).unwrap();
    let Section::Returns(entries) = &normalized.sections[0] else {
        panic!("expected Returns section");
    };
    assert_eq!(entries[1].blank_lines_before, 0, "default should normalize");
    assert!(
        !emit_numpy(&normalized, 0).contains("attributes:\n\n:attr:"),
        "default emit should not contain the blank line"
    );

    // Opt-in: blank line captured and round-trips exactly.
    let preserved = to_model_with_options(
        &parsed,
        ToModelOptions {
            preserve_blank_lines: true,
        },
    )
    .unwrap();
    let Section::Returns(entries) = &preserved.sections[0] else {
        panic!("expected Returns section");
    };
    assert_eq!(entries[0].blank_lines_before, 0);
    assert_eq!(entries[1].blank_lines_before, 1, "blank line not captured");
    assert_eq!(emit_numpy(&preserved, 0), input, "blank line not preserved");
}

/// Blank-line preservation is general: it works for both styles and for entry
/// types other than `Returns` (here Google `Args` and NumPy `Parameters`).
#[test]
fn blank_lines_preserved_across_styles_and_sections() {
    use pydocstring::parse::ToModelOptions;
    use pydocstring::parse::google::{parse_google, to_model::to_model_with_options as google_to_model};
    use pydocstring::parse::numpy::{parse_numpy, to_model::to_model_with_options as numpy_to_model};

    let opts = ToModelOptions {
        preserve_blank_lines: true,
    };

    let google = "Summary.\n\nArgs:\n    x (int): The x.\n\n    y (int): The y.\n";
    assert_eq!(
        emit_google(&google_to_model(&parse_google(google), opts).unwrap(), 0),
        google,
        "Google Args blank line not preserved"
    );

    let numpy = "Summary.\n\nParameters\n----------\nx : int\n    The x.\n\ny : int\n    The y.\n";
    assert_eq!(
        emit_numpy(&numpy_to_model(&parse_numpy(numpy), opts).unwrap(), 0),
        numpy,
        "NumPy Parameters blank line not preserved"
    );
}

/// Google Returns section with a leading `:attr:` rST role must keep its colon.
#[test]
fn google_roundtrip_rst_role_colons() {
    use pydocstring::parse::google::{parse_google, to_model::to_model};

    let input = "Summary.\n\nReturns:\n    :attr:`~module.ClassName.attr1`\n        First attribute\n";
    let output = emit_google(&to_model(&parse_google(input)).unwrap(), 0);
    assert!(
        output.contains(":attr:`~module.ClassName.attr1`"),
        "leading colon eaten:\n{output}"
    );
}

// =============================================================================
// Cross-style conversion: Google → Model → NumPy
// =============================================================================

#[test]
fn google_to_numpy_conversion() {
    use pydocstring::parse::google::{parse_google, to_model::to_model};

    let google_input = "Summary.\n\nArgs:\n    x (int): The value.";
    let doc = to_model(&parse_google(google_input)).unwrap();
    let numpy_output = emit_numpy(&doc, 0);
    assert!(numpy_output.contains("Parameters\n----------\n"));
    assert!(numpy_output.contains("x : int\n    The value.\n"));
}

#[test]
fn numpy_to_google_conversion() {
    use pydocstring::parse::numpy::{parse_numpy, to_model::to_model};

    let numpy_input = "Summary.\n\nParameters\n----------\nx : int\n    The value.\n";
    let doc = to_model(&parse_numpy(numpy_input)).unwrap();
    let google_output = emit_google(&doc, 0);
    assert!(google_output.contains("Args:\n"));
    assert!(google_output.contains("    x (int): The value.\n"));
}

// =============================================================================
// Base indentation
// =============================================================================

#[test]
fn google_emit_with_base_indent() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("The value.".into()),
            is_optional: false,
            default_value: None,
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, 4);
    assert!(text.contains("    Summary.\n"));
    assert!(text.contains("    Args:\n"));
    assert!(text.contains("        x (int): The value.\n"));
}

#[test]
fn numpy_emit_with_base_indent() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::Parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("The value.".into()),
            is_optional: false,
            default_value: None,
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, 4);
    assert!(text.contains("    Summary.\n"));
    assert!(text.contains("    Parameters\n    ----------\n"));
    assert!(text.contains("    x : int\n        The value.\n"));
}

#[test]
fn google_emit_base_indent_preserves_blank_lines() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        extended_summary: Some("Extended.".into()),
        sections: vec![Section::Parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("Val.".into()),
            is_optional: false,
            default_value: None,
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, 2);
    // Blank lines between sections should stay empty (no trailing whitespace).
    assert!(text.contains("  Summary.\n\n  Extended.\n\n  Args:\n"));
}

#[test]
fn numpy_emit_base_indent_preserves_blank_lines() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        extended_summary: Some("Extended.".into()),
        sections: vec![Section::Parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("Val.".into()),
            is_optional: false,
            default_value: None,
            ..Default::default()
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, 2);
    // Blank lines between sections should stay empty (no trailing whitespace).
    assert!(text.contains("  Summary.\n\n  Extended.\n\n  Parameters\n"));
}
