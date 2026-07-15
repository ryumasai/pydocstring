# The editing model — settled lines and rejected alternatives

A design record distilled from the #135 → #140 → recipe arc (2026-07). It
exists so the next reader does not re-derive — or re-litigate — what follows.
The shipped surface this describes: view queries for positions, byte splices
(`Edits`) for changes, reparse for validation, and a documented recipe
(`bindings/python/README.md`) for the one genuinely multi-splice job.

## The three-way split (from the #140 withdrawal)

Three kinds of knowledge, three homes:

- **Tree facts** (where things are, what shape they have) → **view queries**.
  Every position an edit needs is a one-line query on the shipped API.
- **Style notation** (how a construct is *spelled*: `x (int):` vs `x : int`)
  → **emit**. Nothing else may hold it.
- **Layout taste** (inline vs. own line, blank-line aesthetics) → **the
  caller**. The library never decides these, not even via default arguments.

\#140 implemented semantic-edit methods on `Edits` and was withdrawn because
each method collapsed this split: it re-derived tree facts, embedded notation,
and hard-coded taste, all in one place.

## The napoleon razor

**A difference napoleon distinguishes is semantics or notation — the
library's job, testable against the spec. A difference napoleon renders
identically has no oracle — it is taste, and only the caller may hold
taste.** This single criterion generates the whole three-way split, and it is
the line between what the emitters may decide (` : ` spacing — napoleon
distinguishes) and what they may not (single-line vs. own-line description —
napoleon does not).

One refinement: a layout choice is only taste while every option is
well-formed. A payload's own grammar can promote layout to correctness — an
rST *block* (directive, literal block) must own its column, which is why the
recipe's hard branch exists and is not optional.

## Seam vs. interior

For newly generated material, the **interior** gets emit's canonical form —
no author exists inside it, the same contract as whole-document emit. The
**seam** — where new bytes meet the author's — belongs to the caller, and in
practice is mostly *copying* the author rather than inventing: the recipe
takes its indent from the description's own lines and replaces ranges in
place so the author's blank lines are never part of the edit.

## The parse-depth razor

The CST parses only as deep as napoleon assigns meaning. Section bodies:
napoleon interprets their structure (a Returns prose intro becomes the
`:rtype:`), so block structure there is justified and testable. Entry
description *interiors*: napoleon passes them through verbatim, so lines
suffice — parsing rST inside them would be reimplementing docutils with no
oracle to test against.

## Rejected: grafting (Roslyn-style tree transformation) — for good, not deferred

Synthesized-subtree grafting with emit-by-concat was considered as the "v2"
editing model and rejected permanently:

1. It breaks the **absolute-range representation**: after a graft there is no
   source string for ranges to point into; fixing that is a green-tree
   (width-based) rewrite of the core and of everything built on `range`.
2. It breaks the **fixed-point law**: `tree == parse(emit(tree))` cannot be
   guaranteed for constructed trees, because the grammar is line- and
   indent-sensitive — a grafted text line can reparse as a section header. In
   the splice model this is a non-problem: *the parser is the validator*, and
   reparse after apply tells the truth.
3. The payoff is void here. Green trees earn their complexity on megabyte
   files needing incremental reparse; a docstring is tiny, and reparse is
   both free and a validation pass. Roslyn makes grafting workable only by
   pairing it with a formatter — taste in core, the exact thing #140
   withdrew; rust-analyzer's assists emit text edits, not grafts.

`Range | Owned` token text (#42) stays what it is: transient scaffolding
inside the pattern-rewrite engine, never an authoritative tree.

## Rejected: zero-length slots where no marker exists

A missing placeholder is legitimate exactly when the source *created an
obligation* (`x (:` opened a type marker — position determined by the bytes
plus the same grammar rule that recognized them). Placing a slot where
nothing exists would require answering style questions the source never asked
(before or after the colon? with brackets?), and a zero-length token cannot
carry the marker's bytes anyway — the caller would still write ` (int)` by
hand. Same trap, one level down, as the withdrawn `description_slot` query:
it packages a fact the caller can already derive while not providing the one
thing they cannot.

## Dormant, with triggers (see the closed issues for full context)

- **Fragment emit ("the bridge")** — rendering docstring-grammar fragments
  (an entry, a section header, a type marker) in a target style; the missing
  input is quasi-quotation (parse a fragment written in any style → model
  fragment → emit in the target style; the pattern engine already has the
  fragment grammar). Not filed as an issue: today's only consumer injects
  style-free rST payloads and needs none of it. File it when the first
  entry/section/marker-creation request arrives. Out of scope regardless:
  rST payload assembly, seam layout, moving existing bytes.
- **Model-mediated editing** (edit the model, print with provenance) —
  re-evaluate if the model path ever gains a consumer; #104 (closed, not
  planned) is its prerequisite and carries its reopen trigger.
- **A `pydocstring.recipes` policy layer** (pure Python, parity-free,
  explicitly opinionated) — when a second or third consumer starts copying
  the README recipe.
- **Block structure inside entry descriptions** — when a consumer needs to
  *query* structure there (per the parse-depth razor, not before).
