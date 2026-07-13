from typing import TypeVar

import pytest

import pydocstring

_T = TypeVar("_T")


def present(value: _T | None) -> _T:
    """Assert that an optional view accessor is present, and return it.

    Every accessor on the unified view is optional by design — a `Raises:` entry
    has no name, an entry may have no type — so reading one in a test *is* an
    assertion that it is there. Saying so keeps the type checker honest instead
    of silently dereferencing `None`.
    """
    assert value is not None
    return value


def _entries(section, variant):
    """The `.value` of every block of a given `Block` variant in `section`."""
    return [b.value for b in section.blocks if isinstance(b, variant)]


def _paragraph(section):
    """The joined text of a section's `Block.Paragraph` blocks (its prose body)."""
    texts = [b.text for b in section.blocks if isinstance(b, pydocstring.model.Block.Paragraph)]
    return "\n".join(texts) if texts else None


# The raw-CST vocabulary is style-independent (#126), so one alias serves every
# test below.
K = pydocstring.SyntaxKind


def _google(src):
    """The unified view over a Google-forced parse."""
    return pydocstring.Document(pydocstring.parse_google(src))


def _numpy(src):
    """The unified view over a NumPy-forced parse."""
    return pydocstring.Document(pydocstring.parse_numpy(src))


def _plain(src):
    """The unified view over a Plain-forced parse."""
    return pydocstring.Document(pydocstring.parse_plain(src))


class TestDetectStyle:
    def test_google(self):
        assert pydocstring.detect_style("Summary.\n\nArgs:\n    x: Desc.") == pydocstring.Style.GOOGLE

    def test_numpy(self):
        assert (
            pydocstring.detect_style("Summary.\n\nParameters\n----------\nx : int\n    Desc.")
            == pydocstring.Style.NUMPY
        )

    def test_fallback_to_plain(self):
        assert pydocstring.detect_style("Just a summary.") == pydocstring.Style.PLAIN

    def test_str(self):
        assert str(pydocstring.Style.GOOGLE) == "google"
        assert str(pydocstring.Style.NUMPY) == "numpy"
        assert str(pydocstring.Style.PLAIN) == "plain"

    def test_repr(self):
        assert repr(pydocstring.Style.GOOGLE) == "Style.GOOGLE"
        assert repr(pydocstring.Style.NUMPY) == "Style.NUMPY"
        assert repr(pydocstring.Style.PLAIN) == "Style.PLAIN"

    def test_hashable(self):
        emitters = {pydocstring.Style.GOOGLE: "google", pydocstring.Style.NUMPY: "numpy"}
        assert emitters[pydocstring.Style.GOOGLE] == "google"
        assert len({pydocstring.Style.PLAIN, pydocstring.Style.PLAIN}) == 1

    def test_no_int_equality(self):
        # 0.3.0 dropped int equality: Style.GOOGLE == 0 was a footgun.
        assert pydocstring.Style.GOOGLE != 0
        assert pydocstring.Style.NUMPY != 1
        assert pydocstring.Style.PLAIN != 2


class TestParseGoogle:
    def test_summary(self):
        doc = _google("Summary line.")
        assert doc.summary is not None
        assert doc.summary.text == "Summary line."

    def test_args(self):
        parsed = pydocstring.parse_google("Summary.\n\nArgs:\n    x (int): The value.\n    y (str): Another.")
        sections = pydocstring.Document(parsed).sections
        assert len(sections) == 1
        # #119: the per-style GoogleSectionKind.ARGS is now the shared
        # SectionKind.PARAMETERS; the verbatim header survives as header_name.
        assert sections[0].kind == pydocstring.SectionKind.PARAMETERS
        assert sections[0].header_name == "Args"

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.args = []

            def enter_node(self, node, ctx):
                if node.kind == K.ENTRY:
                    self.args.append(node)

        args = pydocstring.walk(parsed, Collector()).args
        assert len(args) == 2
        assert args[0].find_token(K.NAME).text == "x"
        assert args[0].find_token(K.TYPE).text == "int"
        assert args[0].find_node(K.DESCRIPTION).text == "The value."
        assert args[1].find_token(K.NAME).text == "y"
        assert args[1].find_token(K.TYPE).text == "str"

    def test_args_multiple_names_and_default_value(self):
        doc = _google(
            "Summary.\n\nArgs:\n    x1, x2 (int): The values.\n    order (str, optional, default 'C'): Layout."
        )
        args = doc.sections[0].entries
        assert len(args) == 2
        assert [n.text for n in args[0].names] == ["x1", "x2"]
        assert args[0].name.text == "x1"
        assert args[0].default_value is None
        assert [n.text for n in args[1].names] == ["order"]
        assert args[1].type_annotation.text == "str"
        # Was ``arg.optional is not None``; the unified Entry spells the same
        # fact as ``is_optional`` plus the token list it was read from.
        assert args[1].is_optional is True
        assert [t.text for t in args[1].optionals] == ["optional"]
        marker = args[1].defaults[0]
        assert marker.keyword.text == "default"
        assert marker.separator is None
        assert marker.value is not None
        assert marker.value.text == "'C'"
        assert args[1].default_value is not None
        assert args[1].default_value.text == "'C'"

    def test_returns(self):
        section = _google("Summary.\n\nReturns:\n    bool: True if successful.").sections[0]
        assert section.kind == pydocstring.SectionKind.RETURNS

        entries = section.entries
        assert len(entries) == 1
        ret = entries[0]
        # Was ``GoogleReturn.return_type``; every role now reads the same slot.
        assert ret.type_annotation.text == "bool"
        assert ret.description.text == "True if successful."

    def test_raises(self):
        section = _google("Summary.\n\nRaises:\n    ValueError: If x is negative.").sections[0]
        assert section.kind == pydocstring.SectionKind.RAISES

        excepts = section.entries
        assert len(excepts) == 1
        assert excepts[0].type_annotation.text == "ValueError"
        assert excepts[0].description.text == "If x is negative."

    def test_warns_type(self):
        section = _google("Summary.\n\nWarns:\n    UserWarning: When deprecated.").sections[0]
        assert section.kind == pydocstring.SectionKind.WARNS

        warnings = section.entries
        assert len(warnings) == 1
        # 0.3.0 unified on ``.type`` (was ``warning_type``); #119 unified it
        # again on ``Entry.type_annotation``. Neither old alias survives.
        assert warnings[0].type_annotation.text == "UserWarning"
        assert not hasattr(warnings[0], "warning_type")
        assert warnings[0].description.text == "When deprecated."

    def test_extended_summary(self):
        doc = _google("Summary.\n\nExtended description here.")
        assert doc.extended_summary is not None
        assert doc.extended_summary.text == "Extended description here."

    def test_deprecation(self):
        parsed = pydocstring.parse_google("Summary.\n\n.. deprecated:: 1.6.0\n    Use new_func instead.")
        doc = pydocstring.Document(parsed)
        # #119 dropped the ``deprecation`` specialization: a deprecated
        # directive is just a Directive named "deprecated", and what used to be
        # ``version`` is the directive's argument.
        deps = [d for d in doc.directives if d.name.text == "deprecated"]
        assert len(deps) == 1
        dep = deps[0]
        assert present(dep.argument).text == "1.6.0"
        assert dep.description is not None
        assert dep.description.text == "Use new_func instead."
        assert doc.extended_summary is None

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.deps = []

            def enter_node(self, node, ctx):
                if node.kind == K.DIRECTIVE:
                    self.deps.append(node)

        walked = pydocstring.walk(parsed, Collector()).deps
        assert len(walked) == 1
        assert walked[0].find_token(K.ARGUMENT).text == "1.6.0"
        assert repr(dep) == 'Directive("deprecated")'

    def test_generic_directive_hook(self):
        # Every rST directive is a DIRECTIVE node — after #119 there is no
        # deprecation specialization to fire alongside it.
        parsed = pydocstring.parse_google("Summary.\n\n.. versionadded:: 2.0\n\nArgs:\n    x: v\n")

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.events = []

            def enter_node(self, node, ctx):
                if node.kind == K.DIRECTIVE:
                    self.events.append(("directive", node.find_token(K.DIRECTIVE_NAME).text))

        collector = pydocstring.walk(parsed, Collector())
        assert collector.events == [("directive", "versionadded")]

        d = pydocstring.Document(parsed).directives[0]
        assert d.name.text == "versionadded"
        assert present(d.argument).text == "2.0"
        assert d.description is None
        assert repr(d) == 'Directive("versionadded")'

    def test_deprecated_directive_fires_both_hooks_generic_first(self):
        # Was: the generic directive hook fired first, then the deprecation
        # specialization. #119 removed the specialization, so a deprecated
        # directive is *exactly one* DIRECTIVE node whose ARGUMENT is the
        # version that used to be reached as ``GoogleDeprecation.version``.
        parsed = pydocstring.parse_google("Summary.\n\n.. deprecated:: 1.6.0\n\nArgs:\n    x: v\n")

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.events = []

            def enter_node(self, node, ctx):
                if node.kind == K.DIRECTIVE:
                    self.events.append(
                        (
                            node.find_token(K.DIRECTIVE_NAME).text,
                            node.find_token(K.ARGUMENT).text,
                        )
                    )

        events = pydocstring.walk(parsed, Collector()).events
        assert events == [("deprecated", "1.6.0")]

    def test_paragraphs_between_sections(self):
        text = "Summary.\n\nArgs:\n    a: desc.\n\nstray one\nstray two\n\nstray three\n\nReturns:\n    int: result.\n"
        doc = _google(text)
        # Stray prose lines between sections are PARAGRAPH text blocks: lines
        # separated only by a newline form one paragraph, a blank line splits.
        paragraphs = doc.paragraphs
        assert [p.logical_text for p in paragraphs] == ["stray one\nstray two", "stray three"]
        assert [line.text for line in paragraphs[0].lines] == ["stray one", "stray two"]
        # The deprecated ``stray_lines`` alias was removed in 0.3.0.
        assert not hasattr(doc, "stray_lines")

    def test_body_text_section(self):
        parsed = pydocstring.parse_google("Summary.\n\nNotes:\n    Some free text.")
        section = pydocstring.Document(parsed).sections[0]
        assert section.kind == pydocstring.SectionKind.NOTES
        assert section.entries == []
        assert present(section.body).logical_text == "Some free text."

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.sections = []

            def enter_node(self, node, ctx):
                if node.kind == K.SECTION:
                    self.sections.append(node)

        sections = pydocstring.walk(parsed, Collector()).sections
        assert len(sections) == 1
        assert sections[0].range.start == section.range.start
        assert sections[0].range.end == section.range.end

    def test_references(self):
        section = _google(
            'Summary.\n\nReferences:\n    .. [1] Author A, "Title A", 2020.\n    Plain reference line.'
        ).sections[0]
        assert section.kind == pydocstring.SectionKind.REFERENCES

        refs = section.citations
        assert len(refs) == 2
        # ``directive_marker`` is punctuation: it lives in the CST, not the view.
        assert refs[0].syntax.find_token(K.DIRECTIVE_MARKER).text == ".."
        assert refs[0].label.text == "1"
        # Was ``GoogleReference.content``; the unified Citation calls it
        # ``description``.
        assert refs[0].description.text == 'Author A, "Title A", 2020.'
        assert refs[1].syntax.find_token(K.DIRECTIVE_MARKER) is None
        assert refs[1].label is None
        assert refs[1].description.text == "Plain reference line."

    def test_pretty_print(self):
        parsed = pydocstring.parse_google("Summary.\n\nArgs:\n    x: Desc.")
        output = parsed.pretty_print()
        assert "DOCUMENT" in output
        assert "SUMMARY" in output

    def test_source(self):
        text = "Summary.\n\nArgs:\n    x: Desc."
        parsed = pydocstring.parse_google(text)
        assert parsed.source == text

    def test_no_summary(self):
        doc = _google("")
        assert doc.summary is None

    def test_yields_is_optional(self):
        section = _google("Summary.\n\nYields:\n    int: The next value.").sections[0]
        assert section.kind == pydocstring.SectionKind.YIELDS

        entries = section.entries
        assert len(entries) == 1
        assert entries[0].type_annotation.text == "int"

    def test_section_kind_repr(self):
        # #119 collapsed GoogleSectionKind into the shared SectionKind: "Args"
        # resolves to PARAMETERS, and the repr names the shared enum.
        section = _google("Summary.\n\nArgs:\n    x: Desc.").sections[0]
        assert repr(section.kind) == "SectionKind.PARAMETERS"
        assert repr(pydocstring.SectionKind.RETURNS) == "SectionKind.RETURNS"

    def test_range_on_token(self):
        doc = _google("Summary.")
        assert doc.summary is not None
        r = doc.summary.range
        assert r.start == 0
        assert r.end == 8


class TestParseNumPy:
    def test_summary(self):
        doc = _numpy("Summary line.")
        assert doc.summary is not None
        assert doc.summary.text == "Summary line."

    def test_parameters(self):
        parsed = pydocstring.parse_numpy(
            "Summary.\n\nParameters\n----------\nx : int\n    The first.\ny : str\n    The second."
        )
        sections = pydocstring.Document(parsed).sections
        assert len(sections) == 1
        assert sections[0].kind == pydocstring.SectionKind.PARAMETERS
        assert sections[0].header_name == "Parameters"

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.params = []

            def enter_node(self, node, ctx):
                if node.kind == K.ENTRY:
                    self.params.append(node)

        walked = pydocstring.walk(parsed, Collector()).params
        assert len(walked) == 2

        params = sections[0].entries
        assert len(params) == 2
        assert [n.text for n in params[0].names] == ["x"]
        # ``name`` is the first-name convenience — one accessor for every style.
        assert params[0].name is not None
        assert params[0].name.text == "x"
        assert present(params[0].type_annotation).text == "int"
        assert present(params[0].description).text == "The first."
        assert [n.text for n in params[1].names] == ["y"]

    def test_parameter_multiple_names_first_name(self):
        params = _numpy("Summary.\n\nParameters\n----------\nx1, x2 : int\n    The values.").sections[0].entries
        assert [n.text for n in params[0].names] == ["x1", "x2"]
        assert params[0].name.text == "x1"

    def test_paragraphs_property(self):
        # Parity with the Google document's ``paragraphs``. The NumPy grammar
        # lets the extended summary and section bodies absorb stray prose, so
        # the list is typically empty — but the accessor exists, same type.
        doc = _numpy("Summary.\n\nParameters\n----------\nx : int\n    Desc.\n")
        assert doc.paragraphs == []
        assert not hasattr(doc, "stray_lines")

    def test_returns(self):
        section = _numpy("Summary.\n\nReturns\n-------\nbool\n    True if successful.").sections[0]
        assert section.kind == pydocstring.SectionKind.RETURNS

        returns = section.entries
        assert len(returns) == 1
        assert returns[0].type_annotation.text == "bool"
        assert returns[0].description.text == "True if successful."

    def test_raises(self):
        section = _numpy("Summary.\n\nRaises\n------\nValueError\n    If x is negative.").sections[0]
        assert section.kind == pydocstring.SectionKind.RAISES

        excepts = section.entries
        assert len(excepts) == 1
        assert excepts[0].type_annotation.text == "ValueError"

    def test_pretty_print(self):
        parsed = pydocstring.parse_numpy("Summary.\n\nParameters\n----------\nx : int\n    Desc.")
        output = parsed.pretty_print()
        assert "DOCUMENT" in output

    def test_source(self):
        text = "Summary.\n\nParameters\n----------\nx : int\n    Desc."
        parsed = pydocstring.parse_numpy(text)
        assert parsed.source == text

    def test_section_kind_repr(self):
        # #119 collapsed NumPySectionKind into the shared SectionKind.
        section = _numpy("Summary.\n\nParameters\n----------\nx : int\n    Desc.").sections[0]
        assert repr(section.kind) == "SectionKind.PARAMETERS"
        assert repr(pydocstring.SectionKind.RETURNS) == "SectionKind.RETURNS"


class TestToken:
    def test_text_and_range(self):
        doc = _google("Summary.")
        assert doc.summary is not None
        token = doc.summary.lines[0]
        assert token.text == "Summary."
        assert token.range.start == 0
        assert token.range.end == 8

    def test_repr(self):
        doc = _google("Hello.")
        assert doc.summary is not None
        assert repr(doc.summary.lines[0]) == 'Token("Hello.")'

    def test_kind(self):
        """Tokens carry their kind (#126).

        They deliberately did not before: on a typed wrapper the field name
        (`.name`, `.description`) already implied the kind, so `kind` was
        redundant. That reasoning does not survive the raw CST lens, where a
        token arrives from `node.children` with no field name attached to it.
        """
        doc = _google("Summary.")
        assert doc.summary is not None
        assert doc.summary.lines[0].kind == pydocstring.SyntaxKind.TEXT_LINE


class TestTextBlock:
    def test_text_is_raw_slice(self):
        doc = _google("Summary.")
        block = doc.summary
        assert block is not None
        assert block.text == "Summary."
        assert block.range.start == 0
        assert block.range.end == 8

    def test_repr(self):
        doc = _google("Hello.")
        assert repr(doc.summary) == 'TextBlock("Hello.")'

    def test_lines_one_token_per_content_line(self):
        doc = _google("Summary.\n\nArgs:\n    x: First line.\n        Cont.")
        block = doc.sections[0].entries[0].description
        assert [line.text for line in block.lines] == ["First line.", "Cont."]
        # Raw text keeps the interior newline and indentation.
        assert block.text == "First line.\n        Cont."

    def test_logical_text_dedents_continuation(self):
        doc = _google("Summary.\n\nArgs:\n    x: First line.\n        Cont.")
        block = doc.sections[0].entries[0].description
        assert block.logical_text == "First line.\nCont."

    def test_multiline_summary_text_matches_source_slice(self):
        doc = _plain("First summary line.\nSecond summary line.")
        assert doc.summary is not None
        assert doc.summary.text == "First summary line.\nSecond summary line."
        assert [line.text for line in doc.summary.lines] == [
            "First summary line.",
            "Second summary line.",
        ]

    def test_missing_description_block(self):
        parsed = pydocstring.parse_google("Summary.\n\nArgs:\n    x (int):")
        entry = pydocstring.Document(parsed).sections[0].entries[0]
        # The unified view never surfaces a zero-length placeholder: None means
        # "not present". The placeholder itself lives in the CST, where the
        # empty DESCRIPTION node is the block the old wrapper handed back.
        assert entry.description is None
        block = entry.syntax.find_node(K.DESCRIPTION)
        assert block is not None
        assert block.range.is_empty()  # was block.is_missing()
        assert block.text == ""
        assert block.children == []  # was block.lines == []

    def test_is_missing_false_for_present_token(self):
        entry = _google("Summary.\n\nArgs:\n    x (int): Desc.").sections[0].entries[0]
        assert not entry.type_annotation.is_missing()

    def test_is_missing_true_for_empty_parens(self):
        # "x ():" — brackets present but type content is absent
        parsed = pydocstring.parse_google("Summary.\n\nArgs:\n    x (): Desc.")
        entry = pydocstring.Document(parsed).sections[0].entries[0]
        assert entry.type_annotation is None  # hidden by the semantic lens
        placeholder = entry.syntax.find_missing(K.TYPE)
        assert placeholder is not None
        assert placeholder.is_missing()


class TestTextRange:
    def test_range_repr(self):
        doc = _google("Summary.")
        assert doc.summary is not None
        r = doc.summary.range
        assert repr(r) == "TextRange(0..8)"

    def test_section_range(self):
        section = _google("Summary.\n\nArgs:\n    x: Desc.").sections[0]
        r = section.range
        assert r.start < r.end

    def test_is_empty_false_for_normal_range(self):
        doc = _google("Summary.")
        assert doc.summary is not None
        assert not doc.summary.range.is_empty()

    def test_is_empty_true_for_missing_token(self):
        entry = _google("Summary.\n\nArgs:\n    x (): Desc.").sections[0].entries[0]
        placeholder = entry.syntax.find_missing(K.TYPE)
        assert placeholder is not None
        assert placeholder.range.is_empty()


class TestLineColumn:
    def test_summary_start(self):
        parsed = pydocstring.parse_plain("Summary.")
        result = []

        class V(pydocstring.Visitor):
            def enter_node(self, node, ctx):
                if node.kind == K.DOCUMENT:
                    result.append(ctx.line_col(node.find_node(K.SUMMARY).range.start))

        pydocstring.walk(parsed, V())
        assert result[0].lineno == 1
        assert result[0].col == 0

    def test_extended_summary_start(self):
        parsed = pydocstring.parse_plain("Summary.\n\nExtended.")
        result = []

        class V(pydocstring.Visitor):
            def enter_node(self, node, ctx):
                if node.kind == K.DOCUMENT:
                    result.append(ctx.line_col(node.find_node(K.EXTENDED_SUMMARY).range.start))

        pydocstring.walk(parsed, V())
        assert result[0].lineno == 3
        assert result[0].col == 0

    def test_repr(self):
        parsed = pydocstring.parse_plain("Summary.")
        result = []

        class V(pydocstring.Visitor):
            def enter_node(self, node, ctx):
                if node.kind == K.DOCUMENT:
                    result.append(ctx.line_col(0))

        pydocstring.walk(parsed, V())
        assert repr(result[0]) == "LineColumn(lineno=1, col=0)"


class TestModelTypes:
    def test_parameter_construction(self):
        p = pydocstring.model.Parameter(["x"], type_annotation="int", description="The value.")
        assert p.names == ["x"]
        assert p.type_annotation == "int"
        assert p.description == "The value."
        assert p.is_optional is False
        assert p.default_value is None

    def test_parameter_mutability(self):
        p = pydocstring.model.Parameter(["x"])
        p.names = ["x", "y"]
        p.type_annotation = "str"
        p.is_optional = True
        assert p.names == ["x", "y"]
        assert p.type_annotation == "str"
        assert p.is_optional is True

    def test_parameter_construction_validates_names(self):
        with pytest.raises(TypeError):
            pydocstring.model.Parameter([1, 2])  # ty: ignore[invalid-argument-type]

    def test_attribute_names_setter_validates(self):
        a = pydocstring.model.Attribute(names=["x"])
        with pytest.raises(TypeError):
            a.names = [1]  # ty: ignore[invalid-assignment]

    def test_parameter_names_setter_validates(self):
        p = pydocstring.model.Parameter(["x"])
        with pytest.raises(TypeError):
            p.names = [1, 2]  # ty: ignore[invalid-assignment]
        assert p.names == ["x"]

    def test_parameter_repr(self):
        p = pydocstring.model.Parameter(["x", "y"])
        assert repr(p) == "Parameter(names=['x', 'y'])"

    def test_return_construction(self):
        r = pydocstring.model.Return(type_annotation="int", description="The result.")
        assert r.name is None
        assert r.type_annotation == "int"
        assert r.description == "The result."

    def test_exception_entry_construction(self):
        e = pydocstring.model.ExceptionEntry("ValueError", description="If x is negative.")
        assert e.type_name == "ValueError"
        assert e.description == "If x is negative."

    def test_directive_construction(self):
        d = pydocstring.model.Directive("deprecated", argument="1.6.0", description="Use new_func instead.")
        assert d.name == "deprecated"
        assert d.argument == "1.6.0"
        assert d.description == "Use new_func instead."

    def test_directive_defaults_and_repr(self):
        d = pydocstring.model.Directive("versionadded")
        assert d.argument is None
        assert d.description is None
        assert repr(d) == 'Directive("versionadded")'

    def test_see_also_entry_construction_validates_names(self):
        with pytest.raises(TypeError):
            pydocstring.model.SeeAlsoEntry(names=[1])  # ty: ignore[invalid-argument-type]

    def test_attribute_construction(self):
        a = pydocstring.model.Attribute(["name"], type_annotation="str", description="The name.")
        assert a.names == ["name"]
        assert a.type_annotation == "str"

    def test_attribute_construction_multiple_names(self):
        a = pydocstring.model.Attribute(["jac", "hess"], type_annotation="ndarray")
        assert a.names == ["jac", "hess"]

    def test_attribute_construction_validates_names(self):
        with pytest.raises(TypeError):
            pydocstring.model.Attribute(names=[1])  # ty: ignore[invalid-argument-type]

    def test_method_construction(self):
        m = pydocstring.model.Method("run", description="Run the task.")
        assert m.name == "run"
        assert m.type_annotation is None
        assert m.description == "Run the task."

    def test_see_also_entry_construction(self):
        s = pydocstring.model.SeeAlsoEntry(["foo", "bar"], description="Related functions.")
        assert s.names == ["foo", "bar"]
        assert s.description == "Related functions."

    def test_see_also_entry_names_setter_validates(self):
        s = pydocstring.model.SeeAlsoEntry(["foo"])
        with pytest.raises(TypeError):
            s.names = [1]  # ty: ignore[invalid-assignment]
        assert s.names == ["foo"]

    def test_reference_construction(self):
        r = pydocstring.model.Reference(label="1", content="Doe et al. 2020")
        assert r.label == "1"
        assert r.content == "Doe et al. 2020"


class TestSection:
    def test_parameters_section(self):
        p = pydocstring.model.Parameter(["x"], type_annotation="int", description="Value.")
        sec = pydocstring.model.Section(pydocstring.SectionKind.PARAMETERS, [pydocstring.model.Block.Parameter(p)])
        assert sec.kind == pydocstring.SectionKind.PARAMETERS
        params = _entries(sec, pydocstring.model.Block.Parameter)
        assert len(params) == 1
        assert params[0].names == ["x"]
        assert params[0].type_annotation == "int"

    def test_returns_section(self):
        r = pydocstring.model.Return(type_annotation="bool", description="Success.")
        sec = pydocstring.model.Section(pydocstring.SectionKind.RETURNS, [pydocstring.model.Block.Return(r)])
        assert sec.kind == pydocstring.SectionKind.RETURNS
        rets = _entries(sec, pydocstring.model.Block.Return)
        assert len(rets) == 1
        assert rets[0].type_annotation == "bool"

    def test_raises_section(self):
        e = pydocstring.model.ExceptionEntry("ValueError", description="Bad value.")
        sec = pydocstring.model.Section(pydocstring.SectionKind.RAISES, [pydocstring.model.Block.Exception(e)])
        assert sec.kind == pydocstring.SectionKind.RAISES
        exceptions = _entries(sec, pydocstring.model.Block.Exception)
        assert len(exceptions) == 1
        assert exceptions[0].type_name == "ValueError"

    def test_free_text_section(self):
        sec = pydocstring.model.Section(
            pydocstring.SectionKind.NOTES,
            [pydocstring.model.Block.Paragraph("Some notes here.")],
        )
        assert sec.kind == pydocstring.SectionKind.NOTES
        assert _paragraph(sec) == "Some notes here."

    def test_prose_and_entries_interleave(self):
        # A structured section body is a flat block sequence: prose paragraphs
        # interleaved with typed entries, in order (#105).
        sec = pydocstring.model.Section(
            pydocstring.SectionKind.RETURNS,
            [
                pydocstring.model.Block.Paragraph("If data is array-like, returns X."),
                pydocstring.model.Block.Return(
                    pydocstring.model.Return(type_annotation="ndarray", description="the rep.")
                ),
            ],
        )
        assert [type(b).__name__ for b in sec.blocks] == ["Paragraph", "Return"]
        assert _paragraph(sec) == "If data is array-like, returns X."
        assert _entries(sec, pydocstring.model.Block.Return)[0].type_annotation == "ndarray"

    def test_empty_section_has_no_blocks(self):
        sec = pydocstring.model.Section(pydocstring.SectionKind.PARAMETERS)
        assert list(sec.blocks) == []
        assert _entries(sec, pydocstring.model.Block.Return) == []
        assert _paragraph(sec) is None

    def test_unknown_section_requires_name(self):
        with pytest.raises(ValueError, match="unknown_name"):
            pydocstring.model.Section(pydocstring.SectionKind.UNKNOWN)
        sec = pydocstring.model.Section(
            pydocstring.SectionKind.UNKNOWN,
            [pydocstring.model.Block.Paragraph("text")],
            unknown_name="Custom",
        )
        assert sec.kind == pydocstring.SectionKind.UNKNOWN
        assert sec.unknown_name == "Custom"
        # unknown_name is the only distinguishing feature of an UNKNOWN
        # section, so repr must surface it.
        assert repr(sec) == 'Section(SectionKind.UNKNOWN, unknown_name="Custom")'
        assert _paragraph(sec) == "text"

    def test_unknown_name_rejected_for_non_unknown_kind(self):
        with pytest.raises(TypeError, match="unknown_name"):
            pydocstring.model.Section(pydocstring.SectionKind.PARAMETERS, unknown_name="Nope")

    def test_blocks_must_be_block_instances(self):
        with pytest.raises(TypeError):
            pydocstring.model.Section(pydocstring.SectionKind.PARAMETERS, ["not a block"])  # ty: ignore[invalid-argument-type]


class TestDocstringModel:
    def test_construction(self):
        doc = pydocstring.model.Docstring(summary="Brief summary.")
        assert doc.summary == "Brief summary."
        assert doc.extended_summary is None
        assert doc.directives == []
        assert doc.deprecation is None
        assert doc.sections == []

    def test_mutability(self):
        doc = pydocstring.model.Docstring(summary="Old.")
        doc.summary = "New."
        assert doc.summary == "New."

    def test_repr_renders_summary(self):
        assert repr(pydocstring.model.Docstring(summary="Hi.")) == 'Docstring(summary="Hi.")'
        assert repr(pydocstring.model.Docstring()) == "Docstring(summary=None)"

    def test_with_sections(self):
        p = pydocstring.model.Parameter(["x"], type_annotation="int")
        sec = pydocstring.model.Section(pydocstring.SectionKind.PARAMETERS, [pydocstring.model.Block.Parameter(p)])
        doc = pydocstring.model.Docstring(summary="Brief.", sections=[sec])
        assert len(doc.sections) == 1
        assert doc.sections[0].kind == pydocstring.SectionKind.PARAMETERS

        params = _entries(doc.sections[0], pydocstring.model.Block.Parameter)
        assert len(params) == 1
        params[0].description = "foo"
        assert params[0].description == "foo"

    def test_with_directives(self):
        dep = pydocstring.model.Directive("deprecated", argument="2.0", description="Removed.")
        doc = pydocstring.model.Docstring(directives=[dep])
        assert [d.name for d in doc.directives] == ["deprecated"]
        assert doc.deprecation is not None
        assert doc.deprecation.argument == "2.0"
        assert doc.deprecation.description == "Removed."

    def test_deprecation_is_computed_first_match(self):
        doc = pydocstring.model.Docstring()
        assert doc.deprecation is None
        doc.directives = [
            pydocstring.model.Directive("versionadded", argument="1.0"),
            pydocstring.model.Directive("deprecated", argument="2.0"),
            pydocstring.model.Directive("deprecated", argument="3.0"),
        ]
        assert doc.deprecation is not None
        assert doc.deprecation.argument == "2.0"

    def test_deprecation_is_read_only(self):
        doc = pydocstring.model.Docstring()
        with pytest.raises(AttributeError):
            doc.deprecation = pydocstring.model.Directive("deprecated")  # ty: ignore[invalid-assignment]

    def test_deprecation_kwarg_removed(self):
        with pytest.raises(TypeError):
            pydocstring.model.Docstring(deprecation=pydocstring.model.Directive("deprecated"))  # ty: ignore[unknown-argument]

    def test_directives_validated(self):
        with pytest.raises(TypeError):
            pydocstring.model.Docstring(directives=["not a directive"])  # ty: ignore[invalid-argument-type]
        doc = pydocstring.model.Docstring()
        with pytest.raises(TypeError):
            doc.directives = ["not a directive"]  # ty: ignore[invalid-assignment]

    def test_sections_setter_validated(self):
        doc = pydocstring.model.Docstring()
        with pytest.raises(TypeError):
            doc.sections = ["not a section"]  # ty: ignore[invalid-assignment]


class TestToModel:
    def test_google_to_model(self):
        docstr = "Summary.\n\nArgs:\n    x (int): The value.\n"
        doc = pydocstring.parse_google(docstr)
        model = doc.to_model()
        assert model.summary == "Summary."
        assert len(model.sections) == 1
        assert model.sections[0].kind == pydocstring.SectionKind.PARAMETERS
        params = _entries(model.sections[0], pydocstring.model.Block.Parameter)
        assert len(params) == 1
        assert params[0].names == ["x"]
        assert params[0].type_annotation == "int"
        assert params[0].description == "The value."
        assert pydocstring.emit_google(model) == docstr

    def test_numpy_to_model(self):
        docstr = "Summary.\n\nParameters\n----------\nx : int\n    The value.\n"
        doc = pydocstring.parse_numpy(docstr)
        model = doc.to_model()
        assert model.summary == "Summary."
        assert len(model.sections) == 1
        assert model.sections[0].kind == pydocstring.SectionKind.PARAMETERS
        params = _entries(model.sections[0], pydocstring.model.Block.Parameter)
        assert len(params) == 1
        assert params[0].names == ["x"]
        assert params[0].type_annotation == "int"
        assert pydocstring.emit_numpy(model) == docstr

    def test_numpy_yields_to_model_round_trip(self):
        docstr = "Summary.\n\nYields\n------\nint\n    The next value.\n"
        model = pydocstring.parse_numpy(docstr).to_model()
        assert model.sections[0].kind == pydocstring.SectionKind.YIELDS
        assert pydocstring.emit_numpy(model) == docstr

    def test_numpy_warns_to_model_round_trip(self):
        docstr = "Summary.\n\nWarns\n-----\nUserWarning\n    When deprecated.\n"
        model = pydocstring.parse_numpy(docstr).to_model()
        assert model.sections[0].kind == pydocstring.SectionKind.WARNS
        assert pydocstring.emit_numpy(model) == docstr

    def test_emit_sphinx(self):
        docstr = "Summary.\n\nArgs:\n    x (int): The value.\n\nReturns:\n    str: The result.\n"
        model = pydocstring.parse_google(docstr).to_model()
        sphinx = pydocstring.emit_sphinx(model)
        assert ":param x: The value.\n" in sphinx
        assert ":type x: int\n" in sphinx
        assert ":return: The result.\n" in sphinx
        assert ":rtype: str\n" in sphinx

    def test_google_to_model_raises(self):
        doc = pydocstring.parse_google("Summary.\n\nRaises:\n    ValueError: Bad input.\n")
        model = doc.to_model()
        assert model.sections[0].kind == pydocstring.SectionKind.RAISES
        exceptions = _entries(model.sections[0], pydocstring.model.Block.Exception)
        assert exceptions[0].type_name == "ValueError"

    def test_google_to_model_returns(self):
        doc = pydocstring.parse_google("Summary.\n\nReturns:\n    int: The result.\n")
        model = doc.to_model()
        assert model.sections[0].kind == pydocstring.SectionKind.RETURNS
        rets = _entries(model.sections[0], pydocstring.model.Block.Return)
        assert len(rets) == 1
        assert rets[0].type_annotation == "int"

    def test_google_to_model_directives(self):
        docstr = "Summary.\n\n.. deprecated:: 1.6.0\n    Use new_func instead.\n"
        model = pydocstring.parse_google(docstr).to_model()
        assert [d.name for d in model.directives] == ["deprecated"]
        dep = model.deprecation
        assert dep is not None
        assert dep.name == "deprecated"
        assert dep.argument == "1.6.0"
        assert dep.description == "Use new_func instead."

    def test_emit_docstring_with_directive(self):
        doc = pydocstring.model.Docstring(
            summary="Summary.",
            directives=[pydocstring.model.Directive("deprecated", argument="2.0", description="Gone.")],
        )
        emitted = pydocstring.emit_google(doc)
        assert ".. deprecated:: 2.0" in emitted
        assert "Gone." in emitted

    def test_plain_to_model(self):
        doc = pydocstring.parse_plain("Brief summary.\n\nMore details.")
        model = doc.to_model()
        assert model.summary == "Brief summary."
        assert model.extended_summary == "More details."
        assert model.sections == []

    def test_plain_to_model_summary_only(self):
        doc = pydocstring.parse_plain("Just a summary.")
        model = doc.to_model()
        assert model.summary == "Just a summary."
        assert model.extended_summary is None


class TestParsePlain:
    def test_summary(self):
        doc = _plain("Summary line.")
        assert doc.summary is not None
        assert doc.summary.text == "Summary line."
        assert doc.extended_summary is None

    def test_empty(self):
        doc = _plain("")
        assert doc.summary is None
        assert doc.extended_summary is None

    def test_extended_summary(self):
        doc = _plain("Summary.\n\nMore details here.\nContinued.")
        assert doc.summary is not None
        assert doc.summary.text == "Summary."
        assert doc.extended_summary is not None
        assert "More details here." in doc.extended_summary.text

    def test_no_sections(self):
        parsed = pydocstring.parse_plain("Summary.\n\n:param x: A value.\n:returns: Something.")
        model = parsed.to_model()
        assert model.sections == []

    def test_source(self):
        text = "Summary.\n\nExtended."
        parsed = pydocstring.parse_plain(text)
        assert parsed.source == text

    def test_pretty_print(self):
        parsed = pydocstring.parse_plain("Summary.\n\nExtended.")
        output = parsed.pretty_print()
        assert "DOCUMENT" in output
        assert "SUMMARY" in output
        assert "EXTENDED_SUMMARY" in output

    def test_repr(self):
        # #119: the three per-style docstring types collapsed into one
        # ``Parsed``, so the style moved from the type name into the repr.
        parsed = pydocstring.parse_plain("Summary.")
        assert repr(parsed) == "Parsed(style=Plain)"

    def test_line_col_summary(self):
        parsed = pydocstring.parse_plain("Summary.")
        result = []

        class V(pydocstring.Visitor):
            def enter_node(self, node, ctx):
                if node.kind == K.DOCUMENT:
                    result.append(ctx.line_col(node.find_node(K.SUMMARY).range.start))

        pydocstring.walk(parsed, V())
        assert result[0].lineno == 1
        assert result[0].col == 0

    def test_line_col_extended_summary(self):
        parsed = pydocstring.parse_plain("Summary.\n\nExtended.")
        result = []

        class V(pydocstring.Visitor):
            def enter_node(self, node, ctx):
                if node.kind == K.DOCUMENT:
                    result.append(ctx.line_col(node.find_node(K.EXTENDED_SUMMARY).range.start))

        pydocstring.walk(parsed, V())
        assert result[0].lineno == 3
        assert result[0].col == 0

    def test_detect_style_dispatches_to_plain(self):
        assert pydocstring.detect_style("Just a summary.") == pydocstring.Style.PLAIN
        assert pydocstring.detect_style("Summary.\n\n:param x: value.") == pydocstring.Style.PLAIN

    def test_style_property(self):
        parsed = pydocstring.parse_plain("Summary.")
        assert parsed.style == pydocstring.Style.PLAIN

    def test_no_node_attribute(self):
        parsed = pydocstring.parse_plain("Summary.")
        assert not hasattr(parsed, "node"), "Parsed must not expose a 'node' attribute"


class TestParse:
    """Tests for the unified parse() entry point.

    #119 collapsed ``GoogleDocstring``/``NumPyDocstring``/``PlainDocstring``
    into a single ``Parsed``: the style is data (``.style``), not a type, so
    the ``isinstance`` dance is gone.
    """

    def test_google_returns_google_docstring(self):
        parsed = pydocstring.parse("Summary.\n\nArgs:\n    x (int): Value.")
        assert isinstance(parsed, pydocstring.Parsed)
        assert parsed.style == pydocstring.Style.GOOGLE

    def test_numpy_returns_numpy_docstring(self):
        parsed = pydocstring.parse("Summary.\n\nParameters\n----------\nx : int\n    Value.")
        assert isinstance(parsed, pydocstring.Parsed)
        assert parsed.style == pydocstring.Style.NUMPY

    def test_plain_returns_plain_docstring(self):
        parsed = pydocstring.parse("Just a summary.")
        assert isinstance(parsed, pydocstring.Parsed)
        assert parsed.style == pydocstring.Style.PLAIN

    def test_empty_returns_plain_docstring(self):
        parsed = pydocstring.parse("")
        assert isinstance(parsed, pydocstring.Parsed)
        assert parsed.style == pydocstring.Style.PLAIN

    def test_sphinx_returns_plain_docstring(self):
        parsed = pydocstring.parse("Summary.\n\n:param x: A value.\n:returns: Something.")
        assert isinstance(parsed, pydocstring.Parsed)
        assert parsed.style == pydocstring.Style.PLAIN

    def test_google_style_property(self):
        parsed = pydocstring.parse_google("Summary.\n\nArgs:\n    x: Desc.")
        assert parsed.style == pydocstring.Style.GOOGLE

    def test_numpy_style_property(self):
        parsed = pydocstring.parse_numpy("Summary.\n\nParameters\n----------\nx : int\n    Desc.")
        assert parsed.style == pydocstring.Style.NUMPY

    def test_parse_google_summary(self):
        doc = pydocstring.Document(pydocstring.parse("Summary.\n\nArgs:\n    x (int): Value."))
        assert doc.summary is not None
        assert doc.summary.text == "Summary."

    def test_parse_numpy_summary(self):
        doc = pydocstring.Document(pydocstring.parse("Summary.\n\nParameters\n----------\nx : int\n    Value."))
        assert doc.summary is not None
        assert doc.summary.text == "Summary."


class TestWalk:
    def test_google_walk_collects_args(self):
        source = "Summary.\n\nArgs:\n    x (int): The x value.\n    y (str): The y value."
        parsed = pydocstring.parse_google(source)

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.arg_names = []

            def enter_node(self, node, ctx):
                if node.kind == K.ENTRY:
                    self.arg_names.append(node.find_token(K.NAME).text)

        collector = Collector()
        pydocstring.walk(parsed, collector)
        assert collector.arg_names == ["x", "y"]

    def test_numpy_walk_collects_parameters(self):
        source = "Summary.\n\nParameters\n----------\nx : int\n    Desc x.\ny : str\n    Desc y."
        parsed = pydocstring.parse_numpy(source)

        # The very same visitor as the Google case: #119 made walk() generic
        # over the CST, so there is nothing style-specific left to dispatch on.
        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.names = []

            def enter_node(self, node, ctx):
                if node.kind == K.ENTRY:
                    self.names.append(node.find_token(K.NAME).text)

        collector = Collector()
        pydocstring.walk(parsed, collector)
        assert collector.names == ["x", "y"]

    def test_walk_plain_dispatches_plain_docstring(self):
        parsed = pydocstring.parse_plain("Just a summary.")

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.called = False

            def enter_node(self, node, ctx):
                if node.kind == K.DOCUMENT:
                    self.called = True
                    summary = node.find_node(K.SUMMARY)
                    assert summary is not None
                    assert summary.text == "Just a summary."

        collector = Collector()
        pydocstring.walk(parsed, collector)
        assert collector.called

    def test_walk_plain_no_google_numpy_dispatch(self):
        # A plain docstring has no sections and no entries, so the kinds a
        # structured docstring would dispatch on never appear.
        parsed = pydocstring.parse_plain("Just a summary.")

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.called = False

            def enter_node(self, node, ctx):
                if node.kind in (K.SECTION, K.ENTRY):
                    self.called = True

        collector = Collector()
        pydocstring.walk(parsed, collector)
        assert not collector.called

    def test_walk_rejects_wrong_type(self):
        with pytest.raises(TypeError):
            pydocstring.walk("not a docstring", object())  # ty: ignore[invalid-argument-type]

    def test_walk_via_parse_google(self):
        """walk() dispatches correctly when doc comes from auto-detect parse()."""
        source = "Summary.\n\nArgs:\n    z (float): A float."
        parsed = pydocstring.parse(source)
        assert parsed.style == pydocstring.Style.GOOGLE

        names = []

        class V(pydocstring.Visitor):
            def enter_node(self, node, ctx):
                if node.kind == K.ENTRY:
                    names.append(node.find_token(K.NAME).text)

        pydocstring.walk(parsed, V())
        assert names == ["z"]

    def test_walk_via_parse_numpy(self):
        """walk() dispatches correctly when doc comes from auto-detect parse()."""
        source = "Summary.\n\nParameters\n----------\na : int\n    Desc."
        parsed = pydocstring.parse(source)
        assert parsed.style == pydocstring.Style.NUMPY

        names = []

        class V(pydocstring.Visitor):
            def enter_node(self, node, ctx):
                if node.kind == K.ENTRY:
                    names.append(node.find_token(K.NAME).text)

        pydocstring.walk(parsed, V())
        assert names == ["a"]

    def test_walk_visitor_without_methods_is_safe(self):
        """A Visitor with no overrides should not raise."""
        parsed = pydocstring.parse_google("Summary.\n\nArgs:\n    x: Desc.")
        pydocstring.walk(parsed, pydocstring.Visitor())

    def test_walk_non_visitor_raises_type_error(self):
        """Passing a non-Visitor object raises TypeError."""
        parsed = pydocstring.parse_google("Summary.")
        with pytest.raises(TypeError, match="must subclass pydocstring.Visitor"):
            pydocstring.walk(parsed, object())  # ty: ignore[invalid-argument-type]

    def test_walk_returns_visitor(self):
        """walk() returns the visitor object."""
        parsed = pydocstring.parse_google("Summary.\n\nArgs:\n    x (int): Desc.")

        class V(pydocstring.Visitor):
            def enter_node(self, node, ctx):
                pass

        v = V()
        result = pydocstring.walk(parsed, v)
        assert result is v

    def test_ctx_line_col_google(self):
        """ctx.line_col() returns correct LineColumn for a given offset."""
        source = "Summary.\n\nArgs:\n    x (int): The value."
        parsed = pydocstring.parse_google(source)

        line_cols = []

        class V(pydocstring.Visitor):
            def enter_node(self, node, ctx):
                if node.kind == K.ENTRY:
                    lc = ctx.line_col(node.range.start)
                    line_cols.append((lc.lineno, lc.col))

        pydocstring.walk(parsed, V())
        assert len(line_cols) == 1
        # arg starts on line 4 (1-based), col 4 (0-based, after 4 spaces)
        assert line_cols[0] == (4, 4)

    # ── Visitor base class tests ──────────────────────────────────────────

    def test_visitor_is_importable(self):
        """pydocstring.Visitor exists and is instantiable."""
        v = pydocstring.Visitor()
        assert isinstance(v, pydocstring.Visitor)

    def test_visitor_base_methods_not_dispatched(self):
        """Unoverridden Visitor methods are not called during walk()."""
        called = []

        class V(pydocstring.Visitor):
            pass  # override nothing

        parsed = pydocstring.parse_google("Summary.\n\nArgs:\n    x: Desc.")
        pydocstring.walk(parsed, V())
        assert called == []

    def test_visitor_overridden_method_is_dispatched(self):
        """An overridden Visitor method is called during walk()."""
        names = []

        class V(pydocstring.Visitor):
            def enter_node(self, node, ctx):
                if node.kind == K.ENTRY:
                    names.append(node.find_token(K.NAME).text)

        parsed = pydocstring.parse_google("Summary.\n\nArgs:\n    x: Desc.\n    y: Desc.")
        pydocstring.walk(parsed, V())
        assert names == ["x", "y"]

    def test_visitor_only_overridden_methods_dispatched(self):
        """Only overridden methods fire; base no-ops are silent."""
        events = []

        class V(pydocstring.Visitor):
            def enter_node(self, node, ctx):
                if node.kind == K.ENTRY:
                    events.append(("enter_node", node.find_token(K.NAME).text))

            # leave_node intentionally NOT overridden

        parsed = pydocstring.parse_google("Summary.\n\nArgs:\n    x: Desc.")
        pydocstring.walk(parsed, V())
        assert events == [("enter_node", "x")]

    def test_visitor_duck_typing_raises_type_error(self):
        """Non-Visitor objects raise TypeError."""

        class Duck:
            def enter_node(self, node, ctx):
                pass

        parsed = pydocstring.parse_google("Summary.\n\nArgs:\n    z: Desc.")
        with pytest.raises(TypeError, match="must subclass pydocstring.Visitor"):
            pydocstring.walk(parsed, Duck())  # ty: ignore[invalid-argument-type]


class TestMissingDescription:
    def test_numpy_exception_missing_description_is_exposed(self):
        parsed = pydocstring.parse_numpy("Summary.\n\nRaises\n------\nValueError:\n")
        entries = pydocstring.Document(parsed).sections[0].entries
        assert len(entries) == 1
        # The semantic lens hides the placeholder; the fidelity lens keeps it.
        assert entries[0].description is None
        block = entries[0].syntax.find_node(K.DESCRIPTION)
        assert block is not None
        assert block.range.is_empty()  # was description.is_missing()

    def test_numpy_exception_continuation_replaces_placeholder(self):
        entries = _numpy("Summary.\n\nRaises\n------\nValueError:\n    Later description.\n").sections[0].entries
        assert entries[0].description is not None
        assert entries[0].description.text == "Later description."
        assert not entries[0].description.is_missing()


def _first_entry(parsed):
    """The first ENTRY node of ``parsed``'s first section, read off the CST."""
    section = parsed.syntax.find_node(K.SECTION)
    assert section is not None, "no section"
    entry = section.find_node(K.ENTRY)
    assert entry is not None, "no entry"
    return entry


def _missingness(entry, field: str) -> str:
    """The missing-value category of ``entry``'s ``field`` slot.

    Style-independent: the CST is the same vocabulary for both grammars, so
    one reader answers the question for Google and NumPy alike.

    * ``"present"`` — the slot carries content.
    * ``"missing"`` — the syntax marker is there but the content is not, so the
      parser emitted a zero-length placeholder (the insertion anchor).
    * ``"none"``    — the grammar produced no slot at all.
    """
    if field == "description":
        # DESCRIPTION is a node; an omitted-but-marked one is zero-length.
        block = entry.find_node(K.DESCRIPTION)
        if block is None:
            return "none"
        return "missing" if block.range.is_empty() else "present"
    if field == "type":
        # TYPE is a token; find_token skips placeholders, find_missing keeps only them.
        if entry.find_token(K.TYPE) is not None:
            return "present"
        return "missing" if entry.find_missing(K.TYPE) is not None else "none"
    raise AssertionError(f"unknown slot: {field}")


# LAW (cross-style missing-ness parity): for the same role and the same
# semantic omission, the Google and NumPy grammars must expose the same
# missing-value category — a zero-length placeholder when the syntax marker
# is present without content, no slot at all when the parser emits none.
# Mirrors the Rust cross-style slot-kind parity law (tests/unified.rs).
#
# #119 removed the per-style wrappers, so the per-style visitor hooks that used
# to locate each role are gone: the role is now the parent section's kind, and
# every entry is one ENTRY node. The inputs and the expected categories are
# unchanged — only the locator collapsed to a single style-independent one.
MISSINGNESS_CASES = [
    pytest.param(
        "Summary.\n\nRaises:\n    ValueError:\n",
        "Summary.\n\nRaises\n------\nValueError:\n",
        "description",
        "missing",
        id="raises-description-colon-no-text",
    ),
    pytest.param(
        "Summary.\n\nRaises:\n    ValueError\n",
        "Summary.\n\nRaises\n------\nValueError\n",
        "description",
        "none",
        id="raises-description-no-colon",
    ),
    pytest.param(
        "Summary.\n\nWarns:\n    UserWarning:\n",
        "Summary.\n\nWarns\n-----\nUserWarning:\n",
        "description",
        "missing",
        id="warns-description-colon-no-text",
    ),
    pytest.param(
        "Summary.\n\nMethods:\n    run:\n",
        "Summary.\n\nMethods\n-------\nrun:\n",
        "description",
        "missing",
        id="methods-description-colon-no-text",
    ),
    pytest.param(
        "Summary.\n\nSee Also:\n    other_func:\n",
        "Summary.\n\nSee Also\n--------\nother_func :\n",
        "description",
        "missing",
        id="see-also-description-colon-no-text",
    ),
    pytest.param(
        "Summary.\n\nArgs:\n    x (): Desc.\n",
        "Summary.\n\nParameters\n----------\nx :\n    Desc.\n",
        "type",
        "missing",
        id="parameter-type-marker-no-text",
    ),
    pytest.param(
        "Summary.\n\nArgs:\n    x: Desc.\n",
        "Summary.\n\nParameters\n----------\nx\n    Desc.\n",
        "type",
        "none",
        id="parameter-type-no-marker",
    ),
    pytest.param(
        "Summary.\n\nAttributes:\n    attr ():\n",
        "Summary.\n\nAttributes\n----------\nattr :\n",
        "type",
        "missing",
        id="attribute-type-marker-no-text",
    ),
    # Returns/yields descriptions are plain optionals in BOTH styles: even
    # with the marker present ("int:" / a typed entry), an absent description
    # stays None — never a missing placeholder (documented opt symmetry).
    pytest.param(
        "Summary.\n\nReturns:\n    int:\n",
        "Summary.\n\nReturns\n-------\nint\n",
        "description",
        "none",
        id="returns-description-absent-stays-none",
    ),
    pytest.param(
        "Summary.\n\nYields:\n    int:\n",
        "Summary.\n\nYields\n------\nint\n",
        "description",
        "none",
        id="yields-description-absent-stays-none",
    ),
]


class TestMissingnessParity:
    """Google/NumPy parity for description/type missing-ness across roles."""

    @pytest.mark.parametrize(("google_src", "numpy_src", "field", "expected"), MISSINGNESS_CASES)
    def test_parity(self, google_src, numpy_src, field, expected):
        google_entry = _first_entry(pydocstring.parse_google(google_src))
        numpy_entry = _first_entry(pydocstring.parse_numpy(numpy_src))

        google_cat = _missingness(google_entry, field)
        numpy_cat = _missingness(numpy_entry, field)

        assert google_cat == numpy_cat, f"{field}: google={google_cat}, numpy={numpy_cat}"
        assert google_cat == expected


class TestMissingnessGrammarAsymmetries:
    """Slots where google/numpy missingness legitimately differs by grammar.

    These are NOT parity violations: numpy attribute descriptions live on
    continuation lines only (there is no "marker present, content absent"
    spelling), so numpy emits no DESCRIPTION slot at all where google emits a
    zero-length placeholder. Likewise the numpy Methods grammar has no type
    slot, so the method-type column is google-only.
    """

    def test_attribute_description_missingness_differs_by_grammar(self):
        g = _first_entry(pydocstring.parse_google("Summary.\n\nAttributes:\n    x (int):\n"))
        assert _missingness(g, "description") == "missing"
        assert g.find_node(K.DESCRIPTION).range.is_empty()

        n = _first_entry(pydocstring.parse_numpy("Summary.\n\nAttributes\n----------\nx : int\n"))
        assert _missingness(n, "description") == "none"
        assert n.find_node(K.DESCRIPTION) is None

    def test_attribute_multiple_names(self):
        """Multi-name attribute entries keep every name; ``name`` is the first (#89)."""
        g = _google("Summary.\n\nAttributes:\n    jac, hess (ndarray): Derivatives.\n").sections[0].entries[0]
        assert [t.text for t in g.names] == ["jac", "hess"]
        assert g.name.text == "jac"

        n = _numpy("Summary.\n\nAttributes\n----------\njac, hess : ndarray\n    Derivatives.\n").sections[0].entries[0]
        assert [t.text for t in n.names] == ["jac", "hess"]
        assert n.name.text == "jac"

        model = pydocstring.parse_numpy(
            "Summary.\n\nAttributes\n----------\njac, hess : ndarray\n    Derivatives.\n"
        ).to_model()
        attrs = _entries(model.sections[0], pydocstring.model.Block.Attribute)
        assert attrs is not None
        assert attrs[0].names == ["jac", "hess"]


class TestRewrite:
    """`doc.replace` / `doc.findall` — the RFC #48 rewrite capstone (#47)."""

    def test_findall_returns_matches_with_captures(self):
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x (int): The value.\n    y (str): Another.\n")
        matches = doc.findall("$NAME ($TYPE): $DESC")
        assert len(matches) == 2
        assert matches[0].text == "x (int): The value."
        caps = matches[0].captures
        assert caps["NAME"].text == "x"
        assert caps["TYPE"].text == "int"
        assert caps["DESC"].text == "The value."
        first_name = matches[0].capture("NAME")
        second_name = matches[1].capture("NAME")
        assert first_name is not None and first_name.text == "x"
        assert matches[0].capture("MISSING") is None
        assert second_name is not None and second_name.text == "y"

    def test_capture_range_and_is_multi(self):
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x (int): The value.\n")
        m = doc.findall("$NAME ($TYPE): $DESC")[0]
        name = m.capture("NAME")
        assert name is not None
        # `x` sits at byte offset 20 in the source.
        assert doc.source[name.range.start : name.range.end] == "x"
        assert name.is_multi() is False

    def test_replace_roundtrips_when_template_reemits_captures(self):
        src = "Summary.\n\nArgs:\n    x (int): The value.\n    y (str): Another.\n"
        doc = pydocstring.parse_google(src)
        # A template that re-emits every capture reproduces the source exactly.
        assert doc.replace("$NAME ($TYPE): $DESC", "$NAME ($TYPE): $DESC") == src

    def test_replace_issue_26_annotate_one_entry(self):
        src = "Summary.\n\nArgs:\n    x (int): The value.\n    y (str): Kept.\n"
        doc = pydocstring.parse_google(src)
        out = doc.replace("$NAME ($TYPE): $DESC", "$NAME ($TYPE): $DESC (deprecated)")
        assert out == ("Summary.\n\nArgs:\n    x (int): The value. (deprecated)\n    y (str): Kept. (deprecated)\n")

    def test_replace_numpy_style(self):
        # A description-less parameter so the single-line `$NAME : $TYPE`
        # pattern matches the whole entry.
        src = "Summary.\n\nParameters\n----------\nx : int\n"
        doc = pydocstring.parse_numpy(src)
        matches = doc.findall("$NAME : $TYPE")
        assert any((c := m.capture("NAME")) is not None and c.text == "x" for m in matches)
        # Re-emitting the whole entry is identity.
        src_desc = "Summary.\n\nParameters\n----------\nx : int\n    The value.\n"
        assert pydocstring.parse_numpy(src_desc).replace("$$$X", "$$$X") == src_desc

    def test_replace_no_match_is_noop(self):
        src = "Summary.\n\nArgs:\n    x (int): The value.\n"
        doc = pydocstring.parse_google(src)
        assert doc.replace("$NAME (bool): $DESC", "changed") == src

    def test_replace_style_mismatch_is_noop(self):
        # A NumPy pattern against a Google document matches nothing.
        src = "Summary.\n\nArgs:\n    x (int): The value.\n"
        doc = pydocstring.parse_google(src)
        assert doc.replace("$NAME : $TYPE", "changed") == src

    def test_invalid_pattern_raises_pattern_error(self):
        doc = pydocstring.parse_google("Summary.\n")
        with pytest.raises(pydocstring.PatternError):
            doc.findall("")
        # PatternError is a ValueError subclass.
        assert issubclass(pydocstring.PatternError, ValueError)

    def test_unknown_template_metavar_raises(self):
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x (int): The value.\n")
        with pytest.raises(ValueError):
            doc.replace("$NAME ($TYPE): $DESC", "$NOPE")


# ─── Unified views (#116) ────────────────────────────────────────────────────

# The same docstring, written in each style. The point of the unified view is
# that one code path reads all of them.
GOOGLE_SRC = "Summary.\n\nArgs:\n    x (int): The value.\n    y: Another.\n"
NUMPY_SRC = "Summary.\n\nParameters\n----------\nx : int\n    The value.\ny\n    Another.\n"


def _document(src):
    return pydocstring.Document(pydocstring.parse(src))


def _parameters(doc):
    return [
        entry
        for section in doc.sections
        if section.kind == pydocstring.SectionKind.PARAMETERS
        for entry in section.entries
    ]


class TestUnifiedView:
    @pytest.mark.parametrize("src", [GOOGLE_SRC, NUMPY_SRC], ids=["google", "numpy"])
    def test_one_code_path_reads_every_style(self, src):
        """The #115 promise: no style branching, same result."""
        entries = _parameters(_document(src))
        assert [e.name.text for e in entries] == ["x", "y"]
        assert [e.type_annotation.text if e.type_annotation else None for e in entries] == ["int", None]
        assert [e.description.logical_text for e in entries] == ["The value.", "Another."]

    @pytest.mark.parametrize(
        ("src", "header"),
        [(GOOGLE_SRC, "Args"), (NUMPY_SRC, "Parameters")],
        ids=["google", "numpy"],
    )
    def test_kind_is_style_independent_but_header_name_is_verbatim(self, src, header):
        section = _document(src).sections[0]
        assert section.kind == pydocstring.SectionKind.PARAMETERS
        assert section.header_name == header
        assert section.unknown_name is None

    @pytest.mark.parametrize("src", [GOOGLE_SRC, NUMPY_SRC], ids=["google", "numpy"])
    def test_ranges_are_edit_anchors(self, src):
        """A range splice rewrites one description and moves nothing else.

        Ranges are **byte** offsets into the UTF-8 source, so the splice is on
        `src.encode()`. The summary carries a non-ASCII character precisely so
        that slicing the `str` instead would land in the wrong place and fail
        this test.
        """
        src = src.replace("Summary.", "Résumé.")
        assert len(src.encode()) != len(src), "fixture must be non-ASCII to pin byte semantics"

        entry = next(e for e in _parameters(_document(src)) if e.name.text == "y")
        r = entry.description.range

        raw = src.encode()
        edited = (raw[: r.start] + b"The other value." + raw[r.end :]).decode()
        assert edited == src.replace("Another.", "The other value.")
        # Everything outside the splice is byte-identical, including the
        # style's own indentation and punctuation.
        assert edited.count("\n") == src.count("\n")

    def test_document_style_and_source(self):
        doc = _document(GOOGLE_SRC)
        assert doc.style == pydocstring.Style.GOOGLE
        assert doc.source == GOOGLE_SRC
        assert doc.summary.text == "Summary."
        assert doc.extended_summary is None

    def test_mismatched_role_reads_none_instead_of_raising(self):
        """Every Entry accessor is optional — no panic-on-miscast path."""
        src = "Summary.\n\nRaises:\n    ValueError: If bad.\n"
        section = _document(src).sections[0]
        assert section.kind == pydocstring.SectionKind.RAISES
        entry = section.entries[0]
        # A Raises entry carries a type, not a name.
        assert entry.name is None
        assert entry.names == []
        assert entry.type_annotation.text == "ValueError"
        assert entry.description.logical_text == "If bad."
        assert entry.default_value is None
        assert entry.defaults == []
        assert entry.is_optional is False

    def test_optional_and_defaults(self):
        src = "Summary.\n\nParameters\n----------\nx : int, optional, default 1\n    Desc.\n"
        entry = _parameters(_document(src))[0]
        assert entry.is_optional is True
        assert [t.text for t in entry.optionals] == ["optional"]
        assert entry.default_value.text == "1"
        marker = entry.defaults[0]
        assert marker.keyword.text == "default"
        assert marker.value.text == "1"

    def test_repeated_defaults_expose_every_occurrence(self):
        src = "Summary.\n\nParameters\n----------\nx : int, default 1, default 2\n    Desc.\n"
        entry = _parameters(_document(src))[0]
        assert [m.value.text for m in entry.defaults] == ["1", "2"]
        # First occurrence wins, matching the model's normalization rule.
        assert entry.default_value.text == "1"

    def test_comma_separated_names(self):
        src = "Summary.\n\nArgs:\n    x, y (int): The values.\n"
        entry = _parameters(_document(src))[0]
        assert [t.text for t in entry.names] == ["x", "y"]
        assert entry.name.text == "x"

    def test_free_text_section_body(self):
        src = "Summary.\n\nNotes\n-----\nSome prose.\n"
        section = _document(src).sections[0]
        assert section.kind == pydocstring.SectionKind.NOTES
        assert section.entries == []
        assert section.body.logical_text == "Some prose."

    def test_unknown_section_carries_its_header_text(self):
        src = "Summary.\n\nFrobnicate\n----------\nSome prose.\n"
        section = _document(src).sections[0]
        assert section.kind == pydocstring.SectionKind.UNKNOWN
        assert section.unknown_name == "Frobnicate"
        assert section.header_name == "Frobnicate"

    def test_directives(self):
        # A directive is only parsed as one in a sectioned style; a docstring
        # that is nothing but a directive detects as Plain, whose parser reads
        # it as extended summary.
        src = "Summary.\n\n.. deprecated:: 1.6.0\n    Use something else.\n\nArgs:\n    x: The value.\n"
        directive = _document(src).directives[0]
        assert directive.name.text == "deprecated"
        assert directive.argument.text == "1.6.0"
        assert directive.description.logical_text == "Use something else."

    def test_citations(self):
        src = "Summary.\n\nReferences\n----------\n.. [1] The citation.\n"
        section = _document(src).sections[0]
        assert section.kind == pydocstring.SectionKind.REFERENCES
        citation = section.citations[0]
        assert citation.label.text == "1"
        assert citation.description.logical_text == "The citation."

    def test_plain_docstring_has_no_sections(self):
        doc = _document("Just a summary.\n")
        assert doc.style == pydocstring.Style.PLAIN
        assert doc.sections == []
        assert doc.summary.text == "Just a summary."

    def test_type_annotation_is_none_not_a_missing_placeholder(self):
        """The unified view mirrors the core: None means "not present"."""
        entry = _parameters(_document("Summary.\n\nArgs:\n    x ():  Desc.\n"))[0]
        assert entry.type_annotation is None

    def test_construction_rejects_a_non_docstring(self):
        with pytest.raises(TypeError):
            pydocstring.Document("Summary.\n")  # ty: ignore[invalid-argument-type]

    def test_accepts_a_style_forced_parse(self):
        doc = pydocstring.Document(pydocstring.parse_numpy(NUMPY_SRC))
        assert [e.name.text for e in _parameters(doc)] == ["x", "y"]

    def test_repr(self):
        doc = _document(GOOGLE_SRC)
        assert repr(doc.sections[0]) == 'Section("Args")'
        assert repr(doc.sections[0].entries[0]) == 'Entry("x")'


class TestModelNamespace:
    def test_model_is_a_separate_layer_from_the_unified_view(self):
        """Both layers define `Section`; they are distinct types."""
        assert pydocstring.Section is not pydocstring.model.Section
        assert pydocstring.Directive is not pydocstring.model.Directive
        assert pydocstring.Section.__module__ == "pydocstring"
        assert pydocstring.model.Section.__module__ == "pydocstring.model"

    def test_section_kind_is_shared_vocabulary(self):
        assert pydocstring.SectionKind is pydocstring.model.SectionKind

    def test_to_model_returns_the_model_type(self):
        doc = pydocstring.parse(GOOGLE_SRC).to_model()
        assert isinstance(doc, pydocstring.model.Docstring)
        assert doc.sections[0].kind == pydocstring.SectionKind.PARAMETERS


# ─── Edits (#117) ────────────────────────────────────────────────────────────


class TestEdits:
    @pytest.mark.parametrize("src", [GOOGLE_SRC, NUMPY_SRC], ids=["google", "numpy"])
    def test_style_independent_scoped_rewrite(self, src):
        """The whole point of #115: one loop, any style, byte-preserving."""
        parsed = pydocstring.parse(src)
        doc = pydocstring.Document(parsed)
        edits = parsed.edit()

        for section in doc.sections:
            if section.kind == pydocstring.SectionKind.PARAMETERS:
                for entry in section.entries:
                    if present(entry.name).text == "y":
                        edits.replace(present(entry.description).range, "The other value.")

        assert edits.apply() == src.replace("Another.", "The other value.")

    # ── Kernel laws ───────────────────────────────────────────────────────

    @pytest.mark.parametrize("src", [GOOGLE_SRC, NUMPY_SRC], ids=["google", "numpy"])
    def test_empty_edit_list_is_the_identity(self, src):
        assert pydocstring.parse(src).edit().apply() == src

    @pytest.mark.parametrize("src", [GOOGLE_SRC, NUMPY_SRC], ids=["google", "numpy"])
    def test_replacing_an_element_with_its_own_text_is_the_identity(self, src):
        parsed = pydocstring.parse(src)
        doc = pydocstring.Document(parsed)
        edits = parsed.edit()
        for section in doc.sections:
            for entry in section.entries:
                edits.replace(present(entry.description).range, present(entry.description).text)
        assert edits.apply() == src

    # ── Core operations ───────────────────────────────────────────────────

    def test_insert(self):
        parsed = pydocstring.parse(GOOGLE_SRC)
        entry = pydocstring.Document(parsed).sections[0].entries[0]
        edits = parsed.edit()
        edits.insert(present(entry.description).range.start, "NOTE: ")
        assert edits.apply() == GOOGLE_SRC.replace("The value.", "NOTE: The value.")

    def test_delete(self):
        parsed = pydocstring.parse(GOOGLE_SRC)
        entry = pydocstring.Document(parsed).sections[0].entries[0]
        edits = parsed.edit()
        edits.delete(present(entry.description).range)
        assert edits.apply() == GOOGLE_SRC.replace("The value.", "")

    def test_remove_lines_takes_the_whole_line(self):
        parsed = pydocstring.parse(GOOGLE_SRC)
        entry = pydocstring.Document(parsed).sections[0].entries[1]
        edits = parsed.edit()
        edits.remove_lines(entry.range)
        # The entry's indentation and trailing newline go with it.
        assert edits.apply() == "Summary.\n\nArgs:\n    x (int): The value.\n"

    def test_missing_placeholder_is_an_insertion_anchor(self):
        """A zero-length range inserts exactly where the absent element belongs.

        The unified view reports an absent type as ``None`` rather than as a
        placeholder, so the anchor is reached through the CST lens.
        """
        src = "Summary.\n\nArgs:\n    x (): The value.\n"
        parsed = pydocstring.parse_google(src)
        entry = pydocstring.Document(parsed).sections[0].entries[0]
        assert entry.type_annotation is None

        arg_type = entry.syntax.find_missing(pydocstring.SyntaxKind.TYPE)
        assert arg_type is not None
        assert arg_type.is_missing()
        assert present(arg_type).range.is_empty()

        edits = parsed.edit()
        edits.replace(present(arg_type).range, "int")
        assert edits.apply() == "Summary.\n\nArgs:\n    x (int): The value.\n"

    def test_insert_a_type_where_the_grammar_left_no_placeholder(self):
        """No brackets at all: anchor the insert on the name's end offset."""
        src = "Summary.\n\nArgs:\n    x: The value.\n"
        parsed = pydocstring.parse(src)
        entry = pydocstring.Document(parsed).sections[0].entries[0]
        assert entry.type_annotation is None

        edits = parsed.edit()
        edits.insert(present(entry.name).range.end, " (int)")
        assert edits.apply() == "Summary.\n\nArgs:\n    x (int): The value.\n"

    def test_several_edits_apply_together(self):
        parsed = pydocstring.parse(GOOGLE_SRC)
        entries = pydocstring.Document(parsed).sections[0].entries
        edits = parsed.edit()
        edits.replace(present(entries[0].name).range, "a")
        edits.replace(present(entries[1].name).range, "b")
        assert len(edits) == 2
        assert edits.apply() == GOOGLE_SRC.replace("x (int)", "a (int)").replace("    y:", "    b:")

    def test_apply_is_non_consuming(self):
        parsed = pydocstring.parse(GOOGLE_SRC)
        entry = pydocstring.Document(parsed).sections[0].entries[0]
        edits = parsed.edit()
        edits.replace(present(entry.description).range, "First.")
        assert edits.apply() == edits.apply()
        edits.replace(present(entry.name).range, "z")
        assert "z (int): First." in edits.apply()

    # ── Validation ────────────────────────────────────────────────────────

    def test_overlapping_edits_raise(self):
        parsed = pydocstring.parse(GOOGLE_SRC)
        entry = pydocstring.Document(parsed).sections[0].entries[0]
        edits = parsed.edit()
        edits.replace(present(entry.description).range, "a")
        edits.replace(present(entry.description).range, "b")
        with pytest.raises(pydocstring.EditError):
            edits.apply()
        assert issubclass(pydocstring.EditError, ValueError)

    def test_out_of_bounds_edit_raises(self):
        parsed = pydocstring.parse(GOOGLE_SRC)
        edits = parsed.edit()
        edits.insert(len(GOOGLE_SRC) + 100, "x")
        with pytest.raises(pydocstring.EditError):
            edits.apply()

    # ── Reparsing ─────────────────────────────────────────────────────────

    @pytest.mark.parametrize(
        ("src", "expected"),
        [(GOOGLE_SRC, pydocstring.Style.GOOGLE), (NUMPY_SRC, pydocstring.Style.NUMPY)],
        ids=["google", "numpy"],
    )
    def test_apply_reparsed_keeps_the_original_style(self, src, expected):
        """Editing must not silently reinterpret the docstring as another style."""
        parsed = pydocstring.parse(src)
        entry = pydocstring.Document(parsed).sections[0].entries[0]
        edits = parsed.edit()
        edits.replace(present(entry.description).range, "Changed.")
        reparsed = edits.apply_reparsed()
        assert reparsed.style == expected
        assert reparsed.source == edits.apply()
        # The edit is visible in the new tree, read through the unified view.
        again = pydocstring.Document(reparsed)
        assert present(again.sections[0].entries[0].description).logical_text == "Changed."

    def test_edit_is_reachable_from_the_document_too(self):
        doc = pydocstring.Document(pydocstring.parse(GOOGLE_SRC))
        edits = doc.edit()
        edits.replace(present(doc.sections[0].entries[0].description).range, "Changed.")
        assert "x (int): Changed." in edits.apply()

    def test_repr(self):
        edits = pydocstring.parse(GOOGLE_SRC).edit()
        assert repr(edits) == "Edits(0 pending)"
        assert len(edits) == 0


# ─── Scoped rewrite: replace_in / findall_in (#118) ──────────────────────────

SCOPED_SRC = "Summary.\n\nArgs:\n    x: First.\n    y: Second.\n\nRaises:\n    ValueError: Bad.\n"
NUMPY_SCOPED_SRC = (
    "Summary.\n\nParameters\n----------\nx : int\n    First.\ny : str\n    Second.\n"
    "\nRaises\n------\nValueError\n    Bad.\n"
)
PLAIN_SCOPED_SRC = "Summary line.\n\nMore detail here.\n"


def _section(doc, kind):
    return next(s for s in doc.sections if s.kind == kind)


class TestReplaceIn:
    def test_unscoped_replace_reaches_every_section(self):
        """The problem scoping solves: `$NAME: $DESC` also matches Raises."""
        parsed = pydocstring.parse_google(SCOPED_SRC)
        out = parsed.replace("$NAME: $DESC", "$NAME: TOUCHED")
        assert "x: TOUCHED" in out
        assert "ValueError: TOUCHED" in out

    def test_replace_in_scopes_to_one_section(self):
        parsed = pydocstring.parse_google(SCOPED_SRC)
        doc = pydocstring.Document(parsed)
        args = _section(doc, pydocstring.SectionKind.PARAMETERS)

        out = parsed.replace_in(args, "$NAME: $DESC", "$NAME: TOUCHED")
        assert "x: TOUCHED" in out
        assert "y: TOUCHED" in out
        # The Raises section is untouched.
        assert "ValueError: Bad." in out

    def test_the_anchor_selects_the_reading(self):
        """The same shape reads as a type under Raises, a name under Args."""
        parsed = pydocstring.parse_google(SCOPED_SRC)
        doc = pydocstring.Document(parsed)
        raises = _section(doc, pydocstring.SectionKind.RAISES)

        out = parsed.replace_in(raises, "$TYPE: $DESC", "$TYPE: TOUCHED")
        assert "ValueError: TOUCHED" in out
        assert "x: First." in out

    def test_an_entry_is_an_anchor_too(self):
        parsed = pydocstring.parse_google(SCOPED_SRC)
        doc = pydocstring.Document(parsed)
        second = _section(doc, pydocstring.SectionKind.PARAMETERS).entries[1]

        out = parsed.replace_in(second, "$NAME: $DESC", "$NAME: ONLY-Y")
        assert "y: ONLY-Y" in out
        assert "x: First." in out

    def test_findall_in_scopes_the_search(self):
        parsed = pydocstring.parse_google(SCOPED_SRC)
        doc = pydocstring.Document(parsed)
        args = _section(doc, pydocstring.SectionKind.PARAMETERS)

        assert len(parsed.findall("$NAME: $DESC")) == 3  # includes the Raises entry
        assert len(parsed.findall_in(args, "$NAME: $DESC")) == 2

    def test_scoping_works_for_numpy_too(self):
        """The style comes from the parse result, not from the call site."""
        parsed = pydocstring.parse_numpy(NUMPY_SCOPED_SRC)
        doc = pydocstring.Document(parsed)
        params = _section(doc, pydocstring.SectionKind.PARAMETERS)
        raises = _section(doc, pydocstring.SectionKind.RAISES)

        pattern = "$NAME : $TYPE\n    $DESC"
        assert len(parsed.findall_in(params, pattern)) == 2
        assert len(parsed.findall_in(raises, pattern)) == 0

        out = parsed.replace_in(params, pattern, "$NAME : $TYPE\n    TOUCHED")
        assert out.count("TOUCHED") == 2
        assert "ValueError\n    Bad." in out

    def test_a_document_is_the_only_anchor_a_plain_docstring_has(self):
        parsed = pydocstring.parse_plain(PLAIN_SCOPED_SRC)
        doc = pydocstring.Document(parsed)
        # A plain docstring has no section markers, so there is nothing narrower
        # to scope to — but the Document anchor must still dispatch correctly.
        assert doc.sections == []

        pattern = "$SUMMARY\n\n$REST"
        assert len(parsed.findall(pattern)) == 1
        assert len(parsed.findall_in(doc, pattern)) == 1
        assert parsed.replace_in(doc, pattern, "$SUMMARY\n\nREWRITTEN") == "Summary line.\n\nREWRITTEN"

    def test_a_non_view_anchor_is_rejected(self):
        parsed = pydocstring.parse_google(SCOPED_SRC)
        with pytest.raises(TypeError):
            parsed.replace_in("Args", "$NAME: $DESC", "x")  # ty: ignore[invalid-argument-type]

    def test_an_anchor_from_another_parse_is_rejected(self):
        """A NodeRef addresses a node by path, so a foreign anchor would
        silently resolve to some unrelated node of this tree."""
        parsed = pydocstring.parse_google(SCOPED_SRC)
        other = pydocstring.Document(pydocstring.parse_google(SCOPED_SRC))
        with pytest.raises(ValueError):
            parsed.replace_in(other.sections[0], "$NAME: $DESC", "x")
        with pytest.raises(ValueError):
            parsed.findall_in(other.sections[0], "$NAME: $DESC")


# ─── Raw CST — the fidelity lens (#126) ──────────────────────────────────────


class TestRawCST:
    def test_the_tree_vocabulary_is_style_independent(self):
        """The same kinds describe a Google and a NumPy entry."""
        kinds = {}
        for label, src in (("google", GOOGLE_SRC), ("numpy", NUMPY_SRC)):
            entry = pydocstring.Document(pydocstring.parse(src)).sections[0].entries[0]
            kinds[label] = entry.syntax.kind
        assert kinds["google"] == kinds["numpy"] == pydocstring.SyntaxKind.ENTRY

    def test_syntax_is_the_escape_hatch_from_the_semantic_lens(self):
        entry = pydocstring.Document(pydocstring.parse(GOOGLE_SRC)).sections[0].entries[0]
        assert entry.syntax.kind == pydocstring.SyntaxKind.ENTRY
        assert entry.syntax.range.start == entry.range.start
        assert entry.syntax.range.end == entry.range.end

    def test_find_missing_distinguishes_an_empty_type_from_no_type(self):
        """`x ():` has a zero-length TYPE placeholder; `x:` has no TYPE at all.

        The unified view reports both as `type_annotation is None` — the raw CST
        is what tells them apart, and the placeholder's range is the insertion
        anchor for adding a type.
        """
        K = pydocstring.SyntaxKind

        empty = pydocstring.Document(pydocstring.parse("Summary.\n\nArgs:\n    x (): V.\n"))
        node = empty.sections[0].entries[0].syntax
        assert node.find_token(K.TYPE) is None
        placeholder = node.find_missing(K.TYPE)
        assert placeholder is not None
        assert placeholder.is_missing()
        assert placeholder.range.is_empty()

        absent = pydocstring.Document(pydocstring.parse("Summary.\n\nArgs:\n    x: V.\n"))
        node = absent.sections[0].entries[0].syntax
        assert node.find_token(K.TYPE) is None
        assert node.find_missing(K.TYPE) is None

    def test_tokens_excludes_missing_placeholders(self):
        """`find_token` and `tokens` are the singular and plural of one question.

        Both ask "which tokens of this kind are *present*?", so both exclude
        zero-length placeholders; `find_missing` is the only door to one.
        """
        K = pydocstring.SyntaxKind
        node = _document("Summary.\n\nArgs:\n    x (): V.\n").sections[0].entries[0].syntax

        assert node.find_token(K.TYPE) is None
        assert node.tokens(K.TYPE) == []
        assert node.find_missing(K.TYPE) is not None

        # A present token is reported by both.
        assert present(node.find_token(K.NAME)).text == "x"
        assert [t.text for t in node.tokens(K.NAME)] == ["x"]
        assert node.find_missing(K.NAME) is None

    def test_children_mix_nodes_and_tokens_in_source_order(self):
        K = pydocstring.SyntaxKind
        entry = pydocstring.Document(pydocstring.parse(GOOGLE_SRC)).sections[0].entries[0]
        children = entry.syntax.children
        assert [c.kind for c in children] == [
            K.NAME,
            K.WHITESPACE,
            K.OPEN_BRACKET,
            K.TYPE,
            K.CLOSE_BRACKET,
            K.COLON,
            K.WHITESPACE,
            K.DESCRIPTION,
        ]
        assert isinstance(children[-1], pydocstring.Node)
        assert isinstance(children[0], pydocstring.Token)

    def test_queries(self):
        K = pydocstring.SyntaxKind
        root = pydocstring.parse(GOOGLE_SRC).syntax
        assert root.kind == K.DOCUMENT
        section = present(root.find_node(K.SECTION))
        assert len(section.nodes(K.ENTRY)) == 2
        entry = section.nodes(K.ENTRY)[0]
        assert present(entry.find_token(K.NAME)).text == "x"
        assert present(entry.find_token(K.TYPE)).text == "int"
        assert [t.text for t in entry.tokens(K.WHITESPACE)] == [" ", " "]
        assert root.find_node(K.CITATION) is None

    def test_node_text_is_the_raw_source_slice(self):
        entry = pydocstring.Document(pydocstring.parse(GOOGLE_SRC)).sections[0].entries[0]
        assert entry.syntax.text == "x (int): The value."

    def test_the_tree_covers_every_byte(self):
        """The coverage law — this is what makes the CST the *faithful* lens."""

        def leaves(node, out):
            for child in node.children:
                if isinstance(child, pydocstring.Node):
                    leaves(child, out)
                elif not child.is_missing():
                    out.append(child)

        for src in (GOOGLE_SRC, NUMPY_SRC, "Just a summary.\n"):
            tokens = []
            leaves(pydocstring.parse(src).syntax, tokens)
            assert "".join(t.text for t in tokens) == src

    def test_unknown_cannot_be_used_as_a_query(self):
        root = pydocstring.parse(GOOGLE_SRC).syntax
        with pytest.raises(ValueError):
            root.find_token(pydocstring.SyntaxKind.UNKNOWN)
        with pytest.raises(ValueError):
            root.nodes(pydocstring.SyntaxKind.UNKNOWN)

    def test_token_kind(self):
        entry = pydocstring.Document(pydocstring.parse(GOOGLE_SRC)).sections[0].entries[0]
        assert present(entry.name).kind == pydocstring.SyntaxKind.NAME
        assert present(entry.type_annotation).kind == pydocstring.SyntaxKind.TYPE

    def test_repr(self):
        entry = pydocstring.Document(pydocstring.parse(GOOGLE_SRC)).sections[0].entries[0]
        assert repr(entry.syntax) == "Node(ENTRY, 20..39)"
        assert repr(pydocstring.SyntaxKind.NAME) == "SyntaxKind.NAME"
