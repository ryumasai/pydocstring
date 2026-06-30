"""Visitor base class for pydocstring walk()."""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from . import (
        GoogleArg,
        GoogleAttribute,
        GoogleDocstring,
        GoogleException,
        GoogleMethod,
        GoogleReturn,
        GoogleSection,
        GoogleSeeAlsoItem,
        GoogleWarning,
        GoogleYield,
        NumPyAttribute,
        NumPyDeprecation,
        NumPyDocstring,
        NumPyException,
        NumPyMethod,
        NumPyParameter,
        NumPyReference,
        NumPyReturns,
        NumPySection,
        NumPySeeAlsoItem,
        NumPyWarning,
        NumPyYields,
        PlainDocstring,
        WalkContext,
    )


def _pydocstring_noop(fn):  # type: ignore[no-untyped-def]
    fn.__pydocstring_noop__ = True
    return fn


_ALL_VISITOR_METHODS: frozenset[str] = frozenset(
    [
        "enter_google_docstring",
        "exit_google_docstring",
        "enter_google_section",
        "exit_google_section",
        "enter_google_arg",
        "exit_google_arg",
        "enter_google_return",
        "exit_google_return",
        "enter_google_yield",
        "exit_google_yield",
        "enter_google_exception",
        "exit_google_exception",
        "enter_google_warning",
        "exit_google_warning",
        "enter_google_see_also_item",
        "exit_google_see_also_item",
        "enter_google_attribute",
        "exit_google_attribute",
        "enter_google_method",
        "exit_google_method",
        "enter_numpy_docstring",
        "exit_numpy_docstring",
        "enter_numpy_deprecation",
        "exit_numpy_deprecation",
        "enter_numpy_section",
        "exit_numpy_section",
        "enter_numpy_parameter",
        "exit_numpy_parameter",
        "enter_numpy_returns",
        "exit_numpy_returns",
        "enter_numpy_yields",
        "exit_numpy_yields",
        "enter_numpy_exception",
        "exit_numpy_exception",
        "enter_numpy_warning",
        "exit_numpy_warning",
        "enter_numpy_see_also_item",
        "exit_numpy_see_also_item",
        "enter_numpy_reference",
        "exit_numpy_reference",
        "enter_numpy_attribute",
        "exit_numpy_attribute",
        "enter_numpy_method",
        "exit_numpy_method",
        "enter_plain_docstring",
        "exit_plain_docstring",
    ]
)


class Visitor:
    """Base class for docstring visitors.

    Subclass and override only the ``enter_*`` / ``exit_*`` methods you need.
    Unoverridden methods are never called — there is no dispatch overhead for
    them during :func:`walk`.

    .. code-block:: python

        import pydocstring

        class ArgChecker(pydocstring.Visitor):
            def enter_google_arg(self, node: pydocstring.GoogleArg, ctx: pydocstring.WalkContext) -> None:
                print(node.name.text)

        pydocstring.walk(pydocstring.parse(src), ArgChecker())
    """

    # Visitor itself has no active methods.
    __pydocstring_active__: frozenset[str] = frozenset()

    def __init_subclass__(cls, **kwargs: object) -> None:
        super().__init_subclass__(**kwargs)
        # Computed once at class-definition time and stored as a class variable.
        # Rust reads this via a single extract() call — no per-instance cost.
        cls.__pydocstring_active__ = frozenset(
            name
            for name in _ALL_VISITOR_METHODS
            if not getattr(getattr(cls, name, None), "__pydocstring_noop__", False)
        )

    # ── Google ────────────────────────────────────────────────────────────
    @_pydocstring_noop
    def enter_google_docstring(self, node: GoogleDocstring, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_google_docstring(self, node: GoogleDocstring, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_google_section(self, node: GoogleSection, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_google_section(self, node: GoogleSection, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_google_arg(self, node: GoogleArg, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_google_arg(self, node: GoogleArg, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_google_return(self, node: GoogleReturn, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_google_return(self, node: GoogleReturn, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_google_yield(self, node: GoogleYield, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_google_yield(self, node: GoogleYield, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_google_exception(self, node: GoogleException, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_google_exception(self, node: GoogleException, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_google_warning(self, node: GoogleWarning, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_google_warning(self, node: GoogleWarning, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_google_see_also_item(self, node: GoogleSeeAlsoItem, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_google_see_also_item(self, node: GoogleSeeAlsoItem, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_google_attribute(self, node: GoogleAttribute, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_google_attribute(self, node: GoogleAttribute, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_google_method(self, node: GoogleMethod, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_google_method(self, node: GoogleMethod, ctx: WalkContext, /) -> None:
        pass

    # ── NumPy ─────────────────────────────────────────────────────────────
    @_pydocstring_noop
    def enter_numpy_docstring(self, node: NumPyDocstring, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_numpy_docstring(self, node: NumPyDocstring, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_numpy_deprecation(self, node: NumPyDeprecation, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_numpy_deprecation(self, node: NumPyDeprecation, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_numpy_section(self, node: NumPySection, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_numpy_section(self, node: NumPySection, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_numpy_parameter(self, node: NumPyParameter, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_numpy_parameter(self, node: NumPyParameter, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_numpy_returns(self, node: NumPyReturns, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_numpy_returns(self, node: NumPyReturns, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_numpy_yields(self, node: NumPyYields, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_numpy_yields(self, node: NumPyYields, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_numpy_exception(self, node: NumPyException, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_numpy_exception(self, node: NumPyException, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_numpy_warning(self, node: NumPyWarning, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_numpy_warning(self, node: NumPyWarning, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_numpy_see_also_item(self, node: NumPySeeAlsoItem, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_numpy_see_also_item(self, node: NumPySeeAlsoItem, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_numpy_reference(self, node: NumPyReference, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_numpy_reference(self, node: NumPyReference, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_numpy_attribute(self, node: NumPyAttribute, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_numpy_attribute(self, node: NumPyAttribute, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def enter_numpy_method(self, node: NumPyMethod, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_numpy_method(self, node: NumPyMethod, ctx: WalkContext, /) -> None:
        pass

    # ── Plain ─────────────────────────────────────────────────────────────
    @_pydocstring_noop
    def enter_plain_docstring(self, node: PlainDocstring, ctx: WalkContext, /) -> None:
        pass

    @_pydocstring_noop
    def exit_plain_docstring(self, node: PlainDocstring, ctx: WalkContext, /) -> None:
        pass
