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
//! # Dispatch
//!
//! Node kinds are style-neutral ([`SyntaxKind::DOCUMENT`],
//! [`SyntaxKind::SECTION`], [`SyntaxKind::ENTRY`], …), so [`walk`] takes the
//! whole [`Parsed`] result and uses [`Parsed::style`] to route each node to
//! the per-style `visit_*` method.  `ENTRY` nodes are further routed by the
//! kind of their enclosing section (an `ENTRY` in an `Args:` section reaches
//! [`DocstringVisitor::visit_google_arg`], one in a `Raises:` section reaches
//! [`DocstringVisitor::visit_google_exception`], and so on).
//!
//! # Example
//!
//! ```rust
//! use pydocstring::parse::google::{parse_google, GoogleSection};
//! use pydocstring::parse::visitor::{DocstringVisitor, walk};
//! use pydocstring::syntax::{Parsed, SyntaxElement};
//!
//! struct SectionPrinter;
//!
//! impl DocstringVisitor for SectionPrinter {
//!     type Error = std::convert::Infallible;
//!
//!     fn visit_google_section(&mut self, parsed: &Parsed, section: &GoogleSection<'_>) -> Result<(), Self::Error> {
//!         println!("enter: {}", section.header().name().text(parsed.source()));
//!         // continue into children:
//!         for child in section.syntax().children() {
//!             if let SyntaxElement::Node(n) = child { walk(parsed, n, self)?; }
//!         }
//!         println!("leave: {}", section.header().name().text(parsed.source()));
//!         Ok(())
//!     }
//! }
//!
//! let result = parse_google("Args:\n    x: desc\n");
//! let doc = pydocstring::parse::google::GoogleDocstring::cast(result.root()).unwrap();
//! let mut printer = SectionPrinter;
//! printer.visit_google_docstring(&result, &doc).unwrap();
//! ```

use crate::parse::Style;
use crate::parse::google::kind::GoogleSectionKind;
use crate::parse::google::nodes::GoogleArg;
use crate::parse::google::nodes::GoogleAttribute;
use crate::parse::google::nodes::GoogleDeprecation;
use crate::parse::google::nodes::GoogleDocstring;
use crate::parse::google::nodes::GoogleException;
use crate::parse::google::nodes::GoogleMethod;
use crate::parse::google::nodes::GoogleReference;
use crate::parse::google::nodes::GoogleReturn;
use crate::parse::google::nodes::GoogleSection;
use crate::parse::google::nodes::GoogleSeeAlsoItem;
use crate::parse::google::nodes::GoogleWarning;
use crate::parse::google::nodes::GoogleYield;
use crate::parse::numpy::kind::NumPySectionKind;
use crate::parse::numpy::nodes::NumPyAttribute;
use crate::parse::numpy::nodes::NumPyDeprecation;
use crate::parse::numpy::nodes::NumPyDocstring;
use crate::parse::numpy::nodes::NumPyException;
use crate::parse::numpy::nodes::NumPyMethod;
use crate::parse::numpy::nodes::NumPyParameter;
use crate::parse::numpy::nodes::NumPyReference;
use crate::parse::numpy::nodes::NumPyReturns;
use crate::parse::numpy::nodes::NumPySection;
use crate::parse::numpy::nodes::NumPySeeAlsoItem;
use crate::parse::numpy::nodes::NumPyWarning;
use crate::parse::numpy::nodes::NumPyYields;
use crate::parse::plain::nodes::PlainDocstring;
use crate::syntax::Parsed;
use crate::syntax::SyntaxElement;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;

/// Unified typed visitor for Google-style and NumPy-style docstring ASTs.
///
/// Each `visit_*` method's default implementation visits the node's children
/// by calling [`walk`] on each one.  Override a method and either iterate
/// children manually (calling [`walk`]) or omit that loop to prune the subtree.
///
/// The `parsed` parameter is the parse result the node belongs to; use
/// [`Parsed::source`] for reading token text (e.g.
/// `arg.name().text(parsed.source())`).
///
/// `type Error` is the error type returned by all `visit_*` methods.  Use
/// [`std::convert::Infallible`] for infallible visitors.
pub trait DocstringVisitor: Sized {
    /// The error type returned by visitor methods.
    type Error;

    // ── Plain ─────────────────────────────────────────────────────────────
    /// Called for the plain docstring root.
    fn visit_plain_docstring(&mut self, parsed: &Parsed, doc: &PlainDocstring<'_>) -> Result<(), Self::Error> {
        let _ = (parsed, doc);
        Ok(())
    }
    // ── Google ────────────────────────────────────────────────────────────
    /// Called for the Google docstring root.
    fn visit_google_docstring(&mut self, parsed: &Parsed, doc: &GoogleDocstring<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, doc.syntax(), self)
    }
    /// Called for the deprecation notice, if present.
    fn visit_google_deprecation(&mut self, parsed: &Parsed, dep: &GoogleDeprecation<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, dep.syntax(), self)
    }
    /// Called for each Google section.
    fn visit_google_section(&mut self, parsed: &Parsed, sec: &GoogleSection<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, sec.syntax(), self)
    }
    /// Called for each argument entry.
    fn visit_google_arg(&mut self, parsed: &Parsed, arg: &GoogleArg<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, arg.syntax(), self)
    }
    /// Called for the Return entry in a Returns section, if present.
    fn visit_google_return(&mut self, parsed: &Parsed, rtn: &GoogleReturn<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, rtn.syntax(), self)
    }
    /// Called for the Yield entry in a Yields section, if present.
    fn visit_google_yield(&mut self, parsed: &Parsed, yld: &GoogleYield<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, yld.syntax(), self)
    }
    /// Called for each exception entry.
    fn visit_google_exception(&mut self, parsed: &Parsed, exc: &GoogleException<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, exc.syntax(), self)
    }
    /// Called for each warning entry.
    fn visit_google_warning(&mut self, parsed: &Parsed, wrn: &GoogleWarning<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, wrn.syntax(), self)
    }
    /// Called for each See Also item.
    fn visit_google_see_also_item(&mut self, parsed: &Parsed, sai: &GoogleSeeAlsoItem<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, sai.syntax(), self)
    }
    /// Called for each reference entry.
    fn visit_google_reference(&mut self, parsed: &Parsed, r#ref: &GoogleReference<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, r#ref.syntax(), self)
    }
    /// Called for each attribute entry.
    fn visit_google_attribute(&mut self, parsed: &Parsed, att: &GoogleAttribute<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, att.syntax(), self)
    }
    /// Called for each method entry.
    fn visit_google_method(&mut self, parsed: &Parsed, mtd: &GoogleMethod<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, mtd.syntax(), self)
    }
    // ── NumPy ─────────────────────────────────────────────────────────────
    /// Called for the NumPy docstring root.
    fn visit_numpy_docstring(&mut self, parsed: &Parsed, doc: &NumPyDocstring<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, doc.syntax(), self)
    }
    /// Called for the deprecation notice, if present.
    fn visit_numpy_deprecation(&mut self, parsed: &Parsed, dep: &NumPyDeprecation<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, dep.syntax(), self)
    }
    /// Called for each NumPy section.
    fn visit_numpy_section(&mut self, parsed: &Parsed, sec: &NumPySection<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, sec.syntax(), self)
    }
    /// Called for each parameter entry.
    fn visit_numpy_parameter(&mut self, parsed: &Parsed, prm: &NumPyParameter<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, prm.syntax(), self)
    }
    /// Called for each Returns entry.
    fn visit_numpy_returns(&mut self, parsed: &Parsed, rtn: &NumPyReturns<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, rtn.syntax(), self)
    }
    /// Called for each Yields entry.
    fn visit_numpy_yields(&mut self, parsed: &Parsed, yld: &NumPyYields<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, yld.syntax(), self)
    }
    /// Called for each exception entry.
    fn visit_numpy_exception(&mut self, parsed: &Parsed, exc: &NumPyException<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, exc.syntax(), self)
    }
    /// Called for each warning entry.
    fn visit_numpy_warning(&mut self, parsed: &Parsed, wrn: &NumPyWarning<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, wrn.syntax(), self)
    }
    /// Called for each See Also item.
    fn visit_numpy_see_also_item(&mut self, parsed: &Parsed, sai: &NumPySeeAlsoItem<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, sai.syntax(), self)
    }
    /// Called for each reference entry.
    fn visit_numpy_reference(&mut self, parsed: &Parsed, r#ref: &NumPyReference<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, r#ref.syntax(), self)
    }
    /// Called for each attribute entry.
    fn visit_numpy_attribute(&mut self, parsed: &Parsed, att: &NumPyAttribute<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, att.syntax(), self)
    }
    /// Called for each method entry.
    fn visit_numpy_method(&mut self, parsed: &Parsed, mtd: &NumPyMethod<'_>) -> Result<(), Self::Error> {
        walk_children(parsed, mtd.syntax(), self)
    }
}

/// Dispatch `node` to the appropriate `visit_*` method based on its
/// [`SyntaxKind`] and [`Parsed::style`].
///
/// Handles the docstring root (`DOCUMENT`) and all inner nodes (`SECTION`,
/// `ENTRY`, `DIRECTIVE`, `CITATION`).  `ENTRY` nodes are routed to the
/// entry-specific method (`visit_google_arg` vs `visit_google_exception`, …)
/// by the section kind of their enclosing `SECTION` node, looked up from
/// `parsed`'s tree.  Unknown kinds — and nodes that do not belong to
/// `parsed`'s tree — are silently skipped.
///
/// Pass [`Parsed::root`] to start a full traversal, or pass a child node
/// from within a `visit_*` override to continue descent.
pub fn walk<V: DocstringVisitor>(parsed: &Parsed, node: &SyntaxNode, visitor: &mut V) -> Result<(), V::Error> {
    match (node.kind(), parsed.style()) {
        // Roots
        (SyntaxKind::DOCUMENT, Style::Plain) => visitor.visit_plain_docstring(parsed, &PlainDocstring(node))?,
        (SyntaxKind::DOCUMENT, Style::Google) => visitor.visit_google_docstring(parsed, &GoogleDocstring(node))?,
        (SyntaxKind::DOCUMENT, Style::NumPy) => visitor.visit_numpy_docstring(parsed, &NumPyDocstring(node))?,
        // Sections
        (SyntaxKind::SECTION, Style::Google) => visitor.visit_google_section(parsed, &GoogleSection(node))?,
        (SyntaxKind::SECTION, Style::NumPy) => visitor.visit_numpy_section(parsed, &NumPySection(node))?,
        // Directives (deprecation)
        (SyntaxKind::DIRECTIVE, Style::Google) => visitor.visit_google_deprecation(parsed, &GoogleDeprecation(node))?,
        (SyntaxKind::DIRECTIVE, Style::NumPy) => visitor.visit_numpy_deprecation(parsed, &NumPyDeprecation(node))?,
        // Citations (references)
        (SyntaxKind::CITATION, Style::Google) => visitor.visit_google_reference(parsed, &GoogleReference(node))?,
        (SyntaxKind::CITATION, Style::NumPy) => visitor.visit_numpy_reference(parsed, &NumPyReference(node))?,
        // Section entries: routed by the enclosing section's kind.
        (SyntaxKind::ENTRY, Style::Google) => walk_google_entry(parsed, node, visitor)?,
        (SyntaxKind::ENTRY, Style::NumPy) => walk_numpy_entry(parsed, node, visitor)?,
        // Unknown / token-level kinds
        _ => {}
    }
    Ok(())
}

/// Find the direct parent of `target` within the tree rooted at `root`,
/// by node identity (pointer equality).
fn find_parent<'a>(root: &'a SyntaxNode, target: &SyntaxNode) -> Option<&'a SyntaxNode> {
    for child in root.children() {
        if let SyntaxElement::Node(n) = child {
            if core::ptr::eq(n, target) {
                return Some(root);
            }
            if let Some(found) = find_parent(n, target) {
                return Some(found);
            }
        }
    }
    None
}

/// The enclosing `SECTION` node of `entry`, looked up from `parsed`'s root.
fn enclosing_section<'a>(parsed: &'a Parsed, entry: &SyntaxNode) -> Option<&'a SyntaxNode> {
    find_parent(parsed.root(), entry).filter(|parent| parent.kind() == SyntaxKind::SECTION)
}

/// Route a Google `ENTRY` to the entry-specific visit method.
fn walk_google_entry<V: DocstringVisitor>(parsed: &Parsed, node: &SyntaxNode, visitor: &mut V) -> Result<(), V::Error> {
    let Some(section) = enclosing_section(parsed, node).and_then(GoogleSection::cast) else {
        return Ok(());
    };
    match section.section_kind(parsed.source()) {
        GoogleSectionKind::Args
        | GoogleSectionKind::KeywordArgs
        | GoogleSectionKind::OtherParameters
        | GoogleSectionKind::Receives => visitor.visit_google_arg(parsed, &GoogleArg(node)),
        GoogleSectionKind::Returns => visitor.visit_google_return(parsed, &GoogleReturn(node)),
        GoogleSectionKind::Yields => visitor.visit_google_yield(parsed, &GoogleYield(node)),
        GoogleSectionKind::Raises => visitor.visit_google_exception(parsed, &GoogleException(node)),
        GoogleSectionKind::Warns => visitor.visit_google_warning(parsed, &GoogleWarning(node)),
        GoogleSectionKind::SeeAlso => visitor.visit_google_see_also_item(parsed, &GoogleSeeAlsoItem(node)),
        GoogleSectionKind::Attributes => visitor.visit_google_attribute(parsed, &GoogleAttribute(node)),
        GoogleSectionKind::Methods => visitor.visit_google_method(parsed, &GoogleMethod(node)),
        // Free-text sections contain no ENTRY nodes; skip anything else.
        _ => Ok(()),
    }
}

/// Route a NumPy `ENTRY` to the entry-specific visit method.
fn walk_numpy_entry<V: DocstringVisitor>(parsed: &Parsed, node: &SyntaxNode, visitor: &mut V) -> Result<(), V::Error> {
    let Some(section) = enclosing_section(parsed, node).and_then(NumPySection::cast) else {
        return Ok(());
    };
    match section.section_kind(parsed.source()) {
        NumPySectionKind::Parameters
        | NumPySectionKind::OtherParameters
        | NumPySectionKind::Receives
        | NumPySectionKind::KeywordParameters => visitor.visit_numpy_parameter(parsed, &NumPyParameter(node)),
        NumPySectionKind::Returns => visitor.visit_numpy_returns(parsed, &NumPyReturns(node)),
        NumPySectionKind::Yields => visitor.visit_numpy_yields(parsed, &NumPyYields(node)),
        NumPySectionKind::Raises => visitor.visit_numpy_exception(parsed, &NumPyException(node)),
        NumPySectionKind::Warns => visitor.visit_numpy_warning(parsed, &NumPyWarning(node)),
        NumPySectionKind::SeeAlso => visitor.visit_numpy_see_also_item(parsed, &NumPySeeAlsoItem(node)),
        NumPySectionKind::Attributes => visitor.visit_numpy_attribute(parsed, &NumPyAttribute(node)),
        NumPySectionKind::Methods => visitor.visit_numpy_method(parsed, &NumPyMethod(node)),
        // Free-text sections contain no ENTRY nodes; skip anything else.
        _ => Ok(()),
    }
}

/// Iterate the children of `node` and call [`walk`] on each child node.
/// Used by the default `visit_*` implementations to continue traversal.
#[inline]
fn walk_children<V: DocstringVisitor>(parsed: &Parsed, node: &SyntaxNode, visitor: &mut V) -> Result<(), V::Error> {
    for child in node.children() {
        if let SyntaxElement::Node(n) = child {
            walk(parsed, n, visitor)?;
        }
    }
    Ok(())
}
