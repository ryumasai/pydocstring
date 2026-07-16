"""Type stubs for ``pydocstring.model`` — the normalized, position-free IR.

Produced by ``Docstring.to_model()``. Dropping source positions is what lets
this layer apply context-dependent semantics (such as merging consecutive lines
into one ``Block.Paragraph``) that the tree cannot express without breaking edit
locality — and it is why the model is a one-way projection: inspect, transform,
and re-emit through it, but edit through the unified view instead.
"""

from __future__ import annotations

from typing_extensions import Self
from typing_extensions import disjoint_base
from typing_extensions import final

from . import SectionKind as SectionKind

__all__ = [
    "SectionKind",
    "Docstring",
    "Section",
    "Block",
    "Parameter",
    "Return",
    "ExceptionEntry",
    "SeeAlsoEntry",
    "Reference",
    "Attribute",
    "Method",
    "Directive",
]

@final
class Directive:
    """A document-level rST directive (``.. name:: argument`` + body).

    A deprecation notice is a directive with ``name == "deprecated"`` whose
    ``argument`` is the version.
    """

    name: str
    argument: str | None
    description: str | None
    def __new__(
        cls,
        name: str,
        *,
        argument: str | None = None,
        description: str | None = None,
    ) -> Self: ...
    def __repr__(self) -> str: ...

@final
class Parameter:
    names: list[str]
    type_annotation: str | None
    description: str | None
    is_optional: bool
    default_value: str | None
    def __new__(
        cls,
        names: list[str],
        *,
        type_annotation: str | None = None,
        description: str | None = None,
        is_optional: bool = False,
        default_value: str | None = None,
    ) -> Self: ...
    def __repr__(self) -> str: ...

@final
class Return:
    name: str | None
    type_annotation: str | None
    description: str | None
    def __new__(
        cls,
        *,
        name: str | None = None,
        type_annotation: str | None = None,
        description: str | None = None,
    ) -> Self: ...
    def __repr__(self) -> str: ...

@final
class ExceptionEntry:
    type_name: str
    description: str | None
    def __new__(cls, type_name: str, *, description: str | None = None) -> Self: ...
    def __repr__(self) -> str: ...

@final
class SeeAlsoEntry:
    names: list[str]
    description: str | None
    def __new__(cls, names: list[str], *, description: str | None = None) -> Self: ...
    def __repr__(self) -> str: ...

@final
class Reference:
    label: str | None
    content: str | None
    def __new__(cls, *, label: str | None = None, content: str | None = None) -> Self: ...
    def __repr__(self) -> str: ...

@final
class Attribute:
    names: list[str]
    type_annotation: str | None
    description: str | None
    def __new__(
        cls,
        names: list[str],
        *,
        type_annotation: str | None = None,
        description: str | None = None,
    ) -> Self: ...
    def __repr__(self) -> str: ...

@final
class Method:
    name: str
    type_annotation: str | None
    description: str | None
    def __new__(
        cls,
        name: str,
        *,
        type_annotation: str | None = None,
        description: str | None = None,
    ) -> Self: ...
    def __repr__(self) -> str: ...

@disjoint_base
class Block:
    """A body block within a :class:`Section`, mirroring the core ``model::Block``.

    A structured section body is a flat sequence of blocks in source order:
    prose :class:`Block.Paragraph`\\ s interleaved with typed entries. Match a
    block with ``isinstance(block, Block.Parameter)`` etc.; the entry variants
    expose the wrapped model object as ``.value``, and ``Paragraph`` exposes
    ``.text``.
    """

    @final
    class Paragraph(Block):
        __match_args__ = ("text",)
        text: str
        def __new__(cls, text: str) -> Self: ...

    @final
    class Parameter(Block):
        __match_args__ = ("value",)
        value: Parameter
        def __new__(cls, value: Parameter) -> Self: ...

    @final
    class Return(Block):
        __match_args__ = ("value",)
        value: Return
        def __new__(cls, value: Return) -> Self: ...

    @final
    class Exception(Block):
        __match_args__ = ("value",)
        value: ExceptionEntry
        def __new__(cls, value: ExceptionEntry) -> Self: ...

    @final
    class Attribute(Block):
        __match_args__ = ("value",)
        value: Attribute
        def __new__(cls, value: Attribute) -> Self: ...

    @final
    class Method(Block):
        __match_args__ = ("value",)
        value: Method
        def __new__(cls, value: Method) -> Self: ...

    @final
    class SeeAlso(Block):
        __match_args__ = ("value",)
        value: SeeAlsoEntry
        def __new__(cls, value: SeeAlsoEntry) -> Self: ...

    @final
    class Reference(Block):
        __match_args__ = ("value",)
        value: Reference
        def __new__(cls, value: Reference) -> Self: ...

@final
class Section:
    """A docstring section in the model IR: a :class:`SectionKind` paired with a
    flat sequence of :class:`Block`\\ s in source order.

    Filter ``blocks`` by variant to read typed entries, e.g.
    ``[b.value for b in section.blocks if isinstance(b, Block.Parameter)]``.
    ``unknown_name`` carries the header text of an unrecognised free-text
    section (``SectionKind.UNKNOWN``).
    """
    @property
    def kind(self) -> SectionKind: ...
    @property
    def blocks(self) -> list[Block]: ...
    @property
    def unknown_name(self) -> str | None: ...
    def __new__(
        cls,
        kind: SectionKind,
        blocks: list[Block] | None = None,
        *,
        unknown_name: str | None = None,
    ) -> Self: ...
    def __repr__(self) -> str: ...

@final
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
    def __new__(
        cls,
        *,
        summary: str | None = None,
        extended_summary: str | None = None,
        directives: list[Directive] | None = None,
        sections: list[Section] | None = None,
    ) -> Self: ...
    def __repr__(self) -> str: ...
