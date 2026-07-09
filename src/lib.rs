#![deny(missing_docs)]

//! # pydocstring
//!
//! A fast, zero-dependency Rust parser for Python docstrings with full AST and
//! source location tracking. Supports **NumPy** and **Google** styles.
//!
//! ## Quick Start
//!
//! Parse with auto-detection and traverse the style-independent typed views
//! ([`Document`](parse::Document) → [`Section`](parse::Section) →
//! [`Entry`](parse::Entry)) — one code path for every docstring style:
//!
//! ```rust
//! use pydocstring::model::SectionKind;
//! use pydocstring::parse::{parse, Document, Style};
//!
//! let docstring = "\
//! Brief description.
//!
//! Parameters
//! ----------
//! x : int
//!     The first parameter.
//! ";
//!
//! let parsed = parse(docstring);
//! assert_eq!(parsed.style(), Style::NumPy);
//!
//! let doc = Document::new(&parsed);
//! assert_eq!(doc.summary().unwrap().text(), "Brief description.");
//!
//! let section = doc.sections().next().unwrap();
//! assert_eq!(section.kind(), SectionKind::Parameters);
//! let entry = section.entries().next().unwrap();
//! assert_eq!(entry.name().unwrap().text(), "x");
//! assert_eq!(entry.type_annotation().unwrap().text(), "int");
//! ```
//!
//! ## Per-Style Typed Views
//!
//! When you know (or want to force) the style, the per-style parsers expose
//! style-specific wrappers with the same source-free accessors:
//!
//! ```rust
//! use pydocstring::parse::numpy::{parse_numpy, NumPyDocstring};
//!
//! let result = parse_numpy("Brief description.\n\nParameters\n----------\nx : int\n    Desc.\n");
//! let doc = NumPyDocstring::cast(&result, result.root()).unwrap();
//! assert_eq!(doc.summary().unwrap().text(), "Brief description.");
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
//! - Anchored splice edits ([`Parsed::edit`](syntax::Parsed::edit), see
//!   [`edit`]): everything an edit does not touch is preserved byte-for-byte
//! - Emit to Google, NumPy, and Sphinx (reStructuredText) styles (Sphinx is
//!   emit-only; see [`emit::sphinx`])

pub(crate) mod cursor;
pub mod edit;
pub mod emit;
pub mod model;
pub mod parse;
pub mod syntax;
pub mod text;
