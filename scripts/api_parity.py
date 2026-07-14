#!/usr/bin/env python3
"""Check that every item of the Rust public API has a Python counterpart.

#115 opened with "the Rust code has `replace_in`, but that's missing in the
Python bindings". Closing the three gaps it named did not close *how they got
there*: the bindings are hand-written, so every new Rust item is opt-in for
Python and nothing notices when it isn't taken up. Two more of exactly the same
kind (#132, #133) surfaced the moment a real consumer tried to use the 0.4 API,
in a surface I had just finished auditing by hand.

So: enumerate the Rust surface, enumerate the Python surface, and require that
every Rust item is either exposed or **explicitly excused**. The excuse list is
as much the deliverable as the check — "not in Python" should be a decision
someone wrote down, not an oversight.

Run it with `just api-parity`. It regenerates the rustdoc JSON itself, so it
cannot pass by reading a stale copy of the surface.
"""

from __future__ import annotations

import json
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RUSTDOC_JSON = ROOT / "target" / "doc" / "pydocstring.json"
ALLOWLIST = ROOT / "scripts" / "api_parity_allow.toml"

# Rust path -> Python name, where the two legitimately differ.
#
# Every entry here is a naming decision, not a gap: the capability exists on
# both sides. Anything *not* here must match by name.
RENAMES = {
    # The CST types are named for the tree in Rust and for the language in Python.
    "syntax::SyntaxNode": "Node",
    "syntax::SyntaxToken": "Token",
    # Rust needs the `_tree` suffix only because `parse::visitor::walk` once existed.
    "syntax::walk_tree": "walk",
    # A borrowed view over a token in Rust; the same thing as `Token` in Python.
    "parse::TokenRef": "Token",
    # `Parsed::root` is the CST root; Python spells the lens `.syntax` everywhere.
    "syntax::Parsed::root": "Parsed::syntax",
    # Rust splits node- and range-anchored removal; Python's handle is always a range.
    "edit::Edits::remove_lines_range": "Edits::remove_lines",
}

# Rust method names that map onto a Python protocol rather than a method.
METHOD_RENAMES = {
    "new": "__init__",
    "len": "__len__",
    "contains": "__contains__",
}


# ── Rust side ────────────────────────────────────────────────────────────────


def rust_surface(doc: dict) -> dict[str, set[str]]:
    """Items reachable from the crate root through public modules and re-exports.

    This is *not* `visibility == "public"`: a `pub fn` inside a `pub(crate)`
    module is public to the crate and invisible outside it. Only reachability is
    API — which is exactly the distinction #128 turned on when it demoted the
    per-style modules.

    Items are keyed by their module path (`edit::Edits::replace`), so that
    `model::Section` and `parse::unified::Section` — two different types with
    one name — cannot be confused for each other.
    """
    index = doc["index"]
    # id -> (section, shortest public path). An item re-exported from a shorter
    # path (`parse::TextBlock`) is the same item as its definition
    # (`parse::text_block::TextBlock`); key it by the path a user would write.
    found: dict[tuple[str, int], str] = {}
    seen: set[int] = set()

    def record(section: str, item_id: int, path: str) -> None:
        key = (section, item_id)
        if key not in found or len(path) < len(found[key]):
            found[key] = path

    def get(item_id: int) -> dict | None:
        return index.get(str(item_id))

    def is_public(item: dict) -> bool:
        return item.get("visibility") == "public"

    def visit_module(mod_id: int, path: str) -> None:
        if mod_id in seen:
            return
        seen.add(mod_id)
        item = get(mod_id)
        if not item:
            return
        for child_id in item["inner"]["module"]["items"]:
            child = get(child_id)
            if not child:
                continue
            kind = next(iter(child["inner"]))
            if kind == "use":
                target = child["inner"]["use"].get("id")
                if target is not None and get(target):
                    # A re-export makes the target reachable, but its *canonical*
                    # path stays where it is defined — which is what a user reads
                    # in the docs, so key it there.
                    visit_item(target, path)
                continue
            if is_public(child):
                visit_item(child_id, path)

    def visit_item(item_id: int, path: str) -> None:
        item = get(item_id)
        if not item:
            return
        inner = item["inner"]
        kind = next(iter(inner))
        name = item.get("name")
        if not name:
            return
        qual = f"{path}::{name}" if path else name

        if kind == "module":
            if is_public(item):
                visit_module(item_id, qual)
        elif kind == "function":
            record("functions", item_id, qual)
        elif kind in ("struct", "enum", "trait"):
            record("types", item_id, qual)
            for impl_id in inner[kind].get("impls", []):
                impl = get(impl_id)
                if not impl or impl["inner"]["impl"].get("trait") is not None:
                    continue  # derived/trait impls are not the hand-written surface
                for m_id in impl["inner"]["impl"]["items"]:
                    m = get(m_id)
                    if m and m.get("name") and is_public(m):
                        record("methods", m_id, f"{qual}::{m['name']}")

    visit_module(doc["root"], "")

    surface: dict[str, set[str]] = {"functions": set(), "types": set(), "methods": set()}
    for (section, _), path in found.items():
        surface[section].add(path)
    return surface


# ── Python side ──────────────────────────────────────────────────────────────


def python_surface() -> dict[str, set[str]]:
    """`__all__` of `pydocstring` and `pydocstring.model`, plus every public member."""
    protocols = sorted(set(METHOD_RENAMES.values()))
    code = f"PROTOCOL_METHODS = {protocols!r}\n" + r"""
import inspect, json
import pydocstring
from pydocstring import model

out = {"functions": [], "types": [], "methods": []}
for mod, prefix in ((pydocstring, ""), (model, "model.")):
    for name in getattr(mod, "__all__", []):
        obj = getattr(mod, name, None)
        if obj is None:
            continue
        if inspect.isclass(obj):
            out["types"].append(prefix + name)
            for attr in dir(obj):
                # Dunders are private *except* the ones a Rust method maps onto:
                # `new` is `__init__`, `len` is `__len__`, `contains` is
                # `__contains__`. Those are the Python spelling, not an omission.
                if not attr.startswith("_") or attr in PROTOCOL_METHODS:
                    out["methods"].append(f"{prefix}{name}::{attr}")
        elif callable(obj):
            out["functions"].append(prefix + name)
print(json.dumps(out))
"""
    res = subprocess.run(
        ["uv", "run", "python", "-c", code],
        cwd=ROOT / "bindings" / "python",
        capture_output=True,
        text=True,
        check=True,
    )
    return {k: set(v) for k, v in json.loads(res.stdout).items()}


# ── Matching ─────────────────────────────────────────────────────────────────


def candidates(section: str, rust_path: str) -> list[str]:
    """Python names that would satisfy this Rust item.

    A Rust path is `mod::path::Item` (or `…::Item::method`). Python has a flat
    top level plus `model.`, so the module chain is dropped — except that
    `model::` maps onto `model.`, which is the one place the two namespaces
    deliberately agree.
    """
    if rust_path in RENAMES:
        return [RENAMES[rust_path]]

    parts = rust_path.split("::")
    in_model = parts[0] == "model"

    if section == "methods":
        type_path, method = "::".join(parts[:-1]), parts[-1]
        type_name = RENAMES.get(type_path, type_path.split("::")[-1])
        if "::" not in type_name and in_model and not type_name.startswith("model."):
            type_name = f"model.{type_name}"
        names = {method, METHOD_RENAMES.get(method, method)}
        return [f"{type_name}::{n}" for n in names]

    name = parts[-1]
    return [f"model.{name}"] if in_model else [name]


def rustdoc_json() -> dict:
    """Regenerate and load the Rust surface.

    Always regenerate: reading a stale `pydocstring.json` would let the check
    pass on the *previous* commit's surface, which is a silent no-op — exactly
    the failure mode this check exists to prevent. (Found the honest way: a
    probe method I added went undetected because `cargo rustdoc` had failed and
    the script happily read the file from the run before.)
    """
    result = subprocess.run(
        ["cargo", "+nightly", "rustdoc", "-q", "-Z", "unstable-options", "--output-format", "json"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print("error: `cargo +nightly rustdoc` failed:\n" + result.stderr, file=sys.stderr)
        raise SystemExit(2)
    if not RUSTDOC_JSON.exists():
        print(f"error: rustdoc succeeded but {RUSTDOC_JSON.relative_to(ROOT)} is missing", file=sys.stderr)
        raise SystemExit(2)
    return json.loads(RUSTDOC_JSON.read_text())


def main() -> int:
    rust = rust_surface(rustdoc_json())
    py = python_surface()

    with ALLOWLIST.open("rb") as fh:
        data = tomllib.load(fh)
    allow = {s: data.get(s, {}) for s in ("functions", "types", "methods")}

    missing: list[tuple[str, str]] = []
    for section in ("functions", "types", "methods"):
        for item in sorted(rust[section]):
            if item in allow[section]:
                continue
            if any(c in py[section] for c in candidates(section, item)):
                continue
            missing.append((section, item))

    stale = [
        (section, item)
        for section in ("functions", "types", "methods")
        for item in sorted(set(allow[section]) - rust[section])
    ]

    total_rust = sum(len(v) for v in rust.values())
    print(f"Rust surface : {total_rust} items "
          f"({len(rust['functions'])} fns, {len(rust['types'])} types, {len(rust['methods'])} methods)")
    print(f"Excused      : {sum(len(v) for v in allow.values())}")
    print(f"Unaccounted  : {len(missing)}")

    if stale:
        print("\nDead excuses — the Rust item is gone, so remove the entry:")
        for section, item in stale:
            print(f"  [{section}] {item}")

    if missing:
        print("\nIn Rust, absent from Python, and not excused:")
        for section, item in missing:
            print(f"  [{section}] {item}")
        print(
            f"\nEither expose it in the bindings, or add it to "
            f"{ALLOWLIST.relative_to(ROOT)} with a reason.\n"
            "A gap in Python is a gap: the bindings are the product for most users."
        )

    return 1 if (missing or stale) else 0


if __name__ == "__main__":
    sys.exit(main())
