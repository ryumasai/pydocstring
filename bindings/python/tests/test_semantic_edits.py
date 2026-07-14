"""Semantic edits (#135): ``set_description`` / ``prepend_to_description`` / ``set_type``.

Three suites, in the order the design was derived:

1. **The shape matrix** — the same table the Rust suite asserts, because the
   grammar is the point: `x (int):` keeps a zero-length ``DESCRIPTION``
   placeholder to anchor on, while a bare `x` and NumPy's `x : int` have no
   description node at all, and it is the library's job to know where one goes.
2. **The port** — scverse-misc's deprecation injection (#115), which is the
   concrete demand for this layer, checked byte-for-byte against the hand-rolled
   version in ``docs/design/135-semantic-edits-reference.py``. That file is the
   spec these methods were derived from; importing it here is what keeps the two
   from drifting apart, and what makes the claim "the helpers collapse into one
   call, with the same output" a test rather than a promise.
3. **napoleon** — the same eight cases rendered through ``sphinx.ext.napoleon``,
   because a docstring edit that no renderer accepts is not an edit.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from typing import TypeVar

import pytest
from sphinx.ext.napoleon import Config
from sphinx.ext.napoleon import GoogleDocstring
from sphinx.ext.napoleon import NumpyDocstring

import pydocstring as pd

_T = TypeVar("_T")


def present(value: _T | None) -> _T:
    """Assert that an optional view accessor is present, and return it."""
    assert value is not None
    return value


# ── The design reference, imported as the oracle ─────────────────────────────

REFERENCE = Path(__file__).parents[3] / "docs" / "design" / "135-semantic-edits-reference.py"


def _load_reference():
    spec = present(importlib.util.spec_from_file_location("semantic_edits_reference", REFERENCE))
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    present(spec.loader).exec_module(module)
    return module


reference = _load_reference()

GOOGLE = "Summary.\n\nArgs:\n"
NUMPY = "Summary.\n\nParameters\n----------\n"


def first_entry(parsed: pd.Parsed) -> pd.Entry:
    return pd.Document(parsed).sections[0].entries[0]


# ── 1. The shape matrix ──────────────────────────────────────────────────────


class TestSetDescription:
    @pytest.mark.parametrize(
        ("head", "entry", "expected"),
        [
            # Google: an existing description is replaced where it stands.
            (GOOGLE, "    x (int): Old.\n", "    x (int): New.\n"),
            (GOOGLE, "    x: Old.\n", "    x: New.\n"),
            # A zero-length DESCRIPTION placeholder is the anchor.
            (GOOGLE, "    x (int):\n", "    x (int): New.\n"),
            (GOOGLE, "    x:\n", "    x: New.\n"),
            # Whitespace left after the colon is absorbed, not doubled.
            (GOOGLE, "    x (int): \n", "    x (int): New.\n"),
            # No description node at all: the colon is written too, if absent.
            (GOOGLE, "    x\n", "    x: New.\n"),
            (GOOGLE, "    x (int)\n", "    x (int): New.\n"),
            # NumPy: a description is always its own line.
            (NUMPY, "x : int\n    Old.\n", "x : int\n    New.\n"),
            (NUMPY, "x : int\n", "x : int\n    New.\n"),
            (NUMPY, "x\n", "x\n    New.\n"),
        ],
    )
    def test_every_entry_shape(self, head: str, entry: str, expected: str):
        parsed = pd.parse(head + entry)
        edits = parsed.edit()
        edits.set_description(first_entry(parsed), "New.")
        assert edits.apply() == head + expected

    def test_multiline_text_always_gets_its_own_line(self):
        # Spliced inline, a block's second line lands *shallower than its first*
        # — malformed rST that only survives napoleon's field-body dedent.
        parsed = pd.parse(f"{GOOGLE}    x (int): Old.\n")
        edits = parsed.edit()
        edits.set_description(first_entry(parsed), ".. note::\n   Careful.")
        assert edits.apply() == f"{GOOGLE}    x (int):\n        .. note::\n           Careful.\n"

    def test_single_line_keeps_the_entry_shape(self):
        # A description written on its own line stays on its own line.
        src = f"{GOOGLE}    x (int):\n        Old.\n"
        parsed = pd.parse(src)
        edits = parsed.edit()
        edits.set_description(first_entry(parsed), "New.")
        assert edits.apply() == f"{GOOGLE}    x (int):\n        New.\n"

    def test_continuation_indent_is_read_not_computed(self):
        # `entry indent + 4` is a guess, and it is wrong for a docstring that
        # continues at another depth. The block's second line is the evidence.
        src = f"{GOOGLE}    x (int): Old.\n      Continued at six.\n"
        parsed = pd.parse(src)
        edits = parsed.edit()
        edits.set_description(first_entry(parsed), "A.\nB.")
        assert edits.apply() == f"{GOOGLE}    x (int):\n      A.\n      B.\n"

    def test_tabs_are_copied_not_counted(self):
        src = "Summary.\n\nArgs:\n\tx (int): Old.\n\t\tContinued.\n"
        parsed = pd.parse(src)
        edits = parsed.edit()
        edits.set_description(first_entry(parsed), "A.\nB.")
        assert edits.apply() == "Summary.\n\nArgs:\n\tx (int):\n\t\tA.\n\t\tB.\n"


class TestPrependToDescription:
    def test_keeps_the_description_byte_for_byte(self):
        src = f"{GOOGLE}    x (int): First.\n        Second.\n    y: Untouched.\n"
        parsed = pd.parse(src)
        edits = parsed.edit()
        edits.prepend_to_description(first_entry(parsed), ".. deprecated:: 1.10\n   Use `y`.")
        assert edits.apply() == (
            f"{GOOGLE}    x (int):\n        .. deprecated:: 1.10\n           Use `y`.\n\n"
            "        First.\n        Second.\n    y: Untouched.\n"
        )

    def test_an_authors_blank_line_survives(self):
        # NEWLINE and BLANK_LINE are distinct kinds, which is what makes "eat one
        # line break" a rule and not a guess.
        parsed = pd.parse(f"{NUMPY}x : int\n\n    Old.\n")
        edits = parsed.edit()
        edits.prepend_to_description(first_entry(parsed), "Note.")
        assert edits.apply() == f"{NUMPY}x : int\n\n    Note.\n\n    Old.\n"

    def test_without_a_description_writes_one(self):
        parsed = pd.parse(f"{GOOGLE}    x (int):\n")
        edits = parsed.edit()
        edits.prepend_to_description(first_entry(parsed), "Note.")
        assert edits.apply() == f"{GOOGLE}    x (int): Note.\n"


class TestSetType:
    @pytest.mark.parametrize(
        ("head", "entry", "expected"),
        [
            # Present, and the zero-length placeholder of `x ():` — both anchors.
            (GOOGLE, "    x (str): D.\n", "    x (int): D.\n"),
            (GOOGLE, "    x (): D.\n", "    x (int): D.\n"),
            # No marker at all: the brackets are written too.
            (GOOGLE, "    x: D.\n", "    x (int): D.\n"),
            (GOOGLE, "    x\n", "    x (int)\n"),
            # NumPy: `x : int`, and its placeholder sits flush against the colon.
            (NUMPY, "x : str\n    D.\n", "x : int\n    D.\n"),
            (NUMPY, "x :\n    D.\n", "x : int\n    D.\n"),
            (NUMPY, "x\n    D.\n", "x : int\n    D.\n"),
        ],
    )
    def test_every_entry_shape(self, head: str, entry: str, expected: str):
        parsed = pd.parse(head + entry)
        edits = parsed.edit()
        edits.set_type(first_entry(parsed), "int")
        assert edits.apply() == head + expected

    @pytest.mark.parametrize(
        ("head", "entry", "expected"),
        [
            (GOOGLE, "    x, y\n", "    x, y (int)\n"),
            (NUMPY, "x, y\n    D.\n", "x, y : int\n    D.\n"),
        ],
    )
    def test_annotates_all_of_an_entrys_names(self, head: str, entry: str, expected: str):
        # An entry can declare several comma-separated names, and the type
        # annotates all of them — so it is written after the *last* one.
        parsed = pd.parse(head + entry)
        edits = parsed.edit()
        edits.set_type(first_entry(parsed), "int")
        assert edits.apply() == head + expected

    def test_on_an_entry_that_is_all_description(self):
        # A Google `Returns:` entry carries no name — the type goes in front.
        parsed = pd.parse("Summary.\n\nReturns:\n    The value.\n")
        edits = parsed.edit()
        edits.set_type(first_entry(parsed), "int")
        assert edits.apply() == "Summary.\n\nReturns:\n    int: The value.\n"


class TestComposition:
    def test_semantic_edits_are_splices(self):
        """Same ``apply()``, same overlap detection — they compose with the rest."""
        src = f"{GOOGLE}    x: Old.\n    y (str): Keep.\n"
        parsed = pd.parse(src)
        entries = pd.Document(parsed).sections[0].entries
        edits = parsed.edit()
        edits.set_type(entries[0], "int")
        edits.set_description(entries[0], "New.")
        edits.replace(present(entries[1].description).range, "Also new.")
        assert edits.apply() == f"{GOOGLE}    x (int): New.\n    y (str): Also new.\n"

    def test_two_edits_on_one_description_overlap(self):
        parsed = pd.parse(f"{GOOGLE}    x (int): Old.\n")
        entry = first_entry(parsed)
        edits = parsed.edit()
        edits.set_description(entry, "One.")
        edits.prepend_to_description(entry, "Two.")
        with pytest.raises(pd.EditError):
            edits.apply()

    def test_an_entry_from_another_parse_is_rejected(self):
        """A path replayed against another tree would index into the wrong
        children — and a panic across the FFI boundary is an abort."""
        parsed = pd.parse(f"{GOOGLE}    x (int): Old.\n")
        other = pd.parse(f"{GOOGLE}    x (int): Old.\n")
        edits = parsed.edit()
        with pytest.raises(ValueError, match="different Parsed"):
            edits.set_description(first_entry(other), "New.")


# ── 2 & 3. The port, against the reference and against napoleon ──────────────

PROSE = "\nReturns\n-------\nDescription with attributes:\n\n:attr:`~anndata.AnnData.obsm`\n    tSNE coordinates.\n"
CASES = [
    (
        "numpy + prose Returns (#26)",
        NumpyDocstring,
        "S.\n\nParameters\n----------\ncopy : bool\n    Return a copy.\n    More.\n" + PROSE,
    ),
    ("google", GoogleDocstring, "S.\n\nArgs:\n    copy (bool): Return a copy.\n        More.\n"),
    ("google, tabs", GoogleDocstring, "S.\n\nArgs:\n\tcopy (bool): Return a copy.\n\t\tMore.\n"),
    ("6-space continuation", GoogleDocstring, "S.\n\nArgs:\n    copy (bool): Return a copy.\n      More.\n"),
    ("no description", GoogleDocstring, "S.\n\nArgs:\n    copy (bool):\n"),
    (
        "non-ASCII neighbour",
        GoogleDocstring,
        "S.\n\nArgs:\n    café (str): The café.\n    copy (bool): Return a copy.\n",
    ),
    (
        "blank line before desc",
        NumpyDocstring,
        "S.\n\nParameters\n----------\ncopy : bool\n\n    Return a copy.\n",
    ),
    ("plain", GoogleDocstring, "Just a summary.\n"),
]

DEPRECATIONS = [reference.Deprecation("copy", "1.10.0", "Use `inplace`.")]


def inject(source: str) -> str:
    """scverse-misc's deprecation injection, on the API #135 shipped.

    The whole of ``docs/design/135-semantic-edits-reference.py`` — the trivia
    walk, the indent arithmetic, the re-indent, the absent-description branch —
    is the one call below.
    """
    parsed = pd.parse(source)
    if parsed.style is pd.Style.PLAIN:
        return source
    doc, edits = pd.Document(parsed), parsed.edit()
    for section in doc.sections:
        if section.kind not in reference.PARAM_KINDS:
            continue
        for entry in section.entries:
            names = [name.text for name in entry.names]
            for dep in (d for d in DEPRECATIONS if d.arg in names):
                notice = f".. version-deprecated:: {dep.version}\n   {dep.message}"
                edits.prepend_to_description(entry, notice)
    return edits.apply()


@pytest.mark.parametrize(("label", "renderer", "src"), CASES, ids=[c[0] for c in CASES])
def test_the_port_matches_the_hand_rolled_reference_byte_for_byte(label: str, renderer, src: str):
    assert inject(src) == reference.inject(src, DEPRECATIONS)


@pytest.mark.parametrize(("label", "renderer", "src"), CASES, ids=[c[0] for c in CASES])
def test_the_port_renders_through_napoleon(label: str, renderer, src: str):
    out = inject(src)
    rendered = str(renderer(out, Config(napoleon_use_param=True)))
    if "copy" in src:
        assert ".. version-deprecated:: 1.10.0" in rendered

    # The directive's body must be indented *deeper than its own marker*. This is
    # the reason the description is widened onto its own line at all: spliced
    # inline after `copy (bool): `, the marker sits at a column nobody chose and
    # the body lands shallower than it — rST that only napoleon's dedent rescues.
    marker_col = None
    for line in out.splitlines():
        if ".. version-deprecated" in line:
            marker_col = line.index("..")
        elif "Use `inplace`." in line and marker_col is not None:
            assert len(line) - len(line.lstrip()) > marker_col
