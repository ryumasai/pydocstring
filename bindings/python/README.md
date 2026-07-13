# pydocstring-rs

[![PyPI - Version](https://img.shields.io/pypi/v/pydocstring-rs?color=0062A8)](https://pypi.org/project/pydocstring-rs/)
[![PyPI - Python Version](https://img.shields.io/pypi/pyversions/pydocstring-rs?color=0062A8)](https://devguide.python.org/versions/)
[![Crates.io Version](https://img.shields.io/crates/v/pydocstring?color=FFC12d)](https://crates.io/crates/pydocstring)
[![Crates.io MSRV](https://img.shields.io/crates/msrv/pydocstring?color=FFC12d)](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0)

Python bindings for [pydocstring](https://crates.io/crates/pydocstring) — a zero-dependency Rust parser for Python docstrings (Google and NumPy styles).

Produces a **unified syntax tree** with **byte-precise source locations** on every token — designed as infrastructure for linters and formatters.

## Features

- **One code path for every style** — the unified view (`Document` → `Section` → `Entry`) reads Google and NumPy docstrings with no style branching
- **Full syntax tree** — builds a complete AST, not just extracted fields; traverse it with `walk()`
- **Byte-precise source locations** — every view carries its exact byte range, for pinpoint diagnostics and as an anchor for edits
- **Powered by Rust** — native extension with no Python runtime overhead
- **Error-resilient** — never raises exceptions; malformed input still yields a best-effort tree
- **Style auto-detection** — hand it a docstring, get back `Style.GOOGLE`, `Style.NUMPY`, or `Style.PLAIN`

## Installation

```bash
pip install pydocstring-rs
```

## Usage

### Reading a docstring (the unified view)

`parse()` auto-detects the style; `Document` gives you a style-independent view
of the result. This is the recommended way to read a docstring:

```python
from pydocstring import Document, SectionKind, parse

doc = Document(parse(source))

for section in doc.sections:
    if section.kind == SectionKind.PARAMETERS:
        for entry in section.entries:
            print(entry.name.text, entry.description.logical_text)
```

The same loop reads both of these, unchanged — `Args:` and `Parameters` both
resolve to `SectionKind.PARAMETERS`, so the role of a section is *data*, not a
type you have to dispatch on:

```python
"""Summary.              """Summary.

Args:                    Parameters
    x (int): The value.  ----------
"""                      x : int
                             The value.
                         """
```

Every view keeps its range, so the results double as edit anchors:

```python
entry = doc.sections[0].entries[0]
r = entry.description.range

raw = source.encode()
edited = (raw[:r.start] + b"A better description." + raw[r.end:]).decode()
```

Everything outside that range is preserved byte-for-byte — the NumPy version
keeps its indentation, the Google version keeps its `x (int): ` prefix.

> **Ranges are byte offsets into the UTF-8 source, not character indices.** Slice
> `source.encode()`, not `source`: with any non-ASCII character before the range,
> `source[:r.start]` cuts in the wrong place.

Accessors on `Entry` are all optional (`name`, `type_annotation`,
`description`, …), so reading an entry never raises for a role that does not
carry that piece: a `Raises:` entry simply has `name is None` and its exception
type in `type_annotation`.

Every parser returns the same `Parsed` — there is no per-style result type to
dispatch on. Use `parse_google()` / `parse_numpy()` / `parse_plain()` when you
want to force a style instead of detecting one.

### Editing

`edit()` starts a list of anchored splices. Everything an edit does not touch is
preserved byte-for-byte — this is not a re-render:

```python
from pydocstring import Document, SectionKind, parse

parsed = parse(source)
doc = Document(parsed)
edits = parsed.edit()

for section in doc.sections:
    if section.kind == SectionKind.PARAMETERS:
        for entry in section.entries:
            if entry.name.text == "y":
                edits.replace(entry.description.range, "The other value.")

result = edits.apply()
```

Scoping a rewrite to one section is the `if` in that loop. The same code runs
over a Google or a NumPy docstring, and each keeps its own layout:

```diff
 Summary.                        Summary.

 Args:                           Parameters
     x (int): The value.         ----------
-    y: Another.                 x : int
+    y: The other value.             The value.
                                 y
                                -    Another.
                                +    The other value.
```

| Method                     | Effect                                                                    |
|----------------------------|---------------------------------------------------------------------------|
| `replace(range, text)`     | Replace the bytes of `range`. A zero-length range inserts.                |
| `insert(at, text)`         | Insert at byte offset `at`.                                               |
| `delete(range)`            | Delete the bytes of `range`.                                              |
| `remove_lines(range)`      | Delete `range` with its whole line(s): indentation, newline, and one adjacent trailing blank line. |
| `apply()`                  | Validate and splice; returns the new source. Non-consuming.               |
| `apply_reparsed()`         | `apply()`, then re-parse — **with the same style**, never re-detected.    |

Two laws hold, and are property-tested over the corpus: an empty edit list
reproduces the source exactly, and replacing an element with its own text is the
identity. `apply()` raises `EditError` (a `ValueError`) if a range is out of
bounds or two edits overlap.

Editing must not silently reinterpret a docstring as another style, so
`apply_reparsed()` re-parses with the original style even if the edited text
would auto-detect differently.

### Scoped pattern rewrites

`replace()` rewrites every match in the document, which is often too much — the
pattern `$NAME: $DESC` matches an `Args:` entry *and* a `Raises:` one.
`replace_in()` scopes the rewrite to a view's subtree:

```python
from pydocstring import Document, SectionKind, parse_google

parsed = parse_google(source)
doc = Document(parsed)
args = next(s for s in doc.sections if s.kind == SectionKind.PARAMETERS)

parsed.replace_in(args, "$NAME: $DESC", "$NAME: TODO")   # Raises: is untouched
```

The anchor also selects the *reading*: the same shape is a `$NAME` under `Args:`
and a `$TYPE` under `Raises:`. `findall_in()` scopes a search the same way. Any
`Document`, `Section`, or `Entry` of the same parse result works as an anchor.

### The raw CST

The unified view is a *semantic* lens: it answers "is there a type?", and folds
away punctuation, whitespace, and the parser's zero-length placeholders. When
you need the tree exactly as parsed, go down to the CST with `.syntax`. It is on
the parse result and on every *node-backed* view — `Document`, `Section`,
`Entry`, `DefaultMarker`, `Directive`, `Citation` — but not on `TextBlock` or
`Token`, which are already leaves of the tree:

```python
from pydocstring import Document, SyntaxKind, parse

entry = Document(parse(source)).sections[0].entries[0]
node = entry.syntax                       # -> Node(ENTRY, ...)

node.kind                                 # SyntaxKind.ENTRY
node.children                             # [Token(NAME), Token(WHITESPACE), ..., Node(DESCRIPTION)]
node.find_token(SyntaxKind.TYPE)          # the type token, if written
```

The tree's vocabulary is style-independent — a Google entry and a NumPy entry are
both `SyntaxKind.ENTRY` — so one traversal walks any docstring.

The CST is what tells apart cases the semantic lens equates. Both of these report
`entry.type_annotation is None`, but they are not the same docstring:

```python
node.find_missing(SyntaxKind.TYPE)   # x ():  -> a zero-length placeholder
node.find_missing(SyntaxKind.TYPE)   # x:     -> None; no type token at all
```

A missing placeholder's range is an *insertion anchor*: `edits.replace(placeholder.range, "int")`
writes the type exactly where it belongs.

Every byte of the source is covered by exactly one token, so concatenating the
tree's non-missing leaves reproduces the input.

### Style Detection

```python
from pydocstring import detect_style, Style

detect_style("Summary.\n\nArgs:\n    x: Desc.")       # Style.GOOGLE
detect_style("Summary.\n\nParameters\n----------\n")  # Style.NUMPY
detect_style("Just a summary.")                       # Style.PLAIN
```

`Style.PLAIN` covers docstrings with no recognised section markers:
summary-only, summary + extended, and unrecognised styles such as Sphinx.

### Forcing a style

`parse()` auto-detects. When you know the style — or want to force it — use the
explicit parsers. They all return the same `Parsed`:

```python
from pydocstring import parse_google, parse_numpy, parse_plain

parse_google(source)   # read as Google, whatever it looks like
parse_numpy(source)
parse_plain(source)    # no section markers; everything after the summary is extended_summary
```

Docstrings with no recognised section markers parse as plain. Unrecognised styles
such as Sphinx are treated the same way for now: `:param:` lines are preserved
verbatim in `extended_summary`.

### The syntax tree

`pretty_print()` visualises the whole tree:

```python
print(parse_google("Summary.\n\nArgs:\n    x (int): Value.").pretty_print())
```

```text
DOCUMENT@0..35 {
  SUMMARY@0..8 {
    TEXT_LINE: "Summary."@0..8
  }
  NEWLINE: "\n"@8..9
  BLANK_LINE: "\n"@9..10
  SECTION@10..35 {
    SECTION_HEADER@10..15 {
      NAME: "Args"@10..14
      COLON: ":"@14..15
    }
    NEWLINE: "\n"@15..16
    WHITESPACE: "    "@16..20
    ENTRY@20..35 {
      NAME: "x"@20..21
      WHITESPACE: " "@21..22
      OPEN_BRACKET: "("@22..23
      TYPE: "int"@23..26
      CLOSE_BRACKET: ")"@26..27
      COLON: ":"@27..28
      WHITESPACE: " "@28..29
      DESCRIPTION@29..35 {
        TEXT_LINE: "Value."@29..35
      }
    }
  }
}
```

Note the node kinds: `SECTION`, `ENTRY`, `NAME` — nothing in the tree is
Google-specific. The same NumPy docstring produces the same kinds, which is why
one traversal reads both.

### Tree traversal

`walk()` takes a `Visitor` subclass and returns it, so results can be read inline.
Override any of `enter_node`, `leave_node`, `visit_token` — the hooks you leave
alone are never called. Dispatch on `kind`:

```python
from pydocstring import SyntaxKind, Visitor, parse, walk

class NameCollector(Visitor):
    def __init__(self):
        self.names = []

    def visit_token(self, token, ctx):
        if token.kind == SyntaxKind.NAME:
            self.names.append(token.text)

print(walk(parse(source), NameCollector()).names)
```

`walk()` also accepts a `Node`, so you can walk a subtree:

```python
section = parse(source).syntax.find_node(SyntaxKind.SECTION)
walk(section, NameCollector())
```

`WalkContext` is the second argument to every hook, and converts a byte offset to
a line/column in O(log n):

```python
class LocPrinter(Visitor):
    def visit_token(self, token, ctx):
        if token.kind == SyntaxKind.NAME:
            lc = ctx.line_col(token.range.start)
            print(f"{token.text} at line {lc.lineno}, col {lc.col}")
```

### Source Locations

All tokens carry byte-precise source ranges:

```python
doc = parse_google("Summary.\n\nArgs:\n    x (int): Value.")
token = doc.summary
print(token.range.start, token.range.end)  # 0 8
```

### Model IR (`pydocstring.model`)

`to_model()` produces the **model IR**: owned, interpreted data with the source
positions dropped. It lives in its own namespace, mirroring the Rust crate:

```python
from pydocstring import SectionKind, parse_google
from pydocstring.model import Block

parsed = parse_google("Summary.\n\nArgs:\n    x (int): The value.\n")
doc = parsed.to_model()

print(doc.summary)  # "Summary."

for section in doc.sections:
    if section.kind == SectionKind.PARAMETERS:
        for block in section.blocks:
            if isinstance(block, Block.Parameter):
                param = block.value
                print(param.names)            # ["x"]
                print(param.type_annotation)  # "int"
                print(param.description)      # "The value."
```

A section body is a flat sequence of `Block`s in source order: prose
`Block.Paragraph`s interleaved with typed entries (`Block.Parameter`,
`Block.Return`, `Block.Exception`, `Block.Attribute`, `Block.Method`,
`Block.SeeAlso`, `Block.Reference`).

**Model or unified view?** The dividing line is byte positions. The model drops
them, which is what lets it apply semantics the tree cannot express (merging
consecutive lines into one paragraph, for instance) — and it is why the model is
a one-way projection: use it to inspect, transform, and re-emit. To *edit* a
docstring in place, use the position-preserving `Document` view above; re-emitting
from the model rewrites the whole docstring, including the parts you did not touch.

### Emitting (Code Generation)

Re-emit a model `Docstring` in any style — useful for style conversion or formatting:

```python
from pydocstring import SectionKind, emit_google, emit_numpy
from pydocstring.model import Block, Docstring, Parameter, Section

doc = Docstring(
    summary="Brief summary.",
    sections=[
        Section(
            SectionKind.PARAMETERS,
            [
                Block.Parameter(
                    Parameter(
                        ["x"],
                        type_annotation="int",
                        description="The value.",
                    ),
                ),
            ],
        ),
    ],
)

google = emit_google(doc)
print(google)  # Contains "Args:"

numpy = emit_numpy(doc)
print(numpy)  # Contains "Parameters\n----------"
```

Combine parsing and emitting to convert between styles:

```python
from pydocstring import parse_google, emit_numpy

parsed = parse_google("Summary.\n\nArgs:\n    x (int): The value.\n")
doc = parsed.to_model()
numpy_text = emit_numpy(doc)
print(numpy_text)  # Contains "Parameters\n----------"
```

## API Reference

### Functions

| Function                      | Returns  | Description                                                   |
|-------------------------------|----------|---------------------------------------------------------------|
| `parse(text)`                 | `Parsed` | Auto-detect style and parse; check `.style` for the result     |
| `parse_google(text)`          | `Parsed` | Parse as Google style                                          |
| `parse_numpy(text)`           | `Parsed` | Parse as NumPy style                                           |
| `parse_plain(text)`           | `Parsed` | Parse as plain (no section markers)                            |
| `detect_style(text)`          | `Style`  | Detect style: `Style.GOOGLE`, `Style.NUMPY`, or `Style.PLAIN`  |
| `walk(parsed_or_node, visitor)` | the visitor | Depth-first CST traversal                                |
| `emit_google(doc)`            | `str`    | Emit a model `Docstring` as Google-style text                  |
| `emit_numpy(doc)`             | `str`    | Emit a model `Docstring` as NumPy-style text                   |
| `emit_sphinx(doc)`            | `str`    | Emit a model `Docstring` as Sphinx (reStructuredText) text     |

### Objects

#### Unified views — the style-independent read lens

| Class           | Key Properties                                                                                        |
|-----------------|-------------------------------------------------------------------------------------------------------|
| `Document`      | `Document(parsed)`; `style`, `summary`, `extended_summary`, `sections`, `directives`, `paragraphs`, `source`, `range` |
| `Section`       | `kind` (`SectionKind`), `header_name`, `unknown_name`, `entries`, `body`, `citations`, `range`        |
| `Entry`         | `name`, `names`, `type_annotation`, `description`, `is_optional`, `optionals`, `defaults`, `default_value`, `range` |
| `DefaultMarker` | `keyword`, `separator`, `value`, `range`                                                              |
| `Directive`     | `name`, `argument`, `description`, `range`                                                            |
| `Citation`      | `label`, `description`, `range`                                                                       |

Every accessor is optional, so no read raises for a role that does not carry
that piece. `None` means "not present" — unlike the per-style wrappers below,
these views do not surface zero-length missing placeholders.

#### Raw CST — the fidelity lens

Reached with `.syntax`, from a parse result or from any unified view.

| Class        | Key members                                                                                 |
|--------------|----------------------------------------------------------------------------------------------|
| `Node`       | `kind`, `range`, `text`, `children`, `nodes(kind)`, `tokens(kind)`, `find_node(kind)`, `find_token(kind)`, `find_missing(kind)` |
| `Token`      | `kind`, `text`, `range`, `is_missing()`                                                     |
| `SyntaxKind` | `ENTRY`, `SECTION`, `NAME`, `TYPE`, `DESCRIPTION`, `COLON`, … (31 kinds, plus `UNKNOWN`)    |

#### Editing

| Class       | Members                                                                                  |
|-------------|------------------------------------------------------------------------------------------|
| `Edits`     | `replace(range, text)`, `insert(at, text)`, `delete(range)`, `remove_lines(range)`, `apply()`, `apply_reparsed()`, `len()` |
| `EditError` | Raised by `apply()` for an out-of-bounds or overlapping edit (a `ValueError`)             |

Start one with `parsed.edit()` or `doc.edit()`.

#### Core types and per-style CST wrappers

| Class                | Key Properties                                                                                                   |
|----------------------|------------------------------------------------------------------------------------------------------------------|
| `Style`         | `GOOGLE`, `NUMPY`, `PLAIN` (enum)                                                                     |
| `SectionKind`   | `PARAMETERS`, `RETURNS`, `RAISES`, `NOTES`, … (24 variants — shared by `Section.kind` and the model)  |
| `Parsed`        | `style`, `source`, `syntax`, `range`, `pretty_print()`, `to_model()`, `edit()`, `replace()`, `replace_in()`, `findall()`, `findall_in()` |
| `Token`              | `text`, `range`, `is_missing()`                                                                                  |
| `TextRange`          | `start`, `end`, `is_empty()`                                                                                     |
| `Visitor`            | Base class; subclass and override `enter_*` / `exit_*` methods                                                   |
| `WalkContext`        | `line_col(offset)` — passed as second arg to every `enter_*` / `exit_*` hook                                    |

#### Model IR — `pydocstring.model`

Position-free. `SectionKind` is shared with the unified view and stays at the
top level; everything else lives under `pydocstring.model`.

| Class                | Key Properties                                                                                                   |
|----------------------|------------------------------------------------------------------------------------------------------------------|
| `model.Docstring`    | `summary`, `extended_summary`, `directives`, `deprecation` (computed), `sections`                                |
| `model.Section`      | `kind`, `blocks`, `unknown_name`                                                                                 |
| `model.Block`        | variants `Paragraph` (`text`), `Parameter`/`Return`/`Exception`/`Attribute`/`Method`/`SeeAlso`/`Reference` (`value`) |
| `model.Parameter`    | `names`, `type_annotation`, `description`, `is_optional`, `default_value`                                        |
| `model.Return`       | `name`, `type_annotation`, `description`                                                                         |
| `model.ExceptionEntry` | `type_name`, `description`                                                                                     |
| `model.Attribute`    | `names`, `type_annotation`, `description`                                                                        |
| `model.Method`       | `name`, `type_annotation`, `description`                                                                         |
| `model.SeeAlsoEntry` | `names`, `description`                                                                                           |
| `model.Reference`    | `label`, `content`                                                                                              |
| `model.Directive`    | `name`, `argument`, `description`                                                                                |

## Development

### Prerequisites

- Rust (stable)
- [uv](https://docs.astral.sh/uv/) (manages the Python interpreter, venv, and dev tooling)

### Build

```bash
cd bindings/python

# Create the venv and install dev tooling (maturin, pytest) into it.
# uv provisions the pinned Python from .python-version automatically.
uv sync

# Build and install the native extension in development mode
uv run maturin develop --uv

# Verify
uv run python -c "import pydocstring; print(pydocstring.detect_style('Args:\n    x: y'))"
```

After changing the Rust source, re-run `uv run maturin develop --uv` to rebuild.

### Build a wheel

```bash
uv run maturin build --release
# Output: target/wheels/pydocstring-*.whl
```

### Publish to PyPI

```bash
uv run maturin publish
```
