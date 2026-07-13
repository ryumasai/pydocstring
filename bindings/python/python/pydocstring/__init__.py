"""Parse Python docstrings — Google, NumPy, or neither — into one `Parsed`.

Read it through three lenses, all of which keep byte ranges so their results
double as edit anchors:

* `Document(parsed)` — the style-independent semantic view (`Document` →
  `Section` → `Entry`). `Args:` and `Parameters` are both
  `SectionKind.PARAMETERS`, so a section's role is data, not a type to
  dispatch on.
* `parsed.syntax` — the raw CST, which keeps every byte, including the
  zero-length placeholders the semantic view hides.
* `parsed.to_model()` — the normalized, position-free IR that feeds the
  `emit_*` functions.

Edit through `parsed.edit()`: anchored byte-range splices, so everything an
edit does not touch is preserved byte-for-byte.
"""

from . import model
from ._pydocstring import Capture
from ._pydocstring import Citation
from ._pydocstring import DefaultMarker
from ._pydocstring import Directive
from ._pydocstring import Document
from ._pydocstring import EditError
from ._pydocstring import Edits
from ._pydocstring import Entry
from ._pydocstring import LineColumn
from ._pydocstring import Match
from ._pydocstring import Node
from ._pydocstring import Parsed
from ._pydocstring import PatternError
from ._pydocstring import Section
from ._pydocstring import SectionKind
from ._pydocstring import Style
from ._pydocstring import SyntaxKind
from ._pydocstring import TextBlock
from ._pydocstring import TextRange
from ._pydocstring import Token
from ._pydocstring import WalkContext
from ._pydocstring import detect_style
from ._pydocstring import emit_google
from ._pydocstring import emit_numpy
from ._pydocstring import emit_sphinx
from ._pydocstring import parse
from ._pydocstring import parse_google
from ._pydocstring import parse_numpy
from ._pydocstring import parse_plain
from ._pydocstring import walk
from ._visitor import Visitor

__all__ = [
    # ── Core types ────────────────────────────────────────────────────────
    "TextRange",
    "LineColumn",
    "WalkContext",
    "TextBlock",
    "Token",
    "Style",
    # ── Raw CST — the fidelity lens ───────────────────────────────────────
    "SyntaxKind",
    "Node",
    "Parsed",
    # ── Section kinds ─────────────────────────────────────────────────────
    "SectionKind",
    # ── Unified views — the style-independent read lens ───────────────────
    "Document",
    "Section",
    "Entry",
    "DefaultMarker",
    "Directive",
    "Citation",
    # ── Editing ───────────────────────────────────────────────────────────
    "Edits",
    "EditError",
    # ── Model IR (position-free); see pydocstring.model ────────────────────
    "model",
    # ── Pattern matching & rewriting ──────────────────────────────────────
    "Match",
    "Capture",
    "PatternError",
    # ── Visitor ───────────────────────────────────────────────────────────
    "Visitor",
    # ── Functions ─────────────────────────────────────────────────────────
    "parse",
    "parse_google",
    "parse_numpy",
    "parse_plain",
    "detect_style",
    "emit_google",
    "emit_numpy",
    "emit_sphinx",
    "walk",
]
