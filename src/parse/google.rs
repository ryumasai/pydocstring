//! Google-style docstring support.
//!
//! This module contains the AST types and parser for Google-style docstrings.

pub mod kind;
pub mod nodes;
pub mod parser;
pub mod to_model;

pub use crate::parse::text_block::TextBlock;
pub use crate::parse::visitor::DocstringVisitor;
pub use crate::parse::visitor::walk;
pub use kind::GoogleSectionKind;
pub use nodes::GoogleArg;
pub use nodes::GoogleAttribute;
pub use nodes::GoogleDeprecation;
pub use nodes::GoogleDirective;
pub use nodes::GoogleDocstring;
pub use nodes::GoogleException;
pub use nodes::GoogleMethod;
pub use nodes::GoogleReference;
pub use nodes::GoogleReturn;
pub use nodes::GoogleSection;
pub use nodes::GoogleSectionHeader;
pub use nodes::GoogleSeeAlsoItem;
pub use nodes::GoogleWarning;
pub use nodes::GoogleYield;
pub use parser::parse_google;
