//! Plain docstring style.
//!
//! "Plain" covers docstrings that contain no NumPy or Google style section
//! markers — i.e. a summary, an optional extended summary, and nothing else.
//! Unrecognised styles such as Sphinx are also treated as plain.

pub(crate) mod nodes;
pub mod parser;
pub mod to_model;

pub use crate::parse::text_block::TextBlock;
pub use parser::parse_plain;
