# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Closing the Rust↔Python capability gaps that surfaced while porting a real
consumer — scverse-misc's Sphinx extension, which injects `.. version-deprecated::`
directives into individual argument descriptions — onto the 0.4 edit API
([#115](https://github.com/ryumasai/pydocstring/issues/115)).

Everything here is additive except one **behavior change**: `LineColumn.col` is
now a byte column in Python, as it always was in Rust. It only differs on a line
containing a multi-byte character — see **Fixed**.

### Added

- **CI now checks Rust↔Python API parity**
  ([#134](https://github.com/ryumasai/pydocstring/issues/134)). #115 was filed
  because three Rust capabilities had no Python counterpart. Closing those three
  did not close *how they got there* — the bindings are hand-written, so every
  new Rust item is opt-in for Python and nothing noticed when it wasn't taken
  up. Two more (#132, #133) surfaced the moment a real consumer tried to use the
  0.4 API, in a surface that had just been audited by hand.

  `just api-parity` enumerates the Rust public surface from rustdoc's JSON —
  reachability from the crate root, not `pub`, so a `pub fn` inside a
  `pub(crate)` module correctly isn't API — and fails on any item that is
  neither exposed in Python nor listed in `scripts/api_parity_allow.toml` **with
  a reason**. The excuse list is as much the point as the check: "not in Python"
  should be a decision someone wrote down. It also fails on a *stale* excuse, so
  the list cannot rot.

  It covers functions, types, methods, **enum variants** and trait methods.
  Variants matter for the same reason the rest does: a `SyntaxKind` variant
  added in Rust and forgotten in Python fails nothing today — the binding maps
  an unrecognised kind to `UNKNOWN` — so nothing would ever say so.

  311 Rust items; 94 excused, in writing. Running it for the first time found
  four real gaps, below.

- **`SyntaxKind.is_node()` / `.is_token()` / `.is_trivia()` / `.name` (Python)**
  — found by the parity check. Skipping trivia while walking the CST meant
  hard-coding which kinds are trivia and re-deriving that whenever the grammar
  grew one.

- **`TextRange.source_text(source)`, `len(range)`, `offset in range` (Python)**
  — also found by the check. `source_text` is the one that matters: a range is a
  **byte** range and a `str` indexes by code point, so `source[r.start:r.end]`
  cuts in the wrong place on any non-ASCII input. That is the exact bug
  CodeRabbit caught in the 0.4.0 README, and now the API has the correct thing
  to reach for.

- **`RewriteError` (Python)** — also found by the check. A failed rewrite raised
  a bare `ValueError`, while every other error in the API has a named type.

- **`WalkContext.line_indent(offset)` (Python)** — a visitor is where you most
  often need the indent an edit has to match, and it was the one place you could
  not ask for it without closing over the `Parsed`.

- **`Parsed.line_indent(offset)` (Rust and Python)** — the leading whitespace of
  the line an offset falls on, as the literal characters to copy.

  This is what an edit actually needs, and a column cannot express it.
  `" " * line_col(...).col` is the obvious thing to reach for and it is wrong
  twice over: it turns a **tab** into a space, and it over-indents a line whose
  text before the anchor is not ASCII (a byte column counts `é` twice). Neither
  shows up in an ASCII, space-indented fixture — which is every fixture anyone
  writes.

- **`Parsed.line_col(offset)` (Python)**
  ([#132](https://github.com/ryumasai/pydocstring/issues/132)). `line_col` used
  to exist only on `WalkContext` — i.e. only *inside* a `walk()` hook, which is
  not where editing happens. An edit that inserts a multi-line block has to be
  indented, and to indent it you need the column of the anchor, so every caller
  was reduced to byte arithmetic over `parsed.source`.

- **`TextRange(start, end)` is constructible from Python**
  ([#133](https://github.com/ryumasai/pydocstring/issues/133)). `replace()` and
  `delete()` take a `TextRange`, and Python could not make one — so an edit was
  limited to spans some view happened to hand you. Rust never had that
  restriction (`TextRange::new` is public), and the API was inconsistent with
  itself: `insert(at, text)` takes a raw `int` offset, so it already trusted the
  caller with arbitrary byte positions.

  This makes spans like "from the end of the `:` token to the end of the
  description" expressible — the CST gives you both endpoints, but no single
  view covers the span between them.

### Fixed

- **BREAKING (Python, non-ASCII only): `LineColumn.col` is a byte column.** It
  was a *character* count, while the Rust `LineIndex` returned a byte offset —
  the same method name giving different numbers in the two languages, silently,
  and only on lines containing a multi-byte character. Every other offset in the
  API is a byte offset, so a character column did not compose with any of them.
  It now matches both Rust and `ast.col_offset`, which is the convention a Python
  caller would expect (pinned by a test against `ast` itself).

- **`TextRange::source_text` could panic on a hand-built range.** It bounds-checked
  but did not check character boundaries, and `&source[start..end]` panics when an
  endpoint falls inside a multi-byte character — an abort, across the FFI boundary.
  Unreachable while ranges could only come from the parser; reachable the moment
  this release made them constructible. It now returns `""`, like it already did
  for an out-of-bounds range.

- **An empty `TextRange` was falsy.** Adding `len(range)` without `__bool__` made
  `bool(r)` false for a zero-length range — which is not "nothing", it is the
  placeholder marking where a missing element goes, i.e. the anchor you insert
  at. A caller guarding with `if r:` would have silently skipped exactly the
  ranges the edit API exists to serve. (Caught in review, before the tag.)

- **`offset in range` raised instead of answering.** `-1 in r` gave
  `OverflowError` and `"x" in r` a `TypeError`; a membership test answers
  `False` for something that could not be a member.

- **`SyntaxKind.UNKNOWN.name` raised.** A property that raises is a trap in a log
  line or a traceback. It now names itself. The three *predicates* still raise
  for `UNKNOWN` — a kind this build does not know has no structure to report —
  and the parity check above is what guarantees `UNKNOWN` cannot come out of a
  parsed tree.

- **`Parsed.line_col()` rebuilt the line index on every call**, an O(n) scan each
  time — in the very loop it was added for. `Parsed` already caches one.

- **`EditError`'s message no longer says "out of bounds" for a range that is in
  bounds.** The one error variant covers three cases — past the end, inverted,
  or an endpoint inside a multi-byte character — and reported all three as "out
  of bounds". Now that ranges are constructible, callers actually reach the
  latter two.

## [0.4.0] - 2026-07-13

**The style-independent release** — phase 4 of the v2 roadmap
([#48](https://github.com/ryumasai/pydocstring/issues/48)), triggered by an
external report that style-independent rewrites were impossible from Python
([#115](https://github.com/ryumasai/pydocstring/issues/115)). They were, and
the reason was that the whole surface was organized by *style*: 27 per-style
wrapper classes, a 55-hook per-style visitor, and a model whose section type
baked the role into its shape.

All of it collapses into one code path. `parse()`, `parse_google()`,
`parse_numpy()` and `parse_plain()` return the same `Parsed`, read through
three lenses — **semantic** (`Document` → `Section` → `Entry`), **faithful**
(the raw CST), **normalized** (`to_model()`) — and edited through anchored
byte-range splices that preserve every byte they do not touch. A section's
role is *data* (`kind`), not a type to dispatch on. Net ≈ −3,400 lines.

This is a breaking release, deliberately and all at once: see the migration
notes under **Changed** and **Removed**.

### Added

- **The unified view is now available from Python** — `Document` → `Section` →
  `Entry`, the style-independent read lens that Rust has had all along
  ([#116](https://github.com/ryumasai/pydocstring/issues/116),
  reported as [#115](https://github.com/ryumasai/pydocstring/issues/115)).
  One code path reads every style: `Args:` and `Parameters` both resolve to
  `SectionKind.PARAMETERS`, so a section's role is *data*, not a type to
  dispatch on.

  ```python
  doc = pydocstring.Document(pydocstring.parse(src))
  for section in doc.sections:
      if section.kind == pydocstring.SectionKind.PARAMETERS:
          for entry in section.entries:
              print(entry.name.text)
  ```

  Every view keeps its byte range, so results double as edit anchors. Entry
  accessors are all optional, so reading an entry never raises for a role that
  does not carry that piece — a `Raises:` entry has `name is None` and its
  exception type in `type_annotation`.

  Also exposed: `DefaultMarker`, `Directive`, `Citation`.

- **The edit API is now available from Python** — `Edits`, reached with
  `parsed.edit()` or `doc.edit()`
  ([#117](https://github.com/ryumasai/pydocstring/issues/117)). Anchored splice
  edits: `replace(range, text)`, `insert(at, text)`, `delete(range)`,
  `remove_lines(range)`, `apply()`, `apply_reparsed()`. Anchor them on the
  `range` of any view, so scoping a rewrite to one section is an ordinary `if`
  in the traversal loop.

  Everything an edit does not touch is preserved byte-for-byte — this is a
  splice, not a re-render, so a NumPy docstring keeps its indentation and a
  Google one keeps its `x (int): ` prefix. Both kernel laws hold from Python and
  are tested there: an empty edit list reproduces the source exactly, and
  replacing an element with its own text is the identity. `apply()` raises
  `EditError` (a `ValueError`) for an out-of-bounds or overlapping edit.
  `apply_reparsed()` re-parses with the **same** style — editing must not
  silently reinterpret a docstring as another style.

- **The raw CST is now available from Python** — `Node` and `SyntaxKind`, reached
  with `.syntax` from a parse result or any node-backed view (`Document`,
  `Section`, `Entry`, `DefaultMarker`, `Directive`, `Citation`)
  ([#126](https://github.com/ryumasai/pydocstring/issues/126)). `Token` grows a
  `kind`.

  Python previously had no generic node type at all: the only way into the tree
  was `walk()`, which hands back the 27 per-style wrapper classes. But the tree's
  vocabulary is already style-independent — a Google entry and a NumPy entry are
  both `SyntaxKind.ENTRY` — so two classes replace twenty-seven, and the result
  is *more* capable.

  This is the **faithful** lens: it keeps punctuation, trivia, and the zero-length
  missing placeholders the unified view deliberately hides. It is what
  distinguishes `x ():` (an empty type between brackets — a placeholder exists,
  and its range is an insertion anchor) from `x:` (no type token at all), which
  the semantic lens reports identically as `type_annotation is None`.

  The read lenses are now what they should have been: **semantic** (unified view)
  / **faithful** (raw CST) / **normalized** (model).

- **Scoped pattern rewrites from Python** — `replace_in(anchor, pattern, template)`
  and `findall_in(anchor, pattern)`
  ([#118](https://github.com/ryumasai/pydocstring/issues/118)). Rust has had
  `replace_in` since 0.3.1; Python only got the document-wide `replace`, so
  there was no way to say "rewrite this, but only inside the Parameters
  section". Any `Document`, `Section`, or `Entry` of the same parse result is a
  valid anchor, and the anchor's grammar selects the reading — the same pattern
  rewrites `$TYPE`-shaped entries under a `Raises:` anchor and `$NAME`-shaped
  ones under an `Args:` anchor.

- **`Parsed::to_model()` (Rust)** — the normalized lens, dispatched on the
  parsed style. Rust previously had no style-independent way to reach the
  model: you had to call `parse::google::to_model::to_model` and match on the
  style yourself. The dispatch existed only inside the Python binding, so the
  "one code path" promise held in Python and not in Rust. It is infallible —
  the per-style converters returned `None` only on a style mismatch, which the
  dispatch rules out.

- **`parse::{parse_google, parse_numpy, parse_plain}` (Rust)** are re-exported
  next to `parse` and `detect_style`, so nothing reaches into a per-style
  module any more.

- `Edits::remove_lines_range` (Rust) — the range-anchored form of
  `remove_lines`, which only ever read its node's range. Needed by the Python
  binding, whose handle on a construct is a range; pinned equal to
  `remove_lines` for every node of the corpus.

- **Value semantics on the Python view types.** `TextRange`, `Node` and
  `LineColumn` now compare and hash by value, as `Token` already did. Views are
  handed out as a fresh wrapper on every access, so `entry.range ==
  entry.range` was `False` and `{r, r}` held two elements — on the type that
  anchors every edit.

### Fixed

- **`SyntaxNode::tokens` returned missing placeholders** (Rust; surfaced through
  the new Python `Node.tokens`). `find_token` has always excluded zero-length
  placeholders, but its plural form did not — so `tokens(SyntaxKind::TYPE)` on
  `x ():` yielded the placeholder while `find_token` returned `None` for the same
  node. The two are the singular and plural form of one question ("which tokens
  of this kind are *present*?"), and now agree; `find_missing` remains the only
  accessor that returns a placeholder. Caught by CodeRabbit on
  [#127](https://github.com/ryumasai/pydocstring/pull/127).

- **`emit_sphinx` dropped blocks.** It filtered a section's blocks by the
  section's `kind`, so prose paragraphs inside a structured section were
  silently discarded. Every block is now emitted, in source order.

- **Two examples on the crates.io front page did not compile** — and had not
  since 0.3.0. `README.md` was prose that nothing checked; its model examples
  still used the pre-0.4 `Section::Parameters(…)` enum. The README's Rust
  examples are now compiled as doctests (`#[cfg(doctest)] #[doc =
  include_str!("../README.md")]`), so this class of rot fails CI instead of
  shipping. `just doc` (with `RUSTDOCFLAGS=-D warnings`) joins CI too, which
  also cleared four broken intra-doc links in `matcher` / `rewrite`.

- **Python packaging:** `include = ["LICENSE"]` was installing a bare `LICENSE`
  file into the site-packages *root*; the license already ships correctly under
  `dist-info/licenses/`. The project now declares the PEP 639
  `license = "MIT"` expression. `Style` was the one `#[pyclass]` without a
  `module`, so it reported `__module__ == "builtins"`.

### Removed

- **BREAKING: the per-style CST wrappers have left the public API**
  ([#119](https://github.com/ryumasai/pydocstring/issues/119)). `GoogleArg`,
  `GoogleSection`, `NumPyParameter`, `PlainDocstring`, `GoogleSectionKind`,
  `NumPySectionKind` and the twenty-odd others were nominal sugar over a tree
  that has no per-style structure. They derived the entry role a second time,
  carried a panic-on-miscast path, and forced every new construct to be plumbed
  through eight surfaces.

  - **Python:** all 27 wrapper classes are gone, along with the two
    `GoogleSectionKind` / `NumPySectionKind` enums — 29 names out of `__all__`.
    `parse()`, `parse_google()`,
    `parse_numpy()` and `parse_plain()` all return a single **`Parsed`**
    (`style`, `source`, `syntax`, `pretty_print()`, `to_model()`, `edit()`,
    `replace()`, `replace_in()`, `findall()`, `findall_in()`) — no more
    `isinstance` dance over three types.
  - **Rust:** the `parse::{google, numpy, plain}` modules are **`pub(crate)` in
    full** — nodes, `kind`, `parser` and `to_model` alike. Reach a parser as
    `parse::parse_google` and the model as `parsed.to_model()` (both new, see
    **Added**); nothing else in them was ever meant to be public. This also
    removes four redundant paths to `TextBlock`. `parse::visitor` (the typed
    `DocstringVisitor`) is **deleted**: its only consumer was the Python
    `walk()`, which is now generic. The generic `syntax::walk_tree` /
    `syntax::Visitor` were already there.

  Read a docstring through the unified view (`Document` → `Section` → `Entry`),
  the raw CST (`.syntax`), or the model (`to_model()`).

- **BREAKING: `walk()` / `Visitor` are generic over the CST.** The per-style
  hooks (`enter_google_arg`, `enter_numpy_parameter`, … — 55 of them) are
  replaced by three: `enter_node(node, ctx)`, `leave_node(node, ctx)`,
  `visit_token(token, ctx)`. Dispatch on `node.kind` / `token.kind`. `walk()`
  now also accepts a `Node`, so a subtree can be walked.

  ```python
  class NameCollector(pydocstring.Visitor):
      def __init__(self):
          self.names = []

      def visit_token(self, token, ctx):
          if token.kind == pydocstring.SyntaxKind.NAME:
              self.names.append(token.text)

  # `walk()` returns the visitor, so results can be read straight off the call.
  names = pydocstring.walk(pydocstring.parse(src), NameCollector()).names
  ```

  A deprecation cycle was considered and rejected: a deprecated-but-public
  wrapper still has to be *maintained*, so the FIELD work in 0.5.0 would have
  had to be plumbed through all 27 classes anyway, and the debt would have
  survived the entire phase it hurts most. Breaking the public API once,
  properly, beats breaking it twice.

- **BREAKING (Rust): the accessor aliases deprecated in 0.3.0 are gone**
  ([#109](https://github.com/ryumasai/pydocstring/issues/109)), as the 0.3.0
  changelog announced. The only one still reachable from outside the crate was
  **`syntax::walk`** — use `syntax::walk_tree`, its name since 0.3.0. The
  other nineteen (`r#type`, `return_type`, `warning_type`, `optional`,
  `number`, `stray_lines` on the per-style node wrappers) left the public API
  with the wrappers themselves, above; they are now deleted outright. Python
  never carried any of these aliases.

### Changed

- **BREAKING: a model `Section` is now a kind plus a flat sequence of blocks**
  ([#105](https://github.com/ryumasai/pydocstring/issues/105),
  [#106](https://github.com/ryumasai/pydocstring/issues/106)). The old
  `enum Section { Parameters(Vec<Parameter>), Returns(Vec<Return>), FreeText { … }, … }`
  keyed the body's *shape* off the section's role, so a `Returns` section could
  not hold a prose paragraph and a `Parameters` one could not hold anything but
  parameters — which real docstrings do all the time. It is now:

  ```rust
  pub struct Section { pub kind: SectionKind, pub blocks: Vec<Block> }

  #[non_exhaustive]
  pub enum Block { Paragraph(String), Parameter(Parameter), Return(Return), … }
  ```

  This is the same unification as the CST and the unified view: the role is
  `kind` (data), and the body is one sequence in source order. It is what lets
  0.5.0 add reST fields and literal blocks as new `Block` variants without
  touching `Section`.

  - **Rust:** `match` on `section.kind`, then read `section.blocks` (each
    `Block` has an `as_parameter()` / `as_return()` / … accessor). Build one
    with `Section::new(kind, blocks)` or a role-named constructor
    (`Section::parameters(…)`, `Section::returns(…)`, …). `Section` is
    `#[non_exhaustive]`, as 0.3.0 promised the enum would be — struct-literal
    construction is no longer possible, so a future field is not a break. The
    rest of the model IR stays literal-constructible on purpose: those are
    values you build to feed `emit`.
  - **Python:** `Section(kind=…, parameters=[…])` and the `.parameters` /
    `.returns` / `.body` getters are gone; construct `Section(kind=…,
    blocks=[…])` and read `section.blocks`. (This supersedes the migration
    advice given for 0.3.0.)

  ```python
  # before
  for p in section.parameters or []:
      print(p.names)
  # after — `Block` is a variant type: match it, then read `.value`
  for block in section.blocks:
      if isinstance(block, model.Block.Parameter):
          print(block.value.names)
  ```

- **BREAKING: bare prose lines in a structured NumPy section read as a
  paragraph** ([#104](https://github.com/ryumasai/pydocstring/issues/104)). A
  run of base-indent lines carrying no entry — an intro sentence above a
  `Returns` list, say — used to convert to one type-only entry *per line*.
  `to_model()` now reads them as a single `Block::Paragraph`, which is the
  minimal non-destructive reading (and the docutils one) where napoleon would
  reflow or eat text. Only the model changes; the CST always kept the lines
  verbatim.

- **BREAKING (Rust): `range()` returns `TextRange` by value.** `TextRange` is
  `Copy`, but the `parse` and `syntax` layers returned `&TextRange` while
  `pattern` and `matcher` returned it by value — the crate was paying for its
  own inconsistency with `*x.range()` derefs at every call site. Drop the `*`.

- **BREAKING (Python): kind enums no longer compare equal to bare integers.**
  `SectionKind.PARAMETERS == 0` was `True` and `Style.GOOGLE == 0` was `False`;
  the three enums agree now, and none of them compares to an `int`.

- **Emit: adjacent prose paragraphs are separated by a blank line**, so the
  paragraph split survives a re-parse.

- **BREAKING (Python): the model IR moved to `pydocstring.model`.** The Rust
  crate has always kept `model::Section` and `parse::unified::Section` apart by
  module; the Python bindings flattened both layers into one namespace, which is
  why they collided the moment the unified view was exposed. `Docstring`,
  `Section`, `Block`, `Parameter`, `Return`, `ExceptionEntry`, `SeeAlsoEntry`,
  `Reference`, `Attribute`, `Method`, and `Directive` now live under
  `pydocstring.model`; the top level carries the CST / unified / edit surface.
  `SectionKind` is shared vocabulary and stays at the top level (and is
  re-exported from `pydocstring.model`).

  ```python
  # before
  from pydocstring import Docstring, Parameter
  # after
  from pydocstring.model import Docstring, Parameter
  ```

## [0.3.1] - 2026-07-10

**The edit API release** — phase 3 of the v2 roadmap
([#48](https://github.com/ryumasai/pydocstring/issues/48)). This adds a
pattern-based match/rewrite layer on top of the lossless CST: you target
exactly the nodes you want and every other byte is preserved verbatim, which
is the round-trip fidelity the normalizing `to_model()` path could not offer
([#26](https://github.com/ryumasai/pydocstring/issues/26)). The release is
purely additive — no existing API changed.

### Added

- **Pattern match/rewrite over the CST.** Patterns are docstring fragments
  with metavariables — `$X` (a single node) and `$$$X` (a run of siblings);
  rewrite templates re-render at the match site's base indent, and captured
  variables substitute the **original source bytes** so anything you don't
  rewrite is preserved by construction. Matching is whitespace-insensitive
  and indentation-relative ([#45](https://github.com/ryumasai/pydocstring/issues/45),
  [#46](https://github.com/ryumasai/pydocstring/issues/46),
  [#47](https://github.com/ryumasai/pydocstring/issues/47)).
  - Rust: `Pattern`, `Rewriter::replace` / `Rewriter::replace_in`, and
    ambiguity resolution via `PatternOptions` (including
    `Pattern::in_section`).
  - Python: `doc.replace(pattern, template)` and `doc.findall(pattern)`.
- **Low-level anchored edit primitive.** An anchored splice edit list
  (`Editor` with `replace` / `replace_node` / `replace_token`) underneath the
  pattern layer, with byte-identity property tests
  ([#44](https://github.com/ryumasai/pydocstring/issues/44)).
- **Generalized rST directives.** The parser now recognizes any
  reStructuredText directive as a `DIRECTIVE` node rather than only
  `deprecated` ([#84](https://github.com/ryumasai/pydocstring/issues/84)).

### Changed

- Differential parity tests against `sphinx.ext.napoleon` for Google and
  NumPy docstrings, with the known divergences documented.

## [0.3.0] - 2026-07-09

**The style-independent CST release** — phase 2 of the v2 roadmap
([#41](https://github.com/ryumasai/pydocstring/issues/41)). The syntax tree
no longer encodes the docstring style in its node kinds: 55 style-prefixed
kinds collapse into 31 reST-neutral ones, style differences are confined to
the `SECTION_HEADER` shape and the parsers, and a new unified typed layer
(`Document` → `Section` → `Entry`) lets one code path handle Google and
NumPy docstrings identically — spec-tested by cross-style parity laws.
Together with formalized missing placeholders (the future insertion anchors)
and now-private tree mutators, this fixes the tree shape and API surface the
upcoming edit API will build on. The release is validated against a new
real-world corpus: **every within-style law allowlist (byte coverage,
idempotence, model stability) is empty across all 247 corpus inputs,
including 85 docstrings taken verbatim from numpy, scipy, scanpy, anndata,
absl and fire**. **This release is deliberately breaking**; the migration
guides below cover every rename.

### Migration guide — Rust

#### Syntax kinds (`SyntaxKind`)

The style is no longer in the kind — recover it with the new
`Parsed::style()`. Old variants are **removed** (compile errors, not
deprecations):

| 0.2.0 kind(s) | 0.3.0 kind | Notes |
|---|---|---|
| `GOOGLE_DOCSTRING`, `NUMPY_DOCSTRING`, `PLAIN_DOCSTRING` | `DOCUMENT` | match on `parsed.style()` instead |
| `GOOGLE_SECTION`, `NUMPY_SECTION` | `SECTION` | |
| `GOOGLE_SECTION_HEADER`, `NUMPY_SECTION_HEADER` | `SECTION_HEADER` | style lives in its shape: `COLON` vs `UNDERLINE` |
| `GOOGLE_ARG`, `NUMPY_PARAMETER`, `*_RETURNS`, `*_YIELDS`, `*_EXCEPTION`, `*_WARNING`, `*_SEE_ALSO_ITEM`, `*_ATTRIBUTE`, `*_METHOD` (16 kinds) | `ENTRY` | the entry's role derives from the enclosing `SECTION`'s kind |
| `GOOGLE_DEPRECATION`, `NUMPY_DEPRECATION` | `DIRECTIVE` | generalized rST directive; `deprecated` is just the directive name |
| `GOOGLE_REFERENCE`, `NUMPY_REFERENCE` | `CITATION` | rST citation/footnote (`.. [label]`) |
| `BODY_TEXT`, `CONTENT` | `DESCRIPTION` | every construct's prose child is now `DESCRIPTION` |
| `STRAY_LINE` (token) | `PARAGRAPH` (node) | stray prose is now a text-block *node* wrapping `TEXT_LINE` tokens; blank lines split paragraphs |
| `WARNING_TYPE` | `TYPE` | was style-divergent (see Fixed) |
| `RETURN_TYPE` | `TYPE` | role comes from the section, as everywhere else |
| `VERSION` | `ARGUMENT` | the directive argument token |
| `NUMBER` | `LABEL` | citation labels aren't always numbers (`CIT2002`, `#f1`) |
| `KEYWORD` | `DIRECTIVE_NAME` | disambiguates from `DEFAULT_KEYWORD` |
| — | `DEFAULT` (new node) | wraps one `default …` marker occurrence (see Added) |

#### Typed-wrapper accessors

Renames with a `#[deprecated(since = "0.3.0")]` alias still compile with a
warning; **hard** changes do not.

| 0.2.0 | 0.3.0 | Migration |
|---|---|---|
| `r#type()` (`GoogleArg`, `GoogleException`, `GoogleAttribute`, NumPy counterparts) | `type_annotation()` | deprecated alias kept |
| `return_type()` (`GoogleReturn`/`Yield`, `NumPyReturn`/`Yield`) | `type_annotation()` | deprecated alias kept |
| `warning_type()` (`GoogleWarning`, `NumPyWarning`) | `type_annotation()` | deprecated alias kept |
| `optional()` | `optional_marker()`, plus `is_optional() -> bool` | deprecated alias kept; new `optionals()` iterates every occurrence |
| `number()` (`GoogleReference`, `NumPyReference`) | `label()` | deprecated alias kept |
| `stray_lines()` (yielded `&SyntaxToken`) | `paragraphs()` (yields `TextBlock`) | deprecated alias kept, but it now also yields `TextBlock` — a behavioral change even through the alias |
| `syntax::walk` (untyped tree walk) | `syntax::walk_tree` | deprecated alias kept; resolves the collision with the typed `parse::visitor::walk` |

#### Signatures (hard changes)

The typed views now hold `&Parsed`, so `source: &str` disappears from the
entire typed API — `arg.name()` returns a `TokenRef` and
`arg.name().text()` replaces `arg.name().text(source)`. Raw
`SyntaxToken::text(source)` is unchanged (per the
[#42](https://github.com/ryumasai/pydocstring/issues/42) convention:
synthesized trees never enter `Parsed`, so token text stays source-sliced).

| 0.2.0 | 0.3.0 |
|---|---|
| `GoogleDocstring::cast(node)` (and all typed wrappers) | `cast(&parsed, node)` — uniform two-argument cast |
| `section.section_kind(source)` | `section.section_kind()` |
| accessors returning `&SyntaxToken` | return `TokenRef<'a>` (has `.text()`, `.kind()`, `.range()`, `.is_missing()`) |
| `visitor::walk(source, node, visitor)` | `walk(parsed, node, visitor)` |
| `DocstringVisitor::visit_*(&mut self, source: &str, …)` | `visit_*(&mut self, parsed: &Parsed, …)` |
| `emit_google(doc, base_indent)` / `emit_numpy` / `emit_sphinx` | `emit_google(doc, &EmitOptions)`; use `EmitOptions::default().with_base_indent(n)` |
| `Parsed::new(source, root)` | `Parsed::new(source, root, style)` |

#### Model (`model::`)

| 0.2.0 | 0.3.0 | Migration |
|---|---|---|
| `Docstring.deprecation: Option<Deprecation>` | `Docstring.directives: Vec<Directive>` | `deprecation()` convenience method finds the first directive named `deprecated` |
| `Deprecation { version, description }` (struct removed) | `Directive { name, argument, description }` | `version` → `argument` of a `Directive` with `name == "deprecated"` |
| `Reference.number` | `Reference.label` | hard field rename |
| `Attribute.name: String` | `Attribute.names: Vec<String>` | hard field rename; NumPy allows multi-name attribute entries (`jac, hess`), and keeping only the first dropped the rest ([#89](https://github.com/ryumasai/pydocstring/issues/89)) |

#### Removed / newly non-exhaustive

- `SyntaxToken::extend_range` removed (was dead code).
- Tree mutators `SyntaxNode::children_mut` / `push_child` /
  `extend_range_to` and `TextRange::extend` are now crate-private — user
  code can no longer violate the CI-enforced coverage/ordering invariants
  (`TextRange::extend` could also silently *shrink* a range; fixed on the
  way in).
- `Style` and `model::Section` are now `#[non_exhaustive]`: downstream
  exhaustive `match`es need a wildcard arm. A future `Style::Sphinx` or new
  section variant will no longer be a breaking change.

### Migration guide — Python (`pydocstring-rs`)

| 0.2.0 | 0.3.0 | Migration |
|---|---|---|
| `GoogleWarning.warning_type` | `.type` | hard rename (Rust's three-way split unified; Python keeps `.type` per its own conventions) |
| `GoogleDocstring.stray_lines` (`list[Token]`) | `.paragraphs` (`list[TextBlock]`) | removed, no alias; `NumPyDocstring.paragraphs` added for parity |
| `GoogleReference.number` / `NumPyReference.number` | `.label` | hard rename, matching the Rust side — citation labels aren't always numbers |
| `Reference(number=…)` / `.number` (model) | `Reference(label=…)` / `.label` | hard rename |
| `Attribute(name=…)` / `.name` (model) | `Attribute(names=[…])` / `.names` | hard rename, matching `Parameter.names` ([#89](https://github.com/ryumasai/pydocstring/issues/89)) |
| `Deprecation` class | removed | build `Directive("deprecated", argument=version, description=…)`; `Docstring(deprecation=…)` → `Docstring(directives=[…])` |
| `Docstring.deprecation` (read/write field) | read-only computed property (`Directive \| None`) | edit `Docstring.directives` instead |
| `Style == 1` (int equality) | compares only to `Style` members | `Style` is now hashable (usable in sets / as dict keys) |
| `Section.Parameters(…)` etc. (pyo3 variant class-attrs) | removed at module init | these constructors bypassed `Section.__init__` validation; use `Section(kind=…, parameters=…)` |
| `Section.parameters` etc. typed `list[…]` in the stub | honestly typed `list[…] \| None` | runtime already returned `None` for other-kind sections; the stub now says so |

Other Python surface changes:

- **Missing-value conventions unified**: NumPy parameter/attribute/method
  `type`/`description` fields now return `is_missing()` placeholder objects
  exactly like their Google counterparts; the full per-class
  required / optional / or-missing-placeholder table is documented at the
  top of the type stub (`_pydocstring.pyi`), and Google↔NumPy parity is
  spec-tested.
- New: `NumPyParameter.name` (first of `names`, `None` if empty), matching
  `GoogleArg.name`; `GoogleAttribute.names` / `NumPyAttribute.names` (with
  `.name` kept as a `names[0]` convenience).
- Model property setters now validate like the constructors;
  `Section(kind=SectionKind.UNKNOWN)` without `unknown_name` is rejected at
  construction.
- `repr()` no longer leaks Rust pointer addresses, and multi-name entry
  reprs (`GoogleArg`, `NumPyParameter`, attributes, see-also items) now
  show every name, not just the first.

### Added

- **Unified typed layer** (`parse::unified`, re-exported from `parse`):
  `Document`, `Section`, `Entry`, `Directive`, `Citation`, `DefaultMarker`
  — zero-copy, style-independent views over the neutral kinds. One generic
  function can now extract from Google and NumPy docstrings identically;
  a table-driven cross-style parity law over every entry role
  (params/returns/yields/raises/warns/attributes) pins the guarantee
  (`tests/unified.rs`; mirrored in Python by the missingness parity suite).
- `Parsed::style()` — reports the detected/parsed style now that the root
  kind is always `DOCUMENT`.
- `TokenRef` — a `&Parsed`-holding token handle with source-free `text()`.
- `EmitOptions` (`Default` + `#[non_exhaustive]` + `with_base_indent`) —
  future emitter options become non-breaking field additions.
- **`DEFAULT` marker nodes with repeatable-marker semantics**: every
  `optional` / `default …` occurrence gets its own token/node in source
  order (`x : int, default 1, default 2` produces two `DEFAULT` nodes);
  which occurrence wins is a model-layer rule — **the first**, spec-pinned.
  New accessors `optionals()` / `defaults()` iterate all occurrences
  (fixes [#76](https://github.com/ryumasai/pydocstring/issues/76)).
- **`PARAGRAPH` nodes** ([#78](https://github.com/ryumasai/pydocstring/issues/78)):
  stray prose between sections becomes first-class `TextBlock` targets;
  newline-joined lines form one paragraph, blank lines split (reST
  semantics). Also on the unified `Document::paragraphs()`.
- **Missing-placeholder formalization**
  ([#78](https://github.com/ryumasai/pydocstring/issues/78)): zero-length ⇔
  missing ⇔ edit-API insertion anchor, documented in `src/syntax.rs`,
  rendered as `<missing>` by `pretty_print`, and pinned by an invariant test
  over the whole corpus (placeholder set: `TYPE`, `CLOSE_BRACKET`, `COLON`,
  `DEFAULT_VALUE` tokens; zero-length `DESCRIPTION` nodes; the empty-input
  `DOCUMENT`). Placeholders are only ever *replaced*, never extended.

### Changed

- **Python bindings are now lazy views** over the shared parse result
  ([#43](https://github.com/ryumasai/pydocstring/issues/43)): every CST
  class holds a path into the immutable tree and delegates each getter to
  the core Rust accessors on access, eliminating the eager-materialization
  bug class (the source of the released Yields/Warns mislabeling,
  [#50](https://github.com/ryumasai/pydocstring/pull/50)). The visible
  surface is frozen — with one nuance: **getters return a fresh object per
  access** (`doc.summary is doc.summary` is `False`); rely on `==` and
  hashing, which are preserved, never on `is`.
- **See-also emit normal form**: both emitters now always write a see-also
  description on the following indented line (`name\n    desc`) instead of
  collapsing to a `name : desc` one-liner — the one-liner is unparseable
  when the name is an rST role (`` :func:`csd` ``), per the
  [#26](https://github.com/ryumasai/pydocstring/issues/26) colon rule
  ([#91](https://github.com/ryumasai/pydocstring/issues/91)). Emitted
  output for see-also-bearing docstrings changes accordingly and
  round-trips.

### Fixed

- Repeated `optional` / `default` markers dropped bytes: the second
  `default` in `x : int, default 1, default 2` overwrote the first, whose
  bytes got no tokens — a live violation of the byte-coverage law
  ([#76](https://github.com/ryumasai/pydocstring/issues/76)). Structurally
  fixed by the repeatable `DEFAULT` nodes; the byte-coverage law now passes
  the whole corpus with an empty allowlist, repeated-marker inputs included.
- `WARNING_TYPE` was style-divergent: Google warns entries emitted it while
  NumPy warns emitted `TYPE`, so the unified `Entry::type_annotation()`
  returned `Some` for NumPy warns and `None` for Google warns — silently
  breaking the single-code-path guarantee. Both `WARNING_TYPE` and
  `RETURN_TYPE` collapse into `TYPE`
  ([#81](https://github.com/ryumasai/pydocstring/pull/81)).
- Marker-like segments in the middle of a type (`int, optional, str`)
  produced overlapping tokens (an `OPTIONAL` token inside the `TYPE`
  token's range). Markers now only count in the trailing suffix of the
  type, per numpydoc convention (markers are trailing annotations).
- NumPy multi-name Attributes entries (`jac, hess : callable`) dropped
  every name after the first — the extra names' bytes got no tokens and
  the names vanished from the model. NumPy attributes now share the
  parameter grammar (Google splits comma-separated attribute names too),
  the CST wrappers gain `names()`, and the model field became
  `Attribute.names` ([#89](https://github.com/ryumasai/pydocstring/issues/89)).
- NumPy see-also multi-line descriptions were emitted with continuation
  lines at entry indentation, which re-parsed as fake name-only see-also
  entries ([#90](https://github.com/ryumasai/pydocstring/issues/90)).
- The `.. deprecated::` directive body reached the model with its source
  continuation indent attached, so the indentation grew by four spaces on
  every emit/parse cycle; directive bodies are now dedented in the model
  and re-indented exactly once on emit, in both styles
  ([#92](https://github.com/ryumasai/pydocstring/issues/92)).
- Google description-only Returns entries were emitted with continuation
  lines at column 0, dedenting them out of the section — the re-parse
  silently kept only the first line
  ([#93](https://github.com/ryumasai/pydocstring/issues/93)).
- Typed section accessors are guarded by section role: with all entries
  unified to `ENTRY`, a mismatched accessor (`args()` on a `Raises:`
  section) would have wrapped foreign entries and panicked in
  `required_token`; accessors now return empty for sections outside their
  role, preserving the 0.2.0 kind-filtered behavior.
- `TextRange::extend` could silently shrink a range; fixed (and the method
  is no longer public).

### Deprecated

- The ~20 renamed accessors listed in the Rust migration tables
  (`r#type` / `return_type` / `warning_type` → `type_annotation`,
  `optional` → `optional_marker`, `number` → `label`,
  `stray_lines` → `paragraphs`, `syntax::walk` → `walk_tree`) remain as
  `#[deprecated(since = "0.3.0")]` aliases. **They are scheduled for
  removal in 0.4.0** — migrate now while the compiler still points at every
  call site.

### Internal

- Real-world corpus: 85 docstrings ingested verbatim from numpy, scipy,
  scanpy, anndata, absl and fire under `tests/corpus/third_party/` (with
  per-library license notices), bringing the corpus to 247 inputs. The
  five bug clusters it flushed out
  ([#89](https://github.com/ryumasai/pydocstring/issues/89)–[#93](https://github.com/ryumasai/pydocstring/issues/93))
  are fixed above; the within-style law allowlists (coverage, idempotence,
  model stability) are now empty, and every remaining cross-style
  conversion allowlist entry is re-verified and annotated against its
  documented mechanism.
- Coverage tooling for the test suite (profile-data reuse, lcov export).

## [0.2.0] - 2026-07-07

**The lossless-CST release** — phase 1 of the v2 roadmap
([#48](https://github.com/ryumasai/pydocstring/issues/48)). The syntax tree
now accounts for every byte of the input: three invariants hold for the whole
test corpus and are enforced in CI.

1. Concatenating all tokens in source order reproduces the input
   byte-for-byte — no gaps, no overlaps.
2. No token contains a newline, except the trivia kinds `NEWLINE` and
   `BLANK_LINE`.
3. Trivia never overlaps content and always sits inside its parent's range.

### Changed (breaking)

- The CST now contains **trivia tokens**: `WHITESPACE` (intra-line runs),
  `NEWLINE`, and `BLANK_LINE` (a whitespace-only line including its newline).
  `children()` and token iteration yield them; kind-filtered accessors are
  unaffected. Blank lines between sections live at docstring level, entry
  indentation inside its section (syntactic ownership).
- **Multi-line content is split per line**: `SUMMARY`, `EXTENDED_SUMMARY`,
  `DESCRIPTION`, `BODY_TEXT` and `CONTENT` are now *nodes* wrapping one
  `TEXT_LINE` token per content line (plus interior trivia). Typed accessors
  keep their names but return the new `TextBlock` wrapper: `text(source)`
  yields the same raw slice as before, `lines()` iterates per-line tokens,
  `logical_text(source)` returns the dedented join. Python bindings expose the
  same as a `TextBlock` class (`.text` unchanged in value, plus `.lines` /
  `.logical_text`).
- The root node's range now spans the entire input, including the trailing
  newline.
- NumPy google-style entries store children in source order (`COLON` no
  longer precedes `TYPE`), with missing-type placeholders anchored after the
  open bracket, matching the Google parser.
- `SyntaxKind`, `GoogleSectionKind`, `NumPySectionKind`, `SectionKind` and
  `FreeSectionKind` are now `#[non_exhaustive]`, as announced in 0.1.15 —
  future kind additions will no longer break exhaustive matches.
- New `SyntaxKind` variants: `WHITESPACE`, `NEWLINE`, `BLANK_LINE`,
  `TEXT_LINE`, `COMMA`.

### Fixed

- NumPy `Methods`: inline text after a colon (`reset() : Reset the state.`)
  was silently discarded; it is now the method's description
  ([#39](https://github.com/ryumasai/pydocstring/issues/39)).
- NumPy entries with a colon but the description on the next line no longer
  leak a leading newline into the model.
- Separator commas (between names, before `optional` / `default` markers) and
  the brackets of google-style entries inside NumPy sections are now real
  tokens; previously those bytes were unaccounted for.

### Added

- `TextBlock` (Rust and Python): `lines()`, raw `text()`, dedented
  `logical_text()`, `is_missing()`.
- `SyntaxKind::is_trivia()`.
- Test infrastructure: byte-coverage law (`tests/coverage.rs`), trivia
  invariants and lexing spec tests (`tests/trivia.rs`).

## [0.1.15] - 2026-07-07

Bug-fix release: everything flushed out by the new corpus/round-trip test
infrastructure ([#59](https://github.com/ryumasai/pydocstring/issues/59)).

### Fixed

- Python bindings: `to_model()` mislabeled Yields sections as Returns and
  Warns as Raises ([#50](https://github.com/ryumasai/pydocstring/pull/50)).
- NumPy parser: recognizes the `Keyword Parameters` / `Keyword Arguments`
  section headers (parsed as parameter entries) and the Google-style
  admonition headers (`Todo`, `Attention`, `Caution`, `Danger`, `Error`,
  `Hint`, `Important`, `Tip`) instead of degrading them to `Unknown`
  ([#52](https://github.com/ryumasai/pydocstring/issues/52),
  [#53](https://github.com/ryumasai/pydocstring/issues/53)).
- Google parser/emitter: the `.. deprecated::` directive now round-trips
  (`model.deprecation` was silently dropped); both emitters also write the
  directive before the extended summary, matching the parsers and numpydoc
  convention ([#54](https://github.com/ryumasai/pydocstring/issues/54)).
- Google parser: References sections parse into structured
  `Reference { number, content }` entries instead of free text
  ([#55](https://github.com/ryumasai/pydocstring/issues/55)).
- Google parser: comma-separated parameter names split into individual
  names, and a `default X` / `default=X` / `default: X` segment in the type
  parentheses round-trips as `Parameter::default_value`
  ([#56](https://github.com/ryumasai/pydocstring/issues/56),
  [#57](https://github.com/ryumasai/pydocstring/issues/57)).
- NumPy parser: a type whose name merely starts with `default`
  (e.g. `defaultdict`) was eaten as a default-value marker, leaving the type
  empty ([#64](https://github.com/ryumasai/pydocstring/issues/64)).
- Parsers no longer panic on a malformed reStructuredText reference marker
  (`.. [1` without a closing bracket on the same line)
  ([#67](https://github.com/ryumasai/pydocstring/issues/67)).

### Added

- Typed CST nodes and accessors: `GoogleDeprecation`, `GoogleReference` (with
  `GoogleSection::references()`), `GoogleArg::names()` /
  `default_keyword()` / `default_separator()` / `default_value()`; mirrored in
  the Python bindings.

### Changed

- **Technically breaking for exhaustive `match`es in Rust**: new variants on
  the public enums `NumPySectionKind` (`KeywordParameters` + eight admonition
  kinds) and `SyntaxKind` (`GOOGLE_DEPRECATION`, `GOOGLE_REFERENCE`). Accepted
  in this patch release given the crate's age; 0.2.0 will add
  `#[non_exhaustive]` to prevent this class of breakage going forward.
  Python users are unaffected.

### Internal

- Test suite rebuilt around a shared corpus: snapshot harness
  (`tests/corpus/` + `tests/snapshots.rs`), round-trip law tests
  (idempotence / model stability / cross-style conversion with a burn-down
  allowlist), and a Python parity suite that checks the bindings
  byte-for-byte against the Rust snapshots.

## [0.1.14] - 2026-07-06

### Fixed

- NumPy and Google parsers: colons belonging to reStructuredText role
  references (e.g. `:attr:`~module.ClassName.attr1``) and trailing colons in
  prose lines (e.g. `Description with attributes:`) were misinterpreted as
  term/classifier separators, causing the colons to be dropped on re-emit.
  A new reStructuredText-aware separator rule now treats a colon as a
  separator only when it follows whitespace (`name : type`) or is attached to
  a single top-level token (`name:type`), leaving role references and prose
  intact ([#26](https://github.com/ryumasai/pydocstring/issues/26)).

### Added

- Sphinx-style (reStructuredText) emit: `emit::sphinx::emit_sphinx` renders a
  style-independent `Docstring` model as a Sphinx field list (`:param:`,
  `:type:`, `:raises:`, `:return:`, `:rtype:`, …), enabling conversion from
  Google / NumPy to Sphinx. Exposed in the Python bindings as `emit_sphinx`.
  Sphinx support is emit-only; `detect_style` still reports Sphinx docstrings as
  `Style::Plain`.

## [0.1.13] - 2026-05-11

### Added

- Python bindings: added Python 3.14 support by updating PyO3 to 0.28.

### Changed

- Python bindings: switched wheel builds to the stable `abi3-py310` ABI so one
  wheel per platform can support Python 3.10 and newer.

## [0.1.12] - 2026-05-09

### Fixed

- Improved indentation handling for Google and NumPy docstrings when converting
  parsed docstrings to the style-independent model and when emitting docstrings
  back to text. Multi-line descriptions, free-text sections, and round-trip
  parse/edit/emit workflows now preserve real-world indentation more reliably.

## [0.1.11] - 2026-04-15

### Changed

- Updated repository URLs to reflect new GitHub username (`ryumasai`).

## [0.1.10] - 2026-04-09

### Fixed

- Google parser: section entries at the same indentation level as the section
  header (e.g. zero-indented docstrings) were incorrectly emitted as
  `STRAY_LINE` tokens and silently dropped. A new `body_is_deeper: Option<bool>`
  flag is introduced to record, on the first body line, whether the body is
  indented deeper than the header. The flush condition is now a three-way
  decision: when no body line has been seen yet, flush only on a *strictly*
  shallower line; when the body is deeper than the header, flush at the
  header's indentation level (previous behaviour); when the body is at the
  same level as the header, never flush by indentation (a following section
  header detected by keyword is still recognised). This also makes the parser
  tolerant of slightly mis-indented entries (e.g. 3-space indent when the
  first entry used 4 spaces).

## [0.1.9] - 2026-04-01

### Fixed

- Google parser: stray lines without a preceding blank line were incorrectly
  absorbed into the current section as bogus entries. The `had_blank_in_section`
  flag is removed; instead, any non-blank line at or below the section header's
  indentation level unconditionally flushes the current section, regardless of
  whether a blank line preceded it.
- NumPy parser: the `had_blank_in_section` flush introduced in v0.1.8 incorrectly
  terminated a section when two entries were separated by a blank line (e.g.
  `x : int\n\ny : float` inside a `Parameters` block). The flag is removed;
  NumPy sections now end only when the next `name\n---` header is detected,
  matching the NumPy docstring specification (stray lines inside NumPy sections
  are a known limitation documented in the source).

## [0.1.8] - 2026-03-31

### Fixed

- Google parser: a non-indented line following a blank line inside a section
  was incorrectly absorbed into that section as a bogus entry (e.g. `stray
  line 1` became an `Args` entry) or appended to the preceding
  `Returns` description. The parser now flushes the current section when a
  blank line is followed by a line whose indentation is at or below the
  section header's indentation level.
- NumPy parser: same fix applied. `FreeText` sections (Notes, Examples, etc.)
  are exempt because their body lines legitimately share the same indentation
  level as the section header.

## [0.1.7] - 2026-03-30

### Fixed

- Google parser: prevent panic when an RST-style parameter line (e.g.
  `:param int seconds: …`) is misclassified as a Google-style `Args:` entry.
  `parse_entry_header` now falls through to the bare-name branch whenever
  the colon is at position 0, avoiding an empty `NAME` token.
- `required_token` no longer panics on zero-length (missing) placeholder
  tokens. It now scans children directly and panics only when the token kind
  is completely absent — indicating a structural bug in the parser — rather
  than when the token is present but zero-length.
- NumPy parser: all `build_*_node` functions now emit zero-length placeholder
  tokens for grammatically expected but source-absent tokens, matching the
  convention already used by the Google `build_arg_node`.
  Affected builders: `build_parameter_node` (TYPE), `build_returns_node`
  (RETURN_TYPE), `build_yields_node` (RETURN_TYPE), `build_exception_node`
  (DESCRIPTION), `build_warning_node` (DESCRIPTION), `build_see_also_node`
  (DESCRIPTION), `build_attribute_node` (TYPE).
  Callers can now reliably use `find_missing()` to distinguish
  "expected but absent" from "not applicable".

## [0.1.6] - 2026-03-29

### Added

- `GoogleSectionKind` / `NumPySectionKind` enums — each section now carries a
  `section_kind` property in place of a plain string, enabling exhaustive
  pattern matching in Python.
- `pydocstring.Visitor` base class — all `enter_*` / `exit_*` hook methods are
  declared here; `walk()` now raises `TypeError` if the visitor does not
  subclass `Visitor`.
- `WalkContext` — second argument passed to every `enter_*` / `exit_*` method;
  exposes `line_col(offset)` using a cached line-starts table for O(log n)
  offset-to-line/column conversion.
- `walk()` now returns the visitor instance (typed as `_VisitorT` in stubs),
  enabling one-liner patterns like `collector = walk(doc, Collector())`.
- `PlainDocstring` dispatch in `walk()` — `enter_plain_docstring` /
  `exit_plain_docstring` are now called when walking a plain-style docstring.
- All CST punctuation and structural tokens are now exposed on every node
  object: `open_bracket`, `close_bracket`, `colon`, `directive_marker`,
  `double_colon`, `default_keyword`, `default_separator`, etc.
- `Token.is_missing()` — returns `True` for zero-length placeholder tokens
  inserted by the parser for syntactically absent elements (e.g. an empty-
  bracket type `arg (): desc`).
- `TextRange.is_empty()` — returns `True` when `start == end`.
- `SectionKind` enum (24 variants) on the style-independent model IR —
  replaces the previous string-typed `Section.kind`.
- `Section.unknown_name` getter — returns the raw section header string for
  `SectionKind.UNKNOWN` entries.
- Missing entry wrapper classes added to the Python bindings: `GoogleWarning`,
  `GoogleSeeAlsoItem`, `GoogleAttribute`, `GoogleMethod`, `NumPyDeprecation`,
  `NumPyWarning`, `NumPySeeAlsoItem`, `NumPyReference`, `NumPyAttribute`,
  `NumPyMethod`.
- `NumPyDocstring.deprecation` property — direct accessor for the deprecation
  notice node.
- `__all__` in `pydocstring/__init__.py` listing all public symbols.
- `py.typed` marker (PEP 561) — the package now ships inline type stubs.
- Rust core: `DocstringVisitor` gains an associated `Error` type; all
  `visit_*` methods return `Result<(), Self::Error>`, and `walk_node`
  propagates errors via `?`.

### Changed

- **Python bindings**: `GoogleReturns` renamed to `GoogleReturn`; `GoogleYields`
  renamed to `GoogleYield` — consistent with the singular naming convention
  used by all other entry wrapper types.
- **Python bindings**: `parse()` now returns a typed
  `GoogleDocstring | NumPyDocstring | PlainDocstring` union instead of an opaque
  `object`.
- `Token` now implements `__eq__` and `__hash__` so tokens can be used in sets
  and as dictionary keys.
- `pretty_print()` and `to_model()` on all docstring objects now use a cached
  `Arc<Parsed>` internally, avoiding a redundant re-parse on every call.
- `WalkContext.line_col()` replaces the former `doc.line_col()` method on
  docstring objects; cost reduced from O(offset) linear scan to O(log L)
  binary search.
- Visitor package layout converted to a mixed Rust/Python maturin layout:
  the `Visitor` base class is now defined in `pydocstring/_visitor.py` rather
  than being embedded as a string in the Rust source.
- Rust core: `walk` and `walk_node` are now in `parse::visitor` and
  re-exported from `parse::google` and `parse::numpy`.

### Fixed

- `GoogleSection` and `NumPySection` stubs no longer declare 19 child accessor
  properties that had no corresponding Rust implementation.

### Breaking Changes

**Python bindings**

- Typed `*Section` classes (e.g. `GoogleArgsSection`, `NumPyParametersSection`)
  removed; replaced by a single `GoogleSection` / `NumPySection` with a
  `section_kind` property and per-kind accessor methods (`section.args()`,
  `section.parameters()`, `section.returns()`, etc.).
- `cast_google_*` / `cast_numpy_*` methods on docstring objects removed.
- Visitor hook methods renamed: `visit_*` → `enter_*`, `leave_*` → `exit_*`
  (ANTLR convention).
- `walk()` now requires the visitor to be a subclass of `pydocstring.Visitor`;
  passing an arbitrary object raises `TypeError`.
- `doc.line_col(offset)` removed from `GoogleDocstring`, `NumPyDocstring`, and
  `PlainDocstring`; use `ctx.line_col(offset)` inside a visitor hook instead.
- `Section.kind` is now a `SectionKind` enum value, not a string.
- `GoogleReturns` / `GoogleYields` class names changed to `GoogleReturn` /
  `GoogleYield`.

**Rust core**

- `DocstringVisitor`: all `visit_*` methods now return `Result<(), Self::Error>`
  and require `type Error = ...;` in every implementation. Infallible
  implementations should use `type Error = std::convert::Infallible`.

## [0.1.5] - 2026-03-22

### Added

- `SyntaxKind::GOOGLE_YIELDS` — dedicated node kind for entries inside a Google
  `Yields:` section. Previously these were emitted as `GOOGLE_RETURNS`.
- `SyntaxKind::NUMPY_YIELDS` — dedicated node kind for entries inside a NumPy
  `Yields` section. Previously these were emitted as `NUMPY_RETURNS`.
- `GoogleYields` typed wrapper with `return_type()`, `colon()`, and
  `description()` accessors (analogous to `GoogleReturns`).
- `NumPyYields` typed wrapper with `name()`, `colon()`, `return_type()`, and
  `description()` accessors (analogous to `NumPyReturns`).
- `GoogleSection::yields()` — accessor returning the `GoogleYields` node for
  a Yields section, distinct from `returns()`.
- `NumPySection::yields()` — accessor returning an iterator of `NumPyYields`
  nodes for a Yields section, distinct from `returns()`.
- Python bindings: `SyntaxKind.GOOGLE_YIELDS`, `SyntaxKind.NUMPY_YIELDS`,
  `GoogleYields` class, `NumPyYields` class, `GoogleSection.yields` property,
  and `NumPySection.yields` property.

### Changed

- Google parser: `Yields:` sections now produce `GOOGLE_YIELDS` child nodes
  instead of `GOOGLE_RETURNS`.
- NumPy parser: `Yields` sections now produce `NUMPY_YIELDS` child nodes
  instead of `NUMPY_RETURNS`.
- `to_model` (Google & NumPy): `Yields` sections now use the `yields()`
  accessor on the typed section wrapper rather than sharing the `returns()`
  code path.

## [0.1.4] - 2026-03-20

### Added

- `Style::Plain` — new style variant returned by `detect_style` for docstrings
  that contain no NumPy section underlines or Google section headers (e.g.
  summary-only docstrings, Sphinx-style docstrings).
- `SyntaxKind::PLAIN_DOCSTRING` — root node kind for plain-style parse trees.
- `parse_plain(input)` — lightweight parser that extracts only a `SUMMARY` and
  an optional `EXTENDED_SUMMARY` token from the input, without attempting
  section detection.
- `parse(input)` — unified entry point that calls `detect_style` and dispatches
  to `parse_google`, `parse_numpy`, or `parse_plain` automatically.
- `PlainDocstring` typed wrapper with `summary()` and `extended_summary()`
  accessors (mirrors the existing `GoogleDocstring` / `NumPyDocstring` API).
- Python bindings: `Style.PLAIN`, `SyntaxKind.PLAIN_DOCSTRING`, `PlainDocstring`
  class, and `parse_plain(input)` function.
- Google parser: zero-length `DESCRIPTION` token emitted when a colon is
  present but no description text follows (e.g. `a (int):`, `a:`), and
  zero-length `TYPE` token emitted for empty brackets `()`.
- NumPy parser: zero-length `TYPE` token emitted when a colon is present but
  type text is absent (e.g. `a :`); zero-length `DEFAULT_VALUE` token emitted
  when a default separator is present but no value follows (e.g. `default =`).
  Callers can use `find_missing(KIND)` to detect these absent-but-declared
  slots without inspecting surrounding tokens.
- `examples/parse_auto.rs` — demonstrates the unified `parse()` entry point
  with Google, NumPy, and plain-style inputs.

### Changed

- `detect_style` rewritten as a single O(n) pass; returns `Style::Plain` as the
  fallback instead of `Style::Google`.

## [0.1.3] - 2026-03-19

### Added

- Section name matching now accepts additional singular and alias forms for both
  Google and NumPy styles:
  - `"arg"`, `"param"`, `"keyword arg"`, `"keyword param"`, `"other arg"`,
    `"other param"`, `"method"`, `"reference"` (Google)
  - `"arguments"`, `"argument"`, `"args"`, `"arg"`, `"other arguments"`,
    `"other argument"`, `"other args"`, `"other arg"`, `"attribute"`,
    `"method"`, `"reference"` (NumPy)
  - Common typos tolerated: `"argment"`, `"paramter"` (Google)

### Fixed

- Google parser: arg entries with no description (e.g. `b :`) inside a section
  body were incorrectly classified as new section headers. Fixed by comparing
  the indentation of each line against the current section header's indentation
  and skipping header detection for more-indented lines.

### Changed

- Refactored Google entry header parsing to use a left-to-right confirmation
  algorithm. Handles missing close brackets, missing colons, and text after
  brackets without a colon more robustly. `close_bracket` in `TypeInfo` is now
  `Option<TextRange>` to represent the missing-bracket case.
- Added `rustfmt.toml` (`max_width = 120`) and reformatted all source files.

## [0.1.2] - 2026-03-16

### Added

- `LineColumn` struct (`lineno`, `col`) in `text.rs` for representing
  line/column positions; `lineno` is 1-based, `col` is a 0-based byte offset
  within the line.
- `LineIndex` in `text.rs` — a newline-offset lookup table built from source
  text; converts any `TextSize` byte offset to `LineColumn` in O(log n).
- `Parsed::line_col(offset: TextSize) -> LineColumn` method for resolving
  byte offsets in the syntax tree to line/column positions.
- Python bindings: `LineColumn` class with `lineno` and `col` properties.
  `col` is expressed in **Unicode codepoints** (compatible with Python's
  `ast` module convention) rather than raw bytes.
- Python bindings: `GoogleDocstring.line_col(offset)` and
  `NumPyDocstring.line_col(offset)` methods; `offset` is typically obtained
  from `Token.range.start` or `Token.range.end`.

## [0.1.1] - 2026-03-10

### Added

- Python bindings: `SyntaxKind` enum exposed as `enum.IntEnum`, usable for
  pattern matching on `Token.kind` and `Node.kind` instead of raw strings.

### Fixed

- `emit_google` / `emit_numpy` now correctly apply `base_indent` to all lines
  of the emitted docstring, not just the first line.

### Changed

- Python bindings: `Token.kind` and `Node.kind` now return `SyntaxKind` instead
  of `str`.

## [0.1.0] - 2025-03-09

### Added

- Google style docstring parsing (`parse_google`)
- NumPy style docstring parsing (`parse_numpy`)
- Automatic style detection (`detect_style`)
- Unified model IR (`Docstring`, `Section`, `Parameter`, `Return`, etc.)
- Emit back to Google style (`emit_google`)
- Emit back to NumPy style (`emit_numpy`)
- Full syntax tree (AST) with byte-precise source locations (`TextRange`)
- Tree traversal via `walk` and visitor pattern
- Pretty-print for AST debugging (`pretty_print`)
- Conversion from AST to unified model (`to_model`)
- Support for all standard sections:
  - Parameters / Args / Keyword Args / Other Parameters
  - Returns / Yields
  - Raises / Warns
  - Attributes / Methods
  - See Also / References
  - Deprecation
  - Free-text sections (Notes, Examples, Warnings, Todo, etc.)
- Error-resilient parsing — never panics on malformed input
- Zero external crate dependencies
- Python bindings via PyO3 (`pydocstring-rs`)

[Unreleased]: https://github.com/ryumasai/pydocstring/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/ryumasai/pydocstring/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/ryumasai/pydocstring/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/ryumasai/pydocstring/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/ryumasai/pydocstring/compare/v0.1.15...v0.2.0
[0.1.15]: https://github.com/ryumasai/pydocstring/compare/v0.1.14...v0.1.15
[0.1.14]: https://github.com/ryumasai/pydocstring/compare/v0.1.13...v0.1.14
[0.1.13]: https://github.com/ryumasai/pydocstring/compare/v0.1.12...v0.1.13
[0.1.12]: https://github.com/ryumasai/pydocstring/compare/v0.1.11...v0.1.12
[0.1.11]: https://github.com/ryumasai/pydocstring/compare/v0.1.10...v0.1.11
[0.1.10]: https://github.com/ryumasai/pydocstring/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/ryumasai/pydocstring/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/ryumasai/pydocstring/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/ryumasai/pydocstring/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/ryumasai/pydocstring/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/ryumasai/pydocstring/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/ryumasai/pydocstring/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/ryumasai/pydocstring/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/ryumasai/pydocstring/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/ryumasai/pydocstring/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/ryumasai/pydocstring/releases/tag/v0.1.0
