//! Unified typed visitor for all docstring ASTs.
//!
//! A single [`DocstringVisitor`] trait covers both Google-style and NumPy-style
//! nodes.  Call [`walk`] to start traversal from any node, or call it from
//! within a `visit_*` override on child nodes to continue into children.
//!
//! Traversal follows the same protocol as `ast.NodeVisitor` in Python:
//! - Each `visit_*` method's **default implementation visits the node's
//!   children** by calling [`walk`] on each child.
//! - Override a method to add behaviour *before* and/or *after* children are
//!   visited by explicitly iterating children and calling [`walk`], or omit
//!   that loop to prune the subtree entirely.
//!
//! # Example
//!
//! ```rust
//! use pydocstring::parse::google::{parse_google, GoogleSection};
//! use pydocstring::parse::visitor::{DocstringVisitor, walk};
//! use pydocstring::syntax::SyntaxElement;
//!
//! struct SectionPrinter;
//!
//! impl DocstringVisitor for SectionPrinter {
//!     type Error = std::convert::Infallible;
//!
//!     fn visit_google_section(&mut self, source: &str, section: &GoogleSection<'_>) -> Result<(), Self::Error> {
//!         println!("enter: {}", section.header().name().text(source));
//!         // continue into children:
//!         for child in section.syntax().children() {
//!             if let SyntaxElement::Node(n) = child { walk(source, n, self)?; }
//!         }
//!         println!("leave: {}", section.header().name().text(source));
//!         Ok(())
//!     }
//! }
//!
//! let result = parse_google("Args:\n    x: desc\n");
//! let doc = pydocstring::parse::google::GoogleDocstring::cast(result.root()).unwrap();
//! let mut printer = SectionPrinter;
//! printer.visit_google_docstring(result.source(), &doc).unwrap();
//! ```

use crate::parse::google::nodes::{
    GoogleArg, GoogleAttribute, GoogleDeprecation, GoogleDocstring, GoogleException, GoogleMethod, GoogleReference,
    GoogleReturn, GoogleSection, GoogleSeeAlsoItem, GoogleWarning, GoogleYield,
};
use crate::parse::numpy::nodes::{
    NumPyAttribute, NumPyDeprecation, NumPyDocstring, NumPyException, NumPyMethod, NumPyParameter, NumPyReference,
    NumPyReturns, NumPySection, NumPySeeAlsoItem, NumPyWarning, NumPyYields,
};
use crate::parse::plain::nodes::PlainDocstring;
use crate::syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

/// Unified typed visitor for Google-style and NumPy-style docstring ASTs.
///
/// Each `visit_*` method's default implementation visits the node's children
/// by calling [`walk`] on each one.  Override a method and either iterate
/// children manually (calling [`walk`]) or omit that loop to prune the subtree.
///
/// The `source` parameter is the original docstring source text, required for
/// reading token text (e.g. `arg.name().text(source)`).
///
/// `type Error` is the error type returned by all `visit_*` methods.  Use
/// [`std::convert::Infallible`] for infallible visitors.
pub trait DocstringVisitor: Sized {
    /// The error type returned by visitor methods.
    type Error;

    // ── Plain ─────────────────────────────────────────────────────────────
    /// Called for the plain docstring root.
    fn visit_plain_docstring(&mut self, source: &str, doc: &PlainDocstring<'_>) -> Result<(), Self::Error> {
        let _ = (source, doc);
        Ok(())
    }
    // ── Google ────────────────────────────────────────────────────────────
    /// Called for the Google docstring root.
    fn visit_google_docstring(&mut self, source: &str, doc: &GoogleDocstring<'_>) -> Result<(), Self::Error> {
        walk_children(source, doc.syntax(), self)
    }
    /// Called for the deprecation notice, if present.
    fn visit_google_deprecation(&mut self, source: &str, dep: &GoogleDeprecation<'_>) -> Result<(), Self::Error> {
        walk_children(source, dep.syntax(), self)
    }
    /// Called for each Google section.
    fn visit_google_section(&mut self, source: &str, sec: &GoogleSection<'_>) -> Result<(), Self::Error> {
        walk_children(source, sec.syntax(), self)
    }
    /// Called for each argument entry.
    fn visit_google_arg(&mut self, source: &str, arg: &GoogleArg<'_>) -> Result<(), Self::Error> {
        walk_children(source, arg.syntax(), self)
    }
    /// Called for the Return entry in a Returns section, if present.
    fn visit_google_return(&mut self, source: &str, rtn: &GoogleReturn<'_>) -> Result<(), Self::Error> {
        walk_children(source, rtn.syntax(), self)
    }
    /// Called for the Yield entry in a Yields section, if present.
    fn visit_google_yield(&mut self, source: &str, yld: &GoogleYield<'_>) -> Result<(), Self::Error> {
        walk_children(source, yld.syntax(), self)
    }
    /// Called for each exception entry.
    fn visit_google_exception(&mut self, source: &str, exc: &GoogleException<'_>) -> Result<(), Self::Error> {
        walk_children(source, exc.syntax(), self)
    }
    /// Called for each warning entry.
    fn visit_google_warning(&mut self, source: &str, wrn: &GoogleWarning<'_>) -> Result<(), Self::Error> {
        walk_children(source, wrn.syntax(), self)
    }
    /// Called for each See Also item.
    fn visit_google_see_also_item(&mut self, source: &str, sai: &GoogleSeeAlsoItem<'_>) -> Result<(), Self::Error> {
        walk_children(source, sai.syntax(), self)
    }
    /// Called for each reference entry.
    fn visit_google_reference(&mut self, source: &str, r#ref: &GoogleReference<'_>) -> Result<(), Self::Error> {
        walk_children(source, r#ref.syntax(), self)
    }
    /// Called for each attribute entry.
    fn visit_google_attribute(&mut self, source: &str, att: &GoogleAttribute<'_>) -> Result<(), Self::Error> {
        walk_children(source, att.syntax(), self)
    }
    /// Called for each method entry.
    fn visit_google_method(&mut self, source: &str, mtd: &GoogleMethod<'_>) -> Result<(), Self::Error> {
        walk_children(source, mtd.syntax(), self)
    }
    // ── NumPy ─────────────────────────────────────────────────────────────
    /// Called for the NumPy docstring root.
    fn visit_numpy_docstring(&mut self, source: &str, doc: &NumPyDocstring<'_>) -> Result<(), Self::Error> {
        walk_children(source, doc.syntax(), self)
    }
    /// Called for the deprecation notice, if present.
    fn visit_numpy_deprecation(&mut self, source: &str, dep: &NumPyDeprecation<'_>) -> Result<(), Self::Error> {
        walk_children(source, dep.syntax(), self)
    }
    /// Called for each NumPy section.
    fn visit_numpy_section(&mut self, source: &str, sec: &NumPySection<'_>) -> Result<(), Self::Error> {
        walk_children(source, sec.syntax(), self)
    }
    /// Called for each parameter entry.
    fn visit_numpy_parameter(&mut self, source: &str, prm: &NumPyParameter<'_>) -> Result<(), Self::Error> {
        walk_children(source, prm.syntax(), self)
    }
    /// Called for each Returns entry.
    fn visit_numpy_returns(&mut self, source: &str, rtn: &NumPyReturns<'_>) -> Result<(), Self::Error> {
        walk_children(source, rtn.syntax(), self)
    }
    /// Called for each Yields entry.
    fn visit_numpy_yields(&mut self, source: &str, yld: &NumPyYields<'_>) -> Result<(), Self::Error> {
        walk_children(source, yld.syntax(), self)
    }
    /// Called for each exception entry.
    fn visit_numpy_exception(&mut self, source: &str, exc: &NumPyException<'_>) -> Result<(), Self::Error> {
        walk_children(source, exc.syntax(), self)
    }
    /// Called for each warning entry.
    fn visit_numpy_warning(&mut self, source: &str, wrn: &NumPyWarning<'_>) -> Result<(), Self::Error> {
        walk_children(source, wrn.syntax(), self)
    }
    /// Called for each See Also item.
    fn visit_numpy_see_also_item(&mut self, source: &str, sai: &NumPySeeAlsoItem<'_>) -> Result<(), Self::Error> {
        walk_children(source, sai.syntax(), self)
    }
    /// Called for each reference entry.
    fn visit_numpy_reference(&mut self, source: &str, r#ref: &NumPyReference<'_>) -> Result<(), Self::Error> {
        walk_children(source, r#ref.syntax(), self)
    }
    /// Called for each attribute entry.
    fn visit_numpy_attribute(&mut self, source: &str, att: &NumPyAttribute<'_>) -> Result<(), Self::Error> {
        walk_children(source, att.syntax(), self)
    }
    /// Called for each method entry.
    fn visit_numpy_method(&mut self, source: &str, mtd: &NumPyMethod<'_>) -> Result<(), Self::Error> {
        walk_children(source, mtd.syntax(), self)
    }
}

/// Dispatch `node` to the appropriate `visit_*` method based on its
/// [`SyntaxKind`].
///
/// Handles both docstring roots (`GOOGLE_DOCSTRING`, `NUMPY_DOCSTRING`) and
/// all inner nodes (`GOOGLE_SECTION`, `GOOGLE_ARG`, …).  Unknown kinds are
/// silently skipped.
///
/// Pass [`crate::parse::Parsed::root`] to start a full traversal, or pass a
/// child node from within a `visit_*` override to continue descent.
pub fn walk<V: DocstringVisitor>(source: &str, node: &SyntaxNode, visitor: &mut V) -> Result<(), V::Error> {
    match node.kind() {
        // Roots
        SyntaxKind::PLAIN_DOCSTRING => visitor.visit_plain_docstring(source, &PlainDocstring(node))?,
        SyntaxKind::GOOGLE_DOCSTRING => visitor.visit_google_docstring(source, &GoogleDocstring(node))?,
        SyntaxKind::NUMPY_DOCSTRING => visitor.visit_numpy_docstring(source, &NumPyDocstring(node))?,
        // Google inner nodes
        SyntaxKind::GOOGLE_SECTION => visitor.visit_google_section(source, &GoogleSection(node))?,
        SyntaxKind::GOOGLE_DEPRECATION => visitor.visit_google_deprecation(source, &GoogleDeprecation(node))?,
        SyntaxKind::GOOGLE_ARG => visitor.visit_google_arg(source, &GoogleArg(node))?,
        SyntaxKind::GOOGLE_RETURNS => visitor.visit_google_return(source, &GoogleReturn(node))?,
        SyntaxKind::GOOGLE_YIELDS => visitor.visit_google_yield(source, &GoogleYield(node))?,
        SyntaxKind::GOOGLE_EXCEPTION => visitor.visit_google_exception(source, &GoogleException(node))?,
        SyntaxKind::GOOGLE_WARNING => visitor.visit_google_warning(source, &GoogleWarning(node))?,
        SyntaxKind::GOOGLE_SEE_ALSO_ITEM => visitor.visit_google_see_also_item(source, &GoogleSeeAlsoItem(node))?,
        SyntaxKind::GOOGLE_REFERENCE => visitor.visit_google_reference(source, &GoogleReference(node))?,
        SyntaxKind::GOOGLE_ATTRIBUTE => visitor.visit_google_attribute(source, &GoogleAttribute(node))?,
        SyntaxKind::GOOGLE_METHOD => visitor.visit_google_method(source, &GoogleMethod(node))?,
        // NumPy inner nodes
        SyntaxKind::NUMPY_SECTION => visitor.visit_numpy_section(source, &NumPySection(node))?,
        SyntaxKind::NUMPY_DEPRECATION => visitor.visit_numpy_deprecation(source, &NumPyDeprecation(node))?,
        SyntaxKind::NUMPY_PARAMETER => visitor.visit_numpy_parameter(source, &NumPyParameter(node))?,
        SyntaxKind::NUMPY_RETURNS => visitor.visit_numpy_returns(source, &NumPyReturns(node))?,
        SyntaxKind::NUMPY_YIELDS => visitor.visit_numpy_yields(source, &NumPyYields(node))?,
        SyntaxKind::NUMPY_EXCEPTION => visitor.visit_numpy_exception(source, &NumPyException(node))?,
        SyntaxKind::NUMPY_WARNING => visitor.visit_numpy_warning(source, &NumPyWarning(node))?,
        SyntaxKind::NUMPY_SEE_ALSO_ITEM => visitor.visit_numpy_see_also_item(source, &NumPySeeAlsoItem(node))?,
        SyntaxKind::NUMPY_REFERENCE => visitor.visit_numpy_reference(source, &NumPyReference(node))?,
        SyntaxKind::NUMPY_ATTRIBUTE => visitor.visit_numpy_attribute(source, &NumPyAttribute(node))?,
        SyntaxKind::NUMPY_METHOD => visitor.visit_numpy_method(source, &NumPyMethod(node))?,
        // Unknown / token-level kinds
        _ => {}
    }
    Ok(())
}

/// Iterate the children of `node` and call [`walk`] on each child node.
/// Used by the default `visit_*` implementations to continue traversal.
#[inline]
fn walk_children<V: DocstringVisitor>(source: &str, node: &SyntaxNode, visitor: &mut V) -> Result<(), V::Error> {
    for child in node.children() {
        if let SyntaxElement::Node(n) = child {
            walk(source, n, visitor)?;
        }
    }
    Ok(())
}
