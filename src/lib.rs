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
//! ## The Raw CST
//!
//! The unified view is a *semantic* lens: it answers "is there a type?" and
//! folds away punctuation and the parser's zero-length placeholders. For the
//! tree exactly as parsed, go down to the CST with `syntax()`:
//!
//! ```rust
//! use pydocstring::parse::{parse, Document};
//! use pydocstring::syntax::SyntaxKind;
//!
//! let parsed = parse("Summary.\n\nArgs:\n    x (): The value.\n");
//! let entry = Document::new(&parsed).sections().next().unwrap().entries().next().unwrap();
//!
//! // The semantic lens says "no type" …
//! assert!(entry.type_annotation().is_none());
//! // … the CST says *why*: an empty type between brackets, whose zero-length
//! // range is the anchor to write one at.
//! let placeholder = entry.syntax().find_missing(SyntaxKind::TYPE).unwrap();
//! assert!(placeholder.is_missing());
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
//! - Pattern fragments with `$X` / `$$$X` metavariables
//!   ([`Pattern`](pattern::Pattern), see [`pattern`]) — the input side of the
//!   match/rewrite engine
//! - Anchor-based structural matching
//!   ([`Pattern::matches`](pattern::Pattern::matches) /
//!   [`Pattern::matches_in`](pattern::Pattern::matches_in), see [`matcher`]):
//!   trivia-skipping, indentation-relative unification whose captures expose
//!   the original target bytes
//! - Pattern-based rewriting
//!   ([`Parsed::replace`](syntax::Parsed::replace) /
//!   [`Parsed::replace_in`](syntax::Parsed::replace_in), see [`rewrite`]):
//!   splices a template rendered with byte-exact captured content, preserving
//!   everything outside the rewritten regions by construction
//! - Emit to Google, NumPy, and Sphinx (reStructuredText) styles (Sphinx is
//!   emit-only; see [`emit::sphinx`])

pub(crate) mod cursor;
pub mod edit;
pub mod emit;
pub mod matcher;
pub mod model;
pub mod parse;
pub mod pattern;
pub mod rewrite;
pub mod syntax;
pub mod text;
