#![deny(missing_docs)]

//! # pydocstring
//!
//! A fast, zero-dependency Rust parser for Python docstrings with full AST and
//! source location tracking. Supports **NumPy** and **Google** styles.
//!
//! ## Quick Start
//!
//! ```rust
//! use pydocstring::parse::numpy::{parse_numpy, NumPyDocstring};
//!
//! let docstring = r#"
//! Brief description.
//!
//! Parameters
//! ----------
//! x : int
//!     The first parameter.
//! "#;
//!
//! let result = parse_numpy(docstring);
//! let doc = NumPyDocstring::cast(result.root()).unwrap();
//! assert_eq!(doc.summary().unwrap().text(result.source()), "Brief description.");
//! ```
//!
//! ## Style Auto-Detection
//!
//! ```rust
//! use pydocstring::parse::{detect_style, Style};
//!
//! let numpy_doc = "Summary.\n\nParameters\n----------\nx : int\n    Desc.";
//! assert_eq!(detect_style(numpy_doc), Style::NumPy);
//!
//! let google_doc = "Summary.\n\nArgs:\n    x: Desc.";
//! assert_eq!(detect_style(google_doc), Style::Google);
//! ```
//!
//! ## Features
//!
//! - Zero external dependencies — pure Rust
//! - Accurate source spans (byte offsets) on every AST node
//! - NumPy style: fully supported
//! - Google style: fully supported
//! - Emit to Google, NumPy, and Sphinx (reStructuredText) styles (Sphinx is
//!   emit-only; see [`emit::sphinx`])

pub(crate) mod cursor;
pub mod emit;
pub mod model;
pub mod parse;
pub mod syntax;
pub mod text;
