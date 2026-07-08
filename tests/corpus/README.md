# Test corpus

Each `.txt` file is a docstring input, parsed by the harness in
`tests/snapshots.rs` with the parser for its style (`google`, `numpy`,
`plain`) and compared byte-for-byte against the sibling `<name>.snap`. The
style is the top-level directory for first-party inputs, or the directory
just below the library for `third_party/<lib>/<style>/` inputs.

## Layout

Inputs are grouped by the area of the grammar they exercise, mirroring the
module split of the assertion test suite (`tests/google/args.rs` ↔
`corpus/google/args/`, …):

```
google/  args/  edge_cases/  freetext/  raises/  returns/
         sections/  structured/  summary/
numpy/   parameters/  edge_cases/  freetext/  raises/  returns/
         sections/  structured/  summary/  regressions/
plain/   (flat — only a handful of inputs)
third_party/  <lib>/LICENSE  <lib>/<style>/*.txt
```

- `sections/` — section-level behavior: header aliases, ordering, unknown
  sections, blank lines between sections.
- `structured/` — entry-style sections other than
  parameters/returns/raises (attributes, methods, see also, warns, yields, …).
- `regressions/` — issue reproducers, named `issue<NN>_<slug>.txt`
  (e.g. `issue26_rst_roles.txt`). Keep the input exactly as reported.
  Create the directory per style when the first reproducer arrives.
- `third_party/` — production docstrings extracted verbatim from published
  packages via `inspect.getdoc` (which dedents, matching corpus
  expectations). One directory per source library, each holding a `LICENSE`
  file and a `<style>/` directory of inputs named `<qualname>.txt` (the
  library name lives in the path, not the filename). Do not edit these
  inputs; they pin parser behavior on real-world shapes, not hand-minimized
  ones. Current libraries:
  - `third_party/numpy/numpy/` — NumPy 2.5.1
  - `third_party/scipy/numpy/` — SciPy 1.18.0
  - `third_party/scanpy/numpy/` and `third_party/anndata/numpy/` — scanpy
    1.12.2 and anndata 0.13.0, the scverse ecosystem the #26 reporters
    maintain (i.e. the production docstrings this library exists to
    process); extracted via runtime `getdoc` so scanpy's `docrep` templates
    are expanded as real callers see them.
  - `third_party/absl/google/` and `third_party/fire/google/` — absl-py
    2.5.0 and Python Fire 0.7.1.

## Licensing

First-party corpus fixtures (everything outside `third_party/`) are part of
this project and covered by its MIT license (repository root `LICENSE`).

Everything under `third_party/` is **verbatim upstream docstring text**,
included solely as parser test fixtures and remaining © its respective
copyright holders under the license in the adjacent `<lib>/LICENSE`
(NumPy/SciPy/scanpy/anndata are BSD-3-Clause; absl-py/Fire are Apache-2.0).
These files are **not** published in the crates.io or PyPI packages
(`Cargo.toml` excludes `tests/`; the Python distribution packages only
`bindings/python`).

## Workflow

- **Add a test**: drop a `.txt` file into the directory for its style, then
  bless. Input files are read verbatim — a trailing newline in the file is a
  trailing newline in the docstring.
- **Bless snapshots**: `UPDATE_SNAPSHOTS=1 cargo test --test snapshots`
- CI runs in compare mode and fails on any drift.

## Naming conventions

- Name files after the behavior they pin (`args_multiline_description`,
  `tab_indented_parameters`), so a snapshot diff tells you what changed.
- **Combined families**: variants that only differ in one word are merged
  into a single multi-section input rather than one file each —
  `section_aliases.txt` (every section-header alias; the EMIT half of the
  snapshot records each alias normalizing to its canonical header),
  `section_body_variants.txt`, and `freetext_sections.txt`.
- Don't add an input whose shape is a strict subset of an existing file;
  extend the existing one or pick a shape that pins something new.
