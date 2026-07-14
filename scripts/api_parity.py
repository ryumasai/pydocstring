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
from pathlib import Path

if sys.version_info < (3, 11):  # tomllib
    raise SystemExit("api_parity.py needs Python 3.11+ (tomllib)")

import tomllib

SECTIONS = ("functions", "types", "methods", "variants", "fields")

# rustdoc's JSON schema is unstable and changes on nightly. Pin it: a silently
# renamed field would shrink the surface to nothing and the check would go green.
EXPECTED_FORMAT_VERSION = 60

# The smallest surface that is not obviously a parsing failure. A check whose
# failure mode is "0 items, all good" is worse than no check.
MIN_SURFACE = 250

# Item kinds that carry no API of their own.
IGNORED_KINDS = {
    "use",          # handled separately: a re-export makes its target reachable
    "impl",         # visited through its type
    "variant",      # visited through its enum
    "struct_field", # visited through its struct
    "assoc_type",   # part of a trait's contract, not a separate item
    "assoc_const",
}

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
    # The visitor hooks say what they visit in Python, where `enter`/`leave`
    # alone would read as lifecycle methods on the visitor itself.
    "syntax::Visitor::enter": "Visitor::enter_node",
    "syntax::Visitor::leave": "Visitor::leave_node",
}

# Rust method names that map onto a Python protocol rather than a method.
METHOD_RENAMES = {
    # PyO3 exposes `#[new]` as `__new__`, and a class that is deliberately not
    # constructible has neither `__new__` nor `__init__` in its own `vars()`.
    # (Matching against `__init__` would have been an unconditional pass: every
    # PyO3 class inherits one from `object`, including the ones that raise
    # "cannot create instances". That silently excused every `Type::new`.)
    "new": "__new__",
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
        elif kind == "trait":
            record("types", item_id, qual)
            # A trait's own methods live in `items`. It has no `impls` field —
            # `implementations` lists the *impl blocks* of it, which is a
            # different question. Reading `impls` here silently collected
            # nothing, so `Visitor`'s three hooks were invisible to this check.
            for m_id in inner["trait"]["items"]:
                m = get(m_id)
                if m and m.get("name"):
                    record("methods", m_id, f"{qual}::{m['name']}")
        elif kind in ("struct", "enum"):
            record("types", item_id, qual)
            # Public fields are API: `model::Parameter { names, type_annotation, … }`
            # is constructed by hand on both sides. Adding a field in Rust and
            # forgetting the PyO3 getter is the #115 failure mode exactly.
            for f_id in inner.get("struct", {}).get("kind", {}).get("plain", {}).get("fields", []):
                f = get(f_id)
                if f and f.get("name") and is_public(f):
                    record("fields", f_id, f"{qual}::{f['name']}")
            # Enum variants are API too: a `SyntaxKind` variant added in Rust and
            # forgotten in Python does not fail anything — the binding maps an
            # unrecognised kind to `UNKNOWN` — so nothing would ever say so.
            for v_id in inner.get("enum", {}).get("variants", []):
                v = get(v_id)
                if v and v.get("name"):
                    record("variants", v_id, f"{qual}::{v['name']}")
            for impl_id in inner[kind].get("impls", []):
                impl = get(impl_id)
                if not impl or impl["inner"]["impl"].get("trait") is not None:
                    continue  # derived/trait impls are not the hand-written surface
                for m_id in impl["inner"]["impl"]["items"]:
                    m = get(m_id)
                    if m and m.get("name") and is_public(m):
                        record("methods", m_id, f"{qual}::{m['name']}")
        elif kind in IGNORED_KINDS:
            pass
        else:
            # Never skip silently. A rustdoc kind this script does not know is a
            # hole in the surface, and a hole in the surface is the whole thing
            # this check exists to prevent. `type_alias`/`constant`/`static` were
            # sailing straight through until this line existed.
            raise SystemExit(
                f"error: unhandled rustdoc item kind {kind!r} at {qual}.\n"
                "Teach api_parity.py about it, or add it to IGNORED_KINDS with a reason."
            )

    visit_module(doc["root"], "")

    surface: dict[str, set[str]] = {s: set() for s in SECTIONS}
    for (section, _), path in found.items():
        surface[section].add(path)
    return surface


# ── Python side ──────────────────────────────────────────────────────────────


def python_surface() -> dict[str, set[str]]:
    """`__all__` of `pydocstring` and `pydocstring.model`, plus every public member."""
    protocols = sorted(set(METHOD_RENAMES.values()) | {"__init__"})
    code = f"PROTOCOL_METHODS = {protocols!r}\n" + r"""
import inspect, json
import pydocstring
from pydocstring import model

out = {"functions": [], "types": [], "methods": [], "variants": [], "fields": []}
for mod, prefix in ((pydocstring, ""), (model, "model.")):
    for name in getattr(mod, "__all__", []):
        obj = getattr(mod, name, None)
        if obj is None:
            continue
        if inspect.isclass(obj):
            out["types"].append(prefix + name)
            for attr in dir(obj):
                if attr.startswith("_") and attr not in PROTOCOL_METHODS:
                    continue
                # Only count a constructor the class actually defines.
                if attr in ("__new__", "__init__") and attr not in vars(obj):
                    continue
                member = getattr(obj, attr, None)
                # An enum member (SectionKind.PARAMETERS) or a complex-enum
                # variant class (model.Block.Parameter) is a variant, not a method.
                if isinstance(member, obj) or (inspect.isclass(member) and issubclass(member, obj)):
                    out["variants"].append(f"{prefix}{name}::{attr}")
                    continue
                # A PyO3 `#[pyo3(get)]` field arrives as a getset descriptor; a
                # method arrives as a function. A Rust field can be matched by
                # either, since Python is free to expose it as a property.
                qualified = f"{prefix}{name}::{attr}"
                out["methods"].append(qualified)
                if not callable(member):
                    out["fields"].append(qualified)
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


def _letters(name: str) -> str:
    return "".join(c for c in name.lower() if c.isalpha())


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

    if section == "variants":
        enum_path, variant = "::".join(parts[:-1]), parts[-1]
        enum_name = RENAMES.get(enum_path, enum_path.split("::")[-1])
        if in_model and "." not in enum_name:
            enum_name = f"model.{enum_name}"
        # Rust `OtherParameters` is Python `OTHER_PARAMETERS` on a simple enum and
        # `OtherParameters` on a complex one. Compare on letters alone.
        return [f"{enum_name}::{variant}", f"{enum_name}::~{_letters(variant)}"]

    if section == "fields":
        type_path, field = "::".join(parts[:-1]), parts[-1]
        type_name = RENAMES.get(type_path, type_path.split("::")[-1])
        if in_model and "." not in type_name:
            type_name = f"model.{type_name}"
        return [f"{type_name}::{field}"]

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

    doc = json.loads(RUSTDOC_JSON.read_text())
    if doc.get("format_version") != EXPECTED_FORMAT_VERSION:
        raise SystemExit(
            f"error: rustdoc JSON format_version is {doc.get('format_version')}, "
            f"expected {EXPECTED_FORMAT_VERSION}.\n"
            "The schema is unstable and a renamed field would silently shrink the "
            "surface to nothing. Re-read the schema, update this script, then bump "
            "EXPECTED_FORMAT_VERSION."
        )
    return doc


def main() -> int:
    rust = rust_surface(rustdoc_json())
    py = python_surface()

    with ALLOWLIST.open("rb") as fh:
        data = tomllib.load(fh)
    allow = {s: data.get(s, {}) for s in SECTIONS}

    missing: list[tuple[str, str]] = []
    for section in SECTIONS:
        for item in sorted(rust[section]):
            if item in allow[section]:
                continue
            # An excused type excuses its variants and fields: the reason you
            # gave for not exposing `SyntaxElement` is the reason its
            # `Node`/`Token` variants are not exposed either.
            if section in ("variants", "fields") and "::".join(item.split("::")[:-1]) in allow["types"]:
                continue
            names = py[section]
            if section == "variants":
                names = names | {
                    f"{n.split('::')[0]}::~{_letters(n.split('::')[1])}" for n in py[section]
                }
            if any(c in names for c in candidates(section, item)):
                continue
            missing.append((section, item))

    stale = [
        (section, item)
        for section in SECTIONS
        for item in sorted(set(allow[section]) - rust[section])
    ]

    total_rust = sum(len(v) for v in rust.values())
    if total_rust < MIN_SURFACE:
        raise SystemExit(
            f"error: only {total_rust} Rust items found (expected at least {MIN_SURFACE}).\n"
            "The surface did not shrink by 60 items overnight — the rustdoc schema moved, "
            "or the traversal is broken. A check that passes on an empty surface is worse "
            "than no check."
        )
    print(f"Rust surface : {total_rust} items ({len(rust['functions'])} fns, "
          f"{len(rust['types'])} types, {len(rust['methods'])} methods, "
          f"{len(rust['variants'])} variants, {len(rust['fields'])} fields)")
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
