//! Pattern fragments with `$X` / `$$$X` metavariables (the input side of the
//! match/rewrite engine — parsing and introspection only; matching is #46 and
//! rewriting is #47).
//!
//! A [`Pattern`] is a **context-free** docstring fragment — text that may
//! contain **metavariables** marking the positions a matcher will later
//! bind to nodes of a target docstring. [`Pattern::readings`] enumerates
//! every valid interpretation of the text (as a lone entry under one or
//! more section roles, a free-text section body, a whole section, a whole
//! document, …); which one applies is decided at match time (#46).
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
//! - **entry readings**: wrapped as a section body under a synthetic header
//!   of each tried role (`Args:\n    <fragment, indented>` in Google style,
//!   `Parameters\n----------\n<fragment>` in NumPy style);
//! - **body reading**: wrapped the same way under a free-text header
//!   (`Notes`) — the free-text body grammar is identical for every
//!   free-text section kind;
//! - **section / document readings**: the text as-is.
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
//! # Readings — every valid interpretation, enumerated
//!
//! A pattern is context-free: [`Pattern::new`] does not decide what the
//! text "is". Instead [`Pattern::readings`] enumerates **every** valid
//! interpretation as a [`Reading`] — the same text can be a lone entry
//! under several section roles, a free-text section body, a whole section,
//! and a whole document, all at once. Which reading applies is decided at
//! **match time** from the anchor node the pattern is matched against
//! (#46): the anchor's own grammar selects the reading, so no context has
//! to be declared up front.
//!
//! Entry readings that parse to the **same shape** under different roles
//! (identical fragment subtree and metavariable sites, relative to the
//! fragment) are merged into one `Reading` whose
//! [`section_kinds`](Reading::section_kinds) lists every role it applies
//! under — identical shape means identical capture semantics, so the
//! reading is safely shared. Distinct shapes stay separate readings: NumPy
//! `$NAME : $TYPE` yields a parameter-family reading (binding `$NAME` to a
//! `NAME` token) *and* a distinct `Raises`/`Warns` reading (binding it to
//! a `TYPE` token), both enumerated.
//!
//! This replaces an earlier static-context design (a context enum forcing
//! one reading): the reST/Sphinx tree exercise on #41 showed that static
//! context enums do not scale to nested reST sections or `:param:` field
//! lists, while match-time anchors do — and the #41 design record requires
//! the pattern layer not to bake role interpretation into parsing.
//! [`FragmentKind`] is `#[non_exhaustive]` for the same reason: a `Field`
//! reading is expected once Sphinx field lists land (#99).
//!
//! # Enumeration order
//!
//! Readings are enumerated in a fixed, documented order — entry readings
//! by role (a merged reading sits at its first role's position and lists
//! its roles in this same order), then the free-text body reading, then
//! the section reading, then the document reading last:
//!
//! | order | reading                          |
//! |------:|----------------------------------|
//! |     1 | entry: `Parameters`              |
//! |     2 | entry: `KeywordParameters`       |
//! |     3 | entry: `OtherParameters`         |
//! |     4 | entry: `Receives`                |
//! |     5 | entry: `Returns`                 |
//! |     6 | entry: `Yields`                  |
//! |     7 | entry: `Raises`                  |
//! |     8 | entry: `Warns`                   |
//! |     9 | entry: `Attributes`              |
//! |    10 | entry: `Methods`                 |
//! |    11 | entry: `SeeAlso`                 |
//! |    12 | entry: `References` (`CITATION`) |
//! |    13 | free-text body (`Body`)          |
//! |    14 | `Section`                        |
//! |    15 | `Document`                       |
//!
//! **Stability promise**: this order is part of the crate's contract —
//! changing it is a breaking change (spec-pinned in `tests/pattern.rs`).
//! The primary reading (`readings()[0]`, exposed by the [`Pattern`]
//! conveniences) is simply the first enumerated one. `References` sits
//! last in the entry tier because its grammar accepts nearly any line as a
//! plain-text citation; `Body` and `Document` are similarly late because
//! they accept nearly everything.
//!
//! Applicability: entry readings are only tried for texts that are a
//! **single contiguous block** (one content child in the document parse) —
//! Google's `Returns` grammar folds any prose block into one entry, so an
//! entry reading of a multi-block text would be uninformative. The body
//! reading has no such guard: a free-text `DESCRIPTION` owns its blank
//! lines by design, so multi-paragraph bodies are legitimate. The section
//! reading exists when the text parses to exactly one `SECTION` and
//! nothing else (note that Google's grammar accepts *any* `Word:` line as
//! a header, so `x:` has both entry readings and a section reading). The
//! document reading exists for any non-empty text whose metavariables land
//! on bindable sites in the plain document parse.
//!
//! # Finding: entry roles still shape the parse
//!
//! Even after the 0.3.0 kind unification (every style and role produces
//! `ENTRY` nodes), the section role **still affects the entry's internal
//! shape**, because the per-role grammars differ:
//!
//! - parameter-family roles (`Parameters`, `KeywordParameters`,
//!   `OtherParameters`, `Receives`, `Attributes`) share one grammar and
//!   always merge;
//! - `Raises`/`Warns` store the term as a `TYPE` token where the parameter
//!   family stores `NAME` tokens;
//! - Google `Returns`/`Yields` fold the whole body into a single
//!   `TYPE: description` entry, while NumPy `Returns`/`Yields` share the
//!   `name : type` shape with parameters (and merge with them);
//! - `Methods`/`SeeAlso` never parse a bracketed type.
//!
//! Consequently `$NAME ($TYPE): $DESC` (Google) has one all-exact entry
//! reading (the parameter family, merged) plus a separate `Returns`/
//! `Yields` prose reading, while `$NAME: $DESC` (Google) and
//! `$NAME : $TYPE` (NumPy) split into distinct NAME-binding and
//! TYPE-binding readings — all enumerated, none an error.
//!
//! # When is a pattern unparsable?
//!
//! [`PatternError::Unparsable`] is returned only when **zero** readings
//! exist: empty or whitespace-only text, or text whose every candidate
//! reading is rejected. The document reading makes zero readings rare, but
//! it is not unconditional — it too requires every metavariable to land on
//! a bindable site. In practice the all-rejected case needs a metavariable
//! inside a structural token (e.g. `Dict[$K]`) in every grammar that could
//! host the text — for example a NumPy section text with an embedded
//! underlined header (which invalidates the entry/body wraps) whose entry
//! contains such a metavariable (which invalidates the section and
//! document parses).
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
//! Per the source-backed convention (#42), each reading's tree is a
//! regular [`Parsed`] over its own **wrapped** pattern source:
//! [`Reading::parsed`] exposes that tree, and every range in it (including
//! [`MetaVarSite::range`]) is a byte range **into that reading's wrapped
//! source**, not into the original pattern text. [`Reading::fragment`]
//! returns the root node of the fragment the reading denotes (the `ENTRY`
//! / `CITATION` / `DESCRIPTION` body / `SECTION` / `DOCUMENT`).

use core::fmt;

use crate::model::FreeSectionKind;
use crate::model::SectionKind;
use crate::parse::Style;
use crate::parse::google::parse_google;
use crate::parse::numpy::parse_numpy;
use crate::parse::plain::parse_plain;
use crate::syntax::Parsed;
use crate::syntax::SyntaxElement;
use crate::syntax::SyntaxKind;
use crate::syntax::SyntaxNode;
use crate::syntax::SyntaxToken;
use crate::text::TextRange;

// =============================================================================
// Public types
// =============================================================================

/// What kind of docstring fragment a [`Reading`] denotes.
///
/// `#[non_exhaustive]`: new fragment kinds may be added in minor releases —
/// a `Field` variant is expected for Sphinx field lists (#99).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FragmentKind {
    /// A single section-body entry (the fragment root is an `ENTRY` node —
    /// or a `CITATION` node for the `References` reading).
    Entry,
    /// A free-text section body (the fragment root is a `DESCRIPTION`
    /// node): the reading under free-text section kinds (Notes, Examples,
    /// …).
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

/// Why a pattern could not be built. Ambiguity is **not** an error — all
/// readings coexist (see the [module docs](self)); only input with zero
/// valid readings fails.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PatternError {
    /// The text has no valid reading (see the [module
    /// docs](self#when-is-a-pattern-unparsable)).
    #[non_exhaustive]
    Unparsable {
        /// Human-readable explanation.
        message: String,
    },
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatternError::Unparsable { message } => write!(f, "unparsable pattern: {message}"),
        }
    }
}

impl std::error::Error for PatternError {}

/// One valid interpretation of a pattern's text (see the [module
/// docs](self#readings--every-valid-interpretation-enumerated)).
#[derive(Debug, Clone)]
pub struct Reading {
    parsed: Parsed,
    fragment_kind: FragmentKind,
    fragment_path: Vec<usize>,
    section_kinds: Vec<SectionKind>,
    metavars: Vec<MetaVar>,
}

impl Reading {
    /// What kind of fragment this reading denotes.
    pub fn fragment_kind(&self) -> FragmentKind {
        self.fragment_kind
    }

    /// The section roles this reading applies under, in enumeration order —
    /// every role whose grammar produced this exact shape (merged
    /// readings). Empty for `Section` and `Document` readings, which do not
    /// live under a section. For the `Body` reading it lists the known
    /// free-text kinds (the free-text body grammar is identical under any
    /// of them, including unknown-named sections).
    pub fn section_kinds(&self) -> &[SectionKind] {
        &self.section_kinds
    }

    /// The fragment root node this reading denotes: the `ENTRY` /
    /// `CITATION`, the `DESCRIPTION` body block, the `SECTION`, or the
    /// `DOCUMENT` inside [`Reading::parsed`]'s tree.
    pub fn fragment(&self) -> &SyntaxNode {
        node_at(self.parsed.root(), &self.fragment_path)
    }

    /// This reading's metavariable table: one entry per occurrence, in
    /// source order, with sites in this reading's wrapped coordinates.
    pub fn metavars(&self) -> &[MetaVar] {
        &self.metavars
    }

    /// This reading's wrapped parse (see the
    /// [module docs](self#wrapped-coordinates)).
    pub fn parsed(&self) -> &Parsed {
        &self.parsed
    }

    /// Child-index path from the wrapped tree's root to the fragment root
    /// ([`Reading::fragment`]). Crate-private: the matcher (#46) uses it to
    /// relate metavariable site paths to the fragment during unification.
    pub(crate) fn fragment_path(&self) -> &[usize] {
        &self.fragment_path
    }
}

/// A parsed, context-free docstring pattern with metavariables.
///
/// [`Pattern::readings`] enumerates every valid interpretation; the
/// [`Pattern::fragment`]-family conveniences expose the primary reading
/// (`readings()[0]`). See the [module docs](self) for the metavariable
/// syntax, the reading model, and the enumeration order.
#[derive(Debug, Clone)]
pub struct Pattern {
    style: Style,
    text: String,
    readings: Vec<Reading>,
}

impl Pattern {
    /// Parse a pattern, enumerating every valid reading of the text in the
    /// documented [enumeration order](self#enumeration-order).
    ///
    /// Returns [`PatternError::Unparsable`] only when zero readings exist
    /// (see the [module docs](self#when-is-a-pattern-unparsable)).
    pub fn new(style: Style, text: &str) -> Result<Pattern, PatternError> {
        let (substituted, occurrences) = substitute_metavars(text);

        // Parse as a document once; this feeds the entry-tier guard, the
        // section reading, and the document reading.
        let doc_parsed = parse_for(style, &substituted);
        let content: Vec<usize> = content_child_indices(doc_parsed.root());
        if content.is_empty() {
            return Err(PatternError::Unparsable {
                message: "empty pattern: no content to match".to_owned(),
            });
        }

        let mut readings: Vec<Reading> = Vec::new();

        if matches!(style, Style::Google | Style::NumPy) {
            // (a) Entry-tier readings, one trial per role in enumeration
            // order — single contiguous block only (see the module docs).
            // Identically-shaped candidates merge into one reading listing
            // all applicable roles.
            if content.len() == 1 {
                let mut entries: Vec<(Shape, Reading)> = Vec::new();
                for kind in ENTRY_READING_ORDER {
                    if let Ok(analysis) = analyze_in_section(style, kind, &substituted, &occurrences) {
                        let shape = shape_of(&analysis);
                        if let Some((_, reading)) = entries.iter_mut().find(|(existing, _)| *existing == shape) {
                            reading.section_kinds.push(kind.clone());
                        } else {
                            entries.push((shape, analysis.into_reading(vec![kind.clone()])));
                        }
                    }
                }
                readings.extend(entries.into_iter().map(|(_, reading)| reading));
            }

            // (b) The free-text body reading. The free-text body grammar is
            // one shared code path for every free-text kind (only the
            // header name differs), so a single trial covers them all.
            if let Ok(analysis) = analyze_in_section(
                style,
                &SectionKind::FreeText(FreeSectionKind::Notes),
                &substituted,
                &occurrences,
            ) {
                readings.push(analysis.into_reading(free_text_kinds()));
            }
        }

        // (c) The section reading: exactly one SECTION and nothing else.
        if let [index] = content[..]
            && doc_parsed.root().children()[index].kind() == SyntaxKind::SECTION
            && check_coverage(&doc_parsed).is_ok()
        {
            let fragment_path = vec![index];
            if let Ok(metavars) = locate_metavars(&doc_parsed, &occurrences, &fragment_path) {
                readings.push(Reading {
                    parsed: doc_parsed.clone(),
                    fragment_kind: FragmentKind::Section,
                    fragment_path,
                    section_kinds: Vec::new(),
                    metavars,
                });
            }
        }

        // (d) The document reading, always last.
        if check_coverage(&doc_parsed).is_ok()
            && let Ok(metavars) = locate_metavars(&doc_parsed, &occurrences, &[])
        {
            readings.push(Reading {
                parsed: doc_parsed,
                fragment_kind: FragmentKind::Document,
                fragment_path: Vec::new(),
                section_kinds: Vec::new(),
                metavars,
            });
        }

        if readings.is_empty() {
            return Err(PatternError::Unparsable {
                message: "no valid reading: every metavariable must land on a bindable site (a whole token/node, \
                          or inside prose) in at least one grammar that accepts the text"
                    .to_owned(),
            });
        }

        Ok(Pattern {
            style,
            text: text.to_owned(),
            readings,
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

    /// Every valid reading of the text, in the documented
    /// [enumeration order](self#enumeration-order). Never empty.
    pub fn readings(&self) -> &[Reading] {
        &self.readings
    }

    /// The first reading applicable under the given section kind
    /// (convenience lookup over [`Reading::section_kinds`]). `Section` and
    /// `Document` readings are never returned — they carry no section
    /// kinds.
    pub fn reading_for(&self, kind: &SectionKind) -> Option<&Reading> {
        self.readings.iter().find(|r| r.section_kinds.contains(kind))
    }

    /// The primary reading's fragment kind (`readings()[0]`).
    pub fn fragment_kind(&self) -> FragmentKind {
        self.readings[0].fragment_kind()
    }

    /// The primary reading's fragment root node (`readings()[0]`).
    pub fn fragment(&self) -> &SyntaxNode {
        self.readings[0].fragment()
    }

    /// The primary reading's metavariable table (`readings()[0]`).
    pub fn metavars(&self) -> &[MetaVar] {
        self.readings[0].metavars()
    }

    /// The primary reading's wrapped parse (`readings()[0]`).
    pub fn parsed(&self) -> &Parsed {
        self.readings[0].parsed()
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

/// One lexed piece of pattern (or template) text: a run of literal
/// characters, or a metavariable reference.
///
/// This is the shared `$X` / `$$$X` scanner used both when parsing a pattern
/// (below) and when rendering a rewrite template (#47,
/// [`crate::rewrite`]) — the template is *not* a fragment to parse, but its
/// metavariable holes are recognised by the exact same rules, so the two
/// layers share one scanner rather than reimplementing the lexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetaVarToken<'a> {
    /// A run of literal (non-metavariable) text.
    Literal(&'a str),
    /// A `$NAME` (`multi == false`) or `$$$NAME` (`multi == true`) reference;
    /// `name` excludes the sigil.
    Var { name: &'a str, multi: bool },
}

/// Lex `text` into literal runs and metavariable references using the
/// documented [metavariable syntax](self#metavariable-syntax): a `$` starts a
/// metavariable only at a word boundary and only when followed by an
/// uppercase identifier; everything else is literal. Adjacent literal
/// characters are coalesced into a single [`MetaVarToken::Literal`] run.
pub(crate) fn lex_metavars(text: &str) -> Vec<MetaVarToken<'_>> {
    let bytes = text.as_bytes();
    let mut tokens: Vec<MetaVarToken<'_>> = Vec::new();
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
                if run_start < i {
                    tokens.push(MetaVarToken::Literal(&text[run_start..i]));
                }
                tokens.push(MetaVarToken::Var {
                    name: &text[ident_start..end],
                    multi,
                });
                run_start = end;
                i = end;
                continue;
            }
        }
        i += 1;
    }
    if run_start < text.len() {
        tokens.push(MetaVarToken::Literal(&text[run_start..]));
    }
    tokens
}

/// Replace every metavariable occurrence with a unique placeholder name and
/// inventory the occurrences, in source order. Built on the shared
/// [`lex_metavars`] scanner.
fn substitute_metavars(text: &str) -> (String, Vec<Occurrence>) {
    let stem = placeholder_stem(text);
    let mut out = String::with_capacity(text.len());
    let mut occurrences: Vec<Occurrence> = Vec::new();
    for token in lex_metavars(text) {
        match token {
            MetaVarToken::Literal(literal) => out.push_str(literal),
            MetaVarToken::Var { name, multi } => {
                let placeholder = format!("{stem}{}X", occurrences.len());
                out.push_str(&placeholder);
                occurrences.push(Occurrence {
                    name: name.to_owned(),
                    multi,
                    placeholder,
                });
            }
        }
    }
    (out, occurrences)
}

// =============================================================================
// Sub-grammar wrapping
// =============================================================================

/// The entry-tier trial roles, in **enumeration order** (see the
/// [module docs](self#enumeration-order)); changing this order is a
/// breaking change, spec-pinned in `tests/pattern.rs`. `References` sits
/// last because its grammar accepts nearly any line as a plain-text
/// citation.
const ENTRY_READING_ORDER: &[SectionKind] = &[
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
    SectionKind::References,
];

/// The known free-text section kinds a `Body` reading applies under (the
/// grammar also hosts unknown-named sections; matching under those is the
/// matcher's anchor-side concern, #46).
fn free_text_kinds() -> Vec<SectionKind> {
    [
        FreeSectionKind::Notes,
        FreeSectionKind::Examples,
        FreeSectionKind::Warnings,
        FreeSectionKind::Todo,
        FreeSectionKind::Attention,
        FreeSectionKind::Caution,
        FreeSectionKind::Danger,
        FreeSectionKind::Error,
        FreeSectionKind::Hint,
        FreeSectionKind::Important,
        FreeSectionKind::Tip,
    ]
    .into_iter()
    .map(SectionKind::FreeText)
    .collect()
}

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
        range: root.range(),
        is_token: false,
    }];
    let mut path = Vec::new();
    let mut cur = root;
    'descend: loop {
        for (i, child) in cur.children().iter().enumerate() {
            let r = child.range();
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
}

impl InSectionAnalysis {
    fn into_reading(self, section_kinds: Vec<SectionKind>) -> Reading {
        Reading {
            parsed: self.parsed,
            fragment_kind: self.fragment_kind,
            fragment_path: self.fragment_path,
            section_kinds,
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
