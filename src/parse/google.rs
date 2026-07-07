//! Google-style docstring support.
//!
//! This module contains the AST types and parser for Google-style docstrings.

pub mod kind;
pub mod nodes;
pub mod parser;
pub mod to_model;

pub use crate::parse::visitor::{DocstringVisitor, walk};
pub use kind::GoogleSectionKind;
pub use nodes::{
    GoogleArg, GoogleAttribute, GoogleDeprecation, GoogleDocstring, GoogleException, GoogleMethod, GoogleReference,
    GoogleReturn, GoogleSection, GoogleSectionHeader, GoogleSeeAlsoItem, GoogleWarning, GoogleYield,
};
pub use parser::parse_google;
