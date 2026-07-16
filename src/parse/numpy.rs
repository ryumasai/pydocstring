//! NumPy-style docstring support.
//!
//! Internal: the parser and the model conversion for NumPy-style docstrings.
//! Reached from outside through [`parse_numpy`](crate::parse::parse_numpy)
//! and [`Parsed::to_model`](crate::syntax::Parsed::to_model) — the tree the
//! parser builds carries no per-style structure.

pub(crate) mod nodes;
pub(crate) mod parser;
pub(crate) mod to_model;

pub use parser::parse_numpy;
