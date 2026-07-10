"""Type stubs for the native ``pydocstring._pydocstring`` extension module.

Parses Google-style and NumPy-style Python docstrings into typed, traversable
objects with source-location information suitable for linters and formatters.

Missing-value convention (uniform across the Google and NumPy CST wrappers):

* Fields documented as **"or missing-placeholder"** return a zero-length
  placeholder object (``is_missing()`` is ``True``) when the surrounding
  syntax marker is present but the content is absent (e.g. the description
  in ``ValueError:``, or the type in ``x ():`` / ``x :``), and ``None``
  when the parser emitted no placeholder at all (e.g. a Raises entry
  without a colon). So these fields can still be ``None`` — check both.
* Fields documented as **"None when absent"** are plain optionals and are
  never a missing placeholder. This includes the ``description`` of
  returns/yields entries in BOTH styles (symmetric by design).
"""

from __future__ import annotations

from typing import TypeVar

from ._visitor import Visitor

_VisitorT = TypeVar("_VisitorT", bound="Visitor")

# ─── Core types ──────────────────────────────────────────────────────────────

class TextRange:
    """Byte range ``[start, end)`` within the source string."""

    start: int
    end: int
    def is_empty(self) -> bool:
        """Return ``True`` when ``start == end`` (zero-length placeholder)."""
        ...
    def __repr__(self) -> str: ...

class LineColumn:
    """1-based line number and 0-based column offset."""

    lineno: int
    col: int
    def __repr__(self) -> str: ...

class WalkContext:
    """Context passed to every ``enter_*`` method during a ``walk()`` call."""
    def line_col(self, offset: int) -> LineColumn: ...
    def __repr__(self) -> str: ...

class PatternError(ValueError):
    """Raised when a pattern string has no valid reading."""

class Capture:
    """A metavariable capture of a :class:`Match`.

    ``text`` is the original target bytes the metavariable bound, copied
    byte-for-byte (never reformatted); ``range`` is their byte range.
    """

    @property
    def range(self) -> TextRange: ...
    @property
    def text(self) -> str: ...
    def is_multi(self) -> bool:
        """Return ``True`` when bound by a ``$$$NAME`` sequence variable."""
        ...
    def __repr__(self) -> str: ...

class Match:
    """One non-overlapping match of a pattern against a docstring."""

    @property
    def range(self) -> TextRange: ...
    @property
    def text(self) -> str: ...
    @property
    def captures(self) -> dict[str, Capture]:
        """The captures keyed by metavariable name (first-occurrence order)."""
        ...
    def capture(self, name: str) -> Capture | None:
        """The capture bound to ``name`` (without the ``$`` sigil), or ``None``."""
        ...
    def __repr__(self) -> str: ...

class Token:
    """A text fragment plus its source byte range.  Has no ``kind`` field."""
    @property
    def text(self) -> str: ...
    @property
    def range(self) -> TextRange: ...
    def is_missing(self) -> bool:
        """Return ``True`` when this token is a zero-length placeholder.

        The parser inserts missing tokens for syntactically absent elements
        such as the type in ``arg ():`` or a missing closing parenthesis.
        Equivalent to ``token.range.is_empty()``.
        """
        ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

class TextBlock:
    """A multi-line text content block wrapping one ``Token`` per content line.

    Covers summaries, extended summaries, descriptions, free-text section
    bodies, and reference content. ``text`` is the raw source slice of the
    block's range (identical to the pre-0.2 token ``text``); ``logical_text``
    is the dedented, newline-joined convenience form.
    """

    @property
    def text(self) -> str: ...
    @property
    def logical_text(self) -> str: ...
    @property
    def range(self) -> TextRange: ...
    @property
    def lines(self) -> list[Token]: ...
    def is_missing(self) -> bool:
        """Return ``True`` when this block is a zero-length placeholder.

        The parser inserts missing blocks for syntactically absent elements
        such as the description in ``arg (int):``.
        Equivalent to ``block.range.is_empty()``.
        """
        ...
    def __repr__(self) -> str: ...

class Style:
    """Docstring style detected or used for emission.

    Hashable (usable as a dict key / set member). Compares equal only to
    other ``Style`` members — never to ints.
    """

    GOOGLE: Style
    NUMPY: Style
    PLAIN: Style
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

# ─── Section kinds ───────────────────────────────────────────────────────────

class GoogleSectionKind:
    ARGS: GoogleSectionKind
    KEYWORD_ARGS: GoogleSectionKind
    OTHER_PARAMETERS: GoogleSectionKind
    RECEIVES: GoogleSectionKind
    RETURNS: GoogleSectionKind
    YIELDS: GoogleSectionKind
    RAISES: GoogleSectionKind
    WARNS: GoogleSectionKind
    ATTRIBUTES: GoogleSectionKind
    METHODS: GoogleSectionKind
    SEE_ALSO: GoogleSectionKind
    NOTES: GoogleSectionKind
    EXAMPLES: GoogleSectionKind
    TODO: GoogleSectionKind
    REFERENCES: GoogleSectionKind
    WARNINGS: GoogleSectionKind
    ATTENTION: GoogleSectionKind
    CAUTION: GoogleSectionKind
    DANGER: GoogleSectionKind
    ERROR: GoogleSectionKind
    HINT: GoogleSectionKind
    IMPORTANT: GoogleSectionKind
    TIP: GoogleSectionKind
    UNKNOWN: GoogleSectionKind
    def __repr__(self) -> str: ...

class NumPySectionKind:
    PARAMETERS: NumPySectionKind
    RETURNS: NumPySectionKind
    YIELDS: NumPySectionKind
    RECEIVES: NumPySectionKind
    OTHER_PARAMETERS: NumPySectionKind
    KEYWORD_PARAMETERS: NumPySectionKind
    RAISES: NumPySectionKind
    WARNS: NumPySectionKind
    WARNINGS: NumPySectionKind
    SEE_ALSO: NumPySectionKind
    NOTES: NumPySectionKind
    REFERENCES: NumPySectionKind
    EXAMPLES: NumPySectionKind
    ATTRIBUTES: NumPySectionKind
    METHODS: NumPySectionKind
    TODO: NumPySectionKind
    ATTENTION: NumPySectionKind
    CAUTION: NumPySectionKind
    DANGER: NumPySectionKind
    ERROR: NumPySectionKind
    HINT: NumPySectionKind
    IMPORTANT: NumPySectionKind
    TIP: NumPySectionKind
    UNKNOWN: NumPySectionKind
    def __repr__(self) -> str: ...

# ─── Google CST wrappers ─────────────────────────────────────────────────────

class GoogleArg:
    @property
    def range(self) -> TextRange: ...
    @property
    def name(self) -> Token: ...
    @property
    def names(self) -> list[Token]: ...
    @property
    def open_bracket(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def type(self) -> Token | None:
        """Type token, or missing-placeholder (e.g. ``x ():``), or ``None``."""
    @property
    def close_bracket(self) -> Token | None:
        """Closing bracket, or missing-placeholder (unclosed ``(``), or ``None``."""
    @property
    def colon(self) -> Token | None:
        """Colon token, or missing-placeholder, or ``None``."""
    @property
    def description(self) -> TextBlock | None:
        """Description, or missing-placeholder (e.g. ``x (int):``), or ``None``."""
    @property
    def optional(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def default_keyword(self) -> Token | None: ...
    @property
    def default_separator(self) -> Token | None: ...
    @property
    def default_value(self) -> Token | None: ...
    def __repr__(self) -> str: ...

class GoogleReturn:
    @property
    def range(self) -> TextRange: ...
    @property
    def return_type(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def colon(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def description(self) -> TextBlock | None:
        """``None`` when absent (never a missing placeholder; symmetric with NumPy)."""
    def __repr__(self) -> str: ...

class GoogleYield:
    @property
    def range(self) -> TextRange: ...
    @property
    def return_type(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def colon(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def description(self) -> TextBlock | None:
        """``None`` when absent (never a missing placeholder; symmetric with NumPy)."""
    def __repr__(self) -> str: ...

class GoogleException:
    @property
    def range(self) -> TextRange: ...
    @property
    def type(self) -> Token: ...
    @property
    def colon(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def description(self) -> TextBlock | None:
        """Description, or missing-placeholder (``ValueError:``), or ``None`` (no colon)."""
    def __repr__(self) -> str: ...

class GoogleWarning:
    @property
    def range(self) -> TextRange: ...
    @property
    def type(self) -> Token: ...
    @property
    def colon(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def description(self) -> TextBlock | None:
        """Description, or missing-placeholder (``UserWarning:``), or ``None`` (no colon)."""
    def __repr__(self) -> str: ...

class GoogleSeeAlsoItem:
    @property
    def range(self) -> TextRange: ...
    @property
    def names(self) -> list[Token]: ...
    @property
    def colon(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def description(self) -> TextBlock | None:
        """Description, or missing-placeholder (``name:``), or ``None`` (no colon)."""
    def __repr__(self) -> str: ...

class GoogleReference:
    @property
    def range(self) -> TextRange: ...
    @property
    def directive_marker(self) -> Token | None: ...
    @property
    def open_bracket(self) -> Token | None: ...
    @property
    def label(self) -> Token | None: ...
    @property
    def close_bracket(self) -> Token | None: ...
    @property
    def content(self) -> TextBlock | None: ...
    def __repr__(self) -> str: ...

class GoogleAttribute:
    @property
    def range(self) -> TextRange: ...
    @property
    def name(self) -> Token:
        """First name token (convenience for ``names[0]``)."""
    @property
    def names(self) -> list[Token]: ...
    @property
    def open_bracket(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def type(self) -> Token | None:
        """Type token, or missing-placeholder (``attr ():``), or ``None``."""
    @property
    def close_bracket(self) -> Token | None:
        """Closing bracket, or missing-placeholder, or ``None``."""
    @property
    def colon(self) -> Token | None:
        """Colon token, or missing-placeholder, or ``None``."""
    @property
    def description(self) -> TextBlock | None:
        """Description, or missing-placeholder (``attr (int):``), or ``None``."""
    def __repr__(self) -> str: ...

class GoogleMethod:
    @property
    def range(self) -> TextRange: ...
    @property
    def name(self) -> Token: ...
    @property
    def open_bracket(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def type(self) -> Token | None:
        """Type token, or missing-placeholder (``meth ():``), or ``None``."""
    @property
    def close_bracket(self) -> Token | None:
        """Closing bracket, or missing-placeholder, or ``None``."""
    @property
    def colon(self) -> Token | None:
        """Colon token, or missing-placeholder, or ``None``."""
    @property
    def description(self) -> TextBlock | None:
        """Description, or missing-placeholder (``meth:``), or ``None``."""
    def __repr__(self) -> str: ...

class GoogleSection:
    """A section in a Google-style docstring.

    Child nodes are not directly accessible; use :func:`walk` with a
    :class:`Visitor` to iterate over ``GoogleArg``, ``GoogleReturn``, etc.
    """
    @property
    def range(self) -> TextRange: ...
    @property
    def section_kind(self) -> GoogleSectionKind: ...
    @property
    def header_name(self) -> Token: ...
    def __repr__(self) -> str: ...

class GoogleDeprecation:
    @property
    def range(self) -> TextRange: ...
    @property
    def directive_marker(self) -> Token | None: ...
    @property
    def keyword(self) -> Token | None: ...
    @property
    def double_colon(self) -> Token | None: ...
    @property
    def version(self) -> Token: ...
    @property
    def description(self) -> TextBlock | None: ...
    def __repr__(self) -> str: ...

class GoogleDocstring:
    """A parsed Google-style docstring."""
    @property
    def range(self) -> TextRange: ...
    @property
    def summary(self) -> TextBlock | None: ...
    @property
    def extended_summary(self) -> TextBlock | None: ...
    @property
    def deprecation(self) -> GoogleDeprecation | None: ...
    @property
    def paragraphs(self) -> list[TextBlock]:
        """Stray-prose paragraph blocks between sections, in source order."""
    @property
    def sections(self) -> list[GoogleSection]: ...
    @property
    def source(self) -> str: ...
    @property
    def style(self) -> Style: ...
    def pretty_print(self) -> str: ...
    def to_model(self) -> Docstring: ...
    def replace(self, pattern: str, template: str) -> str:
        """Replace every match of ``pattern`` with ``template``, returning new source.

        ``pattern`` and ``template`` use ``$NAME`` / ``$$$NAME`` metavariables;
        captured content is substituted byte-for-byte and everything else is
        preserved. Raises :class:`PatternError` for an invalid pattern.
        """
        ...
    def findall(self, pattern: str) -> list[Match]:
        """Find every match of ``pattern`` in document order (non-overlapping)."""
        ...
    def __repr__(self) -> str: ...

# ─── NumPy CST wrappers ──────────────────────────────────────────────────────

class NumPyDeprecation:
    @property
    def range(self) -> TextRange: ...
    @property
    def directive_marker(self) -> Token | None: ...
    @property
    def keyword(self) -> Token | None: ...
    @property
    def double_colon(self) -> Token | None: ...
    @property
    def version(self) -> Token: ...
    @property
    def description(self) -> TextBlock | None: ...
    def __repr__(self) -> str: ...

class NumPyParameter:
    @property
    def range(self) -> TextRange: ...
    @property
    def name(self) -> Token | None:
        """First name token (``names[0]``); ``None`` when the entry has no names."""
    @property
    def names(self) -> list[Token]: ...
    @property
    def colon(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def type(self) -> Token | None:
        """Type token, or missing-placeholder (``x :``), or ``None`` (no colon)."""
    @property
    def description(self) -> TextBlock | None:
        """Description, or missing-placeholder, or ``None``.

        The NumPy grammar carries descriptions on indented continuation
        lines, so the parser emits no placeholder for a parameter without
        one — expect ``None`` in that case (the placeholder form only
        appears for Google-style ``name (type): desc`` entries).
        """
    @property
    def optional(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def default_keyword(self) -> Token | None: ...
    @property
    def default_separator(self) -> Token | None: ...
    @property
    def default_value(self) -> Token | None: ...
    def __repr__(self) -> str: ...

class NumPyReturns:
    """A single named return entry in a NumPy ``Returns`` section."""
    @property
    def range(self) -> TextRange: ...
    @property
    def name(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def colon(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def return_type(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def description(self) -> TextBlock | None:
        """``None`` when absent (never a missing placeholder; symmetric with Google)."""
    def __repr__(self) -> str: ...

class NumPyYields:
    """A single named yield entry in a NumPy ``Yields`` section."""
    @property
    def range(self) -> TextRange: ...
    @property
    def name(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def colon(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def return_type(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def description(self) -> TextBlock | None:
        """``None`` when absent (never a missing placeholder; symmetric with Google)."""
    def __repr__(self) -> str: ...

class NumPyException:
    @property
    def range(self) -> TextRange: ...
    @property
    def type(self) -> Token: ...
    @property
    def colon(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def description(self) -> TextBlock | None:
        """Description, or missing-placeholder (``ValueError:``), or ``None`` (no colon)."""
    def __repr__(self) -> str: ...

class NumPyWarning:
    @property
    def range(self) -> TextRange: ...
    @property
    def type(self) -> Token: ...
    @property
    def colon(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def description(self) -> TextBlock | None:
        """Description, or missing-placeholder (``UserWarning:``), or ``None`` (no colon)."""
    def __repr__(self) -> str: ...

class NumPySeeAlsoItem:
    @property
    def range(self) -> TextRange: ...
    @property
    def names(self) -> list[Token]: ...
    @property
    def colon(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def description(self) -> TextBlock | None:
        """Description, or missing-placeholder (``name :``), or ``None`` (no colon)."""
    def __repr__(self) -> str: ...

class NumPyReference:
    @property
    def range(self) -> TextRange: ...
    @property
    def directive_marker(self) -> Token | None: ...
    @property
    def open_bracket(self) -> Token | None: ...
    @property
    def label(self) -> Token | None: ...
    @property
    def close_bracket(self) -> Token | None: ...
    @property
    def content(self) -> TextBlock | None: ...
    def __repr__(self) -> str: ...

class NumPyAttribute:
    @property
    def range(self) -> TextRange: ...
    @property
    def name(self) -> Token:
        """First name token (convenience for ``names[0]``)."""
    @property
    def names(self) -> list[Token]: ...
    @property
    def colon(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def type(self) -> Token | None:
        """Type token, or missing-placeholder (``attr :``), or ``None`` (no colon)."""
    @property
    def description(self) -> TextBlock | None:
        """Description, or missing-placeholder, or ``None``.

        NumPy attribute descriptions live on indented continuation lines,
        so the parser emits no placeholder when one is absent — expect
        ``None`` in that case.
        """
    def __repr__(self) -> str: ...

class NumPyMethod:
    @property
    def range(self) -> TextRange: ...
    @property
    def name(self) -> Token: ...
    @property
    def colon(self) -> Token | None:
        """``None`` when absent (never a missing placeholder)."""
    @property
    def description(self) -> TextBlock | None:
        """Description, or missing-placeholder (``meth:``), or ``None`` (no colon)."""
    def __repr__(self) -> str: ...

class NumPySection:
    """A section in a NumPy-style docstring.

    Child nodes are not directly accessible; use :func:`walk` with a
    :class:`Visitor` to iterate over ``NumPyParameter``, ``NumPyReturns``, etc.
    """
    @property
    def range(self) -> TextRange: ...
    @property
    def section_kind(self) -> NumPySectionKind: ...
    @property
    def header_name(self) -> Token: ...
    def __repr__(self) -> str: ...

class NumPyDocstring:
    """A parsed NumPy-style docstring."""
    @property
    def range(self) -> TextRange: ...
    @property
    def summary(self) -> TextBlock | None: ...
    @property
    def extended_summary(self) -> TextBlock | None: ...
    @property
    def deprecation(self) -> NumPyDeprecation | None: ...
    @property
    def paragraphs(self) -> list[TextBlock]:
        """Stray-prose paragraph blocks between sections, in source order.

        Parity with :attr:`GoogleDocstring.paragraphs`. The NumPy grammar
        lets the extended summary and section bodies absorb stray prose, so
        this list is typically empty.
        """
    @property
    def sections(self) -> list[NumPySection]: ...
    @property
    def source(self) -> str: ...
    @property
    def style(self) -> Style: ...
    def pretty_print(self) -> str: ...
    def to_model(self) -> Docstring: ...
    def replace(self, pattern: str, template: str) -> str:
        """Replace every match of ``pattern`` with ``template``, returning new source.

        ``pattern`` and ``template`` use ``$NAME`` / ``$$$NAME`` metavariables;
        captured content is substituted byte-for-byte and everything else is
        preserved. Raises :class:`PatternError` for an invalid pattern.
        """
        ...
    def findall(self, pattern: str) -> list[Match]:
        """Find every match of ``pattern`` in document order (non-overlapping)."""
        ...
    def __repr__(self) -> str: ...

# ─── Plain CST wrapper ───────────────────────────────────────────────────────

class PlainDocstring:
    """A parsed plain docstring (no section markers)."""
    @property
    def range(self) -> TextRange: ...
    @property
    def summary(self) -> TextBlock | None: ...
    @property
    def extended_summary(self) -> TextBlock | None: ...
    @property
    def source(self) -> str: ...
    @property
    def style(self) -> Style: ...
    def pretty_print(self) -> str: ...
    def to_model(self) -> Docstring: ...
    def replace(self, pattern: str, template: str) -> str:
        """Replace every match of ``pattern`` with ``template``, returning new source.

        ``pattern`` and ``template`` use ``$NAME`` / ``$$$NAME`` metavariables;
        captured content is substituted byte-for-byte and everything else is
        preserved. Raises :class:`PatternError` for an invalid pattern.
        """
        ...
    def findall(self, pattern: str) -> list[Match]:
        """Find every match of ``pattern`` in document order (non-overlapping)."""
        ...
    def __repr__(self) -> str: ...

# ─── Model IR ────────────────────────────────────────────────────────────────

class Directive:
    """A document-level rST directive (``.. name:: argument`` + body).

    A deprecation notice is a directive with ``name == "deprecated"`` whose
    ``argument`` is the version.
    """

    name: str
    argument: str | None
    description: str | None
    def __init__(
        self,
        name: str,
        *,
        argument: str | None = None,
        description: str | None = None,
    ) -> None: ...
    def __repr__(self) -> str: ...

class Parameter:
    names: list[str]
    type_annotation: str | None
    description: str | None
    is_optional: bool
    default_value: str | None
    def __init__(
        self,
        names: list[str],
        *,
        type_annotation: str | None = None,
        description: str | None = None,
        is_optional: bool = False,
        default_value: str | None = None,
    ) -> None: ...
    def __repr__(self) -> str: ...

class Return:
    name: str | None
    type_annotation: str | None
    description: str | None
    def __init__(
        self,
        *,
        name: str | None = None,
        type_annotation: str | None = None,
        description: str | None = None,
    ) -> None: ...
    def __repr__(self) -> str: ...

class ExceptionEntry:
    type_name: str
    description: str | None
    def __init__(self, type_name: str, *, description: str | None = None) -> None: ...
    def __repr__(self) -> str: ...

class SeeAlsoEntry:
    names: list[str]
    description: str | None
    def __init__(self, names: list[str], *, description: str | None = None) -> None: ...
    def __repr__(self) -> str: ...

class Reference:
    label: str | None
    content: str | None
    def __init__(self, *, label: str | None = None, content: str | None = None) -> None: ...
    def __repr__(self) -> str: ...

class Attribute:
    names: list[str]
    type_annotation: str | None
    description: str | None
    def __init__(
        self,
        names: list[str],
        *,
        type_annotation: str | None = None,
        description: str | None = None,
    ) -> None: ...
    def __repr__(self) -> str: ...

class Method:
    name: str
    type_annotation: str | None
    description: str | None
    def __init__(
        self,
        name: str,
        *,
        type_annotation: str | None = None,
        description: str | None = None,
    ) -> None: ...
    def __repr__(self) -> str: ...

class SectionKind:
    """Style-independent section kind for the model IR."""

    PARAMETERS: SectionKind
    KEYWORD_PARAMETERS: SectionKind
    OTHER_PARAMETERS: SectionKind
    RECEIVES: SectionKind
    RETURNS: SectionKind
    YIELDS: SectionKind
    RAISES: SectionKind
    WARNS: SectionKind
    ATTRIBUTES: SectionKind
    METHODS: SectionKind
    SEE_ALSO: SectionKind
    REFERENCES: SectionKind
    NOTES: SectionKind
    EXAMPLES: SectionKind
    WARNINGS: SectionKind
    TODO: SectionKind
    ATTENTION: SectionKind
    CAUTION: SectionKind
    DANGER: SectionKind
    ERROR: SectionKind
    HINT: SectionKind
    IMPORTANT: SectionKind
    TIP: SectionKind
    UNKNOWN: SectionKind
    def __repr__(self) -> str: ...

class Section:
    """A section in the model IR.

    The collection getters (``parameters``, ``returns``, ``exceptions``,
    ``attributes``, ``methods``, ``see_also_entries``, ``references``,
    ``body``) return ``None`` when the section is of a different kind —
    e.g. ``returns`` is ``None`` on a ``PARAMETERS`` section.
    """
    @property
    def kind(self) -> SectionKind: ...
    @property
    def unknown_name(self) -> str | None: ...
    @property
    def parameters(self) -> list[Parameter] | None: ...
    @property
    def returns(self) -> list[Return] | None: ...
    @property
    def exceptions(self) -> list[ExceptionEntry] | None: ...
    @property
    def attributes(self) -> list[Attribute] | None: ...
    @property
    def methods(self) -> list[Method] | None: ...
    @property
    def see_also_entries(self) -> list[SeeAlsoEntry] | None: ...
    @property
    def references(self) -> list[Reference] | None: ...
    @property
    def body(self) -> str | None: ...
    def __init__(
        self,
        kind: SectionKind,
        *,
        unknown_name: str | None = None,
        parameters: list[Parameter] | None = None,
        returns: list[Return] | None = None,
        exceptions: list[ExceptionEntry] | None = None,
        attributes: list[Attribute] | None = None,
        methods: list[Method] | None = None,
        see_also_entries: list[SeeAlsoEntry] | None = None,
        references: list[Reference] | None = None,
        body: str | None = None,
    ) -> None: ...
    def __repr__(self) -> str: ...

class Docstring:
    """High-level docstring model used for emit / round-trip."""

    summary: str | None
    extended_summary: str | None
    directives: list[Directive]
    sections: list[Section]
    @property
    def deprecation(self) -> Directive | None:
        """Computed convenience: the first directive named ``deprecated``.

        Read-only — edit ``directives`` to change it.
        """
    def __init__(
        self,
        *,
        summary: str | None = None,
        extended_summary: str | None = None,
        directives: list[Directive] | None = None,
        sections: list[Section] | None = None,
    ) -> None: ...
    def __repr__(self) -> str: ...

# ─── Functions ───────────────────────────────────────────────────────────────

def parse(input: str) -> GoogleDocstring | NumPyDocstring | PlainDocstring:
    """Auto-detect the style and parse ``input``.

    Check ``.style`` or use ``isinstance`` to distinguish the returned type.
    """
    ...

def parse_google(input: str) -> GoogleDocstring:
    """Parse ``input`` as a Google-style docstring."""
    ...

def parse_numpy(input: str) -> NumPyDocstring:
    """Parse ``input`` as a NumPy-style docstring."""
    ...

def parse_plain(input: str) -> PlainDocstring:
    """Parse ``input`` as a plain docstring (no section markers)."""
    ...

def detect_style(input: str) -> Style:
    """Detect the docstring style without fully parsing."""
    ...

def emit_google(doc: Docstring, base_indent: int = 0) -> str:
    """Render a model ``Docstring`` as Google-style text."""
    ...

def emit_numpy(doc: Docstring, base_indent: int = 0) -> str:
    """Render a model ``Docstring`` as NumPy-style text."""
    ...

def emit_sphinx(doc: Docstring, base_indent: int = 0) -> str:
    """Render a model ``Docstring`` as Sphinx-style (reStructuredText) text."""
    ...

def walk(
    doc: GoogleDocstring | NumPyDocstring | PlainDocstring,
    visitor: _VisitorT,
) -> _VisitorT:
    """Walk any docstring depth-first, calling typed methods on ``visitor``.

    Accepts a `GoogleDocstring`, `NumPyDocstring`, or `PlainDocstring`.
    ``visitor`` must subclass :class:`Visitor`; override only the ``enter_*``
    / ``exit_*`` methods you need — all others are silently skipped.
    Returns ``visitor`` so results can be collected inline.

    Every ``enter_*`` method receives ``(node, ctx: WalkContext)`` as arguments
    and fires before the node's children are visited; the matching ``exit_*``
    hook (same signature) fires after the children, e.g. ``exit_google_section``
    runs once every entry of that section has been visited.
    Use ``ctx.line_col(offset)`` to convert byte offsets to line/column positions.

    .. code-block:: python

        class MyChecker(Visitor):
            def enter_google_arg(self, arg: GoogleArg, ctx: WalkContext) -> None:
                lc = ctx.line_col(arg.range.start)
            def enter_numpy_parameter(self, param: NumPyParameter, ctx: WalkContext) -> None: ...

        checker = MyChecker()
        for source_text in all_docstrings:
            doc = pydocstring.parse(source_text)
            pydocstring.walk(doc, checker)  # returns the visitor

    Google-style ``enter_*`` methods (each has a matching ``exit_*`` hook):

    .. code-block:: python

        def enter_google_docstring(self, doc: GoogleDocstring, ctx: WalkContext) -> None: ...
        def enter_google_section(self, section: GoogleSection, ctx: WalkContext) -> None: ...
        def enter_google_deprecation(self, dep: GoogleDeprecation, ctx: WalkContext) -> None: ...
        def enter_google_arg(self, arg: GoogleArg, ctx: WalkContext) -> None: ...
        def enter_google_return(self, ret: GoogleReturn, ctx: WalkContext) -> None: ...
        def enter_google_yield(self, yld: GoogleYield, ctx: WalkContext) -> None: ...
        def enter_google_exception(self, exc: GoogleException, ctx: WalkContext) -> None: ...
        def enter_google_warning(self, wrn: GoogleWarning, ctx: WalkContext) -> None: ...
        def enter_google_see_also_item(self, sai: GoogleSeeAlsoItem, ctx: WalkContext) -> None: ...
        def enter_google_reference(self, ref: GoogleReference, ctx: WalkContext) -> None: ...
        def enter_google_attribute(self, att: GoogleAttribute, ctx: WalkContext) -> None: ...
        def enter_google_method(self, mtd: GoogleMethod, ctx: WalkContext) -> None: ...

    NumPy-style ``enter_*`` methods (each has a matching ``exit_*`` hook):

    .. code-block:: python

        def enter_numpy_docstring(self, doc: NumPyDocstring, ctx: WalkContext) -> None: ...
        def enter_numpy_section(self, section: NumPySection, ctx: WalkContext) -> None: ...
        def enter_numpy_deprecation(self, dep: NumPyDeprecation, ctx: WalkContext) -> None: ...
        def enter_numpy_parameter(self, param: NumPyParameter, ctx: WalkContext) -> None: ...
        def enter_numpy_returns(self, ret: NumPyReturns, ctx: WalkContext) -> None: ...
        def enter_numpy_yields(self, yld: NumPyYields, ctx: WalkContext) -> None: ...
        def enter_numpy_exception(self, exc: NumPyException, ctx: WalkContext) -> None: ...
        def enter_numpy_warning(self, wrn: NumPyWarning, ctx: WalkContext) -> None: ...
        def enter_numpy_see_also_item(self, sai: NumPySeeAlsoItem, ctx: WalkContext) -> None: ...
        def enter_numpy_reference(self, ref: NumPyReference, ctx: WalkContext) -> None: ...
        def enter_numpy_attribute(self, att: NumPyAttribute, ctx: WalkContext) -> None: ...
        def enter_numpy_method(self, mtd: NumPyMethod, ctx: WalkContext) -> None: ...

    Plain ``enter_*`` methods (each has a matching ``exit_*`` hook):

    .. code-block:: python

        def enter_plain_docstring(self, doc: PlainDocstring, ctx: WalkContext) -> None: ...
    """
    ...
