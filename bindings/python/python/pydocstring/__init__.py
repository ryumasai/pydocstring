from . import model
from ._pydocstring import Capture
from ._pydocstring import Citation
from ._pydocstring import DefaultMarker
from ._pydocstring import Directive
from ._pydocstring import Document
from ._pydocstring import EditError
from ._pydocstring import Edits
from ._pydocstring import Entry
from ._pydocstring import GoogleArg
from ._pydocstring import GoogleAttribute
from ._pydocstring import GoogleDeprecation
from ._pydocstring import GoogleDirective
from ._pydocstring import GoogleDocstring
from ._pydocstring import GoogleException
from ._pydocstring import GoogleMethod
from ._pydocstring import GoogleReference
from ._pydocstring import GoogleReturn
from ._pydocstring import GoogleSection
from ._pydocstring import GoogleSectionKind
from ._pydocstring import GoogleSeeAlsoItem
from ._pydocstring import GoogleWarning
from ._pydocstring import GoogleYield
from ._pydocstring import LineColumn
from ._pydocstring import Match
from ._pydocstring import NumPyAttribute
from ._pydocstring import NumPyDeprecation
from ._pydocstring import NumPyDirective
from ._pydocstring import NumPyDocstring
from ._pydocstring import NumPyException
from ._pydocstring import NumPyMethod
from ._pydocstring import NumPyParameter
from ._pydocstring import NumPyReference
from ._pydocstring import NumPyReturns
from ._pydocstring import NumPySection
from ._pydocstring import NumPySectionKind
from ._pydocstring import NumPySeeAlsoItem
from ._pydocstring import NumPyWarning
from ._pydocstring import NumPyYields
from ._pydocstring import PatternError
from ._pydocstring import PlainDocstring
from ._pydocstring import Section
from ._pydocstring import SectionKind
from ._pydocstring import Style
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
    # ── Section kinds ─────────────────────────────────────────────────────
    "SectionKind",
    "GoogleSectionKind",
    "NumPySectionKind",
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
    # ── Google CST wrappers ───────────────────────────────────────────────
    "GoogleDocstring",
    "GoogleSection",
    "GoogleDeprecation",
    "GoogleDirective",
    "GoogleArg",
    "GoogleReturn",
    "GoogleYield",
    "GoogleException",
    "GoogleWarning",
    "GoogleSeeAlsoItem",
    "GoogleReference",
    "GoogleAttribute",
    "GoogleMethod",
    # ── NumPy CST wrappers ────────────────────────────────────────────────
    "NumPyDocstring",
    "NumPySection",
    "NumPyDeprecation",
    "NumPyDirective",
    "NumPyParameter",
    "NumPyReturns",
    "NumPyYields",
    "NumPyException",
    "NumPyWarning",
    "NumPySeeAlsoItem",
    "NumPyReference",
    "NumPyAttribute",
    "NumPyMethod",
    # ── Plain CST wrapper ─────────────────────────────────────────────────
    "PlainDocstring",
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
