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
         sections/  structured/  summary/
numpy/   parameters/  edge_cases/  freetext/  raises/  returns/
         sections/  structured/  summary/  regressions/
plain/   (flat — only a handful of inputs)
```

- `sections/` — section-level behavior: header aliases, ordering, unknown
  sections, blank lines between sections.
- `structured/` — entry-style sections other than
  parameters/returns/raises (attributes, methods, see also, warns, yields, …).
- `regressions/` — issue reproducers, named `issue<NN>_<slug>.txt`
  (e.g. `issue26_rst_roles.txt`). Keep the input exactly as reported.
  Create the directory per style when the first reproducer arrives.

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
