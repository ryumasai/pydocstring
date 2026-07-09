//! Pattern fragments with `$X` / `$$$X` metavariables (the input side of the
//! match/rewrite engine — parsing and introspection only; matching is #46 and
//! rewriting is #47).
//!
//! A [`Pattern`] is a docstring *fragment* — a single entry, a section, or a
//! whole document — whose text may contain **metavariables** marking the
//! positions a matcher will later bind to nodes of a target docstring.
//!
//! # Metavariable syntax
//!
//! - `$NAME` binds **one** node or token at its position.
//! - `$$$NAME` binds a **sequence of consecutive siblings** at its position.
//! - A metavariable identifier is `[A-Z][A-Z0-9_]*` (uppercase-only), so
//!   ordinary prose and code fragments like `$x`, `$3`, or `$path` stay
//!   literal text.
//! - A metavariable is only recognised at a word boundary: a `$` immediately
//!   preceded by an ASCII letter, digit, `_`, or another `$` is literal, so
//!   `a$B` and `$$B` are literal text while `($B)` contains a metavariable.
//! - Everything else containing `$` is literal text. There is **no escape
//!   mechanism** in v1: a literal `$UPPER` token cannot be expressed in a
//!   pattern. This is a known, deliberate limitation.
//!
//! # Parsing strategy — placeholder substitution
//!
//! Patterns are parsed by the **existing docstring parsers**, not by a
//! separate pattern grammar. Each metavariable occurrence is replaced by a
//! unique parse-neutral placeholder identifier (alphanumeric, so it lexes as
//! a plain name in every style), the substituted fragment is wrapped in a
//! synthetic sub-grammar context, and the wrapped text is parsed normally:
//!
//! - **entry fragment**: wrapped as a section body under a synthetic header
//!   of the tried section kind (`Args:\n    <fragment, indented>` in Google
//!   style, `Parameters\n----------\n<fragment>` in NumPy style);
//! - **section fragment**: the text itself, parsed as a section-only
//!   docstring (text starting with a recognisable section header);
//! - **document fragment**: the text as-is.
//!
//! Placeholders are collision-proof: the placeholder stem starts at
//! `PYDOCMV` and is lengthened (`PYDOCMVQ`, `PYDOCMVQQ`, …) until the
//! pattern text does not contain it, so pattern text that spells out a
//! placeholder literally still round-trips.
//!
//! After parsing, each placeholder's **landing site** is recorded in the
//! metavariable table: the child-index path to the element it landed on,
//! what it landed *as* (a `NAME` token, a `TYPE` token, a `TEXT_LINE` inside
//! a `DESCRIPTION`, a whole `ENTRY`, …), and whether the placeholder covers
//! that element exactly. A `$NAME` site is the **deepest** element whose
//! range equals the placeholder; a `$$$NAME` site is the **highest** such
//! element below the tree root (so a `$$$X` standalone line in a section
//! body lands as a whole `ENTRY` — the "zero or more siblings here" anchor).
//! A placeholder that ends up *inside* a `TEXT_LINE` token amid literal
//! prose is recorded with [`MetaVarSite::is_exact`]` == false`; a placeholder
//! that ends up inside any **other** token kind (e.g. inside a `TYPE` like
//! `Dict[$K]`) is not a bindable site and makes that parse invalid.
//!
//! # Resolution order of [`Pattern::new`]
//!
//! 1. **Section**: if the text parses (as a document in the given style) to
//!    exactly one `SECTION` and nothing else — i.e. it starts with a header
//!    the style's grammar recognises — the pattern is a section fragment.
//!    Note that Google's grammar accepts *any* `Word:` line as a header, so
//!    a one-line pattern like `$NAME:` is read as a section pattern; use
//!    [`PatternContext::InSection`] to force the entry reading.
//! 2. **Entry**: otherwise, if the text is a **single contiguous block**
//!    (its document parse has exactly one content child — no blank-line
//!    separated blocks, no summary-plus-section structure), it is tried as
//!    a lone entry of every structured section kind (`References` is
//!    excluded because nearly any text parses as a plain-text citation, but
//!    it can be forced with [`PatternContext::InSection`]). Candidates are ranked
//!    by how many metavariables landed exactly on a token/node (fewest
//!    inexact landings win): a role that realises every metavariable as a
//!    bindable structural site beats a role that lumps them into prose.
//!    Remaining ties are resolved by the [role priority
//!    table](self#ambiguity-is-resolved-by-a-documented-priority).
//! 3. **Document**: otherwise the document parse from step 1 is the pattern
//!    (multi-block texts: several sections, summary + sections, blank-line
//!    separated prose — and single blocks no entry role accepts). The
//!    single-block guard in step 2 exists because Google's `Returns` /
//!    `Yields` grammar folds *any* prose block into one entry: without it,
//!    every multi-block pattern would be swallowed by a Returns entry trial
//!    and document patterns would be unreachable.
//!
//! # Forcing a context — parent framing
//!
//! [`Pattern::new_with`] takes [`PatternOptions`]: `strict` switches the
//! ambiguity policy (next section) and [`PatternContext`] forces a reading
//! instead of inferring one. The section context is spelled
//! **semantically** — [`PatternContext::InSection`]`(kind)` means "the
//! text is body content living under a section of this kind" — because
//! post-#77 the syntactic parent kind carries no disambiguating
//! information (every section is a `SECTION` node; the role lives in the
//! header text). The parent framing generalises naturally to free-text
//! bodies, where "the fragment is an entry" was a false constraint: for
//! structured roles the fragment is the `ENTRY` (a `CITATION` for
//! `References`), while for free-text roles (Notes, Examples, …) it is the
//! `DESCRIPTION` body block ([`FragmentKind::Body`]). `Section` and
//! `Document` stay as fragment-kind names because pure parent framing
//! cannot distinguish them — both live directly under `DOCUMENT`.
//!
//! # Ambiguity is resolved by a documented priority
//!
//! When the best-ranked entry candidates differ in shape, [`Pattern::new`]
//! does **not** error: it picks the reading of the highest-priority role.
//! Ambiguity is a static property of pattern text + style, but grammar
//! evolution across library upgrades could flip a formerly-unique pattern
//! to ambiguous and break CI pipelines — a spec-pinned priority makes the
//! reading deterministic and observable instead.
//!
//! | priority | role                |
//! |---------:|---------------------|
//! |        1 | `Parameters`        |
//! |        2 | `KeywordParameters` |
//! |        3 | `OtherParameters`   |
//! |        4 | `Receives`          |
//! |        5 | `Returns`           |
//! |        6 | `Yields`            |
//! |        7 | `Raises`            |
//! |        8 | `Warns`             |
//! |        9 | `Attributes`        |
//! |       10 | `Methods`           |
//! |       11 | `SeeAlso`           |
//!
//! **Stability promise**: this table is part of the crate's contract —
//! changing the order is a breaking change (it is spec-pinned in
//! `tests/pattern.rs`). The resolved reading is observable via
//! [`Pattern::section_kind`]; [`PatternContext::InSection`] is the explicit
//! override; and [`PatternOptions::strict`] restores fail-fast behaviour,
//! returning [`PatternError::Ambiguous`] (candidates listed in priority
//! order) when differently-shaped best-ranked candidates tie.
//! Identically-shaped candidates are indistinguishable, so for them the
//! priority pick only decides which role [`Pattern::section_kind`] reports.
//!
//! # Finding: entry roles still shape the parse
//!
//! Even after the 0.3.0 kind unification (every style and role produces
//! `ENTRY` nodes), the section role **still affects the entry's internal
//! shape**, because the per-role grammars differ:
//!
//! - parameter-family roles (`Parameters`, `KeywordParameters`,
//!   `OtherParameters`, `Receives`, `Attributes`) share one grammar and
//!   always collapse together;
//! - `Raises`/`Warns` store the term as a `TYPE` token where the parameter
//!   family stores `NAME` tokens;
//! - Google `Returns`/`Yields` fold the whole body into a single
//!   `TYPE: description` entry, while NumPy `Returns`/`Yields` share the
//!   `name : type` shape with parameters;
//! - `Methods`/`SeeAlso` never parse a bracketed type.
//!
//! Consequently `$NAME ($TYPE): $DESC` (Google) is unambiguous — only the
//! parameter family realises all three metavariables exactly — while
//! `$NAME: $DESC` (Google) and `$NAME : $TYPE` (NumPy) are genuinely
//! ambiguous between the parameter family and the returns/raises readings:
//! [`Pattern::new`] resolves them to `Parameters` by priority, and
//! [`PatternOptions::strict`] reports them as [`PatternError::Ambiguous`].
//!
//! # Standalone `$$$X` lines — discovered mapping
//!
//! A `$$$X` on a line of its own parses as, and its site is normalised to:
//!
//! | context                                   | site kind        |
//! |-------------------------------------------|------------------|
//! | entry / structured section body (any role)| whole `ENTRY`    |
//! | free-text section body                    | whole `DESCRIPTION` |
//! | document, first content                   | whole `SUMMARY`  |
//! | document, after a summary + blank line    | whole `EXTENDED_SUMMARY` / `PARAGRAPH` |
//!
//! # Wrapped coordinates
//!
//! Per the source-backed convention (#42), a pattern's tree is a regular
//! [`Parsed`] over its **wrapped** pattern source: [`Pattern::parsed`]
//! exposes that tree, and every range in it (including
//! [`MetaVarSite::range`]) is a byte range **into the wrapped pattern
//! source**, not into the original pattern text. [`Pattern::fragment`]
//! returns the root node of the fragment the pattern denotes (the `ENTRY` /
//! `SECTION` / `DOCUMENT`).

use core::fmt;

use crate::model::FreeSectionKind;
use crate::model::SectionKind;
use crate::parse::Style;
use crate::parse::google::parse_google;
use crate::parse::numpy::parse_numpy;
use crate::parse::plain::parse_plain;
use crate::parse::unified::Section;
use crate::syntax::Parsed;
use crate::syntax::SyntaxElement;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::syntax::SyntaxToken;
use crate::text::TextRange;

// =============================================================================
// Public types
// =============================================================================

/// What kind of docstring fragment a [`Pattern`] denotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FragmentKind {
    /// A single section-body entry (the fragment root is an `ENTRY` node —
    /// or a `CITATION` node for `References` patterns forced with
    /// [`PatternContext::InSection`]).
    Entry,
    /// A free-text section body (the fragment root is a `DESCRIPTION`
    /// node), produced by [`PatternContext::InSection`] with a free-text
    /// section kind (Notes, Examples, …).
    Body,
    /// A complete section, header included (the fragment root is a
    /// `SECTION` node).
    Section,
    /// A whole docstring (the fragment root is the `DOCUMENT` node).
    Document,
}

/// One metavariable occurrence in a pattern, with the site its placeholder
/// landed on after parsing.
///
/// The same variable name may occur several times in a pattern; each
/// occurrence gets its own `MetaVar` entry, in source order. (What repeated
/// occurrences *mean* — equality constraints — is the matcher's concern,
/// #46; this table only inventories them.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaVar {
    name: String,
    multi: bool,
    site: MetaVarSite,
}

impl MetaVar {
    /// The variable name (without the `$` / `$$$` sigil).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether this is a `$$$NAME` sequence variable (`true`) or a `$NAME`
    /// single-node variable (`false`).
    pub fn is_multi(&self) -> bool {
        self.multi
    }

    /// Where the metavariable's placeholder landed in the wrapped tree.
    pub fn site(&self) -> &MetaVarSite {
        &self.site
    }
}

/// The landing site of a metavariable placeholder in the wrapped pattern
/// tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaVarSite {
    path: Vec<usize>,
    kind: SyntaxKind,
    parent_kind: SyntaxKind,
    range: TextRange,
    exact: bool,
}

impl MetaVarSite {
    /// Child-index path from the wrapped tree's root
    /// ([`Pattern::parsed`]`.root()`) to the landed element.
    pub fn path(&self) -> &[usize] {
        &self.path
    }

    /// What the placeholder landed **as**: the kind of the landed element
    /// (e.g. a `NAME` token, a `TYPE` token, a `TEXT_LINE` token, a whole
    /// `ENTRY` node for a standalone `$$$X` line).
    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }

    /// The kind of the landed element's parent node (e.g. `DESCRIPTION` for
    /// a `TEXT_LINE` site).
    pub fn parent_kind(&self) -> SyntaxKind {
        self.parent_kind
    }

    /// The placeholder's byte range **in the wrapped pattern source**
    /// ([`Pattern::parsed`]`.source()`).
    pub fn range(&self) -> TextRange {
        self.range
    }

    /// Whether the placeholder's bytes cover the landed element exactly.
    ///
    /// `false` means the placeholder sits *inside* a `TEXT_LINE` token amid
    /// literal prose (a sub-line binding); other token kinds never produce
    /// inexact sites — they make the pattern unparsable instead.
    pub fn is_exact(&self) -> bool {
        self.exact
    }
}

/// Why a pattern could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PatternError {
    /// The text parses as an entry of several section roles with different
    /// resulting shapes. Only produced with [`PatternOptions::strict`]
    /// ([`Pattern::new`] resolves such ties by the documented priority);
    /// use [`PatternContext::InSection`] to pick a role explicitly.
    #[non_exhaustive]
    Ambiguous {
        /// The section kinds whose entry grammars all accept the text
        /// (equally well ranked, but with differing shapes), in priority
        /// order.
        candidates: Vec<SectionKind>,
    },
    /// The text does not parse as any valid fragment of the requested (or
    /// any tried) sub-grammar.
    #[non_exhaustive]
    Unparsable {
        /// Human-readable explanation.
        message: String,
    },
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatternError::Ambiguous { candidates } => {
                write!(
                    f,
                    "ambiguous pattern: it parses as an entry of multiple section roles with different shapes ("
                )?;
                for (i, kind) in candidates.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{kind:?}")?;
                }
                write!(f, "); disambiguate with PatternContext::InSection")
            }
            PatternError::Unparsable { message } => write!(f, "unparsable pattern: {message}"),
        }
    }
}

impl std::error::Error for PatternError {}

/// Which reading of the pattern text to use (see [`PatternOptions`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum PatternContext {
    /// Infer the fragment kind: section, then entry trials, then document
    /// (the [module docs](self#resolution-order-of-patternnew) resolution
    /// order). The default.
    #[default]
    Auto,
    /// Force the text to be read as **body content living under a section
    /// of the given kind**, bypassing inference and priority resolution —
    /// the explicit override for ambiguous entry patterns (and for texts
    /// `Auto` would read as a section, like Google `x:`). The fragment is
    /// whatever that section's grammar produces for its body: an `ENTRY`
    /// for structured roles, a `CITATION` for [`SectionKind::References`],
    /// and the `DESCRIPTION` body block for free-text roles (Notes,
    /// Examples, …).
    InSection(SectionKind),
    /// Force the section reading: the text must parse as exactly one
    /// section (header + body) and nothing else.
    Section,
    /// Force the document reading: any non-empty text parses as a document
    /// fragment, entry/section inference skipped.
    Document,
}

/// Options controlling pattern parsing, on two orthogonal axes: which
/// reading to use ([`PatternContext`]) and the ambiguity policy (`strict`).
///
/// This struct is `#[non_exhaustive]`: new options may be added in minor
/// releases. Construct it via [`Default`] and adjust fields, or use the
/// [`with_context`](PatternOptions::with_context) /
/// [`with_strict`](PatternOptions::with_strict) builders:
///
/// ```rust
/// use pydocstring::model::SectionKind;
/// use pydocstring::parse::Style;
/// use pydocstring::pattern::Pattern;
/// use pydocstring::pattern::PatternContext;
/// use pydocstring::pattern::PatternOptions;
///
/// let options = PatternOptions::default().with_context(PatternContext::InSection(SectionKind::Parameters));
/// let pattern = Pattern::new_with(Style::Google, "$NAME: $DESC", &options).unwrap();
/// assert_eq!(pattern.section_kind(), Some(&SectionKind::Parameters));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct PatternOptions {
    /// Which reading of the pattern text to use. Defaults to
    /// [`PatternContext::Auto`].
    pub context: PatternContext,
    /// Fail fast on ambiguity: with [`PatternContext::Auto`], return
    /// [`PatternError::Ambiguous`] when differently-shaped best-ranked
    /// entry candidates tie instead of resolving by the documented
    /// priority. Defaults to `false`. A no-op for the forced contexts
    /// (there is nothing to resolve).
    pub strict: bool,
}

impl PatternOptions {
    /// Returns these options with `context` replaced.
    #[must_use]
    pub fn with_context(mut self, context: PatternContext) -> Self {
        self.context = context;
        self
    }

    /// Returns these options with `strict` replaced.
    #[must_use]
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }
}

/// A parsed docstring pattern fragment with metavariables.
///
/// See the [module docs](self) for the metavariable syntax, the parsing
/// strategy, and the ambiguity rules.
#[derive(Debug, Clone)]
pub struct Pattern {
    style: Style,
    text: String,
    parsed: Parsed,
    fragment_kind: FragmentKind,
    fragment_path: Vec<usize>,
    section_kind: Option<SectionKind>,
    metavars: Vec<MetaVar>,
}

impl Pattern {
    /// Parse a pattern, inferring its fragment kind.
    ///
    /// Tries, in order: section fragment (text starting with a recognisable
    /// section header), entry fragment in each structured section role for
    /// single-block texts, then document fragment. Ambiguity between entry
    /// roles is resolved by the documented [role priority
    /// table](self#ambiguity-is-resolved-by-a-documented-priority); the
    /// resolved reading is reported by [`Pattern::section_kind`].
    ///
    /// Equivalent to [`Pattern::new_with`] with default [`PatternOptions`].
    /// Use options to fail fast on ambiguity ([`PatternOptions::strict`]) or
    /// to force a reading ([`PatternContext`]).
    pub fn new(style: Style, text: &str) -> Result<Pattern, PatternError> {
        Self::new_with(style, text, &PatternOptions::default())
    }

    /// Parse a pattern with explicit [`PatternOptions`]:
    ///
    /// - [`PatternContext::Auto`] + `strict: false` (the default, also
    ///   [`Pattern::new`]): infer the fragment kind, resolving entry-role
    ///   ambiguity by the documented priority;
    /// - [`PatternContext::Auto`] + `strict: true`: as above, but return
    ///   [`PatternError::Ambiguous`] (candidates in priority order) when
    ///   differently-shaped best-ranked entry candidates tie;
    /// - [`PatternContext::InSection`]: force the body-content reading
    ///   under the given section kind — the fragment is an `ENTRY` for
    ///   structured roles, a `CITATION` for `References`, the `DESCRIPTION`
    ///   body block for free-text roles (`strict` is a no-op — there is
    ///   nothing to resolve);
    /// - [`PatternContext::Section`] / [`PatternContext::Document`]: force
    ///   those readings, [`PatternError::Unparsable`] if the text cannot be
    ///   read that way (`strict` is a no-op).
    pub fn new_with(style: Style, text: &str, options: &PatternOptions) -> Result<Pattern, PatternError> {
        match &options.context {
            PatternContext::InSection(kind) => Self::build_in_section(style, text, kind),
            PatternContext::Section => Self::build_section(style, text),
            PatternContext::Document => Self::build_document(style, text),
            _ => Self::build_auto(style, text, options.strict),
        }
    }

    /// The `Auto` context: section, then entry trials, then document.
    fn build_auto(style: Style, text: &str, strict: bool) -> Result<Pattern, PatternError> {
        let (substituted, occurrences) = substitute_metavars(text);

        // 1. Parse as a document once; this decides section-vs-rest and
        //    doubles as the document-fragment fallback parse.
        let doc_parsed = parse_for(style, &substituted);
        let content: Vec<usize> = content_child_indices(doc_parsed.root());
        if content.is_empty() {
            return Err(PatternError::Unparsable {
                message: "empty pattern: no content to match".to_owned(),
            });
        }

        // 2. Section fragment: exactly one SECTION and nothing else.
        if let [index] = content[..] {
            if doc_parsed.root().children()[index].kind() == SyntaxKind::SECTION {
                return Self::finish_section(style, text, doc_parsed, index, &occurrences);
            }
        }

        // 3. Entry trials, one per structured section role — but only for a
        //    single contiguous block. A text that already parses into
        //    several blocks (summary + section, blank-line-separated prose,
        //    …) is a document; without this guard Google's Returns grammar,
        //    which accepts ANY prose block as one entry, would swallow every
        //    multi-block pattern.
        if content.len() == 1 && matches!(style, Style::Google | Style::NumPy) {
            // Collect the best-ranked candidates, in priority order.
            let mut best: Vec<(SectionKind, InSectionAnalysis)> = Vec::new();
            let mut best_rank = usize::MAX;
            for kind in ENTRY_ROLE_PRIORITY {
                if let Ok(analysis) = analyze_in_section(style, kind, &substituted, &occurrences) {
                    match analysis.rank.cmp(&best_rank) {
                        core::cmp::Ordering::Less => {
                            best_rank = analysis.rank;
                            best = vec![(kind.clone(), analysis)];
                        }
                        core::cmp::Ordering::Equal => best.push((kind.clone(), analysis)),
                        core::cmp::Ordering::Greater => {}
                    }
                }
            }
            if !best.is_empty() {
                if strict {
                    let first_shape = shape_of(&best[0].1);
                    if !best.iter().skip(1).all(|(_, a)| shape_of(a) == first_shape) {
                        return Err(PatternError::Ambiguous {
                            candidates: best.into_iter().map(|(kind, _)| kind).collect(),
                        });
                    }
                }
                // Priority resolution: the highest-priority best-ranked
                // candidate wins (identically-shaped candidates are
                // indistinguishable anyway).
                let (kind, analysis) = best.swap_remove(0);
                return Ok(analysis.into_pattern(style, text, kind));
            }
        }

        // 4. Document fragment.
        Self::finish_document(style, text, doc_parsed, &occurrences)
    }

    /// The `InSection(kind)` context: parse the text as body content under
    /// a section of the given kind. Structured roles produce an `ENTRY`
    /// fragment, [`SectionKind::References`] a `CITATION`, and free-text
    /// roles the `DESCRIPTION` body block.
    fn build_in_section(style: Style, text: &str, kind: &SectionKind) -> Result<Pattern, PatternError> {
        if !matches!(style, Style::Google | Style::NumPy) {
            return Err(PatternError::Unparsable {
                message: format!("style `{style}` has no sections: section-body patterns require google or numpy"),
            });
        }
        if header_name(style, kind).is_none() {
            return Err(PatternError::Unparsable {
                message: format!("section kind {kind:?} has no usable header name in style `{style}`"),
            });
        }
        let (substituted, occurrences) = substitute_metavars(text);
        let analysis = analyze_in_section(style, kind, &substituted, &occurrences).map_err(|message| {
            PatternError::Unparsable {
                message: format!("pattern does not parse as {kind:?} section body content: {message}"),
            }
        })?;
        Ok(analysis.into_pattern(style, text, kind.clone()))
    }

    /// The `Section` context: the text must read as exactly one section.
    fn build_section(style: Style, text: &str) -> Result<Pattern, PatternError> {
        let (substituted, occurrences) = substitute_metavars(text);
        let doc_parsed = parse_for(style, &substituted);
        let content = content_child_indices(doc_parsed.root());
        if let [index] = content[..] {
            if doc_parsed.root().children()[index].kind() == SyntaxKind::SECTION {
                return Self::finish_section(style, text, doc_parsed, index, &occurrences);
            }
        }
        Err(PatternError::Unparsable {
            message: format!(
                "pattern does not parse as a single section (expected one section header and its body, found {} \
                 non-section top-level item(s))",
                content.len()
            ),
        })
    }

    /// The `Document` context: any non-empty text reads as a document.
    fn build_document(style: Style, text: &str) -> Result<Pattern, PatternError> {
        let (substituted, occurrences) = substitute_metavars(text);
        let doc_parsed = parse_for(style, &substituted);
        if content_child_indices(doc_parsed.root()).is_empty() {
            return Err(PatternError::Unparsable {
                message: "empty pattern: no content to match".to_owned(),
            });
        }
        Self::finish_document(style, text, doc_parsed, &occurrences)
    }

    /// Assemble a section pattern from a document parse whose only content
    /// child is the SECTION at `index`.
    fn finish_section(
        style: Style,
        text: &str,
        doc_parsed: Parsed,
        index: usize,
        occurrences: &[Occurrence],
    ) -> Result<Pattern, PatternError> {
        let fragment_path = vec![index];
        check_coverage(&doc_parsed).map_err(unparsable)?;
        let metavars = locate_metavars(&doc_parsed, occurrences, &fragment_path).map_err(unparsable)?;
        let section_kind = match &doc_parsed.root().children()[index] {
            SyntaxElement::Node(node) => Section::cast(&doc_parsed, node).map(|s| s.kind()),
            SyntaxElement::Token(_) => None,
        };
        Ok(Pattern {
            style,
            text: text.to_owned(),
            parsed: doc_parsed,
            fragment_kind: FragmentKind::Section,
            fragment_path,
            section_kind,
            metavars,
        })
    }

    /// Assemble a document pattern from a document parse.
    fn finish_document(
        style: Style,
        text: &str,
        doc_parsed: Parsed,
        occurrences: &[Occurrence],
    ) -> Result<Pattern, PatternError> {
        check_coverage(&doc_parsed).map_err(unparsable)?;
        let metavars = locate_metavars(&doc_parsed, occurrences, &[]).map_err(unparsable)?;
        Ok(Pattern {
            style,
            text: text.to_owned(),
            parsed: doc_parsed,
            fragment_kind: FragmentKind::Document,
            fragment_path: Vec::new(),
            section_kind: None,
            metavars,
        })
    }

    /// The docstring style the pattern is parsed against.
    pub fn style(&self) -> Style {
        self.style
    }

    /// The original pattern text, metavariables included.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// What kind of fragment the pattern denotes.
    pub fn fragment_kind(&self) -> FragmentKind {
        self.fragment_kind
    }

    /// The section context of the fragment: for an entry pattern this is
    /// the **resolved reading** — the role picked by the [priority
    /// table](self#ambiguity-is-resolved-by-a-documented-priority) (or
    /// forced via [`PatternContext::InSection`]) — making the ambiguity
    /// resolution observable; for a section pattern it is the section's own
    /// kind; `None` for a document pattern.
    pub fn section_kind(&self) -> Option<&SectionKind> {
        self.section_kind.as_ref()
    }

    /// The metavariable table: one entry per occurrence, in source order.
    pub fn metavars(&self) -> &[MetaVar] {
        &self.metavars
    }

    /// The parse of the **wrapped** pattern source (see the
    /// [module docs](self#wrapped-coordinates)): all tree coordinates are
    /// byte offsets into `parsed().source()`, not into [`Pattern::text`].
    pub fn parsed(&self) -> &Parsed {
        &self.parsed
    }

    /// The root node of the fragment the pattern denotes: the `ENTRY` (or
    /// `CITATION`), `SECTION`, or `DOCUMENT` node inside
    /// [`Pattern::parsed`]'s tree.
    pub fn fragment(&self) -> &SyntaxNode {
        node_at(self.parsed.root(), &self.fragment_path)
    }
}

// =============================================================================
// Metavariable scanning & placeholder substitution
// =============================================================================

/// One scanned metavariable occurrence, with its assigned placeholder.
struct Occurrence {
    name: String,
    multi: bool,
    placeholder: String,
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_uppercase()
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'
}

/// Bytes that block a `$` from starting a metavariable when they precede it.
fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Pick a placeholder stem that does not occur in the pattern text (probe &
/// lengthen), making placeholders collision-proof.
fn placeholder_stem(text: &str) -> String {
    let mut stem = String::from("PYDOCMV");
    while text.contains(&stem) {
        stem.push('Q');
    }
    stem
}

/// Replace every metavariable occurrence with a unique placeholder name and
/// inventory the occurrences, in source order.
fn substitute_metavars(text: &str) -> (String, Vec<Occurrence>) {
    let stem = placeholder_stem(text);
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut occurrences: Vec<Occurrence> = Vec::new();
    let mut run_start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && (i == 0 || !is_word_byte(bytes[i - 1])) {
            let (sigil_len, multi) = if text[i..].starts_with("$$$") {
                (3, true)
            } else {
                (1, false)
            };
            let ident_start = i + sigil_len;
            if ident_start < bytes.len() && is_ident_start(bytes[ident_start]) {
                let mut end = ident_start + 1;
                while end < bytes.len() && is_ident_continue(bytes[end]) {
                    end += 1;
                }
                let placeholder = format!("{stem}{}X", occurrences.len());
                out.push_str(&text[run_start..i]);
                out.push_str(&placeholder);
                occurrences.push(Occurrence {
                    name: text[ident_start..end].to_owned(),
                    multi,
                    placeholder,
                });
                run_start = end;
                i = end;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&text[run_start..]);
    (out, occurrences)
}

// =============================================================================
// Sub-grammar wrapping
// =============================================================================

/// The structured section kinds tried by [`Pattern::new`], in **priority
/// order** — this order is the documented ambiguity-resolution table (see
/// the [module docs](self#ambiguity-is-resolved-by-a-documented-priority));
/// changing it is a breaking change, spec-pinned in `tests/pattern.rs`.
/// `References` is deliberately excluded (see the module docs).
const ENTRY_ROLE_PRIORITY: &[SectionKind] = &[
    SectionKind::Parameters,
    SectionKind::KeywordParameters,
    SectionKind::OtherParameters,
    SectionKind::Receives,
    SectionKind::Returns,
    SectionKind::Yields,
    SectionKind::Raises,
    SectionKind::Warns,
    SectionKind::Attributes,
    SectionKind::Methods,
    SectionKind::SeeAlso,
];

/// The synthetic section header name for wrapping a section-body fragment
/// of `kind` in `style`, or `None` when the kind cannot be spelled as a
/// header in that style.
fn header_name(style: Style, kind: &SectionKind) -> Option<String> {
    let name = match (style, kind) {
        (Style::Google, SectionKind::Parameters) => "Args",
        (Style::Google, SectionKind::KeywordParameters) => "Keyword Args",
        (Style::NumPy, SectionKind::Parameters) => "Parameters",
        (Style::NumPy, SectionKind::KeywordParameters) => "Keyword Parameters",
        (Style::Google | Style::NumPy, SectionKind::OtherParameters) => "Other Parameters",
        (Style::Google | Style::NumPy, SectionKind::Receives) => "Receives",
        (Style::Google | Style::NumPy, SectionKind::Returns) => "Returns",
        (Style::Google | Style::NumPy, SectionKind::Yields) => "Yields",
        (Style::Google | Style::NumPy, SectionKind::Raises) => "Raises",
        (Style::Google | Style::NumPy, SectionKind::Warns) => "Warns",
        (Style::Google | Style::NumPy, SectionKind::Attributes) => "Attributes",
        (Style::Google | Style::NumPy, SectionKind::Methods) => "Methods",
        (Style::Google | Style::NumPy, SectionKind::SeeAlso) => "See Also",
        (Style::Google | Style::NumPy, SectionKind::References) => "References",
        (Style::Google | Style::NumPy, SectionKind::FreeText(free)) => return free_header_name(free),
        _ => return None,
    };
    Some(name.to_owned())
}

/// The header name for a free-text section kind. Both style grammars
/// recognise these names (and read any unknown header as a free-text
/// section, so `Unknown` names work too, as long as they can appear on a
/// header line).
fn free_header_name(kind: &FreeSectionKind) -> Option<String> {
    let name = match kind {
        FreeSectionKind::Notes => "Notes",
        FreeSectionKind::Examples => "Examples",
        FreeSectionKind::Warnings => "Warnings",
        FreeSectionKind::Todo => "Todo",
        FreeSectionKind::Attention => "Attention",
        FreeSectionKind::Caution => "Caution",
        FreeSectionKind::Danger => "Danger",
        FreeSectionKind::Error => "Error",
        FreeSectionKind::Hint => "Hint",
        FreeSectionKind::Important => "Important",
        FreeSectionKind::Tip => "Tip",
        FreeSectionKind::Unknown(name) => {
            let trimmed = name.trim();
            return (!trimmed.is_empty() && !trimmed.contains('\n')).then(|| trimmed.to_owned());
        }
    };
    Some(name.to_owned())
}

/// Wrap a (substituted) section-body fragment under a synthetic section
/// header.
fn wrap_section_body(style: Style, header: &str, fragment: &str) -> String {
    let fragment = fragment.strip_suffix('\n').unwrap_or(fragment);
    let mut out = String::new();
    match style {
        Style::Google => {
            out.push_str(header);
            out.push_str(":\n");
            for line in fragment.split('\n') {
                if !line.trim().is_empty() {
                    out.push_str("    ");
                    out.push_str(line);
                }
                out.push('\n');
            }
        }
        _ => {
            // NumPy: entries sit at the header's own indent level.
            out.push_str(header);
            out.push('\n');
            for _ in 0..header.len() {
                out.push('-');
            }
            out.push('\n');
            for line in fragment.split('\n') {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

/// Parse `text` with the parser for `style` (no auto-detection).
fn parse_for(style: Style, text: &str) -> Parsed {
    match style {
        Style::NumPy => parse_numpy(text),
        Style::Google => parse_google(text),
        _ => parse_plain(text),
    }
}

// =============================================================================
// Landing-site analysis
// =============================================================================

/// One link of the containment chain from the root down to the placeholder.
struct ChainLink {
    path: Vec<usize>,
    kind: SyntaxKind,
    range: TextRange,
    is_token: bool,
}

/// The chain of elements containing `ph`, root first, ending at the token
/// that owns the placeholder bytes.
fn landing_chain(root: &SyntaxNode, ph: TextRange) -> Vec<ChainLink> {
    let mut chain = vec![ChainLink {
        path: Vec::new(),
        kind: root.kind(),
        range: *root.range(),
        is_token: false,
    }];
    let mut path = Vec::new();
    let mut cur = root;
    'descend: loop {
        for (i, child) in cur.children().iter().enumerate() {
            let r = *child.range();
            if !r.is_empty() && r.start() <= ph.start() && ph.end() <= r.end() {
                path.push(i);
                match child {
                    SyntaxElement::Node(n) => {
                        chain.push(ChainLink {
                            path: path.clone(),
                            kind: n.kind(),
                            range: r,
                            is_token: false,
                        });
                        cur = n;
                        continue 'descend;
                    }
                    SyntaxElement::Token(t) => {
                        chain.push(ChainLink {
                            path: path.clone(),
                            kind: t.kind(),
                            range: r,
                            is_token: true,
                        });
                        break 'descend;
                    }
                }
            }
        }
        break;
    }
    chain
}

/// Pick the landing site for one placeholder from its containment chain.
///
/// `$X` takes the deepest exactly-covered element; `$$$X` takes the highest
/// exactly-covered element below the root. With no exact match the
/// placeholder must sit inside a `TEXT_LINE` (recorded as inexact); inside
/// any other token it is not a bindable site.
fn choose_site(
    chain: &[ChainLink],
    sigil: &str,
    name: &str,
    ph: TextRange,
    multi: bool,
) -> Result<MetaVarSite, String> {
    let exact: Vec<usize> = (1..chain.len()).filter(|&i| chain[i].range == ph).collect();
    let chosen = if let Some(&i) = if multi { exact.first() } else { exact.last() } {
        i
    } else {
        let last = chain.len() - 1;
        if last == 0 || !chain[last].is_token {
            return Err(format!("metavariable {sigil}{name} did not land on a single token"));
        }
        if chain[last].kind != SyntaxKind::TEXT_LINE {
            return Err(format!(
                "metavariable {sigil}{name} lands inside a {} token mixed with literal text and cannot bind a whole node",
                chain[last].kind
            ));
        }
        last
    };
    Ok(MetaVarSite {
        path: chain[chosen].path.clone(),
        kind: chain[chosen].kind,
        parent_kind: chain[chosen - 1].kind,
        range: ph,
        exact: chain[chosen].range == ph,
    })
}

/// Locate every placeholder in the wrapped source and build the metavariable
/// table. All sites must lie under `required_prefix` (the fragment path).
fn locate_metavars(
    parsed: &Parsed,
    occurrences: &[Occurrence],
    required_prefix: &[usize],
) -> Result<Vec<MetaVar>, String> {
    let mut metavars = Vec::with_capacity(occurrences.len());
    for occ in occurrences {
        let sigil = if occ.multi { "$$$" } else { "$" };
        let offset = parsed
            .source()
            .find(&occ.placeholder)
            .ok_or_else(|| format!("metavariable {sigil}{} was lost during parsing", occ.name))?;
        let ph = TextRange::from_offset_len(offset, occ.placeholder.len());
        let chain = landing_chain(parsed.root(), ph);
        let site = choose_site(&chain, sigil, &occ.name, ph, occ.multi)?;
        if !site.path.starts_with(required_prefix) {
            return Err(format!("metavariable {sigil}{} landed outside the fragment", occ.name));
        }
        metavars.push(MetaVar {
            name: occ.name.clone(),
            multi: occ.multi,
            site,
        });
    }
    Ok(metavars)
}

// =============================================================================
// Fragment validation
// =============================================================================

fn unparsable(message: String) -> PatternError {
    PatternError::Unparsable { message }
}

/// Indices of the non-trivia children of `node`.
fn content_child_indices(node: &SyntaxNode) -> Vec<usize> {
    node.children()
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.kind().is_trivia())
        .map(|(i, _)| i)
        .collect()
}

/// Resolve a child-index path to a node.
fn node_at<'a>(root: &'a SyntaxNode, path: &[usize]) -> &'a SyntaxNode {
    let mut cur = root;
    for &i in path {
        match &cur.children()[i] {
            SyntaxElement::Node(n) => cur = n,
            SyntaxElement::Token(_) => unreachable!("fragment path points at a token"),
        }
    }
    cur
}

/// Defensive re-check of the byte-coverage law on a wrapped parse: a gap
/// would mean the fragment silently lost bytes.
fn check_coverage(parsed: &Parsed) -> Result<(), String> {
    fn collect<'a>(node: &'a SyntaxNode, out: &mut Vec<&'a SyntaxToken>) {
        for child in node.children() {
            match child {
                SyntaxElement::Node(n) => collect(n, out),
                SyntaxElement::Token(t) => out.push(t),
            }
        }
    }
    let mut tokens = Vec::new();
    collect(parsed.root(), &mut tokens);
    tokens.sort_by_key(|t| (t.range().start(), t.range().end()));
    let mut pos = 0usize;
    for token in tokens {
        let (start, end) = (usize::from(token.range().start()), usize::from(token.range().end()));
        if start > pos {
            return Err(format!("pattern parse lost bytes at {pos}..{start}"));
        }
        pos = pos.max(end);
    }
    if pos != parsed.source().len() {
        return Err(format!("pattern parse lost trailing bytes at {pos}.."));
    }
    Ok(())
}

/// A validated section-body candidate parse (an entry, a citation, or a
/// free-text body block, per the section kind's grammar).
struct InSectionAnalysis {
    parsed: Parsed,
    fragment_kind: FragmentKind,
    fragment_path: Vec<usize>,
    metavars: Vec<MetaVar>,
    /// Number of inexact metavariable landings (0 = every metavariable is a
    /// whole token/node — the best possible realisation).
    rank: usize,
}

impl InSectionAnalysis {
    fn into_pattern(self, style: Style, text: &str, kind: SectionKind) -> Pattern {
        Pattern {
            style,
            text: text.to_owned(),
            parsed: self.parsed,
            fragment_kind: self.fragment_kind,
            fragment_path: self.fragment_path,
            section_kind: Some(kind),
            metavars: self.metavars,
        }
    }
}

/// Try to parse the (substituted) fragment as the body content of one
/// `kind` section: exactly one `ENTRY` for structured roles, one `CITATION`
/// for `References`, or the `DESCRIPTION` body block for free-text roles.
fn analyze_in_section(
    style: Style,
    kind: &SectionKind,
    substituted: &str,
    occurrences: &[Occurrence],
) -> Result<InSectionAnalysis, String> {
    let header =
        header_name(style, kind).ok_or_else(|| format!("section kind {kind:?} cannot be spelled as a header"))?;
    let wrapped = wrap_section_body(style, &header, substituted);
    let parsed = parse_for(style, &wrapped);
    check_coverage(&parsed)?;

    let root_content = content_child_indices(parsed.root());
    let [section_index] = root_content[..] else {
        return Err(format!(
            "expected a single section, found {} top-level items",
            root_content.len()
        ));
    };
    let SyntaxElement::Node(section) = &parsed.root().children()[section_index] else {
        return Err("expected a section node".to_owned());
    };
    if section.kind() != SyntaxKind::SECTION {
        return Err(format!("expected a SECTION, found {}", section.kind()));
    }

    let expected_body_kind = match kind {
        SectionKind::References => SyntaxKind::CITATION,
        SectionKind::FreeText(_) => SyntaxKind::DESCRIPTION,
        _ => SyntaxKind::ENTRY,
    };
    let section_content = content_child_indices(section);
    if section_content.len() != 2 {
        return Err(format!(
            "expected exactly one {expected_body_kind} in the section body, found {}",
            section_content.len().saturating_sub(1)
        ));
    }
    if section.children()[section_content[0]].kind() != SyntaxKind::SECTION_HEADER {
        return Err("section header missing from the wrapped parse".to_owned());
    }
    let body_index = section_content[1];
    let body_kind = section.children()[body_index].kind();
    if body_kind != expected_body_kind {
        return Err(format!(
            "expected the section body to be one {expected_body_kind}, found {body_kind}"
        ));
    }

    let fragment_path = vec![section_index, body_index];
    let metavars = locate_metavars(&parsed, occurrences, &fragment_path)?;
    let rank = metavars.iter().filter(|m| !m.site.exact).count();
    let fragment_kind = if expected_body_kind == SyntaxKind::DESCRIPTION {
        FragmentKind::Body
    } else {
        FragmentKind::Entry
    };
    Ok(InSectionAnalysis {
        parsed,
        fragment_kind,
        fragment_path,
        metavars,
        rank,
    })
}

// =============================================================================
// Shape comparison (candidate collapsing)
// =============================================================================

/// A role-independent fingerprint of a candidate's fragment: the fragment
/// subtree (kinds + ranges rebased to the fragment start) plus the
/// metavariable sites (paths rebased to the fragment root). Candidates with
/// equal shapes denote the same pattern and collapse.
#[derive(PartialEq, Eq)]
struct Shape {
    tree: Vec<(SyntaxKind, bool, u32, u32)>,
    vars: Vec<VarShape>,
}

/// One metavariable's contribution to a candidate's shape, rebased to the
/// fragment root.
#[derive(PartialEq, Eq)]
struct VarShape {
    name: String,
    multi: bool,
    kind: SyntaxKind,
    parent_kind: SyntaxKind,
    exact: bool,
    start: u32,
    end: u32,
    rel_path: Vec<usize>,
}

fn shape_of(analysis: &InSectionAnalysis) -> Shape {
    fn dfs(node: &SyntaxNode, base: u32, out: &mut Vec<(SyntaxKind, bool, u32, u32)>) {
        out.push((
            node.kind(),
            true,
            node.range().start().raw() - base,
            node.range().end().raw() - base,
        ));
        for child in node.children() {
            match child {
                SyntaxElement::Node(n) => dfs(n, base, out),
                SyntaxElement::Token(t) => out.push((
                    t.kind(),
                    false,
                    t.range().start().raw() - base,
                    t.range().end().raw() - base,
                )),
            }
        }
    }

    let fragment = node_at(analysis.parsed.root(), &analysis.fragment_path);
    let base = fragment.range().start().raw();
    let mut tree = Vec::new();
    dfs(fragment, base, &mut tree);
    let vars = analysis
        .metavars
        .iter()
        .map(|m| VarShape {
            name: m.name.clone(),
            multi: m.multi,
            kind: m.site.kind,
            parent_kind: m.site.parent_kind,
            exact: m.site.exact,
            start: m.site.range.start().raw() - base,
            end: m.site.range.end().raw() - base,
            rel_path: m.site.path[analysis.fragment_path.len()..].to_vec(),
        })
        .collect();
    Shape { tree, vars }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_single_and_multi() {
        let (out, occs) = substitute_metavars("$NAME ($TYPE): $$$REST");
        assert_eq!(occs.len(), 3);
        assert_eq!(occs[0].name, "NAME");
        assert!(!occs[0].multi);
        assert_eq!(occs[2].name, "REST");
        assert!(occs[2].multi);
        assert_eq!(out, "PYDOCMV0X (PYDOCMV1X): PYDOCMV2X");
    }

    #[test]
    fn test_substitute_literals() {
        // Lowercase, digits, word-adjacent, and double-dollar stay literal.
        for text in ["$x", "$3", "a$B", "$$B", "cost: $5", "US$ 3"] {
            let (out, occs) = substitute_metavars(text);
            assert!(occs.is_empty(), "{text:?} should have no metavariables");
            assert_eq!(out, text);
        }
    }

    #[test]
    fn test_substitute_identifier_charset() {
        let (_, occs) = substitute_metavars("$A_2B $Aa");
        // `$Aa`: identifier stops before the lowercase letter.
        assert_eq!(occs.len(), 2);
        assert_eq!(occs[0].name, "A_2B");
        assert_eq!(occs[1].name, "A");
    }

    #[test]
    fn test_placeholder_stem_probing() {
        assert_eq!(placeholder_stem("no collision"), "PYDOCMV");
        assert_eq!(placeholder_stem("contains PYDOCMV literal"), "PYDOCMVQ");
        assert_eq!(placeholder_stem("PYDOCMV and PYDOCMVQ"), "PYDOCMVQQ");
    }

    #[test]
    fn test_wrap_section_body_google_indents_and_keeps_blank_lines() {
        let wrapped = wrap_section_body(Style::Google, "Args", "x: d\n\n    more\n");
        assert_eq!(wrapped, "Args:\n    x: d\n\n        more\n");
    }

    #[test]
    fn test_wrap_section_body_numpy_underline_matches_header() {
        let wrapped = wrap_section_body(Style::NumPy, "Keyword Parameters", "x : int");
        assert_eq!(wrapped, "Keyword Parameters\n------------------\nx : int\n");
    }
}
