//! Google-style docstring support.
//!
//! This module contains the AST types and parser for Google-style docstrings.

pub(crate) mod kind;
pub(crate) mod nodes;
pub mod parser;
pub mod to_model;

pub use crate::parse::text_block::TextBlock;
pub use parser::parse_google;
