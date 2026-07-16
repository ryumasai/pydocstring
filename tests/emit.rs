//! Integration tests for emit (Model → docstring text).

use pydocstring::emit::EmitOptions;
use pydocstring::emit::google::emit_google;
use pydocstring::emit::numpy::emit_numpy;
use pydocstring::emit::sphinx::emit_sphinx;
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
    assert_eq!(emit_google(&doc, &EmitOptions::default()), "Brief summary.\n");
}

#[test]
fn google_emit_summary_and_extended() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        extended_summary: Some("Extended description\nspanning lines.".into()),
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
    assert_eq!(text, "Summary.\n\nExtended description\nspanning lines.\n");
}

#[test]
fn google_emit_args() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::parameters(vec![
            Parameter {
                names: vec!["x".into()],
                type_annotation: Some("int".into()),
                description: Some("The value.\nMore description.\n\n    blockquote\n\nlast line.".into()),
                is_optional: false,
                default_value: None,
            },
            Parameter {
                names: vec!["y".into()],
                type_annotation: Some("str".into()),
                description: Some("The name.".into()),
                is_optional: false,
                default_value: None,
            },
        ])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
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
        sections: vec![Section::parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("The value.".into()),
            is_optional: true,
            default_value: None,
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
    assert!(text.contains("    x (int, optional): The value.\n"));
}

#[test]
fn google_emit_args_no_type() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: None,
            description: Some("The value.".into()),
            is_optional: false,
            default_value: None,
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
    assert!(text.contains("    x: The value.\n"));
}

#[test]
fn google_emit_returns() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::returns(vec![Return {
            name: None,
            type_annotation: Some("int".into()),
            description: Some("The result.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
    assert!(text.contains("Returns:\n"));
    assert!(text.contains("    int: The result.\n"));
}

#[test]
fn google_emit_returns_no_type() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::returns(vec![Return {
            name: None,
            type_annotation: None,
            description: Some("The computed result.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
    assert!(text.contains("    The computed result.\n"));
}

/// A description-only Return's continuation lines stay indented inside the
/// section body; at column 0 they would dedent out of the Returns section
/// and be dropped on re-parse (#93).
#[test]
fn google_emit_returns_no_type_multiline() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::returns(vec![Return {
            name: None,
            type_annotation: None,
            description: Some("The result of executing the command.\nExecution begins with the target.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
    assert!(
        text.contains("    The result of executing the command.\n    Execution begins with the target.\n"),
        "continuation lines must stay in the section body:\n{text}"
    );
}

#[test]
fn google_emit_raises() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::raises(vec![
            ExceptionEntry {
                type_name: "ValueError".into(),
                description: Some("If the input is invalid.".into()),
            },
            ExceptionEntry {
                type_name: "TypeError".into(),
                description: Some("If wrong type.".into()),
            },
        ])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
    assert!(text.contains("Raises:\n"));
    assert!(text.contains("    ValueError: If the input is invalid.\n"));
    assert!(text.contains("    TypeError: If wrong type.\n"));
}

#[test]
fn google_emit_notes() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::free_text(FreeSectionKind::Notes, "Some notes here.".into())],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
    assert!(text.contains("Notes:\n"));
    assert!(text.contains("    Some notes here.\n"));
}

#[test]
fn google_emit_multiple_sections() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![
            Section::parameters(vec![Parameter {
                names: vec!["x".into()],
                type_annotation: Some("int".into()),
                description: Some("Val.".into()),
                is_optional: false,
                default_value: None,
            }]),
            Section::returns(vec![Return {
                name: None,
                type_annotation: Some("str".into()),
                description: Some("Result.".into()),
            }]),
            Section::raises(vec![ExceptionEntry {
                type_name: "ValueError".into(),
                description: Some("Bad.".into()),
            }]),
        ],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
    assert!(text.contains("Args:\n"));
    assert!(text.contains("Returns:\n"));
    assert!(text.contains("Raises:\n"));
}

#[test]
fn google_emit_attributes() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::attributes(vec![Attribute {
            names: vec!["name".into()],
            type_annotation: Some("str".into()),
            description: Some("The name.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
    assert!(text.contains("Attributes:\n"));
    assert!(text.contains("    name (str): The name.\n"));
}

/// Multi-name attribute entries emit the FULL name list (#89).
#[test]
fn google_emit_attributes_multiple_names() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::attributes(vec![Attribute {
            names: vec!["jac".into(), "hess".into()],
            type_annotation: Some("ndarray".into()),
            description: Some("Derivatives.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
    assert!(text.contains("    jac, hess (ndarray): Derivatives.\n"));
}

#[test]
fn google_emit_see_also() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::see_also(vec![SeeAlsoEntry {
            names: vec!["func1".into(), "func2".into()],
            description: Some("Related functions.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
    assert!(text.contains("See Also:\n"));
    // Normal form (#91): the description goes on the following
    // deeper-indented line, never a `name: desc` one-liner.
    assert!(text.contains("    func1, func2\n        Related functions.\n"));
}

/// rST-role names have no valid `name: desc` one-liner (the #26 leading-colon
/// guard rejects it on re-parse); the next-line form round-trips (#91).
#[test]
fn google_emit_see_also_role_name_multiline() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::see_also(vec![SeeAlsoEntry {
            names: vec![":func:`csd`".into()],
            description: Some("Cross power spectral density\nusing Welch's method".into()),
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
    assert!(text.contains("    :func:`csd`\n        Cross power spectral density\n        using Welch's method\n"));
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
    assert_eq!(emit_numpy(&doc, &EmitOptions::default()), "Brief summary.\n");
}

#[test]
fn numpy_emit_summary_and_extended() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        extended_summary: Some("Extended description.".into()),
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert_eq!(text, "Summary.\n\nExtended description.\n");
}

#[test]
fn numpy_emit_parameters() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::parameters(vec![
            Parameter {
                names: vec!["x".into()],
                type_annotation: Some("int".into()),
                description: Some("The first number.\nMore description\n\n    blockquote\n\nlast line.".into()),
                is_optional: false,
                default_value: None,
            },
            Parameter {
                names: vec!["y".into()],
                type_annotation: Some("int".into()),
                description: Some("The second number.".into()),
                is_optional: false,
                default_value: None,
            },
        ])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
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
        sections: vec![Section::parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("The value.".into()),
            is_optional: true,
            default_value: None,
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert!(text.contains("x : int, optional\n"));
}

#[test]
fn numpy_emit_parameters_multiple_names() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::parameters(vec![Parameter {
            names: vec!["x".into(), "y".into()],
            type_annotation: Some("float".into()),
            description: Some("Values.".into()),
            is_optional: false,
            default_value: None,
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert!(text.contains("x, y : float\n"));
}

#[test]
fn numpy_emit_parameters_default() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("The value.".into()),
            is_optional: false,
            default_value: Some("0".into()),
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert!(text.contains("x : int, default: 0\n"));
}

#[test]
fn numpy_emit_returns_named() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::returns(vec![Return {
            name: Some("result".into()),
            type_annotation: Some("int".into()),
            description: Some("The result.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert!(text.contains("Returns\n-------\n"));
    assert!(text.contains("result : int\n    The result.\n"));
}

#[test]
fn numpy_emit_returns_type_only() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::returns(vec![Return {
            name: None,
            type_annotation: Some("int".into()),
            description: Some("The result.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert!(text.contains("int\n    The result.\n"));
}

#[test]
fn numpy_emit_raises() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::raises(vec![
            ExceptionEntry {
                type_name: "ValueError".into(),
                description: Some("If the input is invalid.".into()),
            },
            ExceptionEntry {
                type_name: "TypeError".into(),
                description: Some("If the type is wrong.".into()),
            },
        ])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert!(text.contains("Raises\n------\n"));
    assert!(text.contains("ValueError\n    If the input is invalid.\n"));
    assert!(text.contains("TypeError\n    If the type is wrong.\n"));
}

#[test]
fn numpy_emit_deprecation() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        directives: vec![Directive {
            name: "deprecated".into(),
            argument: Some("1.6.0".into()),
            description: Some("Use `other` instead.".into()),
        }],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert!(text.contains(".. deprecated:: 1.6.0\n"));
    assert!(text.contains("    Use `other` instead.\n"));
}

#[test]
fn numpy_emit_notes() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::free_text(FreeSectionKind::Notes, "Some notes here.".into())],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert!(text.contains("Notes\n-----\n"));
    assert!(text.contains("Some notes here.\n"));
}

#[test]
fn numpy_emit_references() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::references(vec![
            Reference {
                label: Some("1".into()),
                content: Some("Author, Title, Journal.".into()),
            },
            Reference {
                label: Some("2".into()),
                content: Some("Another reference.".into()),
            },
        ])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert!(text.contains("References\n----------\n"));
    assert!(text.contains(".. [1] Author, Title, Journal.\n"));
    assert!(text.contains(".. [2] Another reference.\n"));
}

#[test]
fn numpy_emit_attributes() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::attributes(vec![Attribute {
            names: vec!["name".into()],
            type_annotation: Some("str".into()),
            description: Some("The name.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert!(text.contains("Attributes\n----------\n"));
    assert!(text.contains("name : str\n    The name.\n"));
}

/// Multi-name attribute entries emit the FULL name list (#89).
#[test]
fn numpy_emit_attributes_multiple_names() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::attributes(vec![Attribute {
            names: vec!["jac".into(), "hess".into()],
            type_annotation: Some("ndarray".into()),
            description: Some("Derivatives.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert!(text.contains("jac, hess : ndarray\n    Derivatives.\n"));
}

#[test]
fn numpy_emit_see_also() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::see_also(vec![SeeAlsoEntry {
            names: vec!["func1".into(), "func2".into()],
            description: Some("Related.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert!(text.contains("See Also\n--------\n"));
    // Normal form (#91): the description goes on the following indented
    // line, never a `name : desc` one-liner.
    assert!(text.contains("func1, func2\n    Related.\n"));
}

/// rST-role names have no valid `name : desc` one-liner (the #26
/// leading-colon guard rejects it on re-parse); the next-line form
/// round-trips (#91).
#[test]
fn numpy_emit_see_also_role_name_multiline() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::see_also(vec![SeeAlsoEntry {
            names: vec![":func:`csd`".into()],
            description: Some("Cross power spectral density\nusing Welch's method".into()),
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert!(text.contains(":func:`csd`\n    Cross power spectral density\n    using Welch's method\n"));
}

#[test]
fn numpy_emit_multiple_sections() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![
            Section::parameters(vec![Parameter {
                names: vec!["x".into()],
                type_annotation: Some("int".into()),
                description: Some("Val.".into()),
                is_optional: false,
                default_value: None,
            }]),
            Section::returns(vec![Return {
                name: None,
                type_annotation: Some("int".into()),
                description: Some("Result.".into()),
            }]),
        ],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
    assert!(text.contains("Parameters\n----------\n"));
    assert!(text.contains("Returns\n-------\n"));
}

// =============================================================================
// Sphinx emit
// =============================================================================

#[test]
fn sphinx_emit_summary_only() {
    let doc = Docstring {
        summary: Some("Brief summary.".into()),
        ..Default::default()
    };
    assert_eq!(emit_sphinx(&doc, &EmitOptions::default()), "Brief summary.\n");
}

#[test]
fn sphinx_emit_summary_and_extended() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        extended_summary: Some("Extended description.".into()),
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert_eq!(text, "Summary.\n\nExtended description.\n");
}

#[test]
fn sphinx_emit_params() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::parameters(vec![
            Parameter {
                names: vec!["x".into()],
                type_annotation: Some("int".into()),
                description: Some("The first number.\nMore description.".into()),
                is_optional: false,
                default_value: None,
            },
            Parameter {
                names: vec!["y".into()],
                type_annotation: Some("str".into()),
                description: Some("The name.".into()),
                is_optional: false,
                default_value: None,
            },
        ])],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(text.contains(":param x: The first number.\n    More description.\n"));
    assert!(text.contains(":type x: int\n"));
    assert!(text.contains(":param y: The name.\n"));
    assert!(text.contains(":type y: str\n"));
}

#[test]
fn sphinx_emit_params_optional_and_default() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("The value.".into()),
            is_optional: true,
            default_value: Some("0".into()),
        }])],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(text.contains(":param x: The value., defaults to 0\n"));
    assert!(text.contains(":type x: int, optional\n"));
}

#[test]
fn sphinx_emit_params_default_no_description() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: None,
            is_optional: false,
            default_value: Some("0".into()),
        }])],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(text.contains(":param x: defaults to 0\n"));
}

#[test]
fn sphinx_emit_params_multiple_names_duplicated() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::parameters(vec![Parameter {
            names: vec!["x".into(), "y".into()],
            type_annotation: Some("float".into()),
            description: Some("Values.".into()),
            is_optional: false,
            default_value: None,
        }])],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(text.contains(":param x: Values.\n"));
    assert!(text.contains(":type x: float\n"));
    assert!(text.contains(":param y: Values.\n"));
    assert!(text.contains(":type y: float\n"));
}

#[test]
fn sphinx_emit_returns() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::returns(vec![Return {
            name: None,
            type_annotation: Some("int".into()),
            description: Some("The result.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(text.contains(":return: The result.\n"));
    assert!(text.contains(":rtype: int\n"));
}

#[test]
fn sphinx_emit_raises() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::raises(vec![ExceptionEntry {
            type_name: "ValueError".into(),
            description: Some("If the input is invalid.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(text.contains(":raises ValueError: If the input is invalid.\n"));
}

#[test]
fn sphinx_emit_attributes() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::attributes(vec![Attribute {
            names: vec!["name".into()],
            type_annotation: Some("str".into()),
            description: Some("The name.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(text.contains(":var name: The name.\n"));
    assert!(text.contains(":vartype name: str\n"));
}

/// Multi-name attributes duplicate the `:var:` / `:vartype:` pair per name,
/// like multi-name parameters (#89).
#[test]
fn sphinx_emit_attributes_multiple_names() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::attributes(vec![Attribute {
            names: vec!["jac".into(), "hess".into()],
            type_annotation: Some("ndarray".into()),
            description: Some("Derivatives.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(text.contains(":var jac: Derivatives.\n"));
    assert!(text.contains(":vartype jac: ndarray\n"));
    assert!(text.contains(":var hess: Derivatives.\n"));
    assert!(text.contains(":vartype hess: ndarray\n"));
}

#[test]
fn sphinx_emit_notes_admonition() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::free_text(FreeSectionKind::Notes, "Some notes here.".into())],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(text.contains(".. note::\n\n    Some notes here.\n"));
}

#[test]
fn sphinx_emit_see_also() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::see_also(vec![SeeAlsoEntry {
            names: vec!["func1".into(), "func2".into()],
            description: Some("Related.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(text.contains(".. seealso::\n\n    func1, func2: Related.\n"));
}

#[test]
fn sphinx_emit_deprecation() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        directives: vec![Directive {
            name: "deprecated".into(),
            argument: Some("1.6.0".into()),
            description: Some("Use `other` instead.".into()),
        }],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(text.contains(".. deprecated:: 1.6.0\n"));
    assert!(text.contains("    Use `other` instead.\n"));
}

#[test]
fn sphinx_emit_with_base_indent() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("The value.".into()),
            is_optional: false,
            default_value: None,
        }])],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default().with_base_indent(4));
    assert!(text.contains("    Summary.\n"));
    assert!(text.contains("    :param x: The value.\n"));
    assert!(text.contains("    :type x: int\n"));
}

#[test]
fn google_to_sphinx_conversion() {
    use pydocstring::parse::parse_google;

    let google_input = "Summary.\n\nArgs:\n    x (int): The value.\n\nReturns:\n    str: The result.";
    let doc = parse_google(google_input).to_model();
    let sphinx_output = emit_sphinx(&doc, &EmitOptions::default());
    assert!(sphinx_output.contains(":param x: The value.\n"));
    assert!(sphinx_output.contains(":type x: int\n"));
    assert!(sphinx_output.contains(":return: The result.\n"));
    assert!(sphinx_output.contains(":rtype: str\n"));
}

#[test]
fn numpy_to_sphinx_conversion() {
    use pydocstring::parse::parse_numpy;

    let numpy_input = "Summary.\n\nParameters\n----------\nx : int\n    The value.\n";
    let doc = parse_numpy(numpy_input).to_model();
    let sphinx_output = emit_sphinx(&doc, &EmitOptions::default());
    assert!(sphinx_output.contains(":param x: The value.\n"));
    assert!(sphinx_output.contains(":type x: int\n"));
}

// =============================================================================
// Round-trip: parse → to_model → emit
// =============================================================================

#[test]
fn google_roundtrip_summary() {
    use pydocstring::parse::parse_google;

    let input = "Summary line.";
    let doc = parse_google(input).to_model();
    let output = emit_google(&doc, &EmitOptions::default());
    assert_eq!(output.trim(), input);
}

#[test]
fn numpy_roundtrip_summary() {
    use pydocstring::parse::parse_numpy;

    let input = "Summary line.";
    let doc = parse_numpy(input).to_model();
    let output = emit_numpy(&doc, &EmitOptions::default());
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
    use pydocstring::parse::parse_numpy;

    let input = "Returns\n-------\nDescription with attributes:\n:attr:`~module.ClassName.attr1`\n    First attribute\n:attr:`~module.ClassName.attr2`\n    Second attribute\n";
    let output = emit_numpy(&parse_numpy(input).to_model(), &EmitOptions::default());
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

/// Google Returns section with a leading `:attr:` rST role must keep its colon.
#[test]
fn google_roundtrip_rst_role_colons() {
    use pydocstring::parse::parse_google;

    let input = "Summary.\n\nReturns:\n    :attr:`~module.ClassName.attr1`\n        First attribute\n";
    let output = emit_google(&parse_google(input).to_model(), &EmitOptions::default());
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
    use pydocstring::parse::parse_google;

    let google_input = "Summary.\n\nArgs:\n    x (int): The value.";
    let doc = parse_google(google_input).to_model();
    let numpy_output = emit_numpy(&doc, &EmitOptions::default());
    assert!(numpy_output.contains("Parameters\n----------\n"));
    assert!(numpy_output.contains("x : int\n    The value.\n"));
}

#[test]
fn numpy_to_google_conversion() {
    use pydocstring::parse::parse_numpy;

    let numpy_input = "Summary.\n\nParameters\n----------\nx : int\n    The value.\n";
    let doc = parse_numpy(numpy_input).to_model();
    let google_output = emit_google(&doc, &EmitOptions::default());
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
        sections: vec![Section::parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("The value.".into()),
            is_optional: false,
            default_value: None,
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default().with_base_indent(4));
    assert!(text.contains("    Summary.\n"));
    assert!(text.contains("    Args:\n"));
    assert!(text.contains("        x (int): The value.\n"));
}

#[test]
fn numpy_emit_with_base_indent() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("The value.".into()),
            is_optional: false,
            default_value: None,
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default().with_base_indent(4));
    assert!(text.contains("    Summary.\n"));
    assert!(text.contains("    Parameters\n    ----------\n"));
    assert!(text.contains("    x : int\n        The value.\n"));
}

#[test]
fn google_emit_base_indent_preserves_blank_lines() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        extended_summary: Some("Extended.".into()),
        sections: vec![Section::parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("Val.".into()),
            is_optional: false,
            default_value: None,
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default().with_base_indent(2));
    // Blank lines between sections should stay empty (no trailing whitespace).
    assert!(text.contains("  Summary.\n\n  Extended.\n\n  Args:\n"));
}

#[test]
fn numpy_emit_base_indent_preserves_blank_lines() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        extended_summary: Some("Extended.".into()),
        sections: vec![Section::parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("Val.".into()),
            is_optional: false,
            default_value: None,
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default().with_base_indent(2));
    // Blank lines between sections should stay empty (no trailing whitespace).
    assert!(text.contains("  Summary.\n\n  Extended.\n\n  Parameters\n"));
}

// =============================================================================
// Sphinx: every model block emits, in source order
// =============================================================================

/// A prose paragraph in a structured section survives sphinx emission at its
/// source-order position (it used to be silently dropped by the kind-filtered
/// emitter).
#[test]
fn sphinx_emit_paragraph_block_in_returns() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::new(
            SectionKind::Returns,
            vec![
                Block::Paragraph("Sets the following fields:".into()),
                Block::Return(Return {
                    name: None,
                    type_annotation: Some("ndarray".into()),
                    description: Some("PCA rep.".into()),
                }),
            ],
        )],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(
        text.contains("Sets the following fields:\n:return: PCA rep.\n"),
        "paragraph must precede the fields it introduces:\n{text}"
    );
}

/// Adjacent paragraphs stay separate paragraphs (blank-line separated), and a
/// free-text body of several paragraphs keeps its boundaries.
#[test]
fn sphinx_emit_adjacent_paragraphs_blank_line_separated() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![
            Section::new(
                SectionKind::Returns,
                vec![
                    Block::Paragraph("First paragraph.".into()),
                    Block::Paragraph("Second paragraph.".into()),
                ],
            ),
            Section::free_text(FreeSectionKind::Notes, "Notes body.".into()),
        ],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(
        text.contains("First paragraph.\n\nSecond paragraph.\n"),
        "adjacent paragraphs need a blank line to stay separate:\n{text}"
    );
    assert!(text.contains(".. note::\n\n    Notes body.\n"), "{text}");
}

// =============================================================================
// #152: emit output must not break its own structure
// =============================================================================

/// A summary-less model emits its first block at line one: blocks are
/// separated, not prefixed, so no emitter opens with a blank line.
#[test]
fn emit_summary_less_docstring_has_no_leading_blank_line() {
    let doc = Docstring {
        sections: vec![Section::parameters(vec![Parameter {
            names: vec!["x".into()],
            type_annotation: Some("int".into()),
            description: Some("Value.".into()),
            is_optional: false,
            default_value: None,
        }])],
        ..Default::default()
    };
    let options = EmitOptions::default();
    assert!(emit_google(&doc, &options).starts_with("Args:\n"));
    assert!(emit_numpy(&doc, &options).starts_with("Parameters\n"));
    assert!(emit_sphinx(&doc, &options).starts_with(":param x:"));
}

/// A multi-line method description stays inside its bullet: continuation
/// lines at column 0 would terminate the rST bullet list.
#[test]
fn sphinx_emit_method_multiline_description_stays_in_the_bullet() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::methods(vec![Method {
            name: "run".into(),
            type_annotation: None,
            description: Some("First line.\nSecond line.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(text.contains("* run: First line.\n  Second line.\n"), "got:\n{text}");
}

/// Multi-line reference content stays inside its citation: continuation
/// lines at column 0 would escape the rST citation body.
#[test]
fn sphinx_emit_reference_multiline_content_stays_in_the_citation() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::references(vec![Reference {
            label: Some("1".into()),
            content: Some("Author, Title.\nSecond line.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(
        text.contains(".. [1] Author, Title.\n    Second line.\n"),
        "got:\n{text}"
    );
}
