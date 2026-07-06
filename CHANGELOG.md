# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
