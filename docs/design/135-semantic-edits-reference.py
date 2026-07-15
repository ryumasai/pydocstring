"""Reference material for #135 (semantic edits) — NOT shipped as API.

This is scverse-misc's deprecation injection, ported onto the 0.4.1 splice API
and verified through `sphinx.ext.napoleon` on ten cases (Google, NumPy, tabs,
a 6-space continuation indent, an entry with no description, a non-ASCII
neighbour, a blank line before the description, a single-line description on
its own line at a non-default depth, a multi-name entry with two deprecated
arguments, plain).

History: this file was written as the specification for an
`edits.prepend_to_description(entry, notice)` API. That API was implemented,
reviewed, and *withdrawn* (#140) — the operations mix grammar, style rendering,
and layout taste, which belong to three different homes. What ships instead is
the documented RECIPE in `bindings/python/README.md`, and this file is its
oracle: `tests/test_recipe_deprecation_injection.py` asserts the recipe
reproduces this implementation byte for byte on every case below.

Run it:  cd bindings/python && uv run python ../../docs/design/135-semantic-edits-reference.py
"""

from __future__ import annotations

from dataclasses import dataclass

import pydocstring as pd

PARAM_KINDS = {
    pd.SectionKind.PARAMETERS,
    pd.SectionKind.KEYWORD_PARAMETERS,
    pd.SectionKind.OTHER_PARAMETERS,
}
LINE_BREAKS = {pd.SyntaxKind.NEWLINE, pd.SyntaxKind.BLANK_LINE}


def _body_indent(parsed, entry, block) -> str:
    """The indent this description's body should continue at.

    Ask the source, in order of how directly it answers: the description's own
    first line, if the description starts one; otherwise its second line; only
    a single-line *inline* description leaves nothing to ask, and there
    `entry indent + 4` is the convention. Guessing earlier than that is wrong
    for a docstring that continues at another depth (a 6-space one is in the
    cases below — twice).
    """
    first = block.lines[0].range.start
    own = parsed.line_indent(first)
    if parsed.line_col(first).col == len(own.encode()):
        return own
    if len(block.lines) > 1:
        return parsed.line_indent(block.lines[1].range.start)
    return parsed.line_indent(entry.range.start) + "    "


def _detached(entry, block) -> pd.TextRange:
    """`block`'s range, widened over the trivia separating it from its sibling.

    This is what lets one code path serve both styles. Google writes the
    description inline (`x (int): desc`), NumPy on its own line, and the tree
    says which: the siblings before DESCRIPTION are WHITESPACE in one, NEWLINE +
    WHITESPACE in the other. Eat them, re-emit starting on a fresh line, and the
    two become the same edit.

    At most ONE line break — NEWLINE and BLANK_LINE are distinct kinds, so a
    blank line the author wrote survives.

    Why widen at all, when napoleon renders either shape: a directive spliced
    inline after `x (int): ` sits at a column we do not control, and its body
    then lands *shallower* than its own marker — malformed rST that only
    survives because napoleon dedents the field body before docutils sees it.
    """
    kids = entry.syntax.children
    i = next(n for n, child in enumerate(kids) if child.range == block.range)
    start, took_break = block.range.start, False
    while i > 0:
        prev = kids[i - 1]
        if not (isinstance(prev, pd.Token) and prev.kind.is_trivia()):
            break
        if prev.kind in LINE_BREAKS:
            if took_break:
                break
            took_break = True
        start, i = prev.range.start, i - 1
    return pd.TextRange(start, block.range.end)


def _indented(text: str, indent: str) -> str:
    """`text`'s continuation lines pushed under `indent`; empty lines stay
    empty rather than gaining trailing whitespace. The first line is placed by
    the caller, which is why it is left alone."""
    head, *rest = text.split("\n")
    return "\n".join([head, *(indent + line if line else line for line in rest)])


def prepend_to_description(parsed, edits, entry, text: str) -> None:
    """What `Edits.prepend_to_description(entry, text)` has to do. #135."""
    block = entry.description
    if block is None:
        # No description means no anchor to replace — pick the end of the entry
        # and insert. (Re-deriving placement the parser already knows: this is
        # half of why #135 exists.)
        indent = parsed.line_indent(entry.range.start) + "    "
        edits.insert(entry.syntax.range.end, f"\n{indent}{_indented(text, indent)}")
        return
    indent = _body_indent(parsed, entry, block)
    body = _indented(text, indent)
    # `block.text` already carries the continuation lines' indentation — only the
    # first line lost its own to the range's start — so the description goes back
    # byte-for-byte rather than being re-indented.
    edits.replace(_detached(entry, block), f"\n{indent}{body}\n\n{indent}{block.text}")


# ── The caller, as it would read today ───────────────────────────────────────


@dataclass
class Deprecation:
    arg: str
    version: str
    message: str


def inject(source: str, deprecations: list[Deprecation]) -> str:
    parsed = pd.parse(source)
    if parsed.style is pd.Style.PLAIN:
        return source
    doc, edits = pd.Document(parsed), parsed.edit()

    for section in doc.sections:
        if section.kind not in PARAM_KINDS:
            continue
        for entry in section.entries:
            names = [name.text for name in entry.names]
            notices = [
                f".. version-deprecated:: {dep.version}\n   {dep.message}"
                for dep in deprecations
                if dep.arg in names
            ]
            if notices:
                # One call per entry: a multi-name entry (`copy, deep : bool`)
                # can match several deprecations, and a second call would queue
                # an overlapping replace of the same description — which
                # `apply()` rejects. Joined notices are sibling rST blocks, a
                # blank line apart.
                prepend_to_description(parsed, edits, entry, "\n\n".join(notices))

    return edits.apply()


# ── The nine verification cases ──────────────────────────────────────────────
# Shared with tests/test_recipe_deprecation_injection.py, which uses them (and
# `inject` above) as the oracle for the README recipe. The style tag keeps this
# module importable without sphinx; the napoleon check below maps it.

_PROSE = (
    "\nReturns\n-------\nDescription with attributes:\n\n"
    ":attr:`~anndata.AnnData.obsm`\n    tSNE coordinates.\n"
)
CASES = [
    ("numpy + prose Returns (#26)", "numpy",
     "S.\n\nParameters\n----------\ncopy : bool\n    Return a copy.\n    More.\n" + _PROSE),
    ("google", "google", "S.\n\nArgs:\n    copy (bool): Return a copy.\n        More.\n"),
    ("google, tabs", "google", "S.\n\nArgs:\n\tcopy (bool): Return a copy.\n\t\tMore.\n"),
    ("6-space continuation", "google", "S.\n\nArgs:\n    copy (bool): Return a copy.\n      More.\n"),
    ("no description", "google", "S.\n\nArgs:\n    copy (bool):\n"),
    ("non-ASCII neighbour", "google",
     "S.\n\nArgs:\n    café (str): The café.\n    copy (bool): Return a copy.\n"),
    ("blank line before desc", "numpy",
     "S.\n\nParameters\n----------\ncopy : bool\n\n    Return a copy.\n"),
    ("single-line own-line desc at 6sp", "numpy",
     "S.\n\nParameters\n----------\ncopy : bool\n      Return a copy.\n"),
    ("multi-name entry, two deprecated args", "numpy",
     "S.\n\nParameters\n----------\ncopy, deep : bool\n    Return a copy.\n"),
    ("plain", "google", "Just a summary.\n"),
]
DEPRECATIONS = [
    Deprecation("copy", "1.10.0", "Use `inplace`."),
    Deprecation("deep", "1.11.0", "Never copied anyway."),
]


def napoleon_ok(out: str, src: str, style: str) -> bool:
    """The acceptance check: the directive renders, and its body sits deeper
    than its own marker (well-formed rST, not merely napoleon-survivable)."""
    from sphinx.ext.napoleon import Config, GoogleDocstring, NumpyDocstring

    renderer = {"google": GoogleDocstring, "numpy": NumpyDocstring}[style]
    rendered = str(renderer(out, Config(napoleon_use_param=True)))
    bodies = {dep.message for dep in DEPRECATIONS}
    marker, well_formed = None, True
    for line in out.splitlines():
        if ".. version-deprecated" in line:
            marker = line.find("..")
        elif marker is not None and any(body in line for body in bodies):
            well_formed &= (len(line) - len(line.lstrip())) > marker
    return well_formed and (
        ".. version-deprecated:: 1.10.0" in rendered if "copy" in src else True
    )


if __name__ == "__main__":
    failures = 0
    for label, style, src in CASES:
        ok = napoleon_ok(inject(src, DEPRECATIONS), src, style)
        failures += not ok
        print(f"{'OK  ' if ok else 'FAIL'} {label}")
    print("\nPASS" if not failures else f"\n{failures} FAILED")
