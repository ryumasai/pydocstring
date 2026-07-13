//! Spec + contract tests for Parameters-like sections (Parameters, Other
//! Parameters, Receives) and google-style entry compatibility.
//! Exhaustive input coverage lives in tests/corpus/numpy/ + tests/snapshots.rs;
//! these tests pin deliberate spec decisions and the typed-accessor contract.

use super::*;

// =============================================================================
// Parameters section — accessor contract
// =============================================================================

/// CONTRACT: parameter Entry accessors (names / type / description) and the
/// return Entry's type annotation on a canonical docstring.
#[test]
fn test_with_parameters() {
    let docstring = r#"Calculate the sum of two numbers.

Parameters
----------
x : int
    The first number.
y : int
    The second number.

Returns
-------
int
    The sum of x and y.
"#;
    let result = parse_numpy(docstring);

    assert_eq!(
        doc(&result).summary().unwrap().text(),
        "Calculate the sum of two numbers."
    );
    assert_eq!(parameters(&result).len(), 2);

    let names0: Vec<_> = parameters(&result)[0].names().collect();
    assert_eq!(names0[0].text(), "x");
    assert_eq!(parameters(&result)[0].type_annotation().map(|t| t.text()), Some("int"));
    assert_eq!(
        parameters(&result)[0].description().unwrap().text(),
        "The first number."
    );

    let names1: Vec<_> = parameters(&result)[1].names().collect();
    assert_eq!(names1[0].text(), "y");
    assert_eq!(parameters(&result)[1].type_annotation().map(|t| t.text()), Some("int"));

    assert!(!returns(&result).is_empty());
    assert_eq!(returns(&result)[0].type_annotation().map(|t| t.text()), Some("int"));
}

// =============================================================================
// Parameters — spec decisions
// =============================================================================

/// SPEC: trailing `, optional` marker is recognized and stripped from the type.
#[test]
fn test_optional_parameters() {
    let docstring = r#"Function with optional parameters.

Parameters
----------
required : str
    A required parameter.
optional : int, optional
    An optional parameter.
"#;
    let result = parse_numpy(docstring);

    assert_eq!(parameters(&result).len(), 2);
    assert!(!parameters(&result)[0].is_optional());
    assert!(parameters(&result)[1].is_optional());
    assert_eq!(parameters(&result)[1].type_annotation().map(|t| t.text()), Some("int"));
}

/// SPEC (issues #26/#31): no space before colon: `x: int` still splits name/type.
#[test]
fn test_parameters_no_space_before_colon() {
    let docstring = "Summary.\n\nParameters\n----------\nx: int\n    The value.\n";
    let result = parse_numpy(docstring);
    let p = parameters(&result);
    assert_eq!(p.len(), 1);
    let names: Vec<_> = p[0].names().collect();
    assert_eq!(names[0].text(), "x");
    assert_eq!(p[0].type_annotation().unwrap().text(), "int");
    assert_eq!(p[0].description().unwrap().text(), "The value.");
}

/// SPEC (issues #26/#31): no space after colon: `x :int` still splits name/type.
#[test]
fn test_parameters_no_space_after_colon() {
    let docstring = "Summary.\n\nParameters\n----------\nx :int\n    The value.\n";
    let result = parse_numpy(docstring);
    let p = parameters(&result);
    assert_eq!(p.len(), 1);
    let names: Vec<_> = p[0].names().collect();
    assert_eq!(names[0].text(), "x");
    assert_eq!(p[0].type_annotation().unwrap().text(), "int");
}

/// SPEC (issues #26/#31): no spaces around colon: `x:int` still splits name/type.
#[test]
fn test_parameters_no_spaces_around_colon() {
    let docstring = "Summary.\n\nParameters\n----------\nx:int\n    The value.\n";
    let result = parse_numpy(docstring);
    let p = parameters(&result);
    assert_eq!(p.len(), 1);
    let names: Vec<_> = p[0].names().collect();
    assert_eq!(names[0].text(), "x");
    assert_eq!(p[0].type_annotation().unwrap().text(), "int");
}

/// SPEC: `x1, x2 : array_like` splits into multiple parameter names.
#[test]
fn test_multiple_parameter_names() {
    let docstring = r#"Summary.

Parameters
----------
x1, x2 : array_like
    Input arrays.
"#;
    let result = parse_numpy(docstring);
    let p = &parameters(&result)[0];
    let names: Vec<_> = p.names().collect();
    assert_eq!(names.len(), 2);
    assert_eq!(names[0].text(), "x1");
    assert_eq!(names[1].text(), "x2");
}

/// SPEC: a blank line between parameter entries does not end the section.
#[test]
fn test_multiple_parameters_with_blank_line_between() {
    let docstring = "Summary.\n\nParameters\n----------\nx : int\n    First.\n\ny : str\n    Second.\n";
    let result = parse_numpy(docstring);
    let p = parameters(&result);
    assert_eq!(p.len(), 2, "both parameters should be in the same section");
    assert_eq!(p[0].names().next().unwrap().text(), "x");
    assert_eq!(p[1].names().next().unwrap().text(), "y");
}

/// SPEC: a colon inside an indented description line does not start a new entry.
#[test]
fn test_description_with_colon_not_treated_as_param() {
    let docstring = r#"Brief summary.

Parameters
----------
x : int
    A value like key: value should not split.
"#;
    let result = parse_numpy(docstring);
    assert_eq!(parameters(&result).len(), 1);
    let names: Vec<_> = parameters(&result)[0].names().collect();
    assert_eq!(names[0].text(), "x");
    assert!(
        parameters(&result)[0]
            .description()
            .unwrap()
            .text()
            .contains("key: value")
    );
}

// =============================================================================
// Enum / choices type — spec decisions
// =============================================================================

/// SPEC: `{'C', 'F', 'A'}` enum type is kept whole (commas inside braces do not split).
#[test]
fn test_enum_type_as_string() {
    let docstring = "Summary.\n\nParameters\n----------\norder : {'C', 'F', 'A'}\n    Memory layout.";
    let result = parse_numpy(docstring);
    let params = parameters(&result);
    assert_eq!(params.len(), 1);

    let p = &params[0];
    let names: Vec<_> = p.names().collect();
    assert_eq!(names[0].text(), "order");
    assert_eq!(p.type_annotation().unwrap().text(), "{'C', 'F', 'A'}");
    assert_eq!(p.description().unwrap().text(), "Memory layout.");
}

/// SPEC: `, optional` after a brace-enclosed enum type is still recognized.
#[test]
fn test_enum_type_with_optional() {
    let docstring = "Summary.\n\nParameters\n----------\norder : {'C', 'F'}, optional\n    Memory layout.";
    let result = parse_numpy(docstring);
    let params = parameters(&result);
    let p = &params[0];

    assert!(p.is_optional());
    assert_eq!(p.type_annotation().unwrap().text(), "{'C', 'F'}");
}

/// SPEC: `default 'C'` marker splits into keyword/value (no separator token).
#[test]
fn test_enum_type_with_default() {
    let docstring = "Summary.\n\nParameters\n----------\norder : {'C', 'F', 'A'}, default 'C'\n    Memory layout.";
    let result = parse_numpy(docstring);
    let params = parameters(&result);
    let p = &params[0];

    assert_eq!(p.type_annotation().unwrap().text(), "{'C', 'F', 'A'}");
    let marker = p.defaults().next().expect("a `default …` marker");
    assert_eq!(marker.keyword().text(), "default");
    assert!(marker.separator().is_none());
    assert_eq!(marker.value().unwrap().text(), "'C'");
    assert_eq!(p.default_value().unwrap().text(), "'C'");
}

// =============================================================================
// Other Parameters / Receives — thin section-body contract
// =============================================================================

/// CONTRACT: an OtherParameters section exposes its entries via `entries()`.
#[test]
fn test_other_parameters_section_body_variant() {
    let docstring = "Summary.\n\nOther Parameters\n----------------\nx : int\n    Extra.\n";
    let result = parse_numpy(docstring);
    let s = &all_sections(&result)[0];
    assert_eq!(s.kind(), SectionKind::OtherParameters);
    let params: Vec<_> = s.entries().collect();
    assert_eq!(params.len(), 1);
}

/// CONTRACT: Receives entries expose names / type / description.
#[test]
fn test_receives_basic() {
    let docstring = "Summary.\n\nReceives\n--------\ndata : bytes\n    The received data.\n";
    let result = parse_numpy(docstring);
    let r = receives(&result);
    assert_eq!(r.len(), 1);
    let names: Vec<_> = r[0].names().collect();
    assert_eq!(names[0].text(), "data");
    assert_eq!(r[0].type_annotation().unwrap().text(), "bytes");
    assert_eq!(r[0].description().unwrap().text(), "The received data.");
}

// =============================================================================
// Google-style entry format in NumPy sections — compat spec
// =============================================================================

/// SPEC (compat): google-style `name (str): desc` entries are accepted inside
/// NumPy Parameters sections.
#[test]
fn test_google_style_entry_in_numpy_section() {
    let docstring = "Summary.\n\nParameters\n----------\nname (str): The name.\n";
    let result = parse_numpy(docstring);
    let params = parameters(&result);
    assert_eq!(params.len(), 1);

    let names: Vec<_> = params[0].names().collect();
    assert_eq!(names[0].text(), "name");
    assert_eq!(params[0].type_annotation().map(|t| t.text()), Some("str"));
    assert_eq!(params[0].description().map(|t| t.text()), Some("The name."));
}

/// SPEC (compat): google-style and NumPy-style entries may coexist in one section.
#[test]
fn test_google_style_mixed_with_numpy_style() {
    let docstring = "Summary.\n\nParameters\n----------\nx (int): First.\ny : str\n    Second.\n";
    let result = parse_numpy(docstring);
    let params = parameters(&result);
    assert_eq!(params.len(), 2);

    // Google-style entry
    assert_eq!(params[0].names().next().unwrap().text(), "x");
    assert_eq!(params[0].type_annotation().map(|t| t.text()), Some("int"));
    assert_eq!(params[0].description().map(|t| t.text()), Some("First."));

    // NumPy-style entry
    assert_eq!(params[1].names().next().unwrap().text(), "y");
    assert_eq!(params[1].type_annotation().map(|t| t.text()), Some("str"));
    assert_eq!(params[1].description().map(|t| t.text()), Some("Second."));
}

/// SPEC (compat): `name (int)` without a trailing colon is still a google-style entry.
#[test]
fn test_google_style_entry_no_colon_after_bracket() {
    let docstring = "Summary.\n\nParameters\n----------\nname (int)\n    Desc.\n";
    let result = parse_numpy(docstring);
    let params = parameters(&result);
    assert_eq!(params.len(), 1);

    assert_eq!(params[0].names().next().unwrap().text(), "name");
    assert_eq!(params[0].type_annotation().map(|t| t.text()), Some("int"));
    assert_eq!(params[0].description().map(|t| t.text()), Some("Desc."));
}

/// SPEC: a type whose name merely starts with `default` (e.g. `defaultdict`)
/// is a type, not a default-value marker (#64).
#[test]
fn test_defaultdict_type_not_default_marker() {
    let docstring = "Summary.\n\nParameters\n----------\nx : defaultdict\n    A mapping.\n";
    let result = parse_numpy(docstring);
    let params = parameters(&result);
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].type_annotation().unwrap().text(), "defaultdict");
    assert!(params[0].default_value().is_none());
    assert_eq!(params[0].defaults().count(), 0);
}
