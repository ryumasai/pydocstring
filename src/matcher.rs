//! The matching engine (#46): anchor-based structural matching of
//! [`Pattern`]s against parsed docstrings.
//!
//! A pattern matches by **structural unification** of a reading's fragment
//! tree (in pattern coordinates) against candidate subtrees of the target
//! (in target coordinates). [`Pattern::matches`] searches the whole
//! document; [`Pattern::matches_in`] scopes the search to an anchor node
//! and derives the grammar binding from it (the maintainer's parent-node
//! design from the #46 discussion). Rewriting is #47.
//!
//! # Style strictness
//!
//! Matching is **style-strict**: a pattern only matches targets of the
//! exact [`Style`](crate::parse::Style) it was parsed with
//! ([`Pattern::style`]` == `[`Parsed::style`]); on a mismatch both entry
//! points return no matches. Cross-style "smart" matching (a Google-spelled
//! pattern matching a NumPy document via the unified kinds) is explicitly
//! deferred, per the #46 design notes.
//!
//! # Unification rules
//!
//! Fragment and candidate trees unify element-by-element:
//!
//! - **Nodes** unify when their kinds are equal and their child lists unify
//!   (below).
//! - **Trivia is skipped on both sides**: `WHITESPACE` and `NEWLINE` tokens
//!   are invisible to matching. Combined with the per-line `TEXT_LINE`
//!   design (whose ranges exclude leading indentation), this makes matching
//!   **indentation-relative**: a pattern written at one indent matches the
//!   same content at any other indent (spaces or tabs). Note that this also
//!   means *interior* extra indentation of continuation lines is layout,
//!   not structure — only line content is compared.
//! - **`BLANK_LINE` is structure**, not trivia, for matching: a blank line
//!   inside a text block is a paragraph break, so a multi-paragraph pattern
//!   only matches a multi-paragraph target. Blank lines pair up one-to-one
//!   (consecutive blank lines produce one token per line), but their
//!   **text** is not compared — the bytes of a blank line are invisible
//!   layout.
//! - **`UNDERLINE` text is not compared** either (kinds must still pair
//!   up): a NumPy header underline's length is presentation, not content.
//! - Every other **token** pairs with a target token of the same kind and
//!   **byte-identical text**. In particular a zero-length (missing
//!   placeholder) pattern token only matches a zero-length target token:
//!   **missing matches missing** (`x ():` matches exactly the targets whose
//!   `TYPE` is missing too). The same holds for zero-length placeholder
//!   nodes (e.g. the empty `DESCRIPTION` of a Google `x:` entry): their
//!   empty child lists unify only with empty child lists.
//!
//! # Metavariable binding
//!
//! - A `$X` site binds **one** target element of the same
//!   [`SyntaxKind`] as the pattern element it landed on (a `NAME` token
//!   binds a `NAME` token, a node site binds a node of that kind). The
//!   bound element's subtree is *not* inspected. `$X` never binds a missing
//!   (zero-length) placeholder — to match a missing element, spell the
//!   missing form literally.
//! - A `$$$X` site is a **hole in its sibling list**: literal siblings
//!   before it match forward, literal siblings after it match backwards,
//!   and the hole binds the (possibly empty) contiguous middle of the
//!   target's sibling list — whatever the kinds of those siblings. When a
//!   `$$$X` covers a whole fragment root (e.g. the entry reading of a lone
//!   `$$$X` line), it binds that single candidate fragment.
//! - An **empty `$$$X`** capture has a zero-length range positioned at the
//!   end of the last sibling matched before the hole; if the hole is first
//!   in its list, at the start of the first sibling matched after it; if
//!   the list is empty, at the start of the parent node. This is the exact
//!   offset where sequence content would be inserted (#47).
//! - **Repeated metavariables** (the same name at several sites, `$X` or
//!   `$$$X`) must all bind **byte-identical** target text; the reported
//!   capture is the first occurrence's binding, in pattern source order.
//!
//! # Readings the matcher cannot use (documented v1 limits)
//!
//! Both limits are per-reading, panic-free, and silent: an affected reading
//! simply contributes no matches, while the pattern's other readings match
//! normally.
//!
//! - A reading with an **inexact site** (a metavariable amid literal prose
//!   *inside* one `TEXT_LINE` token,
//!   [`MetaVarSite::is_exact`](crate::pattern::MetaVarSite::is_exact)` == false`)
//!   is not matchable: sub-line text matching is regex territory and is
//!   deferred.
//! - A reading with **two or more `$$$` holes in the same sibling list** is
//!   not matchable: the split of the middle would be ambiguous. (#45
//!   deliberately inventories such patterns instead of rejecting them, so
//!   the matcher — the layer that gives `$$$` its sequence semantics —
//!   enforces the limit at match time.)
//!
//! # Candidate enumeration and grammar binding
//!
//! Each reading is tried against the target sites its
//! [`FragmentKind`] and
//! [`Reading::section_kinds`] admit. Section roles are resolved with the
//! same section-kind resolution the unified views use
//! ([`Section::kind`](crate::parse::unified::Section::kind)):
//!
//! - **Entry readings** unify against each `ENTRY` (or, for the
//!   `References` reading, `CITATION`) child of every `SECTION` whose
//!   resolved kind is in the reading's `section_kinds`.
//! - **Body readings** unify against the `DESCRIPTION` body of every
//!   free-text section — *any* [`SectionKind::FreeText`], including
//!   [`Unknown`](crate::model::FreeSectionKind::Unknown)-named sections
//!   (the free-text body grammar is the same for all of them; the
//!   reading's `section_kinds` can only list the known names).
//! - **Section readings** unify against every `SECTION` node. Section
//!   readings are concrete syntax: the header `NAME` must match literally
//!   (case-sensitively) unless it is a metavariable.
//! - **Document readings** unify against the `DOCUMENT` root only. Policy:
//!   [`Pattern::matches`] **skips** Document readings unless the pattern
//!   has no other reading (a Document reading of a short prose pattern
//!   could only ever match a whole document of exactly that shape — noise
//!   for a global search); [`Pattern::matches_in`] uses them exactly when
//!   the anchor **is** the document root (explicitly anchoring at the root
//!   opts in).
//!
//! # Anchored matching
//!
//! [`Pattern::matches_in`] restricts the search to the subtree of `anchor`
//! (the anchor node itself included) and derives the grammar binding from
//! it: anchored at a `SECTION`, only the readings admitted by that
//! section's role can bind (`$TYPE: $DESC` anchored at a `Raises:` section
//! selects the Raises-shape reading — `$TYPE` binds a `TYPE` token — where
//! the same pattern anchored at an `Args:` section selects the
//! parameter-shape reading binding a `NAME` token). Anchored at the
//! document root it behaves like [`Pattern::matches`] plus Document
//! readings. An `anchor` that is not a node of `target`'s tree yields no
//! matches.
//!
//! # Match order and overlap
//!
//! Candidate sites are visited in **document pre-order** (a section before
//! its entries), and at each site the readings are tried in the pattern's
//! documented enumeration order. A candidate whose span overlaps an
//! already-accepted match is skipped — **first match wins** — so the
//! returned matches are non-overlapping and in document order. Ties at the
//! same site are resolved by reading order.

use std::collections::HashMap;

use crate::model::SectionKind;
use crate::parse::unified::Section;
use crate::pattern::FragmentKind;
use crate::pattern::Pattern;
use crate::pattern::Reading;
use crate::syntax::Parsed;
use crate::syntax::SyntaxElement;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::syntax::SyntaxToken;
use crate::text::TextRange;

// =============================================================================
// Public types
// =============================================================================

/// One non-overlapping match of a [`Pattern`] against a target document,
/// produced by [`Pattern::matches`] / [`Pattern::matches_in`].
///
/// All ranges and texts are in **target coordinates** — byte offsets into,
/// and slices of, the target's original source (the RFC preservation
/// guarantee: captures expose original bytes, never re-rendered text).
#[derive(Debug, Clone)]
pub struct Match<'t> {
    source: &'t str,
    range: TextRange,
    reading: &'t Reading,
    captures: Vec<(String, Capture<'t>)>,
}

impl<'t> Match<'t> {
    /// The matched span, in target coordinates (the range of the target
    /// fragment the reading unified with: the `ENTRY` / `CITATION`, the
    /// `DESCRIPTION` body, the `SECTION`, or the whole document).
    pub fn range(&self) -> TextRange {
        self.range
    }

    /// The matched span's original target bytes.
    pub fn text(&self) -> &'t str {
        self.range.source_text(self.source)
    }

    /// The [`Reading`] under which the pattern matched — the interpretation
    /// that determines the capture semantics.
    pub fn reading(&self) -> &'t Reading {
        self.reading
    }

    /// The captures, one per distinct metavariable name, in pattern source
    /// order of first occurrence.
    pub fn captures(&self) -> impl Iterator<Item = (&str, &Capture<'t>)> {
        self.captures.iter().map(|(name, capture)| (name.as_str(), capture))
    }

    /// The capture bound to metavariable `name` (without the `$` / `$$$`
    /// sigil), if the pattern has such a metavariable.
    pub fn capture(&self, name: &str) -> Option<&Capture<'t>> {
        self.captures
            .iter()
            .find_map(|(n, capture)| (n == name).then_some(capture))
    }
}

/// What a metavariable bound in one [`Match`].
///
/// A `$X` capture holds exactly one element; a `$$$X` capture holds the
/// bound sibling slice (possibly empty). Ranges and texts are in target
/// coordinates; [`Capture::text`] is the **original target bytes** of the
/// captured span (for a sibling slice this includes the layout bytes
/// between the siblings, verbatim).
#[derive(Debug, Clone)]
pub struct Capture<'t> {
    source: &'t str,
    range: TextRange,
    multi: bool,
    elements: Vec<CapturedElement<'t>>,
}

impl<'t> Capture<'t> {
    /// The captured span in target coordinates. Empty (zero-length) for an
    /// empty `$$$X` capture, positioned where the sequence would be
    /// inserted (see the [module docs](self#metavariable-binding)).
    pub fn range(&self) -> TextRange {
        self.range
    }

    /// The original target bytes of the captured span (empty string for an
    /// empty `$$$X` capture).
    pub fn text(&self) -> &'t str {
        self.range.source_text(self.source)
    }

    /// Whether this capture was bound by a `$$$X` sequence variable.
    pub fn is_multi(&self) -> bool {
        self.multi
    }

    /// The captured target elements: exactly one for `$X`, the bound
    /// sibling slice (possibly empty) for `$$$X`.
    pub fn elements(&self) -> &[CapturedElement<'t>] {
        &self.elements
    }

    /// The single captured element, when the capture holds exactly one
    /// (always the case for `$X`).
    pub fn element(&self) -> Option<CapturedElement<'t>> {
        match self.elements[..] {
            [element] => Some(element),
            _ => None,
        }
    }
}

/// A reference to one captured target element — a node or a token of the
/// target tree.
#[derive(Debug, Clone, Copy)]
pub enum CapturedElement<'t> {
    /// A captured branch node.
    Node(&'t SyntaxNode),
    /// A captured leaf token.
    Token(&'t SyntaxToken),
}

impl<'t> CapturedElement<'t> {
    /// The kind of the captured element.
    pub fn kind(&self) -> SyntaxKind {
        match self {
            Self::Node(n) => n.kind(),
            Self::Token(t) => t.kind(),
        }
    }

    /// The source range of the captured element, in target coordinates.
    pub fn range(&self) -> TextRange {
        match self {
            Self::Node(n) => n.range(),
            Self::Token(t) => t.range(),
        }
    }
}

// =============================================================================
// Entry points
// =============================================================================

impl Pattern {
    /// Find every match of this pattern in `target`: each reading, matched
    /// wherever its section kinds and fragment kind admit it, in document
    /// order, non-overlapping (see the [module docs](self) for the exact
    /// semantics). Style-strict: a target of a different style yields no
    /// matches. Document readings are skipped unless they are the pattern's
    /// only reading.
    pub fn matches<'t>(&'t self, target: &'t Parsed) -> Vec<Match<'t>> {
        let allow_document = self.readings().len() == 1 && self.readings()[0].fragment_kind() == FragmentKind::Document;
        self.run(target, target.root(), allow_document)
    }

    /// Find every match of this pattern inside `anchor`'s subtree (`anchor`
    /// itself included), with the grammar binding derived from the anchor:
    /// anchored at a `SECTION`, only the readings its role admits can bind;
    /// anchored at the document root, Document readings participate too.
    /// See the [module docs](self#anchored-matching). An `anchor` that is
    /// not a node of `target`'s tree yields no matches.
    pub fn matches_in<'t>(&'t self, target: &'t Parsed, anchor: &'t SyntaxNode) -> Vec<Match<'t>> {
        let allow_document = std::ptr::eq(anchor, target.root());
        self.run(target, anchor, allow_document)
    }

    fn run<'t>(&'t self, target: &'t Parsed, anchor: &'t SyntaxNode, allow_document: bool) -> Vec<Match<'t>> {
        if self.style() != target.style() {
            return Vec::new();
        }
        let plans: Vec<ReadingPlan<'_>> = self.readings().iter().map(ReadingPlan::new).collect();
        let mut search = Search {
            target,
            anchor,
            allow_document,
            readings: self.readings(),
            plans,
            matches: Vec::new(),
        };
        search.visit(target.root(), false, None, None);
        search.matches
    }
}

// =============================================================================
// Per-reading match plan
// =============================================================================

/// Pre-computed matchability data for one reading.
struct ReadingPlan<'p> {
    /// Whether the matcher can use this reading at all (all sites exact, at
    /// most one `$$$` hole per sibling list — see the
    /// [module docs](self#readings-the-matcher-cannot-use-documented-v1-limits)).
    usable: bool,
    /// Full wrapped-tree child-index path of each exact metavariable site →
    /// index into [`Reading::metavars`].
    sites: HashMap<&'p [usize], usize>,
}

impl<'p> ReadingPlan<'p> {
    fn new(reading: &'p Reading) -> Self {
        let mut sites: HashMap<&'p [usize], usize> = HashMap::new();
        let mut usable = true;
        let mut hole_parents: Vec<&[usize]> = Vec::new();
        for (index, mv) in reading.metavars().iter().enumerate() {
            if !mv.site().is_exact() {
                usable = false; // Sub-line prose sites: deferred.
                continue;
            }
            let path = mv.site().path();
            sites.insert(path, index);
            if mv.is_multi() {
                let parent = &path[..path.len() - 1];
                if hole_parents.contains(&parent) {
                    usable = false; // Two `$$$` holes in one sibling list.
                }
                hole_parents.push(parent);
            }
        }
        ReadingPlan { usable, sites }
    }
}

// =============================================================================
// Candidate enumeration
// =============================================================================

/// One search run: walks the target tree in pre-order, trying every usable
/// reading at every admissible site, first match wins on overlap.
struct Search<'t> {
    target: &'t Parsed,
    anchor: &'t SyntaxNode,
    allow_document: bool,
    readings: &'t [Reading],
    plans: Vec<ReadingPlan<'t>>,
    matches: Vec<Match<'t>>,
}

impl<'t> Search<'t> {
    fn visit(
        &mut self,
        node: &'t SyntaxNode,
        inside: bool,
        section_kind: Option<&SectionKind>,
        parent_kind: Option<SyntaxKind>,
    ) {
        let inside = inside || std::ptr::eq(node, self.anchor);
        // Resolve a SECTION's role once, with the same resolution the
        // unified views use.
        let own_kind: Option<SectionKind> = (node.kind() == SyntaxKind::SECTION)
            .then(|| Section::cast(self.target, node).expect("SECTION node casts").kind());
        let section_kind = own_kind.as_ref().or(section_kind);

        if inside {
            self.try_site(node, section_kind, parent_kind);
        }
        for child in node.children() {
            if let SyntaxElement::Node(n) = child {
                self.visit(n, inside, section_kind, Some(node.kind()));
            }
        }
    }

    /// Try every reading against one candidate node, in reading order.
    fn try_site(&mut self, node: &'t SyntaxNode, section_kind: Option<&SectionKind>, parent_kind: Option<SyntaxKind>) {
        for (reading, plan) in self.readings.iter().zip(&self.plans) {
            if !plan.usable || !admits(reading, node, section_kind, parent_kind, self.allow_document) {
                continue;
            }
            if self
                .matches
                .iter()
                .any(|accepted| ranges_overlap(accepted.range, node.range()))
            {
                continue; // First match wins.
            }
            let unifier = Unifier {
                reading,
                plan,
                psrc: reading.parsed().source(),
                tsrc: self.target.source(),
                bindings: Vec::new(),
            };
            if let Some(captures) = unifier.unify_fragment(node) {
                self.matches.push(Match {
                    source: self.target.source(),
                    range: node.range(),
                    reading,
                    captures,
                });
            }
        }
    }
}

/// Whether `reading` may be tried against candidate `node` in its grammar
/// context (see the
/// [module docs](self#candidate-enumeration-and-grammar-binding)).
fn admits(
    reading: &Reading,
    node: &SyntaxNode,
    section_kind: Option<&SectionKind>,
    parent_kind: Option<SyntaxKind>,
    allow_document: bool,
) -> bool {
    match reading.fragment_kind() {
        FragmentKind::Entry => {
            node.kind() == reading.fragment().kind()
                && section_kind.is_some_and(|kind| reading.section_kinds().contains(kind))
        }
        FragmentKind::Body => {
            node.kind() == SyntaxKind::DESCRIPTION
                && parent_kind == Some(SyntaxKind::SECTION)
                && matches!(section_kind, Some(SectionKind::FreeText(_)))
        }
        FragmentKind::Section => node.kind() == SyntaxKind::SECTION,
        FragmentKind::Document => allow_document && node.kind() == SyntaxKind::DOCUMENT,
        // `FragmentKind` is `#[non_exhaustive]`; a future variant (e.g.
        // `Field`, #99) must extend this dispatch.
        #[allow(unreachable_patterns)]
        _ => false,
    }
}

fn ranges_overlap(a: TextRange, b: TextRange) -> bool {
    a.start() < b.end() && b.start() < a.end()
}

/// Whether matching skips this token kind entirely (trivia). `BLANK_LINE`
/// is deliberately *not* skipped: a paragraph break is structure.
fn skipped(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::WHITESPACE | SyntaxKind::NEWLINE)
}

/// Token kinds whose text is layout, compared by kind only.
fn text_is_layout(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::BLANK_LINE | SyntaxKind::UNDERLINE)
}

// =============================================================================
// Structural unification
// =============================================================================

/// One unification attempt of a reading's fragment against one candidate
/// node. Consumed by the attempt; bindings accumulate as sites are reached.
struct Unifier<'a, 't> {
    reading: &'t Reading,
    plan: &'a ReadingPlan<'t>,
    psrc: &'t str,
    tsrc: &'t str,
    bindings: Vec<(&'t str, Capture<'t>)>,
}

impl<'t> Unifier<'_, 't> {
    /// Unify the fragment root against the candidate; on success return the
    /// captures in pattern source order of first occurrence.
    fn unify_fragment(mut self, node: &'t SyntaxNode) -> Option<Vec<(String, Capture<'t>)>> {
        let mut path: Vec<usize> = self.reading.fragment_path().to_vec();
        // A metavariable covering the whole fragment root binds the
        // candidate itself (e.g. the entry reading of a lone `$$$X` line).
        let ok = if let Some(&mv) = self.plan.sites.get(path.as_slice()) {
            let multi = self.reading.metavars()[mv].is_multi();
            (multi || !node.range().is_empty())
                && self.bind(
                    mv,
                    Capture {
                        source: self.tsrc,
                        range: node.range(),
                        multi,
                        elements: vec![CapturedElement::Node(node)],
                    },
                )
        } else {
            self.reading.fragment().kind() == node.kind()
                && self.unify_children(self.reading.fragment(), node, &mut path)
        };
        if !ok {
            return None;
        }
        // Report captures by first occurrence in the metavariable table.
        let mut captures: Vec<(String, Capture<'t>)> = Vec::new();
        for mv in self.reading.metavars() {
            if !captures.iter().any(|(name, _)| name == mv.name()) {
                let capture = self
                    .bindings
                    .iter()
                    .find_map(|(name, c)| (*name == mv.name()).then(|| c.clone()))
                    .expect("every exact metavariable site is visited during unification");
                captures.push((mv.name().to_owned(), capture));
            }
        }
        Some(captures)
    }

    /// Unify two sibling lists (trivia skipped on both sides), honouring at
    /// most one `$$$` hole.
    fn unify_children(&mut self, pnode: &'t SyntaxNode, tnode: &'t SyntaxNode, path: &mut Vec<usize>) -> bool {
        let pcs: Vec<(usize, &'t SyntaxElement)> = pnode
            .children()
            .iter()
            .enumerate()
            .filter(|(_, c)| !skipped(c.kind()))
            .collect();
        let tcs: Vec<&'t SyntaxElement> = tnode.children().iter().filter(|c| !skipped(c.kind())).collect();

        // Locate the (at most one, per ReadingPlan) `$$$` hole.
        let hole: Option<usize> = pcs.iter().position(|(index, _)| {
            path.push(*index);
            let is_hole = self
                .plan
                .sites
                .get(path.as_slice())
                .is_some_and(|&mv| self.reading.metavars()[mv].is_multi());
            path.pop();
            is_hole
        });

        let Some(hole) = hole else {
            return pcs.len() == tcs.len()
                && pcs
                    .iter()
                    .zip(&tcs)
                    .all(|((index, pc), tc)| self.unify_element(*index, pc, tc, path));
        };

        let suffix_len = pcs.len() - hole - 1;
        if tcs.len() < hole + suffix_len {
            return false;
        }
        // Literal siblings before the hole, forward.
        if !pcs[..hole]
            .iter()
            .zip(&tcs[..hole])
            .all(|((index, pc), tc)| self.unify_element(*index, pc, tc, path))
        {
            return false;
        }
        // Literal siblings after the hole, backwards.
        let middle_end = tcs.len() - suffix_len;
        if !pcs[hole + 1..]
            .iter()
            .zip(&tcs[middle_end..])
            .all(|((index, pc), tc)| self.unify_element(*index, pc, tc, path))
        {
            return false;
        }
        // The middle binds to the hole.
        path.push(pcs[hole].0);
        let mv = self.plan.sites[path.as_slice()];
        path.pop();
        let middle = &tcs[hole..middle_end];
        let range = match middle {
            [] => {
                let position = if hole > 0 {
                    tcs[hole - 1].range().end()
                } else if middle_end < tcs.len() {
                    tcs[middle_end].range().start()
                } else {
                    tnode.range().start()
                };
                TextRange::new(position, position)
            }
            [first, .., last] => TextRange::new(first.range().start(), last.range().end()),
            [only] => only.range(),
        };
        self.bind(
            mv,
            Capture {
                source: self.tsrc,
                range,
                multi: true,
                elements: middle.iter().map(|c| captured(c)).collect(),
            },
        )
    }

    /// Unify one pattern child element against one target child element.
    fn unify_element(
        &mut self,
        index: usize,
        pc: &'t SyntaxElement,
        tc: &'t SyntaxElement,
        path: &mut Vec<usize>,
    ) -> bool {
        path.push(index);
        let ok = if let Some(&mv) = self.plan.sites.get(path.as_slice()) {
            // A `$X` site: bind one same-kind, non-missing target element.
            tc.kind() == pc.kind()
                && !tc.range().is_empty()
                && self.bind(
                    mv,
                    Capture {
                        source: self.tsrc,
                        range: tc.range(),
                        multi: false,
                        elements: vec![captured(tc)],
                    },
                )
        } else {
            match (pc, tc) {
                (SyntaxElement::Token(p), SyntaxElement::Token(t)) => {
                    p.kind() == t.kind() && (text_is_layout(p.kind()) || p.text(self.psrc) == t.text(self.tsrc))
                }
                (SyntaxElement::Node(p), SyntaxElement::Node(t)) => {
                    p.kind() == t.kind() && self.unify_children(p, t, path)
                }
                _ => false,
            }
        };
        path.pop();
        ok
    }

    /// Record a binding; a repeated metavariable must bind byte-identical
    /// text.
    fn bind(&mut self, mv: usize, capture: Capture<'t>) -> bool {
        let name = self.reading.metavars()[mv].name();
        match self.bindings.iter().find(|(n, _)| *n == name) {
            Some((_, existing)) => existing.text() == capture.text(),
            None => {
                self.bindings.push((name, capture));
                true
            }
        }
    }
}

fn captured<'t>(element: &'t SyntaxElement) -> CapturedElement<'t> {
    match element {
        SyntaxElement::Node(n) => CapturedElement::Node(n),
        SyntaxElement::Token(t) => CapturedElement::Token(t),
    }
}
