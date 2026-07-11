# pydocstring-rs

[![PyPI - Version](https://img.shields.io/pypi/v/pydocstring-rs?color=0062A8)](https://pypi.org/project/pydocstring-rs/)
[![PyPI - Python Version](https://img.shields.io/pypi/pyversions/pydocstring-rs?color=0062A8)](https://devguide.python.org/versions/)
[![Crates.io Version](https://img.shields.io/crates/v/pydocstring?color=FFC12d)](https://crates.io/crates/pydocstring)
[![Crates.io MSRV](https://img.shields.io/crates/msrv/pydocstring?color=FFC12d)](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0)

Python bindings for [pydocstring](https://crates.io/crates/pydocstring) — a zero-dependency Rust parser for Python docstrings (Google and NumPy styles).

Produces a **unified syntax tree** with **byte-precise source locations** on every token — designed as infrastructure for linters and formatters.

## Features

- **Full syntax tree** — builds a complete AST, not just extracted fields; traverse it with `walk()`
- **Typed objects per style** — style-specific classes like `GoogleArg`, `NumPyParameter`
- **Byte-precise source locations** — every token carries its exact byte range for pinpoint diagnostics
- **Powered by Rust** — native extension with no Python runtime overhead
- **Error-resilient** — never raises exceptions; malformed input still yields a best-effort tree
- **Style auto-detection** — hand it a docstring, get back `Style.GOOGLE`, `Style.NUMPY`, or `Style.PLAIN`

## Installation

```bash
pip install pydocstring-rs
```

## Usage

### Unified Parse (auto-detect)

Use `parse()` when you don't know the style in advance.
The returned object has a `.style` property so you can dispatch without `isinstance` checks:

```python
from pydocstring import parse, Style

doc = parse(source)

match doc.style:
    case Style.GOOGLE:
        for arg in doc.sections[0].args:
            print(arg.name.text, arg.description.text)
    case Style.NUMPY:
        for param in doc.sections[0].parameters:
            print([n.text for n in param.names], param.description.text)
    case Style.PLAIN:
        print(doc.summary.text)
```

When you only need the style-independent model, no dispatch is necessary:

```python
model = parse(source).to_model()  # works for all three styles
```

If you already know the style, prefer the explicit functions `parse_google()`,
`parse_numpy()`, or `parse_plain()` — they return a concrete type and are
slightly more efficient.

### Style Detection

```python
from pydocstring import detect_style, Style

detect_style("Summary.\n\nArgs:\n    x: Desc.")       # Style.GOOGLE
detect_style("Summary.\n\nParameters\n----------\n")  # Style.NUMPY
detect_style("Just a summary.")                       # Style.PLAIN
```

`Style.PLAIN` covers docstrings with no recognised section markers:
summary-only, summary + extended, and unrecognised styles such as Sphinx.

### Plain Style

Docstrings with no NumPy or Google section markers are parsed as plain:

```python
from pydocstring import parse_plain

doc = parse_plain("""Brief summary.

More detail here.
Spanning multiple lines.
""")

print(doc.summary.text)            # "Brief summary."
print(doc.extended_summary.text)   # "More detail here.\nSpanning multiple lines."
```

Unrecognised styles such as Sphinx are also treated as plain: the `:param:`
lines are preserved verbatim in `extended_summary`.

### Google Style

```python
from pydocstring import parse_google

doc = parse_google("""Summary line.

Args:
    x (int): The first value.
    y (str): The second value.

Returns:
    bool: True if successful.

Raises:
    ValueError: If x is negative.
""")

# Summary
print(doc.summary.text)  # "Summary line."

# Sections
for section in doc.sections:
    print(section.section_kind)  # GoogleSectionKind.ARGS, .RETURNS, .RAISES

# Walk the tree to access entries
from pydocstring import walk, Visitor

class GoogleArgCollector(Visitor):
    def __init__(self): self.args = []
    def enter_google_arg(self, arg, ctx): self.args.append(arg)

for arg in walk(doc, GoogleArgCollector()).args:
    print(f"  {arg.name.text}: {arg.type.text} — {arg.description.text}")
```

### NumPy Style

```python
from pydocstring import parse_numpy

doc = parse_numpy("""Summary line.

Parameters
----------
x : int
    The first value.
y : str
    The second value.

Returns
-------
bool
    True if successful.
""")

print(doc.summary.text)  # "Summary line."

for section in doc.sections:
    print(section.section_kind)  # NumPySectionKind.PARAMETERS, .RETURNS

# Walk the tree to access entries
class NumPyParamCollector(Visitor):
    def __init__(self): self.params = []
    def enter_numpy_parameter(self, p, ctx): self.params.append(p)

for param in walk(doc, NumPyParamCollector()).params:
    names = [n.text for n in param.names]
    print(f"  {names}: {param.type.text} — {param.description.text}")
```

### AST Access

Use `pretty_print()` to visualise the full syntax tree:

```python
doc = parse_google("Summary.\n\nArgs:\n    x (int): Value.")

print(doc.pretty_print())
```

Output:

```text
GOOGLE_DOCSTRING@0..42 {
  SUMMARY: "Summary."@0..8
  GOOGLE_SECTION@10..42 {
    GOOGLE_SECTION_HEADER@10..15 {
      NAME: "Args"@10..14
      COLON: ":"@14..15
    }
    GOOGLE_ARG@20..42 {
      NAME: "x"@20..21
      OPEN_BRACKET: "("@22..23
      TYPE: "int"@23..26
      CLOSE_BRACKET: ")"@26..27
      COLON: ":"@27..28
      DESCRIPTION: "Value."@29..35
    }
  }
}
```

### Tree Traversal

Use `walk()` with a `Visitor` subclass for depth-first traversal. `walk()` returns
the visitor instance so you can read results inline:

```python
from pydocstring import parse_google, walk, Visitor

doc = parse_google("Summary.\n\nArgs:\n    x (int): Value.")

class NameCollector(Visitor):
    def __init__(self): self.names = []
    def enter_google_arg(self, arg, ctx): self.names.append(arg.name.text)

print(walk(doc, NameCollector()).names)  # ["x"]
```

`WalkContext` is passed as the second argument to every `enter_*` / `exit_*` hook
and exposes `line_col(offset)` for O(log n) byte-offset-to-line/column conversion:

```python
class LocPrinter(Visitor):
    def enter_google_arg(self, arg, ctx):
        lc = ctx.line_col(arg.name.range.start)
        print(f"{arg.name.text} at line {lc.lineno}, col {lc.col}")

walk(doc, LocPrinter())
```

### Source Locations

All tokens carry byte-precise source ranges:

```python
doc = parse_google("Summary.\n\nArgs:\n    x (int): Value.")
token = doc.summary
print(token.range.start, token.range.end)  # 0 8
```

### Style-Independent Model (IR)

Convert any parsed docstring into a style-independent intermediate representation for analysis or transformation:

```python
from pydocstring import parse_google, Block, SectionKind

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

### Emitting (Code Generation)

Re-emit a `Docstring` model in any style — useful for style conversion or formatting:

```python
from pydocstring import Docstring, Section, SectionKind, Block, Parameter, emit_google, emit_numpy

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

| Function             | Returns                                         | Description                                                   |
|----------------------|-------------------------------------------------|---------------------------------------------------------------|
| `parse(text)`        | `GoogleDocstring \| NumPyDocstring \| PlainDocstring` | Auto-detect style and parse                             |
| `parse_google(text)` | `GoogleDocstring`                               | Parse a Google-style docstring                                |
| `parse_numpy(text)`  | `NumPyDocstring`                                | Parse a NumPy-style docstring                                 |
| `parse_plain(text)`  | `PlainDocstring`                                | Parse a plain docstring (no section markers)                  |
| `detect_style(text)` | `Style`                                         | Detect style: `Style.GOOGLE`, `Style.NUMPY`, or `Style.PLAIN` |
| `emit_google(doc)`   | `str`                                           | Emit a `Docstring` model as Google-style text                 |
| `emit_numpy(doc)`    | `str`                                           | Emit a `Docstring` model as NumPy-style text                  |

### Objects

| Class                | Key Properties                                                                                                   |
|----------------------|------------------------------------------------------------------------------------------------------------------|
| `Style`              | `GOOGLE`, `NUMPY`, `PLAIN` (enum)                                                                                |
| `GoogleSectionKind`  | `ARGS`, `RETURNS`, `YIELDS`, `RAISES`, `NOTES`, `EXAMPLES`, … (enum)                                            |
| `NumPySectionKind`   | `PARAMETERS`, `RETURNS`, `YIELDS`, `RAISES`, `NOTES`, `EXAMPLES`, … (enum)                                      |
| `GoogleDocstring`    | `style`, `summary`, `extended_summary`, `paragraphs`, `sections`, `deprecation`, `source`, `pretty_print()`, `to_model()` |
| `GoogleSection`      | `section_kind`, `header_name`, `range`                                                                           |
| `GoogleArg`          | `name`, `type`, `description`, `optional`, `open_bracket`, `close_bracket`, `colon`                             |
| `GoogleReturn`       | `return_type`, `description`, `colon`                                                                            |
| `GoogleYield`        | `return_type`, `description`, `colon`                                                                            |
| `GoogleException`    | `type`, `description`, `colon`                                                                                   |
| `GoogleWarning`      | `type`, `description`, `colon`                                                                                   |
| `GoogleSeeAlsoItem`  | `name`, `description`, `colon`                                                                                   |
| `GoogleAttribute`    | `name`, `type`, `description`, `open_bracket`, `close_bracket`, `colon`                                         |
| `GoogleMethod`       | `name`, `type`, `description`, `open_bracket`, `close_bracket`, `colon`                                         |
| `PlainDocstring`     | `style`, `summary`, `extended_summary`, `source`, `pretty_print()`, `to_model()`                                |
| `NumPyDocstring`     | `style`, `summary`, `extended_summary`, `paragraphs`, `sections`, `deprecation`, `source`, `pretty_print()`, `to_model()` |
| `NumPySection`       | `section_kind`, `header_name`, `range`                                                                           |
| `NumPyParameter`     | `name`, `names`, `type`, `description`, `optional`, `default_value`, `colon`, `default_keyword`, `default_separator` |
| `NumPyReturns`       | `name`, `return_type`, `description`, `colon`                                                                    |
| `NumPyYields`        | `name`, `return_type`, `description`, `colon`                                                                    |
| `NumPyException`     | `type`, `description`, `colon`                                                                                   |
| `NumPyWarning`       | `type`, `description`, `colon`                                                                                   |
| `NumPySeeAlsoItem`   | `name`, `description`, `colon`                                                                                   |
| `NumPyReference`     | `label`, `content`, `directive_marker`, `open_bracket`, `close_bracket`                                        |
| `NumPyAttribute`     | `name`, `type`, `description`, `colon`                                                                           |
| `NumPyMethod`        | `name`, `type`, `description`, `colon`                                                                           |
| `NumPyDeprecation`   | `version`, `description`, `directive_marker`, `keyword`, `double_colon`                                         |
| `Token`              | `text`, `range`, `is_missing()`                                                                                  |
| `TextRange`          | `start`, `end`, `is_empty()`                                                                                     |
| `Visitor`            | Base class; subclass and override `enter_*` / `exit_*` methods                                                   |
| `WalkContext`        | `line_col(offset)` — passed as second arg to every `enter_*` / `exit_*` hook                                    |
| `SectionKind`        | `PARAMETERS`, `RETURNS`, `RAISES`, `NOTES`, … (enum, 24 variants — model IR)                                    |
| `Docstring`          | `summary`, `extended_summary`, `directives`, `deprecation` (computed), `sections`                                |
| `Section` (model)    | `kind`, `blocks`, `unknown_name`                                                                                 |
| `Block` (model)      | variants `Paragraph` (`text`), `Parameter`/`Return`/`Exception`/`Attribute`/`Method`/`SeeAlso`/`Reference` (`value`) |
| `Parameter`          | `names`, `type_annotation`, `description`, `is_optional`, `default_value`                                        |
| `Return`             | `name`, `type_annotation`, `description`                                                                         |
| `ExceptionEntry`     | `type_name`, `description`                                                                                       |
| `Attribute`          | `name`, `type_annotation`, `description`                                                                         |
| `Method`             | `name`, `type_annotation`, `description`                                                                         |
| `SeeAlsoEntry`       | `names`, `description`                                                                                           |
| `Reference`          | `label`, `content`                                                                                              |
| `Directive`          | `name`, `argument`, `description`                                                                                |

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
