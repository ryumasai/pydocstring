"""Differential parity against ``sphinx.ext.napoleon``.

napoleon is the reference Google/NumPy docstring implementation. This suite
re-extracts every corpus input with *our* parser (``to_model()``) and with
napoleon, projects both onto a shared, whitespace-normalized *role map*, and
asserts they agree. Where they legitimately disagree — our documented
extensions (issue #26 colon rules, richer type brackets, google-style entries
inside NumPy sections, wider section-header aliases, ``optional`` detection) or
napoleon's own limitations/quirks — the file is listed in
``KNOWN_NAPOLEON_DIVERGENCES`` with a one-line reason. That allowlist is
*stale-detecting*: a listed file that starts matching again fails the suite so
the entry gets removed. Any new, unlisted divergence fails with a diff.

Scope
-----
Only roles both sides express as recoverable reStructuredText fields are
compared: parameters (``:param:``/``:type:``), keyword parameters
(``:keyword:``/``:kwtype:``), returns (``:returns:``/``:rtype:``), raises
(``:raises:``) and attributes (``:ivar:``/``:vartype:``, via
``napoleon_use_ivar``). Sections napoleon renders as prose, rubrics,
admonitions or compound fields — See Also, Notes, Examples, References, Yields,
Warns, Methods — carry no structured field output and are intentionally out of
scope on both sides.

Normalization (so divergences are semantic, never formatting)
-------------------------------------------------------------
* Descriptions are whitespace-collapsed (``" ".join(s.split())``) and stripped;
  ``None`` and ``""`` are unified. This absorbs napoleon's continuation-line
  reflow and paragraph re-wrapping.
* Types are stripped; a trailing ``optional`` / ``default ...`` clause is
  removed (our model carries these as ``is_optional`` / ``default_value``
  out-of-band, napoleon leaves them inside the type string).
* Napoleon escapes RST metacharacters in names (``\\*args``); the backslash
  escapes are removed before comparison.
* Napoleon reflows return entries into ``**name** -- desc`` (single) or a
  ``* **name** (*type*) -- desc`` bullet list (multiple); that rendering is
  parsed back into structured ``(name, type, desc)`` tuples.
* Multi-name entries: napoleon splits parameter groups (``x, y``) into one
  field per name but keeps attribute groups (``jac, hess``) as a single field;
  we mirror that (parameters expanded per name, attributes kept as a tuple).
"""

import re
from pathlib import Path

import pytest

pytest.importorskip("sphinx.ext.napoleon")

from sphinx.ext.napoleon import Config  # noqa: E402
from sphinx.ext.napoleon import GoogleDocstring  # noqa: E402
from sphinx.ext.napoleon import NumpyDocstring  # noqa: E402

import pydocstring  # noqa: E402

CORPUS = Path(__file__).resolve().parents[3] / "tests" / "corpus"

CONFIG = Config(
    napoleon_use_param=True,
    napoleon_use_rtype=True,
    napoleon_use_keyword=True,
    napoleon_use_ivar=True,
)

PARSERS = {
    "google": (pydocstring.parse_google, GoogleDocstring),
    "numpy": (pydocstring.parse_numpy, NumpyDocstring),
}

# ── Deliberate, documented divergences (path relative to tests/corpus) ─────────
#
# A listed file is *allowed* to diverge from napoleon; the suite still asserts
# that it DOES diverge, so a fix or a napoleon upgrade that removes the
# divergence is flagged (remove the entry then). None of these is a bug in our
# parser — each is a documented behavioral choice or a napoleon limitation.
# Shared reason strings (kept short so the table stays under the line limit).
_BRACKET = "we parse this type-bracket form; napoleon reads the brackets as part of the name"
_INDENT = "napoleon needs column-0 section headers; the indented docstring defeats its section detection"
_ALIAS = (
    "'Params' header: recognized by our default alias set, and by scverse's "
    "own napoleon config via napoleon_custom_sections=[('Params','Parameters')] "
    "(this differential runs stock napoleon without that setting, so it diverges "
    "here while scverse's actual render agrees with us)."
)
_GOOGLE_IN_NUMPY = "we parse a google-style 'name (type)' entry in a NumPy section; napoleon reads it as one name"
_ANON_RETURN = (
    "multi-block prose intro in Returns: we now model the prose as PARAGRAPH "
    "blocks (#104/#105), excluded from the entries-only returns role, while "
    "napoleon reflows the prose into a bulleted :returns: description. (A SINGLE "
    "bare line is a type on both sides — verified.) The divergence is the "
    "differential's entries-only projection vs napoleon's prose reflow; the "
    "lossless CST + model preserve the prose either way."
)
_MORE_ALIASES = "we accept more header aliases than napoleon; extra aliased sections are extracted"
_ISSUE26_COLON = "issue #26: we strip the role colon from the description; napoleon keeps it"
_KW_ALIAS = "'Keyword Parameters' is our alias; napoleon knows only 'Keyword Args'"
_ISSUE26_ROLES = "issue #26: we keep rst-role colons; napoleon reflows them to a bullet blob"
_UNKNOWN_SEC = "napoleon leaks unknown-section lines into :param: fields; we do not"
_FIRE_NOCOLON = "malformed 'Returns' header (no colon): we recover it; napoleon needs the colon"

KNOWN_NAPOLEON_DIVERGENCES: dict[str, str] = {
    # ── our richer type brackets: napoleon recognizes only "(type)" ──────────
    "google/args/args_angle_bracket_type.txt": _BRACKET,
    "google/args/args_curly_bracket_type.txt": _BRACKET,
    "google/args/args_square_bracket_type.txt": _BRACKET,
    "google/args/args_square_bracket_complex_type.txt": _BRACKET,
    "google/args/args_square_bracket_optional.txt": _BRACKET,
    # ── our optional detection ───────────────────────────────────────────────
    "google/args/optional_only_in_parens.txt": "we read a lone '(optional)' as optionality; napoleon keeps it as type",
    # ── napoleon cannot parse fully-indented docstrings (our parser dedents) ──
    "google/edge_cases/indented_docstring.txt": _INDENT,
    "numpy/edge_cases/indented_docstring.txt": _INDENT,
    "numpy/edge_cases/deeply_indented_docstring.txt": _INDENT,
    "numpy/edge_cases/indented_with_deprecation.txt": _INDENT,
    "numpy/edge_cases/mixed_indent_first_line.txt": _INDENT,
    # ── our wider section-header alias set ───────────────────────────────────
    "google/sections/section_aliases.txt": _MORE_ALIASES,
    "numpy/sections/section_aliases.txt": _MORE_ALIASES,
    "numpy/sections/keyword_parameters_and_admonitions.txt": _KW_ALIAS,
    "third_party/anndata/numpy/concat.txt": _ALIAS,  # 'Params'
    "third_party/anndata/numpy/to_df.txt": _ALIAS,  # 'Params'
    # ── google-style "name (type): desc" entries inside NumPy sections ───────
    "numpy/parameters/google_style_entry_in_numpy_section.txt": _GOOGLE_IN_NUMPY,
    "numpy/parameters/google_style_entry_complex_type.txt": _GOOGLE_IN_NUMPY,
    "numpy/parameters/google_style_entry_no_colon_after_bracket.txt": _GOOGLE_IN_NUMPY,
    "numpy/parameters/google_style_entry_no_description.txt": _GOOGLE_IN_NUMPY,
    "numpy/parameters/google_style_entry_with_continuation.txt": _GOOGLE_IN_NUMPY,
    "numpy/parameters/google_style_entry_with_optional.txt": _GOOGLE_IN_NUMPY,
    "numpy/parameters/google_style_mixed_with_numpy_style.txt": _GOOGLE_IN_NUMPY,
    # ── issue #26 colon handling ─────────────────────────────────────────────
    "numpy/raises/raises_colon_description_on_next_line.txt": _ISSUE26_COLON,
    "numpy/raises/raises_colon_with_continuation.txt": _ISSUE26_COLON,
    "numpy/regressions/issue26_rst_roles.txt": _ISSUE26_ROLES,
    # ── napoleon quirks / spec-interpretation differences ────────────────────
    "numpy/structured/unknown_section_with_known_sections.txt": _UNKNOWN_SEC,
    "third_party/fire/google/completion_membervisible.txt": _FIRE_NOCOLON,
    # ── NumPy anonymous-return prose: numpydoc reads a bare line as a type,
    #    napoleon reads it as a description (and strips a trailing colon) ──────
    "third_party/anndata/numpy/obs_vector.txt": "'Params' header (see _ALIAS) plus " + _ANON_RETURN,
    "third_party/scanpy/numpy/pca.txt": _ANON_RETURN,
    "third_party/scanpy/numpy/filter_cells.txt": _ANON_RETURN,
    "third_party/scanpy/numpy/leiden.txt": _ANON_RETURN,
    "third_party/scanpy/numpy/neighbors.txt": _ANON_RETURN,
    "third_party/scanpy/numpy/normalize_total.txt": _ANON_RETURN,
    "third_party/scanpy/numpy/rank_genes_groups.txt": _ANON_RETURN,
    "third_party/scanpy/numpy/regress_out.txt": _ANON_RETURN,
    "third_party/scanpy/numpy/score_genes_cell_cycle.txt": _ANON_RETURN,
    "third_party/scanpy/numpy/tsne.txt": _ANON_RETURN,
    "third_party/scanpy/numpy/umap.txt": _ANON_RETURN,
}

# ── Normalization helpers ─────────────────────────────────────────────────────

_FIELD_RE = re.compile(r"^:([^:]+):\s?(.*)$")
_NAMED_RETURN_RE = re.compile(r"^\*\*(.+?)\*\*(?:\s*\((.*?)\))?\s*--\s*(.*)$", re.S)


def _collapse(text: str | None) -> str:
    """Whitespace-collapse and strip; ``None`` unifies with ``""``."""
    return " ".join(text.split()).strip() if text else ""


def _unescape(name: str) -> str:
    """Drop RST backslash escapes napoleon adds to names (``\\*args`` -> ``*args``)."""
    return re.sub(r"\\(.)", r"\1", name)


def _strip_emphasis(text: str | None) -> str | None:
    if text is None:
        return None
    text = text.strip()
    if text.startswith("*") and text.endswith("*"):
        text = text.strip("*").strip()
    return text or None


def _norm_type(type_str: str | None) -> str | None:
    """Strip; drop a trailing ``optional`` / ``default ...`` clause; ``""`` -> ``None``."""
    if type_str is None:
        return None
    type_str = type_str.strip()
    if not type_str:
        return None
    parts = [seg.strip() for seg in type_str.split(",")]
    while len(parts) > 1 and (parts[-1].lower() == "optional" or parts[-1].lower().startswith("default")):
        parts.pop()
    return ", ".join(parts).strip() or None


def _split_names(target: str) -> tuple[str, ...]:
    return tuple(part.strip() for part in _unescape(target).split(",") if part.strip())


# ── Reference side: parse napoleon's RST field output ─────────────────────────


def _field_blocks(rst: str) -> list[tuple[str, list[str]]]:
    """Group napoleon output into (fieldname, body_lines).

    A field starts at column 0 with ``:name:``; indented lines (and blank lines
    inside a run) continue it; any other column-0 line ends it.
    """
    blocks: list[tuple[str, list[str]]] = []
    cur: tuple[str, list[str]] | None = None
    for line in rst.splitlines():
        if line and not line[:1].isspace():  # column-0, non-empty
            if cur is not None:
                blocks.append(cur)
                cur = None
            m = _FIELD_RE.match(line)
            if m:
                cur = (m.group(1).strip(), [m.group(2)])
        elif cur is not None and line.strip():  # indented continuation
            cur[1].append(line.strip())
        # blank lines keep the current block open (paragraph break)
    if cur is not None:
        blocks.append(cur)
    return blocks


def _attach_type(entries: list[list], target: str | None, type_val: str | None) -> None:
    """Set the type on the most recent same-name entry that has none yet."""
    for entry in reversed(entries):
        if entry[0] == target and entry[1] is None:
            entry[1] = type_val
            return


def _parse_returns(body_lines: list[str], rtype: str | None) -> list[tuple[str | None, str | None, str]]:
    """napoleon return rendering -> list of (name_or_None, type, desc)."""
    if any(ln.lstrip().startswith("* ") for ln in body_lines):  # bulleted multi-return
        items: list[str] = []
        cur: list[str] = []
        for ln in body_lines:
            stripped = ln.strip()
            if stripped.startswith("* "):
                if cur:
                    items.append(" ".join(cur))
                cur = [stripped[2:]]
            else:
                cur.append(stripped)
        if cur:
            items.append(" ".join(cur))
        parsed: list[tuple[str | None, str | None, str]] = []
        for item in items:
            m = _NAMED_RETURN_RE.match(item.strip())
            if m:
                parsed.append((_unescape(m.group(1)), _norm_type(_strip_emphasis(m.group(2))), _collapse(m.group(3))))
            else:
                parsed.append((None, None, _collapse(_strip_emphasis(item))))
        return parsed
    body = " ".join(body_lines).strip()
    m = _NAMED_RETURN_RE.match(body)
    if m:
        return [(_unescape(m.group(1)), _norm_type(m.group(2)) or _norm_type(rtype), _collapse(m.group(3)))]
    return [(None, _norm_type(rtype), _collapse(body))]


def napoleon_role_map(rst: str) -> dict[str, list[tuple]]:
    params: list[list] = []
    keywords: list[list] = []
    ivars: list[list] = []
    raises: list[tuple] = []
    return_body: list[str] | None = None
    rtype: str | None = None
    has_returns = False

    for name, lines in _field_blocks(rst):
        head = name.split(None, 1)
        role = head[0]
        target = head[1] if len(head) > 1 else ""
        body = " ".join(lines)
        if role == "param":
            params.append([target, None, _collapse(body)])
        elif role == "type":
            _attach_type(params, target, _norm_type(body))
        elif role == "keyword":
            keywords.append([target, None, _collapse(body)])
        elif role == "kwtype":
            _attach_type(keywords, target, _norm_type(body))
        elif role == "ivar":
            ivars.append([target, None, _collapse(body)])
        elif role == "vartype":
            _attach_type(ivars, target, _norm_type(body))
        elif role == "raises":
            raises.append(((_unescape(target),), None, _collapse(body)))
        elif role == "returns":
            return_body, has_returns = lines, True
        elif role == "rtype":
            rtype, has_returns = body, True

    role_map: dict[str, list[tuple]] = {}
    # parameters/keywords: napoleon emits one field per name (multi-names split)
    role_map["parameters"] = [((_unescape(n),), t, d) for n, t, d in params]
    role_map["keyword"] = [((_unescape(n),), t, d) for n, t, d in keywords]
    # attributes: napoleon keeps a multi-name group as one field -> split to a tuple
    role_map["attributes"] = [(_split_names(n), t, d) for n, t, d in ivars]
    role_map["raises"] = raises
    role_map["returns"] = []
    if has_returns:
        for name, type_, desc in _parse_returns(return_body or [""], rtype):
            role_map["returns"].append(((name,) if name else (), type_, desc))
    return {k: v for k, v in role_map.items() if v}


# ── Our side: project to_model() onto the same role map ───────────────────────

# napoleon renders Receives and Other Parameters as ordinary :param: fields, so
# we fold both onto the parameters role to match.
_PARAM_KINDS = {
    pydocstring.SectionKind.PARAMETERS,
    pydocstring.SectionKind.RECEIVES,
    pydocstring.SectionKind.OTHER_PARAMETERS,
}


def _entries(section: pydocstring.Section, cls: type) -> list:
    """The `.value` of every block in `section` of the given `Block` variant.

    Prose `Block.Paragraph`s are ignored — the role map compares typed entries
    only (a section-intro paragraph is not a return/param/etc.).
    """
    # ``cls`` is a dynamic Block variant, so the checker can't narrow the type.
    return [block.value for block in section.blocks if isinstance(block, cls)]  # ty: ignore[unresolved-attribute]


def our_role_map(model: pydocstring.Docstring) -> dict[str, list[tuple]]:
    role_map: dict[str, list[tuple]] = {
        "parameters": [],
        "keyword": [],
        "attributes": [],
        "raises": [],
        "returns": [],
    }
    for section in model.sections:
        kind = section.kind
        parameters = _entries(section, pydocstring.Block.Parameter)
        attributes = _entries(section, pydocstring.Block.Attribute)
        exceptions = _entries(section, pydocstring.Block.Exception)
        returns = _entries(section, pydocstring.Block.Return)
        if kind in _PARAM_KINDS and parameters:
            for prm in parameters:
                for name in prm.names:  # expand multi-name groups, matching napoleon
                    role_map["parameters"].append(
                        ((name,), _norm_type(prm.type_annotation), _collapse(prm.description))
                    )
        elif kind == pydocstring.SectionKind.KEYWORD_PARAMETERS and parameters:
            for prm in parameters:
                for name in prm.names:
                    role_map["keyword"].append(((name,), _norm_type(prm.type_annotation), _collapse(prm.description)))
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


# ── Corpus walking (mirrors test_corpus_parity, incl. the third_party layout) ─


def corpus_cases():
    cases = []
    for txt in sorted(CORPUS.rglob("*.txt")):
        parts = txt.relative_to(CORPUS).parts
        style = parts[2] if parts[0] == "third_party" else parts[0]
        if style not in PARSERS:  # napoleon only speaks google/numpy; skip plain
            continue
        rel = str(txt.relative_to(CORPUS))
        cases.append(pytest.param(txt, style, rel, id=rel))
    assert cases, f"no google/numpy corpus inputs found under {CORPUS}"
    return cases


def _diff(ours: dict, theirs: dict) -> str:
    lines = []
    for role in sorted(set(ours) | set(theirs)):
        o, t = ours.get(role, []), theirs.get(role, [])
        if o != t:
            lines.append(f"  [{role}]")
            lines.append(f"    ours    : {o}")
            lines.append(f"    napoleon: {t}")
    return "\n".join(lines)


@pytest.mark.parametrize(("txt", "style", "rel"), corpus_cases())
def test_napoleon_differential(txt: Path, style: str, rel: str) -> None:
    text = txt.read_text()
    our_parse, napoleon_cls = PARSERS[style]

    ours = our_role_map(our_parse(text).to_model())
    theirs = napoleon_role_map(str(napoleon_cls(text, CONFIG)))  # ty: ignore[invalid-argument-type]

    if rel in KNOWN_NAPOLEON_DIVERGENCES:
        # Stale-detection: a listed file must STILL diverge. If it now matches,
        # the divergence was fixed (here or in napoleon) — drop the entry.
        assert ours != theirs, (
            f"{rel} is in KNOWN_NAPOLEON_DIVERGENCES but now MATCHES napoleon.\n"
            f"Remove it from the allowlist (reason was: "
            f"{KNOWN_NAPOLEON_DIVERGENCES[rel]!r})."
        )
        return

    assert ours == theirs, (
        f"{rel} diverges from napoleon and is not allowlisted.\n"
        f"Triage it: fix the parser (or file a bug) if it's a genuine defect, "
        f"or add it to KNOWN_NAPOLEON_DIVERGENCES with a reason if the difference "
        f"is deliberate.\n{_diff(ours, theirs)}"
    )
