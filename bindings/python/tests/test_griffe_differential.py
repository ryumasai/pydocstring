"""Differential parity against ``griffe`` (the mkdocstrings docstring parser).

The second reference implementation (#112), alongside the napoleon
differential. Where napoleon is a one-way *renderer* (docstring -> rST field
lists), griffe is a *parser* (docstring -> structured sections) — the closer
analogue of our ``to_model()`` — so structure napoleon flattens into prose is
directly comparable here. This suite re-extracts every corpus input with our
parser and with griffe, projects both onto a shared, whitespace-normalized
*role map*, and asserts they agree. Deliberate divergences are listed in
``KNOWN_GRIFFE_DIVERGENCES`` with a one-line reason; the allowlist is
*stale-detecting*: a listed file that starts matching again fails the suite so
the entry gets removed. Any new, unlisted divergence fails with a diff.

The allowlist check is deliberately shape-agnostic — a listed file passes on
*any* divergence, not just the documented one (same contract as the napoleon
differential). Pinning each entry's exact diff would turn every griffe upgrade
and every parser improvement into 48 snapshot updates; the corpus snapshots
and spec tests already pin our own behavior precisely.

Scope
-----
Roles both sides express structurally: parameters, returns, raises and
attributes. griffe has no keyword-arguments concept (google ``Keyword Args``
becomes its "other parameters" section), so keyword parameters are folded into
the parameters role on BOTH sides. Yields, Warns, See Also, Notes, Examples
and References are out of scope (griffe renders most of them as text or
admonitions).

Parser options
--------------
``returns_named_value=False`` for google, so ``bool: Success.`` reads ``bool``
as the type — the numpydoc/napoleon reading, and ours. The numpy parser has no
such option and already agrees.

Normalization (so divergences are semantic, never formatting)
-------------------------------------------------------------
* Descriptions are whitespace-collapsed and stripped; ``None``/``""`` unify.
* Types are stripped; a trailing ``optional`` / ``default ...`` clause is
  removed, then the outer braces of a numpydoc enum type (``{'C', 'F'}``) are
  dropped — griffe strips them, we keep them verbatim; the brace is notation,
  not semantics.
* Multi-name groups are expanded to one row per name on both sides for
  parameters (griffe pre-splits numpy ``y, z : float`` but keeps google
  ``x1, x2`` as one entry) and kept as a tuple for attributes.
"""

import logging
import re
from pathlib import Path

import pytest

pytest.importorskip("griffe")

import griffe  # noqa: E402

import pydocstring  # noqa: E402


@pytest.fixture(autouse=True)
def _quiet_griffe():
    """Silence griffe's per-oddity warnings (missing types, unknown sections)
    for the duration of each test, restoring the logger level afterwards so
    the override never leaks into unrelated tests."""
    logger = logging.getLogger("griffe")
    previous = logger.level
    logger.setLevel(logging.CRITICAL)
    yield
    logger.setLevel(previous)


CORPUS = Path(__file__).resolve().parents[3] / "tests" / "corpus"

# ``bool: Success.`` reads bool as the TYPE (numpydoc/napoleon/us), not a name.
GOOGLE_OPTIONS = {"returns_named_value": False}
NUMPY_OPTIONS: dict = {}

PARSERS = {
    "google": (pydocstring.parse_google, GOOGLE_OPTIONS),
    "numpy": (pydocstring.parse_numpy, NUMPY_OPTIONS),
}

# ── Deliberate, documented divergences (path relative to tests/corpus) ─────────
#
# A listed file is *allowed* to diverge from griffe; the suite still asserts
# that it DOES diverge, so a fix or a griffe upgrade that removes the
# divergence is flagged (remove the entry then). None of these is a bug in our
# parser — each is a documented behavioral choice or a griffe limitation.
_BRACKET = "our extended type-bracket forms; griffe reads the brackets as part of the name"
_OPTIONAL_PARENS = "we read a lone '(optional)' as optionality; griffe keeps it as the type"
_MULTI_RETURN = (
    "griffe parses each 'type: desc' line of a google Returns body as a separate "
    "return; for us (and napoleon) the second line is a continuation of the one entry"
)
_MORE_ALIASES = "we accept more section-header aliases than griffe; unrecognized headers become prose for griffe"
_BLANKLESS_HEADER = (
    "griffe requires a blank line before a google section header (and reads a "
    "document-leading header as summary text); we recover the glued section"
)
_GOOGLE_IN_NUMPY = "we parse a google-style 'name (type): desc' entry in a NumPy section; griffe keeps only the name"
_NOSPACE_COLON = "numpydoc's separator is ' : '; we also recover 'x: int' / 'x :int' (#31), griffe drops the type"
_RAISES_COLON = "issue #26 colon rules: we strip/split the exception's trailing colon; griffe keeps it in the name"
_ISSUE26_ROLES = (
    "issue #26: our prose intro is a model Paragraph (excluded) and role colons are "
    "kept; griffe emits the prose as a return and eats the role's leading colon"
)
_NUMPY_KWARGS = "griffe's numpy parser has no Keyword Arguments section; the whole section is lost"
_ALIAS = (
    "'Params' header: recognized by our default alias set (and scverse's napoleon "
    "config); griffe does not know the alias, so the section is lost"
)
_FIRE_NOCOLON = "malformed 'Returns' header (no colon, no blank line): we recover it; griffe reads prose"
_NDARRAY_PROSE = "bare prose line in Parameters: we follow napoleon (a type-less parameter); griffe drops the line"
_ELLIPSIS = "the 'in1, in2, ... : array_like' name list: griffe loses the type and the '...' name"
_COMPOUND_BRACE = "compound '{...}, array_like' type: griffe keeps only the braced enum, dropping the trailing type"
_NONIDENT_RETURN = (
    "griffe splits a 'name : type' return only when the name is identifier-like; "
    "'sub-arrays' / 'p, q' / 'b, a' stay unsplit in the annotation"
)
_PROSE_RETURNS = (
    "prose in Returns: our model carries it as Paragraph blocks (excluded from the "
    "entries-only returns role); griffe emits one type-only return per prose line, "
    "and keeps '`name`' : type' definition-list items unsplit (non-identifier terms)"
)

KNOWN_GRIFFE_DIVERGENCES: dict[str, str] = {
    # #147 napoleon-strict headers: an unregistered underlined name is not a
    # header, so its lines are absorbed into the preceding section — exactly
    # napoleon's reading (that differential now matches here). griffe instead
    # drops the lines entirely; the two oracles disagree, and napoleon is the
    # spec.
    "numpy/structured/unknown_section_with_known_sections.txt": (
        "absorbed unknown-section lines: napoleon leaks them into :param: fields (we match); griffe drops them"
    ),
    # ── our richer type brackets ─────────────────────────────────────────────
    "google/args/args_angle_bracket_type.txt": _BRACKET,
    "google/args/args_curly_bracket_type.txt": _BRACKET,
    "google/args/args_square_bracket_complex_type.txt": _BRACKET,
    "google/args/args_square_bracket_optional.txt": _BRACKET,
    "google/args/args_square_bracket_type.txt": _BRACKET,
    # ── our optional detection ───────────────────────────────────────────────
    "google/args/optional_only_in_parens.txt": _OPTIONAL_PARENS,
    # ── google Returns continuation lines ────────────────────────────────────
    "google/returns/returns_multiple_lines.txt": _MULTI_RETURN,
    # ── our wider section-header alias set ───────────────────────────────────
    "google/sections/section_aliases.txt": _MORE_ALIASES,
    "numpy/sections/section_aliases.txt": _MORE_ALIASES,
    "third_party/anndata/numpy/concat.txt": _ALIAS,
    "third_party/anndata/numpy/to_df.txt": _ALIAS,
    "third_party/anndata/numpy/obs_vector.txt": "'Params' header (see _ALIAS) plus " + _PROSE_RETURNS,
    # ── griffe needs blank-line-separated google headers ─────────────────────
    "google/summary/multiline_summary_then_section.txt": _BLANKLESS_HEADER,
    "google/summary/section_only_no_summary.txt": _BLANKLESS_HEADER,
    "third_party/absl/google/flags_validator.txt": _BLANKLESS_HEADER,
    "third_party/fire/google/decorators_setparsefns.txt": _BLANKLESS_HEADER,
    "third_party/fire/google/fire.txt": _BLANKLESS_HEADER,
    "third_party/fire/google/completion_membervisible.txt": _FIRE_NOCOLON,
    # ── google-style "name (type): desc" entries inside NumPy sections ───────
    "numpy/parameters/google_style_entry_complex_type.txt": _GOOGLE_IN_NUMPY,
    "numpy/parameters/google_style_entry_in_numpy_section.txt": _GOOGLE_IN_NUMPY,
    "numpy/parameters/google_style_entry_no_colon_after_bracket.txt": _GOOGLE_IN_NUMPY,
    "numpy/parameters/google_style_entry_no_description.txt": _GOOGLE_IN_NUMPY,
    "numpy/parameters/google_style_entry_with_continuation.txt": _GOOGLE_IN_NUMPY,
    "numpy/parameters/google_style_entry_with_optional.txt": _GOOGLE_IN_NUMPY,
    "numpy/parameters/google_style_mixed_with_numpy_style.txt": _GOOGLE_IN_NUMPY,
    # ── colon-spacing recovery (#31) ─────────────────────────────────────────
    "numpy/parameters/parameters_no_space_after_colon.txt": _NOSPACE_COLON,
    "numpy/parameters/parameters_no_space_before_colon.txt": _NOSPACE_COLON,
    "numpy/parameters/parameters_no_spaces_around_colon.txt": _NOSPACE_COLON,
    # ── issue #26 colon handling ─────────────────────────────────────────────
    "numpy/raises/raises_colon_description_on_next_line.txt": _RAISES_COLON,
    "numpy/raises/raises_colon_with_continuation.txt": _RAISES_COLON,
    "numpy/regressions/issue26_rst_roles.txt": _ISSUE26_ROLES,
    # ── griffe section/table limitations ─────────────────────────────────────
    "numpy/sections/keyword_parameters_and_admonitions.txt": _NUMPY_KWARGS,
    "third_party/numpy/numpy/broadcast.txt": _ELLIPSIS,
    "third_party/numpy/numpy/linalg_solve.txt": _COMPOUND_BRACE,
    "third_party/numpy/numpy/ndarray.txt": _NDARRAY_PROSE,
    # ── non-identifier return names ──────────────────────────────────────────
    "third_party/numpy/numpy/split.txt": _NONIDENT_RETURN,
    "third_party/scipy/numpy/interpolate_pade.txt": _NONIDENT_RETURN,
    "third_party/scipy/numpy/signal_butter.txt": _NONIDENT_RETURN,
    # ── NumPy anonymous-return prose (see the napoleon _ANON_RETURN family) ──
    "third_party/scanpy/numpy/filter_cells.txt": _PROSE_RETURNS,
    "third_party/scanpy/numpy/leiden.txt": _PROSE_RETURNS,
    "third_party/scanpy/numpy/neighbors.txt": _PROSE_RETURNS,
    "third_party/scanpy/numpy/normalize_total.txt": _PROSE_RETURNS,
    "third_party/scanpy/numpy/pca.txt": _PROSE_RETURNS,
    "third_party/scanpy/numpy/rank_genes_groups.txt": _PROSE_RETURNS,
    "third_party/scanpy/numpy/regress_out.txt": _PROSE_RETURNS,
    "third_party/scanpy/numpy/score_genes_cell_cycle.txt": _PROSE_RETURNS,
    "third_party/scanpy/numpy/tsne.txt": _PROSE_RETURNS,
    "third_party/scanpy/numpy/umap.txt": _PROSE_RETURNS,
}

# ── Normalization helpers ─────────────────────────────────────────────────────


def _collapse(text: str | None) -> str:
    """Whitespace-collapse and strip; ``None`` unifies with ``""``."""
    return " ".join(text.split()).strip() if text else ""


# A trailing ", optional" or ", default ..." marker clause. ``default``
# consumes to the end of the string so a comma-containing default value
# ("tuple[int, str], default (1, 2)") is removed whole, not comma-split.
_TRAILING_MARKER_RE = re.compile(r",\s*(?:optional|default\b.*)\s*$", re.IGNORECASE | re.DOTALL)


def _norm_type(type_str) -> str | None:
    """Strip; drop trailing ``optional``/``default``; unbrace enum types."""
    if type_str is None:
        return None
    type_str = str(type_str).strip()
    if not type_str:
        return None
    # Applied repeatedly: "int, optional, default 1" needs two passes (the
    # ``default`` alternative eats to end-of-string, exposing ", optional").
    previous = None
    while previous != type_str:
        previous = type_str
        type_str = _TRAILING_MARKER_RE.sub("", type_str).strip()
    # numpydoc enum braces are notation, not semantics: griffe strips the
    # outer {} of "{'C', 'F'}"; unify on the stripped form. (After the
    # marker strip, so "{...}, default 'C'" normalizes too.)
    if type_str.startswith("{") and type_str.endswith("}"):
        type_str = type_str[1:-1].strip()
    return type_str or None


def _split_names(target: str) -> tuple[str, ...]:
    return tuple(part.strip() for part in target.split(",") if part.strip())


# ── Our side: project to_model() onto the role map ────────────────────────────

# griffe has no keyword-arguments concept (google "Keyword Args" becomes its
# "other parameters" section), so keyword parameters fold into the parameters
# role on both sides, alongside Receives / Other Parameters.
_PARAM_KINDS = {
    pydocstring.SectionKind.PARAMETERS,
    pydocstring.SectionKind.RECEIVES,
    pydocstring.SectionKind.OTHER_PARAMETERS,
    pydocstring.SectionKind.KEYWORD_PARAMETERS,
}


def _entries(section: pydocstring.model.Section, cls: type) -> list:
    """The `.value` of every block in `section` of the given `Block` variant.

    Prose `Block.Paragraph`s are ignored — the role map compares typed entries
    only (a section-intro paragraph is not a return/param/etc.).
    """
    # ``cls`` is a dynamic Block variant, so the checker can't narrow the type.
    return [block.value for block in section.blocks if isinstance(block, cls)]  # ty: ignore[unresolved-attribute]


def our_role_map(model: pydocstring.model.Docstring) -> dict[str, list[tuple]]:
    role_map: dict[str, list[tuple]] = {
        "parameters": [],
        "attributes": [],
        "raises": [],
        "returns": [],
    }
    for section in model.sections:
        kind = section.kind
        parameters = _entries(section, pydocstring.model.Block.Parameter)
        attributes = _entries(section, pydocstring.model.Block.Attribute)
        exceptions = _entries(section, pydocstring.model.Block.Exception)
        returns = _entries(section, pydocstring.model.Block.Return)
        if kind in _PARAM_KINDS and parameters:
            for prm in parameters:
                for name in prm.names:  # expand multi-name groups
                    role_map["parameters"].append(
                        ((name,), _norm_type(prm.type_annotation), _collapse(prm.description))
                    )
        elif kind == pydocstring.SectionKind.ATTRIBUTES and attributes:
            for attr in attributes:  # keep multi-name groups as a tuple
                role_map["attributes"].append(
                    (tuple(attr.names), _norm_type(attr.type_annotation), _collapse(attr.description))
                )
        elif kind == pydocstring.SectionKind.RAISES and exceptions:
            for exc in exceptions:
                role_map["raises"].append(((exc.type_name,), None, _collapse(exc.description)))
        elif kind == pydocstring.SectionKind.RETURNS and returns:
            for ret in returns:
                role_map["returns"].append(
                    ((ret.name,) if ret.name else (), _norm_type(ret.type_annotation), _collapse(ret.description))
                )
    return {k: v for k, v in role_map.items() if v}


# ── Reference side: project griffe's DocstringSections onto the role map ──────


def griffe_role_map(sections) -> dict[str, list[tuple]]:
    role_map: dict[str, list[tuple]] = {
        "parameters": [],
        "attributes": [],
        "raises": [],
        "returns": [],
    }
    for section in sections:
        kind = section.kind.value
        if kind in ("parameters", "other parameters", "receives"):
            for p in section.value:
                # griffe keeps google multi-name groups ("x1, x2") as one
                # entry; expand per name like ours (numpy is pre-split).
                for name in _split_names(p.name) or ("",):
                    role_map["parameters"].append(((name,), _norm_type(p.annotation), _collapse(p.description)))
        elif kind == "attributes":
            for a in section.value:
                role_map["attributes"].append(
                    (_split_names(a.name), _norm_type(a.annotation), _collapse(a.description))
                )
        elif kind == "raises":
            for r in section.value:
                role_map["raises"].append(
                    ((str(r.annotation) if r.annotation else "",), None, _collapse(r.description))
                )
        elif kind == "returns":
            for r in section.value:
                name = r.name or None
                role_map["returns"].append(
                    ((name,) if name else (), _norm_type(r.annotation), _collapse(r.description))
                )
    return {k: v for k, v in role_map.items() if v}


# ── Corpus walking (mirrors the napoleon differential) ────────────────────────


def corpus_cases():
    cases = []
    rels: set[str] = set()
    for txt in sorted(CORPUS.rglob("*.txt")):
        relative = txt.relative_to(CORPUS)
        style = relative.parts[2] if relative.parts[0] == "third_party" else relative.parts[0]
        if style not in PARSERS:  # griffe only speaks google/numpy here; skip plain
            continue
        rel = relative.as_posix()  # allowlist keys use "/" on every platform
        rels.add(rel)
        cases.append(pytest.param(txt, style, rel, id=rel))
    assert cases, f"no google/numpy corpus inputs found under {CORPUS}"
    # A deleted/renamed fixture must not leave a silently-dead allowlist row.
    orphaned = set(KNOWN_GRIFFE_DIVERGENCES) - rels
    assert not orphaned, f"KNOWN_GRIFFE_DIVERGENCES entries without a corpus file: {sorted(orphaned)}"
    return cases


def _diff(ours: dict, theirs: dict) -> str:
    lines = []
    for role in sorted(set(ours) | set(theirs)):
        o, t = ours.get(role, []), theirs.get(role, [])
        if o != t:
            lines.append(f"  [{role}]")
            lines.append(f"    ours  : {o}")
            lines.append(f"    griffe: {t}")
    return "\n".join(lines)


@pytest.mark.parametrize(("txt", "style", "rel"), corpus_cases())
def test_griffe_differential(txt: Path, style: str, rel: str) -> None:
    text = txt.read_text()
    our_parse, griffe_options = PARSERS[style]

    ours = our_role_map(our_parse(text).to_model())
    # ``style`` is a plain str; griffe's overloads want a Parser literal.
    theirs = griffe_role_map(griffe.Docstring(text).parse(style, **griffe_options))  # ty: ignore[invalid-argument-type]

    if rel in KNOWN_GRIFFE_DIVERGENCES:
        # Stale-detection: a listed file must STILL diverge. If it now matches,
        # the divergence was fixed (here or in griffe) — drop the entry.
        assert ours != theirs, (
            f"{rel} is in KNOWN_GRIFFE_DIVERGENCES but now MATCHES griffe.\n"
            f"Remove it from the allowlist (reason was: "
            f"{KNOWN_GRIFFE_DIVERGENCES[rel]!r})."
        )
        return

    assert ours == theirs, (
        f"{rel} diverges from griffe and is not allowlisted.\n"
        f"Triage it: fix the parser (or file a bug) if it's a genuine defect, "
        f"or add it to KNOWN_GRIFFE_DIVERGENCES with a reason if the difference "
        f"is deliberate.\n{_diff(ours, theirs)}"
    )
