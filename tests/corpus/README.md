# Test corpus

Each `.txt` file under `<style>/` is a docstring input, parsed by the harness
in `tests/snapshots.rs` with the parser named by its top-level directory
(`google`, `numpy`, `plain`) and compared byte-for-byte against the sibling
`<name>.snap`.

## Layout

Inputs are grouped by the area of the grammar they exercise, mirroring the
module split of the assertion test suite (`tests/google/args.rs` ↔
`corpus/google/args/`, …):

```
google/  args/  edge_cases/  freetext/  raises/  returns/
         sections/  structured/  summary/  realworld/
numpy/   parameters/  edge_cases/  freetext/  raises/  returns/
         sections/  structured/  summary/  regressions/  realworld/
plain/   (flat — only a handful of inputs)
```

- `sections/` — section-level behavior: header aliases, ordering, unknown
  sections, blank lines between sections.
- `structured/` — entry-style sections other than
  parameters/returns/raises (attributes, methods, see also, warns, yields, …).
- `regressions/` — issue reproducers, named `issue<NN>_<slug>.txt`
  (e.g. `issue26_rst_roles.txt`). Keep the input exactly as reported.
  Create the directory per style when the first reproducer arrives.
- `realworld/` — production docstrings extracted verbatim from published
  packages via `inspect.getdoc` (which dedents, matching corpus
  expectations), named `<pkg>_<qualname>.txt`. Do not edit these inputs;
  they pin parser behavior on real-world shapes, not hand-minimized ones.
  - **License**: the realworld fixtures are verbatim third-party text —
    full copyright notices and license texts in
    [`THIRD_PARTY_NOTICES.md`](./THIRD_PARTY_NOTICES.md).
  - `numpy/realworld/` — NumPy 2.5.1 and SciPy 1.18.0 (both BSD-3-Clause;
    docstring text is included verbatim for testing purposes only, and
    remains © the NumPy/SciPy developers under their licenses).
  - `numpy/realworld/scverse_*` — the scanpy/anndata ecosystem the #26
    reporters maintain, i.e. the production docstrings this library exists
    to process: anndata 0.13.0 and scanpy 1.12.2 (both BSD-3-Clause; same
    verbatim-for-testing terms). Extracted via runtime `getdoc`, so scanpy's
    `docrep` templates are expanded as real callers see them.
  - `google/realworld/` — absl-py 2.5.0 (Apache-2.0) and Python Fire 0.7.1
    (Apache-2.0); same verbatim-for-testing terms.

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
