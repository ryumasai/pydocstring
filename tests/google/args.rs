//! Spec pins and typed-accessor contract for Args-family sections.
//! Exhaustive input coverage lives in tests/corpus/google/ + tests/snapshots.rs;
//! these tests pin deliberate parsing decisions and the accessor API.

use super::*;

// =============================================================================
// GoogleArg accessor contract
// =============================================================================

#[test]
fn test_args_basic() {
    let docstring = "Summary.\n\nArgs:\n    x (int): The value.";
    let result = parse_google(docstring);
    let a = args(&result);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].name().text(result.source()), "x");
    assert_eq!(a[0].r#type().unwrap().text(result.source()), "int");
    assert_eq!(a[0].description().unwrap().text(result.source()), "The value.");
}

#[test]
fn test_args_name_span() {
    let docstring = "Summary.\n\nArgs:\n    x (int): Value.";
    let result = parse_google(docstring);
    let arg = &args(&result)[0];
    let name = arg.name();
    // "x" starts at byte offset 20 (line 3, col 4)
    assert_eq!(name.range().start(), TextSize::new(20));
    assert_eq!(name.range().end(), TextSize::new(name.range().start().raw() + 1));
    assert_eq!(name.text(result.source()), "x");
}

#[test]
fn test_args_no_bracket_fields_when_no_type() {
    let docstring = "Summary.\n\nArgs:\n    x: The value.";
    let result = parse_google(docstring);
    let a = &args(&result)[0];
    assert!(a.open_bracket().is_none());
    assert!(a.close_bracket().is_none());
    assert!(a.r#type().is_none());
}

// =============================================================================
// Colon-separator rules (spec)
// =============================================================================

/// Colon with no space after it: `name:description`
#[test]
fn test_args_no_space_after_colon() {
    let docstring = "Summary.\n\nArgs:\n    x:The value.";
    let result = parse_google(docstring);
    let a = args(&result);
    assert_eq!(a[0].name().text(result.source()), "x");
    assert_eq!(a[0].description().unwrap().text(result.source()), "The value.");
}

/// Colon with extra spaces: `name:   description`
#[test]
fn test_args_extra_spaces_after_colon() {
    let docstring = "Summary.\n\nArgs:\n    x:   The value.";
    let result = parse_google(docstring);
    let a = args(&result);
    assert_eq!(a[0].name().text(result.source()), "x");
    assert_eq!(a[0].description().unwrap().text(result.source()), "The value.");
}

// =============================================================================
// Description shapes (spec)
// =============================================================================

/// Continuation lines keep their raw indentation inside the description token.
#[test]
fn test_args_multiline_description() {
    let docstring = "Summary.\n\nArgs:\n    x (int): First line.\n        Second line.\n        Third line.";
    let result = parse_google(docstring);
    assert_eq!(
        args(&result)[0].description().unwrap().text(result.source()),
        "First line.\n        Second line.\n        Third line."
    );
}

/// `name (type):` with the description starting on the next line.
#[test]
fn test_args_description_on_next_line() {
    let docstring = "Summary.\n\nArgs:\n    x (int):\n        The description.";
    let result = parse_google(docstring);
    let a = args(&result);
    assert_eq!(a[0].name().text(result.source()), "x");
    assert_eq!(a[0].r#type().unwrap().text(result.source()), "int");
    assert_eq!(a[0].description().unwrap().text(result.source()), "The description.");
}

/// `*args` / `**kwargs` names keep their star prefixes.
#[test]
fn test_args_varargs() {
    let docstring = "Summary.\n\nArgs:\n    *args: Positional args.\n    **kwargs: Keyword args.";
    let result = parse_google(docstring);
    let a = args(&result);
    assert_eq!(a.len(), 2);
    assert_eq!(a[0].name().text(result.source()), "*args");
    assert_eq!(a[0].description().unwrap().text(result.source()), "Positional args.");
    assert_eq!(a[1].name().text(result.source()), "**kwargs");
    assert_eq!(a[1].description().unwrap().text(result.source()), "Keyword args.");
}

// =============================================================================
// Bracket styles around the type (spec)
// =============================================================================

/// All four recognised bracket styles delimit a type annotation.
#[test]
fn test_args_bracket_styles() {
    for (open, close) in [("(", ")"), ("[", "]"), ("{", "}"), ("<", ">")] {
        let input = format!("Summary.\n\nArgs:\n    x {open}int{close}: The value.");
        let result = parse_google(&input);
        let a = args(&result);
        assert_eq!(a.len(), 1, "brackets {open}{close}");
        assert_eq!(a[0].name().text(result.source()), "x", "brackets {open}{close}");
        assert_eq!(
            a[0].r#type().unwrap().text(result.source()),
            "int",
            "brackets {open}{close}"
        );
        assert_eq!(a[0].open_bracket().unwrap().text(result.source()), open);
        assert_eq!(a[0].close_bracket().unwrap().text(result.source()), close);
        assert_eq!(
            a[0].description().unwrap().text(result.source()),
            "The value.",
            "brackets {open}{close}"
        );
    }
}

// =============================================================================
// Optional marker inside type brackets (spec)
// =============================================================================

#[test]
fn test_args_optional() {
    let docstring = "Summary.\n\nArgs:\n    x (int, optional): The value.";
    let result = parse_google(docstring);
    let a = args(&result);
    assert_eq!(a[0].name().text(result.source()), "x");
    assert_eq!(a[0].r#type().unwrap().text(result.source()), "int");
    assert!(a[0].optional().is_some());
}

/// `(optional)` with no type: optional marker set, type absent.
#[test]
fn test_optional_only_in_parens() {
    let docstring = "Summary.\n\nArgs:\n    x (optional): Value.";
    let result = parse_google(docstring);
    let a = args(&result);
    assert_eq!(a[0].name().text(result.source()), "x");
    assert!(a[0].r#type().is_none());
    assert!(a[0].optional().is_some());
}

// =============================================================================
// Args-family section variants (contract: section.args() works for each kind)
// =============================================================================

#[test]
fn test_keyword_args_section_body_variant() {
    let docstring = "Summary.\n\nKeyword Args:\n    k (str): Key.";
    let result = parse_google(docstring);
    let sections = all_sections(&result);
    assert_eq!(
        sections[0].section_kind(result.source()),
        GoogleSectionKind::KeywordArgs
    );
    assert_eq!(sections[0].args().count(), 1);
}

#[test]
fn test_other_parameters_section_body_variant() {
    let docstring = "Summary.\n\nOther Parameters:\n    x (int): Extra.";
    let result = parse_google(docstring);
    let sections = all_sections(&result);
    assert_eq!(
        sections[0].section_kind(result.source()),
        GoogleSectionKind::OtherParameters
    );
    assert_eq!(sections[0].args().count(), 1);
}

#[test]
fn test_receives() {
    let docstring = "Summary.\n\nReceives:\n    data (bytes): The received data.";
    let result = parse_google(docstring);
    let r = receives(&result);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].name().text(result.source()), "data");
    assert_eq!(r[0].r#type().unwrap().text(result.source()), "bytes");
}
