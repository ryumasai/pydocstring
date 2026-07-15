"""The README's deprecation-injection recipe, tested against its oracle.

``docs/design/135-semantic-edits-reference.py`` is the runnable specification
for #135: scverse-misc's deprecation injection ported onto the 0.4.1 splice
API and verified through ``sphinx.ext.napoleon``. The semantic-edit API it
originally specified was implemented and withdrawn (#140) — those operations
mix grammar facts, style rendering, and layout taste, which belong to three
different homes — so what ships is the RECIPE in ``bindings/python/README.md``.

Three assertions keep the three copies honest:

* the recipe reproduces the reference byte for byte on every case,
* its output renders through napoleon as well-formed rST,
* the README carries this file's recipe verbatim (run the docs, don't read them).
"""

import importlib.util
import sys
from pathlib import Path

import pytest

from pydocstring import Document
from pydocstring import Style
from pydocstring import SyntaxKind
from pydocstring import TextRange
from pydocstring import parse

ROOT = Path(__file__).resolve().parents[3]
README = Path(__file__).resolve().parents[1] / "README.md"


def _load_reference():
    path = ROOT / "docs" / "design" / "135-semantic-edits-reference.py"
    spec = importlib.util.spec_from_file_location("ref135", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules["ref135"] = module
    spec.loader.exec_module(module)
    return module


REFERENCE = _load_reference()


# The recipe, exactly as the README shows it. The --8<-- markers delimit the
# block the README must carry verbatim.


# --8<-- [start:recipe]
def _indented(block: str, indent: str) -> str:
    """`block` with its continuation lines pushed under `indent`.
    The first line is placed by the caller."""
    return block.replace("\n", "\n" + indent)


def prepend_to_description(parsed, edits, entry, block: str) -> None:
    """Queue edits that put `block` (rST, e.g. a directive) in front of
    `entry`'s description, in whichever shape the author wrote it."""
    desc = entry.description
    if desc is None:
        # Nothing to displace: hang the block under the entry.
        indent = parsed.line_indent(entry.range.start) + "    "
        edits.insert(entry.syntax.range.end, "\n" + indent + _indented(block, indent))
        return
    indent = parsed.line_indent(desc.range.start)
    if parsed.line_col(desc.range.start).col == len(indent.encode()):
        # The description starts its own line (the NumPy shape): replace it in
        # place. The bytes before it — the author's newline, blank line,
        # indent — are never part of the edit.
        edits.replace(desc.range, f"{_indented(block, indent)}\n\n{indent}{desc.text}")
        return
    # Inline description (the Google shape). A block after `x (int): ` would
    # start at a column we don't control, so take over the line after the
    # colon. The continuation indent comes from the description's second line
    # if it has one; only a single-line inline description leaves nothing to
    # ask, and there `entry indent + 4` is the convention.
    if len(desc.lines) > 1:
        indent = parsed.line_indent(desc.lines[1].range.start)
    else:
        indent = parsed.line_indent(entry.range.start) + "    "
    colon = entry.syntax.find_token(SyntaxKind.COLON)
    edits.replace(
        TextRange(colon.range.end, desc.range.end),
        f"\n{indent}{_indented(block, indent)}\n\n{indent}{desc.text}",
    )


# --8<-- [end:recipe]


def _inject(source: str, deprecations) -> str:
    """The reference's `inject`, driving the recipe instead of the helpers."""
    parsed = parse(source)
    if parsed.style is Style.PLAIN:
        return source
    doc, edits = Document(parsed), parsed.edit()
    for section in doc.sections:
        if section.kind not in REFERENCE.PARAM_KINDS:
            continue
        for entry in section.entries:
            names = [name.text for name in entry.names]
            for dep in (d for d in deprecations if d.arg in names):
                notice = f".. version-deprecated:: {dep.version}\n   {dep.message}"
                prepend_to_description(parsed, edits, entry, notice)
    return edits.apply()


CASE_IDS = [label for label, _, _ in REFERENCE.CASES]


@pytest.mark.parametrize(("label", "style", "source"), REFERENCE.CASES, ids=CASE_IDS)
def test_recipe_matches_the_reference_byte_for_byte(label, style, source):
    expected = REFERENCE.inject(source, REFERENCE.DEPRECATIONS)
    assert _inject(source, REFERENCE.DEPRECATIONS) == expected, label


@pytest.mark.parametrize(("label", "style", "source"), REFERENCE.CASES, ids=CASE_IDS)
def test_recipe_output_is_well_formed_through_napoleon(label, style, source):
    pytest.importorskip("sphinx.ext.napoleon")
    out = _inject(source, REFERENCE.DEPRECATIONS)
    assert REFERENCE.napoleon_ok(out, source, style), label


def test_readme_carries_this_exact_recipe():
    here = Path(__file__).read_text()
    recipe = here.split("# --8<-- [start:recipe]\n")[1].split("# --8<-- [end:recipe]")[0]
    assert recipe.strip("\n") in README.read_text(), "the README's recipe block has drifted from the tested code"
