//! Spec pins and typed-accessor contract for Returns (GoogleReturn) and Yields (GoogleYield).
//! Exhaustive input coverage lives in tests/corpus/google/ + tests/snapshots.rs.

use super::*;

/// GoogleReturn accessor contract.
#[test]
fn test_returns_with_type() {
    let docstring = "Summary.\n\nReturns:\n    int: The result.";
    let result = parse_google(docstring);
    let r = returns(&result).unwrap();
    assert_eq!(r.return_type().unwrap().text(result.source()), "int");
    assert_eq!(r.description().unwrap().text(result.source()), "The result.");
}

/// A Returns section holds a single entry: subsequent `type: desc` lines are
/// folded into the description, not parsed as additional returns.
#[test]
fn test_returns_multiple_lines() {
    let docstring = "Summary.\n\nReturns:\n    int: The count.\n    str: The message.";
    let result = parse_google(docstring);
    let r = returns(&result).unwrap();
    assert_eq!(r.return_type().unwrap().text(result.source()), "int");
    assert_eq!(
        r.description().unwrap().text(result.source()),
        "The count.\n    str: The message."
    );
}

/// A bare description with no colon is all description — never mistaken for a type.
#[test]
fn test_returns_without_type() {
    let docstring = "Summary.\n\nReturns:\n    The computed result.";
    let result = parse_google(docstring);
    let r = returns(&result).unwrap();
    assert!(r.return_type().is_none());
    assert_eq!(r.description().unwrap().text(result.source()), "The computed result.");
}

/// GoogleYield accessor contract.
#[test]
fn test_yields() {
    let docstring = "Summary.\n\nYields:\n    int: The next value.";
    let result = parse_google(docstring);
    let y = yields(&result).unwrap();
    assert_eq!(y.return_type().unwrap().text(result.source()), "int");
    assert_eq!(y.description().unwrap().text(result.source()), "The next value.");
}
