# pydocstring

[![Crates.io Version](https://img.shields.io/crates/v/pydocstring?color=FFC12d)](https://crates.io/crates/pydocstring)
[![Crates.io MSRV](https://img.shields.io/crates/msrv/pydocstring?color=FFC12d)](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0)
[![PyPI - Version](https://img.shields.io/pypi/v/pydocstring-rs?color=0062A8)](https://pypi.org/project/pydocstring-rs/)
[![PyPI - Python Version](https://img.shields.io/pypi/pyversions/pydocstring-rs?color=0062A8)](https://devguide.python.org/versions/)

A zero-dependency Rust parser for Python docstrings (Google / NumPy style).

Produces a **unified syntax tree** with **byte-precise source locations** on every token — designed as infrastructure for linters and formatters.

Python bindings are also available as [`pydocstring-rs`](https://pypi.org/project/pydocstring-rs/).

## Features

- **Full syntax tree** — builds a complete AST, not just extracted fields; traverse it with the built-in `Visitor` + `walk`
- **Typed nodes per style** — style-specific accessors like `GoogleArg`, `NumPyParameter` with full type safety
- **Byte-precise source locations** — every token carries its exact byte range for pinpoint diagnostics
- **Zero dependencies** — pure Rust, no external crates, no regex
- **Error-resilient** — never panics; malformed input still yields a best-effort tree
- **Style auto-detection** — hand it a docstring, get back `Style::Google`, `Style::NumPy`, or `Style::Plain`

## Installation

```toml
[dependencies]
pydocstring = "0.2.0"
```

## Usage

### Parsing

```rust
use pydocstring::parse::google::{parse_google, GoogleDocstring, GoogleSectionKind};

let input = "Summary.\n\nArgs:\n    x (int): The value.\n    y (int): Another value.";
let result = parse_google(input);
let doc = GoogleDocstring::cast(result.root()).unwrap();

println!("{}", doc.summary().unwrap().text(result.source()));

for section in doc.sections() {
    if section.section_kind(result.source()) == GoogleSectionKind::Args {
        for arg in section.args(result.source()) {
            println!("{}: {}",
                arg.name().text(result.source()),
                arg.r#type().map(|t| t.text(result.source())).unwrap_or(""));
        }
    }
}
```

NumPy style works the same way — use `parse_numpy` / `NumPyDocstring` instead.

### Style Auto-Detection

```rust
use pydocstring::parse::{detect_style, Style};

assert_eq!(detect_style("Summary.\n\nArgs:\n    x: Desc."), Style::Google);
assert_eq!(detect_style("Summary.\n\nParameters\n----------\nx : int"), Style::NumPy);
assert_eq!(detect_style("Just a summary."), Style::Plain);
```

`Style::Plain` covers docstrings with no recognised section markers: summary-only,
summary + extended summary, and unrecognised styles such as Sphinx.

### Unified Auto-Detecting Parser

Use `parse()` to let the library detect the style and parse in one step:

```rust
use pydocstring::parse::{parse, Style};
use pydocstring::syntax::SyntaxKind;

let result = parse("Summary.\n\nArgs:\n    x: Desc.");
assert_eq!(result.root().kind(), SyntaxKind::DOCUMENT);
assert_eq!(result.style(), Style::Google);

let result = parse("Just a summary.");
assert_eq!(result.style(), Style::Plain);
```

### Source Locations

Every token carries byte offsets for precise diagnostics:

```rust
use pydocstring::parse::google::{parse_google, GoogleDocstring, GoogleSectionKind};

let result = parse_google("Summary.\n\nArgs:\n    x (int): The value.");
let doc = GoogleDocstring::cast(result.root()).unwrap();

for section in doc.sections() {
    if section.section_kind(result.source()) == GoogleSectionKind::Args {
        for arg in section.args(result.source()) {
            let name = arg.name();
            println!("'{}' at byte {}..{}",
                name.text(result.source()), name.range().start(), name.range().end());
        }
    }
}
```

### Syntax Tree

The parse result is a tree of `SyntaxNode` (branches) and `SyntaxToken` (leaves), each tagged with a `SyntaxKind`. Use `pretty_print()` to visualize:

```rust
use pydocstring::parse::google::parse_google;

let result = parse_google("Summary.\n\nArgs:\n    x (int): The value.");
println!("{}", result.pretty_print());
```

```text
DOCUMENT@0..42 {
  SUMMARY: "Summary."@0..8
  SECTION@10..42 {
    SECTION_HEADER@10..15 {
      NAME: "Args"@10..14
      COLON: ":"@14..15
    }
    ENTRY@20..42 {
      NAME: "x"@20..21
      OPEN_BRACKET: "("@22..23
      TYPE: "int"@23..26
      CLOSE_BRACKET: ")"@26..27
      COLON: ":"@27..28
      DESCRIPTION: "The value."@29..39
    }
  }
}
```

### Visitor Pattern

Walk the tree with the `Visitor` trait for style-agnostic analysis:

```rust
use pydocstring::syntax::{Visitor, walk, SyntaxToken, SyntaxKind};
use pydocstring::parse::google::parse_google;

struct NameCollector<'a> {
    source: &'a str,
    names: Vec<String>,
}

impl Visitor for NameCollector<'_> {
    fn visit_token(&mut self, token: &SyntaxToken) {
        if token.kind() == SyntaxKind::NAME {
            self.names.push(token.text(self.source).to_string());
        }
    }
}

let result = parse_google("Summary.\n\nArgs:\n    x: Desc.\n    y: Desc.");
let mut collector = NameCollector { source: result.source(), names: vec![] };
walk(result.root(), &mut collector);
assert_eq!(collector.names, vec!["Args", "x", "y"]);
```

### Style-Independent Model (IR)

Convert any parsed docstring into a style-independent intermediate representation for analysis or transformation:

```rust
use pydocstring::parse::google::{parse_google, to_model::to_model};

let parsed = parse_google("Summary.\n\nArgs:\n    x (int): The value.\n");
let doc = to_model(&parsed).unwrap();

assert_eq!(doc.summary.as_deref(), Some("Summary."));
for section in &doc.sections {
    if let pydocstring::model::Section::Parameters(params) = section {
        assert_eq!(params[0].names, vec!["x"]);
        assert_eq!(params[0].type_annotation.as_deref(), Some("int"));
    }
}
```

### Emitting (Code Generation)

Re-emit a `Docstring` model in any style — useful for style conversion or formatting.
Google, NumPy, and Sphinx (reStructuredText) output are supported:

```rust
use pydocstring::model::{Docstring, Section, Parameter};
use pydocstring::emit::google::emit_google;
use pydocstring::emit::numpy::emit_numpy;
use pydocstring::emit::sphinx::emit_sphinx;

let doc = Docstring {
    summary: Some("Brief summary.".into()),
    sections: vec![Section::Parameters(vec![Parameter {
        names: vec!["x".into()],
        type_annotation: Some("int".into()),
        description: Some("The value.".into()),
        is_optional: false,
        default_value: None,
    }])],
    ..Default::default()
};

let google = emit_google(&doc);
assert!(google.contains("Args:"));

let numpy = emit_numpy(&doc);
assert!(numpy.contains("Parameters\n----------"));

let sphinx = emit_sphinx(&doc);
assert!(sphinx.contains(":param x:"));
assert!(sphinx.contains(":type x: int"));
```

> **Note:** Sphinx support is emit-only. `detect_style` reports Sphinx docstrings
> as `Style::Plain`, so parsing them yields a summary/extended-summary only.

Combine parsing and emitting to convert between styles:

```rust
use pydocstring::parse::google::{parse_google, to_model::to_model};
use pydocstring::emit::numpy::emit_numpy;

let parsed = parse_google("Summary.\n\nArgs:\n    x (int): The value.\n");
let doc = to_model(&parsed).unwrap();
let numpy_text = emit_numpy(&doc);
assert!(numpy_text.contains("Parameters\n----------"));
```

## Supported Sections

Both styles support the following section categories. Typed accessor methods are available on each style's section node.

| Category                          | Google                                   | NumPy                                   |
|-----------------------------------|------------------------------------------|-----------------------------------------|
| Parameters                        | `args()` → `GoogleArg`                   | `parameters()` → `NumPyParameter`       |
| Returns                           | `returns()` → `GoogleReturns`            | `returns()` → `NumPyReturns`            |
| Yields                            | `yields()` → `GoogleYields`              | `yields()` → `NumPyYields`              |
| Raises                            | `exceptions()` → `GoogleException`       | `exceptions()` → `NumPyException`       |
| Warns                             | `warnings()` → `GoogleWarning`           | `warnings()` → `NumPyWarning`           |
| See Also                          | `see_also_items()` → `GoogleSeeAlsoItem` | `see_also_items()` → `NumPySeeAlsoItem` |
| Attributes                        | `attributes()` → `GoogleAttribute`       | `attributes()` → `NumPyAttribute`       |
| Methods                           | `methods()` → `GoogleMethod`             | `methods()` → `NumPyMethod`             |
| Free text (Notes, Examples, etc.) | `body_text()`                            | `body_text()`                           |

Root-level accessors: `summary()`, `extended_summary()` (NumPy also has `deprecation()`). `PlainDocstring` exposes only `summary()` and `extended_summary()`.

## Development

Common tasks are wrapped in a [`justfile`](justfile) — run `just` to list them:

```bash
just            # list all recipes
just lint       # cargo clippy (warnings as errors)
just test       # Rust test suite
just py-test    # build the Python extension (uv + maturin) and run pytest
just ci         # everything CI runs: fmt-check, lint, test, py-test
```

Or invoke cargo directly:

```bash
cargo build
cargo test
cargo run --example parse_auto
cargo run --example parse_google
cargo run --example parse_numpy
```
