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
        sections: vec![Section::Parameters(vec![
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
        sections: vec![Section::Parameters(vec![Parameter {
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
        sections: vec![Section::Parameters(vec![Parameter {
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
        sections: vec![Section::Returns(vec![Return {
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
        sections: vec![Section::Returns(vec![Return {
            name: None,
            type_annotation: None,
            description: Some("The computed result.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
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
        sections: vec![Section::FreeText {
            kind: FreeSectionKind::Notes,
            body: "Some notes here.".into(),
        }],
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
            Section::Parameters(vec![Parameter {
                names: vec!["x".into()],
                type_annotation: Some("int".into()),
                description: Some("Val.".into()),
                is_optional: false,
                default_value: None,
            }]),
            Section::Returns(vec![Return {
                name: None,
                type_annotation: Some("str".into()),
                description: Some("Result.".into()),
            }]),
            Section::Raises(vec![ExceptionEntry {
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
        sections: vec![Section::Attributes(vec![Attribute {
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
        sections: vec![Section::Attributes(vec![Attribute {
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
        sections: vec![Section::SeeAlso(vec![SeeAlsoEntry {
            names: vec!["func1".into(), "func2".into()],
            description: Some("Related functions.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_google(&doc, &EmitOptions::default());
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
        sections: vec![Section::Parameters(vec![
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
        sections: vec![Section::Parameters(vec![Parameter {
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
        sections: vec![Section::Parameters(vec![Parameter {
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
        sections: vec![Section::Parameters(vec![Parameter {
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
        sections: vec![Section::Returns(vec![Return {
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
        sections: vec![Section::Returns(vec![Return {
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
        sections: vec![Section::Raises(vec![
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
        sections: vec![Section::FreeText {
            kind: FreeSectionKind::Notes,
            body: "Some notes here.".into(),
        }],
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
        sections: vec![Section::References(vec![
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
        sections: vec![Section::Attributes(vec![Attribute {
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
        sections: vec![Section::Attributes(vec![Attribute {
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
        sections: vec![Section::SeeAlso(vec![SeeAlsoEntry {
            names: vec!["func1".into(), "func2".into()],
            description: Some("Related.".into()),
        }])],
        ..Default::default()
    };
    let text = emit_numpy(&doc, &EmitOptions::default());
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
            }]),
            Section::Returns(vec![Return {
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
        sections: vec![Section::Parameters(vec![
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
        sections: vec![Section::Parameters(vec![Parameter {
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
        sections: vec![Section::Parameters(vec![Parameter {
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
        sections: vec![Section::Parameters(vec![Parameter {
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
        sections: vec![Section::Returns(vec![Return {
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
        sections: vec![Section::Raises(vec![ExceptionEntry {
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
        sections: vec![Section::Attributes(vec![Attribute {
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
        sections: vec![Section::Attributes(vec![Attribute {
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
        sections: vec![Section::FreeText {
            kind: FreeSectionKind::Notes,
            body: "Some notes here.".into(),
        }],
        ..Default::default()
    };
    let text = emit_sphinx(&doc, &EmitOptions::default());
    assert!(text.contains(".. note::\n\n    Some notes here.\n"));
}

#[test]
fn sphinx_emit_see_also() {
    let doc = Docstring {
        summary: Some("Summary.".into()),
        sections: vec![Section::SeeAlso(vec![SeeAlsoEntry {
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
        sections: vec![Section::Parameters(vec![Parameter {
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
    use pydocstring::parse::google::parse_google;
    use pydocstring::parse::google::to_model::to_model;

    let google_input = "Summary.\n\nArgs:\n    x (int): The value.\n\nReturns:\n    str: The result.";
    let doc = to_model(&parse_google(google_input)).unwrap();
    let sphinx_output = emit_sphinx(&doc, &EmitOptions::default());
    assert!(sphinx_output.contains(":param x: The value.\n"));
    assert!(sphinx_output.contains(":type x: int\n"));
    assert!(sphinx_output.contains(":return: The result.\n"));
    assert!(sphinx_output.contains(":rtype: str\n"));
}

#[test]
fn numpy_to_sphinx_conversion() {
    use pydocstring::parse::numpy::parse_numpy;
    use pydocstring::parse::numpy::to_model::to_model;

    let numpy_input = "Summary.\n\nParameters\n----------\nx : int\n    The value.\n";
    let doc = to_model(&parse_numpy(numpy_input)).unwrap();
    let sphinx_output = emit_sphinx(&doc, &EmitOptions::default());
    assert!(sphinx_output.contains(":param x: The value.\n"));
    assert!(sphinx_output.contains(":type x: int\n"));
}

// =============================================================================
// Round-trip: parse → to_model → emit
// =============================================================================

#[test]
fn google_roundtrip_summary() {
    use pydocstring::parse::google::parse_google;
    use pydocstring::parse::google::to_model::to_model;

    let input = "Summary line.";
    let doc = to_model(&parse_google(input)).unwrap();
    let output = emit_google(&doc, &EmitOptions::default());
    assert_eq!(output.trim(), input);
}

#[test]
fn numpy_roundtrip_summary() {
    use pydocstring::parse::numpy::parse_numpy;
    use pydocstring::parse::numpy::to_model::to_model;

    let input = "Summary line.";
    let doc = to_model(&parse_numpy(input)).unwrap();
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
    use pydocstring::parse::numpy::parse_numpy;
    use pydocstring::parse::numpy::to_model::to_model;

    let input = "Returns\n-------\nDescription with attributes:\n:attr:`~module.ClassName.attr1`\n    First attribute\n:attr:`~module.ClassName.attr2`\n    Second attribute\n";
    let output = emit_numpy(&to_model(&parse_numpy(input)).unwrap(), &EmitOptions::default());
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
    use pydocstring::parse::google::parse_google;
    use pydocstring::parse::google::to_model::to_model;

    let input = "Summary.\n\nReturns:\n    :attr:`~module.ClassName.attr1`\n        First attribute\n";
    let output = emit_google(&to_model(&parse_google(input)).unwrap(), &EmitOptions::default());
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
    use pydocstring::parse::google::parse_google;
    use pydocstring::parse::google::to_model::to_model;

    let google_input = "Summary.\n\nArgs:\n    x (int): The value.";
    let doc = to_model(&parse_google(google_input)).unwrap();
    let numpy_output = emit_numpy(&doc, &EmitOptions::default());
    assert!(numpy_output.contains("Parameters\n----------\n"));
    assert!(numpy_output.contains("x : int\n    The value.\n"));
}

#[test]
fn numpy_to_google_conversion() {
    use pydocstring::parse::numpy::parse_numpy;
    use pydocstring::parse::numpy::to_model::to_model;

    let numpy_input = "Summary.\n\nParameters\n----------\nx : int\n    The value.\n";
    let doc = to_model(&parse_numpy(numpy_input)).unwrap();
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
        sections: vec![Section::Parameters(vec![Parameter {
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
        sections: vec![Section::Parameters(vec![Parameter {
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
        sections: vec![Section::Parameters(vec![Parameter {
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
        sections: vec![Section::Parameters(vec![Parameter {
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
