//! NumPy-style docstring support.
//!
//! This module contains the AST types and parser for NumPy-style docstrings.

pub mod kind;
pub mod nodes;
pub mod parser;
pub mod to_model;

pub use crate::parse::text_block::TextBlock;
pub use crate::parse::visitor::DocstringVisitor;
pub use crate::parse::visitor::walk;
pub use kind::NumPySectionKind;
pub use nodes::NumPyAttribute;
pub use nodes::NumPyDeprecation;
pub use nodes::NumPyDocstring;
pub use nodes::NumPyException;
pub use nodes::NumPyMethod;
pub use nodes::NumPyParameter;
pub use nodes::NumPyReference;
pub use nodes::NumPyReturns;
pub use nodes::NumPySection;
pub use nodes::NumPySectionHeader;
pub use nodes::NumPySeeAlsoItem;
pub use nodes::NumPyWarning;
pub use nodes::NumPyYields;
pub use parser::parse_numpy;
