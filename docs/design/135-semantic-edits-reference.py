"""Reference material for #135 (semantic edits) — NOT shipped, NOT imported.

This is scverse-misc's deprecation injection, ported onto the 0.4.1 splice API
and verified through `sphinx.ext.napoleon` on eight cases (Google, NumPy, tabs,
a 6-space continuation indent, an entry with no description, a non-ASCII
neighbour, a blank line before the description, plain).

It is kept because it *is* the specification for #135: every helper below is
work the library should be doing, and each one was derived by hand and checked
against the tree. The target is for all of this to collapse into

    edits.prepend_to_description(entry, notice)

with the same output, byte for byte.

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
    """The indent this description's own continuation lines use.

    A `TextBlock` is a list of lines, so ask the second one where it starts.
    `entry indent + 4` is a guess, and it is wrong for a docstring that
    continues at another depth.
    """
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
    """`text`'s continuation lines pushed under `indent`. The first line is placed
    by the caller, which is why it is left alone."""
    return text.replace("\n", f"\n{indent}")


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
            for dep in (d for d in deprecations if d.arg in names):
                notice = f".. version-deprecated:: {dep.version}\n   {dep.message}"
                prepend_to_description(parsed, edits, entry, notice)

    return edits.apply()


if __name__ == "__main__":
    from sphinx.ext.napoleon import Config, GoogleDocstring, NumpyDocstring

    cfg = Config(napoleon_use_param=True)
    PROSE = (
        "\nReturns\n-------\nDescription with attributes:\n\n"
        ":attr:`~anndata.AnnData.obsm`\n    tSNE coordinates.\n"
    )
    CASES = [
        ("numpy + prose Returns (#26)", NumpyDocstring,
         "S.\n\nParameters\n----------\ncopy : bool\n    Return a copy.\n    More.\n" + PROSE),
        ("google", GoogleDocstring, "S.\n\nArgs:\n    copy (bool): Return a copy.\n        More.\n"),
        ("google, tabs", GoogleDocstring, "S.\n\nArgs:\n\tcopy (bool): Return a copy.\n\t\tMore.\n"),
        ("6-space continuation", GoogleDocstring, "S.\n\nArgs:\n    copy (bool): Return a copy.\n      More.\n"),
        ("no description", GoogleDocstring, "S.\n\nArgs:\n    copy (bool):\n"),
        ("non-ASCII neighbour", GoogleDocstring,
         "S.\n\nArgs:\n    café (str): The café.\n    copy (bool): Return a copy.\n"),
        ("blank line before desc", NumpyDocstring,
         "S.\n\nParameters\n----------\ncopy : bool\n\n    Return a copy.\n"),
        ("plain", GoogleDocstring, "Just a summary.\n"),
    ]
    deps = [Deprecation("copy", "1.10.0", "Use `inplace`.")]
    failures = 0
    for label, renderer, src in CASES:
        out = inject(src, deps)
        rendered = str(renderer(out, cfg))
        marker, well_formed = None, True
        for line in out.splitlines():
            if ".. version-deprecated" in line:
                marker = line.find("..")
            elif "Use `inplace`." in line and marker is not None:
                well_formed &= (len(line) - len(line.lstrip())) > marker
        ok = well_formed and (
            ".. version-deprecated:: 1.10.0" in rendered if "copy" in src else True
        )
        failures += not ok
        print(f"{'OK  ' if ok else 'FAIL'} {label}")
    print("\nPASS" if not failures else f"\n{failures} FAILED")
