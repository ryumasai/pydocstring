"""Type stubs for ``pydocstring.model`` — the normalized, position-free IR.

Produced by ``Docstring.to_model()``. Dropping source positions is what lets
this layer apply context-dependent semantics (such as merging consecutive lines
into one ``Block.Paragraph``) that the tree cannot express without breaking edit
locality — and it is why the model is a one-way projection: inspect, transform,
and re-emit through it, but edit through the unified view instead.
"""

from __future__ import annotations

from . import SectionKind as SectionKind

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

class Block:
    """A body block within a :class:`Section`, mirroring the core ``model::Block``.

    A structured section body is a flat sequence of blocks in source order:
    prose :class:`Block.Paragraph`\\ s interleaved with typed entries. Match a
    block with ``isinstance(block, Block.Parameter)`` etc.; the entry variants
    expose the wrapped model object as ``.value``, and ``Paragraph`` exposes
    ``.text``.
    """

    class Paragraph(Block):
        text: str
        def __init__(self, text: str) -> None: ...

    class Parameter(Block):
        value: Parameter
        def __init__(self, value: Parameter) -> None: ...

    class Return(Block):
        value: Return
        def __init__(self, value: Return) -> None: ...

    class Exception(Block):
        value: ExceptionEntry
        def __init__(self, value: ExceptionEntry) -> None: ...

    class Attribute(Block):
        value: Attribute
        def __init__(self, value: Attribute) -> None: ...

    class Method(Block):
        value: Method
        def __init__(self, value: Method) -> None: ...

    class SeeAlso(Block):
        value: SeeAlsoEntry
        def __init__(self, value: SeeAlsoEntry) -> None: ...

    class Reference(Block):
        value: Reference
        def __init__(self, value: Reference) -> None: ...

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
    def __init__(
        self,
        kind: SectionKind,
        blocks: list[Block] | None = None,
        *,
        unknown_name: str | None = None,
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
