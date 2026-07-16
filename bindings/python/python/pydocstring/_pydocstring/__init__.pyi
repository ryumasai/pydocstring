"""Type stubs for the native ``pydocstring._pydocstring`` extension module.

Parses Google-style and NumPy-style Python docstrings into a single ``Parsed``,
read through three lenses: the style-independent ``Document`` view, the raw CST
(``.syntax``), and the normalized model (``to_model()``). Every view keeps its
byte range, so results double as edit anchors.

Missing values, and which lens shows them:

* On the ``Document`` view, ``None`` means **not present** — it is the semantic
  lens, and it hides the parser's zero-length missing placeholders. It cannot
  tell ``x ():`` from ``x:``, and that is deliberate.
* The raw CST keeps them: ``Node.find_missing(kind)`` returns the placeholder
  (``Token.is_missing()`` is ``True``, its range is zero-length) where the
  syntax marker is present but the content is absent. That zero-length range is
  the anchor to write the missing content at. ``Node.tokens(kind)`` and
  ``find_token(kind)`` both exclude placeholders.
"""

from __future__ import annotations

from typing import TypeVar

from typing_extensions import Self
from typing_extensions import final

from .._visitor import Visitor
from . import model as model
from .model import Docstring

__all__ = [
    "parse",
    "parse_google",
    "parse_numpy",
    "parse_plain",
    "detect_style",
    "emit_google",
    "emit_numpy",
    "emit_sphinx",
    "walk",
    "Style",
    "Parsed",
    "TextRange",
    "LineColumn",
    "Token",
    "TextBlock",
    "WalkContext",
    "Match",
    "Capture",
    "PatternError",
    "SyntaxKind",
    "Node",
    "Document",
    "Section",
    "Entry",
    "DefaultMarker",
    "Directive",
    "Citation",
    "Edits",
    "EditError",
    "RewriteError",
    "SectionKind",
    "model",
]

_VisitorT = TypeVar("_VisitorT", bound="Visitor")

# ─── Core types ──────────────────────────────────────────────────────────────

@final
class TextRange:
    """Byte range ``[start, end)`` within the source string.

    A value type: compares and hashes by ``(start, end)``. It is a **byte**
    range, not a code-point range, so slicing it into a `str` is wrong for
    non-ASCII sources — anchor an ``Edits`` on it instead.

    The range carries no link back to the parse result it came from. Anchoring
    an edit on a range from a *different* ``Parsed`` is a byte offset into a
    different string: it will splice at the wrong place rather than raise.
    """

    start: int
    end: int
    def __new__(cls, start: int, end: int) -> Self:
        """Build a range from two byte offsets.

        Every range a view hands you is already one of these; this is for the
        spans a view *doesn't* hand you. Nothing is validated here — a range is
        two numbers; ``Edits.apply()`` rejects an invalid one.
        """
    def is_empty(self) -> bool:
        """Return ``True`` when ``start == end`` (zero-length placeholder)."""
        ...
    def source_text(self, source: str) -> str:
        """The slice of ``source`` this range covers.

        Use this rather than slicing yourself: the range is in **bytes** and a
        ``str`` indexes by code point, so ``source[r.start:r.end]`` cuts in the
        wrong place as soon as anything upstream of the range is non-ASCII.

        Returns ``""`` for a range that is out of bounds, inverted, or splits a
        character — all reachable, since a range is just two numbers.
        """
        ...
    def __len__(self) -> int:
        """The length of the range, in bytes."""
        ...
    def __bool__(self) -> bool:
        """Always ``True``.

        Without this, ``__len__`` would make an **empty range falsy** — and an
        empty range is the zero-length placeholder that marks where a missing
        element goes, i.e. the anchor you insert at. Say ``is_empty()`` when you
        mean empty.
        """
        ...
    def __contains__(self, offset: object, /) -> bool:
        """Whether a byte offset falls within ``[start, end)``. Never raises."""
        ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

@final
class LineColumn:
    """1-based line number, 0-based **byte** column. Compares by value.

    ``col`` is the UTF-8 byte offset within the line — the same convention as
    :attr:`ast.AST.col_offset`. Every offset in this API is a byte offset, so a
    column measured in characters would not compose with them.
    """

    lineno: int
    col: int
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

@final
class WalkContext:
    """Context passed to every ``Visitor`` hook during a ``walk()`` call."""
    def line_col(self, offset: int) -> LineColumn:
        """Byte offset -> ``LineColumn``. Same as ``Parsed.line_col``."""
        ...
    def line_indent(self, offset: int) -> str:
        """The leading whitespace of the line ``offset`` falls on.

        Same as ``Parsed.line_indent``. A visitor is where you usually want it.
        """
        ...
    def __repr__(self) -> str: ...

class PatternError(ValueError):
    """Raised when a pattern string has no valid reading."""

class RewriteError(ValueError):
    """Raised by ``replace()`` / ``replace_in()`` when a template names a
    metavariable the matched reading does not bind."""

@final
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

@final
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

@final
class Token:
    """A text fragment plus its source byte range and :class:`SyntaxKind`."""
    @property
    def kind(self) -> SyntaxKind: ...
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
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

@final
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

@final
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
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

# ─── Section kinds ───────────────────────────────────────────────────────────

@final
class SectionKind:
    """Style-independent section kind.

    Shared vocabulary: it is what ``Section.kind`` resolves a header name to in
    the unified view, and what a :class:`pydocstring.model.Section` carries.
    """

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

# ─── Raw CST — the fidelity lens ─────────────────────────────────────────────

@final
class SyntaxKind:
    """The kind of a CST node or token.

    ``UNKNOWN`` is a read-only result: the crate's ``SyntaxKind`` is
    non-exhaustive, so a newer core can produce a kind this build has no member
    for. It cannot be passed as a query argument.
    """

    NAME: SyntaxKind
    TYPE: SyntaxKind
    COLON: SyntaxKind
    COMMA: SyntaxKind
    DESCRIPTION: SyntaxKind
    OPEN_BRACKET: SyntaxKind
    CLOSE_BRACKET: SyntaxKind
    OPTIONAL: SyntaxKind
    SUMMARY: SyntaxKind
    EXTENDED_SUMMARY: SyntaxKind
    TEXT_LINE: SyntaxKind
    WHITESPACE: SyntaxKind
    NEWLINE: SyntaxKind
    BLANK_LINE: SyntaxKind
    UNDERLINE: SyntaxKind
    DIRECTIVE_MARKER: SyntaxKind
    DIRECTIVE_NAME: SyntaxKind
    DOUBLE_COLON: SyntaxKind
    ARGUMENT: SyntaxKind
    DEFAULT_KEYWORD: SyntaxKind
    DEFAULT_SEPARATOR: SyntaxKind
    DEFAULT_VALUE: SyntaxKind
    LABEL: SyntaxKind
    DOCUMENT: SyntaxKind
    SECTION: SyntaxKind
    SECTION_HEADER: SyntaxKind
    ENTRY: SyntaxKind
    DIRECTIVE: SyntaxKind
    CITATION: SyntaxKind
    DEFAULT: SyntaxKind
    PARAGRAPH: SyntaxKind
    UNKNOWN: SyntaxKind
    @property
    def name(self) -> str:
        """The kind's name, e.g. ``"ENTRY"``. ``UNKNOWN`` names itself."""
    def is_node(self) -> bool:
        """Whether this kind is a branch of the tree (it has children)."""
        ...
    def is_token(self) -> bool:
        """Whether this kind is a leaf of the tree."""
        ...
    def is_trivia(self) -> bool:
        """Whether this kind is trivia — whitespace, a newline, or a blank line.

        The CST keeps trivia because an edit has to; a *reader* usually wants to
        skip it. Without this a caller hard-codes the set of trivia kinds and
        re-derives it whenever the grammar grows one.
        """
        ...
    def __repr__(self) -> str: ...

@final
class Node:
    """A node of the concrete syntax tree — the faithful lens.

    Keeps every byte: punctuation, trivia, and the zero-length missing
    placeholders the unified view deliberately hides. The tree's vocabulary is
    style-independent, so one type walks any docstring.

    Reach it from any unified view or parse result with ``.syntax``.
    """
    @property
    def kind(self) -> SyntaxKind: ...
    @property
    def range(self) -> TextRange: ...
    @property
    def text(self) -> str:
        """The raw source slice of this node's range."""
    @property
    def children(self) -> list[Node | Token]:
        """Every child, in source order — a mix of nodes and tokens."""
    def nodes(self, kind: SyntaxKind) -> list[Node]:
        """Direct child nodes of ``kind``, in source order."""
        ...
    def tokens(self, kind: SyntaxKind) -> list[Token]:
        """Direct child tokens of ``kind``. Missing placeholders are excluded."""
        ...
    def find_node(self, kind: SyntaxKind) -> Node | None: ...
    def find_token(self, kind: SyntaxKind) -> Token | None:
        """The first present (non-missing) direct child token of ``kind``."""
        ...
    def find_missing(self, kind: SyntaxKind) -> Token | None:
        """The first *missing* (zero-length) direct child token of ``kind``.

        This is what tells ``x ():`` — an empty type between brackets, so a
        placeholder exists — apart from ``x:``, where the grammar produced no
        type token at all. The placeholder's range is the insertion anchor.
        """
        ...
    def __eq__(self, other: object, /) -> bool:
        """Same kind, same range, same source text.

        Accessors hand out a fresh wrapper on every access, so identity would
        make even ``node == node`` false.
        """
        ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

# ─── Unified views — the style-independent read lens ─────────────────────────

@final
class Document:
    """Style-independent view of a parsed docstring.

    One code path for every style: ``"Args:"`` (Google) and ``"Parameters"``
    (NumPy) both resolve to ``SectionKind.PARAMETERS``, so callers never branch
    on style::

        doc = pydocstring.Document(pydocstring.parse(src))
        for section in doc.sections:
            if section.kind == pydocstring.SectionKind.PARAMETERS:
                for entry in section.entries:
                    print(entry.name.text)

    Every view keeps its byte range, so results are usable directly as edit
    anchors. Unlike the per-style CST wrappers, these views never surface
    zero-length missing placeholders: ``None`` means "not present".
    """

    def __new__(cls, parsed: Parsed) -> Self: ...
    @property
    def range(self) -> TextRange: ...
    @property
    def style(self) -> Style: ...
    @property
    def source(self) -> str: ...
    @property
    def summary(self) -> TextBlock | None: ...
    @property
    def extended_summary(self) -> TextBlock | None: ...
    @property
    def sections(self) -> list[Section]: ...
    @property
    def directives(self) -> list[Directive]: ...
    @property
    def paragraphs(self) -> list[TextBlock]:
        """Stray-prose paragraph blocks between sections, in source order."""
    def edit(self) -> Edits:
        """Start an empty edit list anchored on this docstring."""
        ...
    @property
    def syntax(self) -> Node:
        """The underlying CST node — the escape hatch down to the faithful lens."""
    def __repr__(self) -> str: ...

@final
class Section:
    """Style-independent view of one section.

    ``kind`` is the section's role as *data*, resolved from the header name via
    the source style's section-name table.
    """
    @property
    def range(self) -> TextRange: ...
    @property
    def header_name(self) -> str:
        """The header text as written (e.g. ``"Args"``, ``"Parameters"``)."""
    @property
    def kind(self) -> SectionKind: ...
    @property
    def unknown_name(self) -> str | None:
        """Header text of an unrecognised section (``kind == UNKNOWN``), else ``None``."""
    @property
    def entries(self) -> list[Entry]: ...
    @property
    def body(self) -> TextBlock | None:
        """Free-text body, for sections carrying prose rather than entries."""
    @property
    def citations(self) -> list[Citation]: ...
    @property
    def syntax(self) -> Node:
        """The underlying CST node — the escape hatch down to the faithful lens."""
    def __repr__(self) -> str: ...

@final
class Entry:
    """Style-independent view of one entry: a parameter, return, yield,
    exception, warning, attribute, method, or "See Also" item.

    All roles share one type — the role is the parent section's ``kind``. Every
    accessor is optional, so reading an entry never raises for a role that does
    not carry that piece.
    """
    @property
    def range(self) -> TextRange: ...
    @property
    def name(self) -> Token | None:
        """The first name. ``None`` for entries that carry a type instead (Raises)."""
    @property
    def names(self) -> list[Token]:
        """All names — an entry can declare several comma-separated ones."""
    @property
    def type_annotation(self) -> Token | None: ...
    @property
    def description(self) -> TextBlock | None: ...
    @property
    def is_optional(self) -> bool: ...
    @property
    def optionals(self) -> list[Token]: ...
    @property
    def defaults(self) -> list[DefaultMarker]:
        """Every ``default …`` marker, one per occurrence, in source order."""
    @property
    def default_value(self) -> Token | None:
        """The first ``default …`` value — first occurrence wins, as in the model."""
    @property
    def syntax(self) -> Node:
        """The underlying CST node — the escape hatch down to the faithful lens."""
    def __repr__(self) -> str: ...

@final
class DefaultMarker:
    """One ``default …`` marker inside a type annotation."""
    @property
    def range(self) -> TextRange: ...
    @property
    def keyword(self) -> Token: ...
    @property
    def separator(self) -> Token | None: ...
    @property
    def value(self) -> Token | None: ...
    @property
    def syntax(self) -> Node:
        """The underlying CST node — the escape hatch down to the faithful lens."""
    def __repr__(self) -> str: ...

@final
class Directive:
    """Style-independent view of a directive (e.g. ``.. deprecated:: 1.6.0``)."""
    @property
    def range(self) -> TextRange: ...
    @property
    def name(self) -> Token: ...
    @property
    def argument(self) -> Token | None: ...
    @property
    def description(self) -> TextBlock | None: ...
    @property
    def syntax(self) -> Node:
        """The underlying CST node — the escape hatch down to the faithful lens."""
    def __repr__(self) -> str: ...

@final
class Citation:
    """Style-independent view of a citation in a References section."""
    @property
    def range(self) -> TextRange: ...
    @property
    def label(self) -> Token | None: ...
    @property
    def description(self) -> TextBlock | None: ...
    @property
    def syntax(self) -> Node:
        """The underlying CST node — the escape hatch down to the faithful lens."""
    def __repr__(self) -> str: ...

# ─── Parsed ──────────────────────────────────────────────────────────────────

@final
class Parsed:
    """A parsed docstring, whatever its style.

    One type for every style: the tree's vocabulary is style-independent, so
    there is nothing for a per-style wrapper to add. Read it through
    ``Document(parsed)`` (the semantic lens), ``parsed.syntax`` (the faithful
    CST), or ``parsed.to_model()`` (the normalized IR); edit it through
    ``parsed.edit()``.
    """
    @property
    def style(self) -> Style: ...
    @property
    def source(self) -> str: ...
    @property
    def range(self) -> TextRange: ...
    @property
    def syntax(self) -> Node:
        """The root CST node — the faithful lens."""
    def line_col(self, offset: int) -> LineColumn:
        """Byte offset -> ``LineColumn`` (1-based line, 0-based **byte** column).

        A byte column composes with the rest of the API — ``offset - col`` is
        the start of the line. It is **not** a display width and **not** an
        indent: ``" " * col`` over-indents a line containing a multi-byte
        character and turns a tab into a space. Use ``line_indent()``.

        Raises ``ValueError`` if the offset is past the end of the source.
        """
        ...
    def line_indent(self, offset: int) -> str:
        """The leading whitespace of the line ``offset`` falls on.

        The indent an edit anchored there has to match, as the literal
        characters to copy — the only form that survives a tab-indented
        docstring and a non-ASCII line alike.

        Raises ``ValueError`` if the offset is past the end of the source.
        """
        ...
    def pretty_print(self) -> str:
        """A debug rendering of the syntax tree."""
        ...
    def to_model(self) -> Docstring:
        """Convert to the normalized, position-free model IR."""
        ...
    def edit(self) -> Edits:
        """Start an empty edit list anchored on this parse result."""
        ...
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
    def replace_in(
        self,
        anchor: Document | Section | Entry,
        pattern: str,
        template: str,
    ) -> str:
        """Like :meth:`replace`, but scoped to ``anchor``'s subtree.

        ``anchor`` is a :class:`Document`, :class:`Section`, or :class:`Entry`
        view of *this* parse result — a plain docstring has no sections, so only
        a ``Document`` anchor applies there. Raises ``TypeError`` for anything
        else, and ``ValueError`` for a view of a different parse result.

        The anchor also selects the *reading*: an entry line is a ``$NAME`` under
        a parameters section and a ``$TYPE`` under a raises section, so the same
        pattern reads differently depending on where it is scoped.
        """
        ...
    def findall_in(self, anchor: Document | Section | Entry, pattern: str) -> list[Match]:
        """Like :meth:`findall`, but scoped to ``anchor``'s subtree.

        Same anchor rules as :meth:`replace_in`.
        """
        ...
    def __repr__(self) -> str: ...

# ─── Editing — anchored splice edits ─────────────────────────────────────────

class EditError(ValueError):
    """Raised by :meth:`Edits.apply` when the edit list is invalid.

    Either a range is out of bounds, or two edits overlap (touching ranges are
    fine). A ``ValueError`` subclass.
    """

@final
class Edits:
    """A list of pending edits anchored on one parse result.

    Everything an edit does not touch is preserved byte-for-byte: an empty edit
    list reproduces the source exactly, and replacing an element with its own
    text is the identity. Anchor edits on the ``range`` of any view::

        doc = pydocstring.Document(pydocstring.parse(src))
        edits = doc.edit()
        for section in doc.sections:
            if section.kind == pydocstring.SectionKind.PARAMETERS:
                for entry in section.entries:
                    edits.replace(entry.description.range, "Better.")
        result = edits.apply()
    """

    def replace(self, range: TextRange, text: str) -> None:
        """Replace the bytes of ``range`` with ``text``.

        A zero-length range inserts at that offset — which is how a missing
        placeholder token (``token.is_missing()``) works as an insertion anchor.
        Empty ``text`` deletes. Ranges are validated by ``apply()``, not here.
        """
        ...
    def insert(self, at: int, text: str) -> None:
        """Insert ``text`` at byte offset ``at``.

        Multiple inserts at the same offset are applied in call order.
        """
        ...
    def delete(self, range: TextRange) -> None:
        """Delete the bytes of ``range``."""
        ...
    def remove_lines(self, range: TextRange) -> None:
        """Delete ``range`` together with the whole line(s) it occupies.

        Takes the leading indentation, the trailing newline, and one adjacent
        trailing blank line if the tree has one there.
        """
        ...
    def apply(self) -> str:
        """Validate the edit list and splice it into a new source string.

        Non-consuming: the list can be applied again or added to afterwards.
        Raises :class:`EditError` for an out-of-bounds or overlapping edit.
        """
        ...
    def apply_reparsed(self) -> Parsed:
        """``apply()`` the edits, then re-parse the result.

        The style is deliberately **not** re-detected: editing must not silently
        reinterpret the docstring as another style.
        """
        ...
    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

# ─── Functions ───────────────────────────────────────────────────────────────

def parse(input: str) -> Parsed:
    """Auto-detect the style and parse ``input``. Check ``.style`` for the result."""
    ...

def parse_google(input: str) -> Parsed:
    """Parse ``input`` as a Google-style docstring."""
    ...

def parse_numpy(input: str) -> Parsed:
    """Parse ``input`` as a NumPy-style docstring."""
    ...

def parse_plain(input: str) -> Parsed:
    """Parse ``input`` as a plain docstring (no section markers)."""
    ...

def detect_style(input: str) -> Style:
    """Detect the docstring style without fully parsing."""
    ...

def emit_google(doc: Docstring, base_indent: int = 0) -> str:
    """Emit a model ``Docstring`` as Google-style text."""
    ...

def emit_numpy(doc: Docstring, base_indent: int = 0) -> str:
    """Emit a model ``Docstring`` as NumPy-style text."""
    ...

def emit_sphinx(doc: Docstring, base_indent: int = 0) -> str:
    """Emit a model ``Docstring`` as Sphinx-style (reStructuredText) text."""
    ...

def walk(target: Parsed | Node, visitor: _VisitorT) -> _VisitorT:
    """Walk a parse result or a subtree depth-first, calling the visitor's hooks.

    The visitor may override any of ``enter_node(node, ctx)``,
    ``leave_node(node, ctx)``, and ``visit_token(token, ctx)``; the ones it
    leaves alone are never called. Dispatch on ``node.kind`` / ``token.kind`` —
    the traversal is style-independent. Returns the visitor, so state can be read
    straight off the call. Exceptions raised in a hook propagate out of ``walk``.
    """
    ...
