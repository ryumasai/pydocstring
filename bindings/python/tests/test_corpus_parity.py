"""Cross-language parity against the Rust corpus snapshots.

The corpus under ``tests/corpus/`` (repo root) is the single source of truth
for parser/emitter behavior; its ``.snap`` files are blessed by the Rust
harness (``tests/snapshots.rs``). This suite re-runs every corpus input
through the Python bindings and checks the result against those same
snapshots, so any divergence between the bindings and the Rust crate —
today's eager wrappers or the future lazy views — fails here.
"""

from pathlib import Path

import pytest

import pydocstring

CORPUS = Path(__file__).resolve().parents[3] / "tests" / "corpus"

PARSERS = {
    "google": (pydocstring.parse_google, pydocstring.emit_google),
    "numpy": (pydocstring.parse_numpy, pydocstring.emit_numpy),
    "plain": (pydocstring.parse_plain, None),
}


def _ensure_trailing_newline(text: str) -> str:
    return text if text.endswith("\n") else text + "\n"


def corpus_cases():
    cases = []
    for txt in sorted(CORPUS.rglob("*.txt")):
        style = txt.relative_to(CORPUS).parts[0]
        cases.append(pytest.param(txt, style, id=str(txt.relative_to(CORPUS))))
    assert cases, f"no corpus inputs found under {CORPUS}"
    return cases


@pytest.mark.parametrize(("txt", "style"), corpus_cases())
def test_corpus_parity(txt: Path, style: str) -> None:
    snap = txt.with_suffix(".snap").read_text()
    cst_section, emit_marker, emit_section = snap.removeprefix("=== CST ===\n").partition("=== EMIT ===\n")

    parse, emit = PARSERS[style]
    doc = parse(txt.read_text())

    assert _ensure_trailing_newline(doc.pretty_print()) == cst_section

    if emit_marker:
        assert emit is not None, f"snapshot has an EMIT section but style {style!r} has no emitter"
        assert _ensure_trailing_newline(emit(doc.to_model())) == emit_section
