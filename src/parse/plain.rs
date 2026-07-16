//! Plain docstring style.
//!
//! "Plain" covers docstrings that contain no NumPy or Google style section
//! markers — i.e. a summary, an optional extended summary, and nothing else.
//! Unrecognised styles such as Sphinx are also treated as plain.
//!
//! Internal: reached from outside through
//! [`parse_plain`](crate::parse::parse_plain) and
//! [`Parsed::to_model`](crate::syntax::Parsed::to_model).

pub(crate) mod parser;

pub use parser::parse_plain;
