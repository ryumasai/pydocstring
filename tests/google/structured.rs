//! Typed-accessor contract for Attributes, Methods, and See Also nodes.
//! Exhaustive input coverage lives in tests/corpus/google/ + tests/snapshots.rs;
//! alias→kind mapping is pinned by the table test in sections.rs.

use super::*;

/// GoogleAttribute accessor contract.
#[test]
fn test_attributes() {
    let docstring = "Summary.\n\nAttributes:\n    name (str): The name.\n    age (int): The age.";
    let result = parse_google(docstring);
    let a = attributes(&result);
    assert_eq!(a.len(), 2);
    assert_eq!(a[0].name().text(), "name");
    assert_eq!(a[0].type_annotation().unwrap().text(), "str");
    assert_eq!(a[1].name().text(), "age");
}

/// GoogleMethod accessor contract; method names keep their parenthesised
/// signature verbatim.
#[test]
fn test_methods_basic() {
    let docstring = "Summary.\n\nMethods:\n    reset(): Reset the state.\n    update(data): Update with new data.";
    let result = parse_google(docstring);
    let m = methods(&result);
    assert_eq!(m.len(), 2);
    assert_eq!(m[0].name().text(), "reset()");
    assert_eq!(m[0].description().unwrap().text(), "Reset the state.");
    assert_eq!(m[1].name().text(), "update(data)");
}

/// GoogleSeeAlsoItem accessor contract.
#[test]
fn test_see_also_basic() {
    let docstring = "Summary.\n\nSee Also:\n    other_func: Does something else.";
    let result = parse_google(docstring);
    let sa = see_also(&result);
    assert_eq!(sa.len(), 1);
    let names: Vec<_> = sa[0].names().collect();
    assert_eq!(names.len(), 1);
    assert_eq!(names[0].text(), "other_func");
    assert_eq!(sa[0].description().unwrap().text(), "Does something else.");
}

/// Comma-separated names on one See Also line split into multiple NAME tokens
/// with no description.
#[test]
fn test_see_also_multiple_names() {
    let docstring = "Summary.\n\nSee Also:\n    func_a, func_b, func_c";
    let result = parse_google(docstring);
    let sa = see_also(&result);
    assert_eq!(sa.len(), 1);
    let names: Vec<_> = sa[0].names().collect();
    assert_eq!(names.len(), 3);
    assert_eq!(names[0].text(), "func_a");
    assert_eq!(names[1].text(), "func_b");
    assert_eq!(names[2].text(), "func_c");
    assert!(sa[0].description().is_none());
}
