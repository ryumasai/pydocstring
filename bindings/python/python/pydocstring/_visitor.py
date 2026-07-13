"""Visitor base class for pydocstring's walk()."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from . import Node
    from . import Token
    from . import WalkContext


def _pydocstring_noop(fn):  # type: ignore[no-untyped-def]
    fn.__pydocstring_noop__ = True
    return fn


class Visitor:
    """Base class for a CST traversal.

    Override whichever hooks you need; the ones you leave alone are never
    called, so an unused hook costs nothing per node. `walk()` returns the
    visitor, so state can be read straight off the call::

        class Names(pydocstring.Visitor):
            def __init__(self):
                self.names = []

            def visit_token(self, token, ctx):
                if token.kind == pydocstring.SyntaxKind.NAME:
                    self.names.append(token.text)

        names = pydocstring.walk(pydocstring.parse(src), Names()).names

    The traversal is style-independent: dispatch on `node.kind` / `token.kind`,
    never on the docstring's style. An exception raised in a hook propagates out
    of `walk()`.
    """

    @_pydocstring_noop
    def enter_node(self, node: Node, ctx: WalkContext) -> None:
        """Called on entering a node, before its children are visited."""

    @_pydocstring_noop
    def leave_node(self, node: Node, ctx: WalkContext) -> None:
        """Called on leaving a node, after its children have been visited."""

    @_pydocstring_noop
    def visit_token(self, token: Token, ctx: WalkContext) -> None:
        """Called for each token leaf, in source order."""
