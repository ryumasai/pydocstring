//! NumPy-style docstring support.
//!
//! Internal: the NumPy-dialect header recognition and entry grammar; the
//! document loop is `parse::dispatch`, and the model conversion is
//! `parse::to_model` — both shared across styles.
//! Reached from outside through [`parse_numpy`](crate::parse::parse_numpy)
//! and [`Parsed::to_model`](crate::syntax::Parsed::to_model) — the tree the
//! parser builds carries no per-style structure.

pub(crate) mod parser;

pub use parser::parse_numpy;
pub use parser::parse_numpy_with;
