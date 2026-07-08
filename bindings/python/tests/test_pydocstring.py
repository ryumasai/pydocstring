import pytest

import pydocstring


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
        doc = pydocstring.parse_google("Summary line.")
        assert doc.summary is not None
        assert doc.summary.text == "Summary line."

    def test_args(self):
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x (int): The value.\n    y (str): Another.")
        sections = doc.sections
        assert len(sections) == 1
        assert sections[0].section_kind == pydocstring.GoogleSectionKind.ARGS

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.args = []

            def enter_google_arg(self, arg, ctx):
                self.args.append(arg)

        args = pydocstring.walk(doc, Collector()).args
        assert len(args) == 2
        assert args[0].name.text == "x"
        assert args[0].type.text == "int"
        assert args[0].description.text == "The value."
        assert args[1].name.text == "y"
        assert args[1].type.text == "str"

    def test_args_multiple_names_and_default_value(self):
        doc = pydocstring.parse_google(
            "Summary.\n\nArgs:\n    x1, x2 (int): The values.\n    order (str, optional, default 'C'): Layout."
        )

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.args = []

            def enter_google_arg(self, arg, ctx):
                self.args.append(arg)

        args = pydocstring.walk(doc, Collector()).args
        assert len(args) == 2
        assert [n.text for n in args[0].names] == ["x1", "x2"]
        assert args[0].name.text == "x1"
        assert args[0].default_value is None
        assert [n.text for n in args[1].names] == ["order"]
        assert args[1].type.text == "str"
        assert args[1].optional is not None
        assert args[1].default_keyword is not None
        assert args[1].default_keyword.text == "default"
        assert args[1].default_separator is None
        assert args[1].default_value is not None
        assert args[1].default_value.text == "'C'"

    def test_returns(self):
        doc = pydocstring.parse_google("Summary.\n\nReturns:\n    bool: True if successful.")
        section = doc.sections[0]
        assert section.section_kind == pydocstring.GoogleSectionKind.RETURNS

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.ret = None

            def enter_google_return(self, ret, ctx):
                self.ret = ret

        ret = pydocstring.walk(doc, Collector()).ret
        assert ret is not None
        assert ret.return_type.text == "bool"
        assert ret.description.text == "True if successful."

    def test_raises(self):
        doc = pydocstring.parse_google("Summary.\n\nRaises:\n    ValueError: If x is negative.")
        section = doc.sections[0]
        assert section.section_kind == pydocstring.GoogleSectionKind.RAISES

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.excepts = []

            def enter_google_exception(self, exc, ctx):
                self.excepts.append(exc)

        excepts = pydocstring.walk(doc, Collector()).excepts
        assert len(excepts) == 1
        assert excepts[0].type.text == "ValueError"
        assert excepts[0].description.text == "If x is negative."

    def test_warns_type(self):
        doc = pydocstring.parse_google("Summary.\n\nWarns:\n    UserWarning: When deprecated.")

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.warnings = []

            def enter_google_warning(self, wrn, ctx):
                self.warnings.append(wrn)

        warnings = pydocstring.walk(doc, Collector()).warnings
        assert len(warnings) == 1
        # 0.3.0: unified on ``.type`` (was ``warning_type``).
        assert warnings[0].type.text == "UserWarning"
        assert not hasattr(warnings[0], "warning_type")
        assert warnings[0].description.text == "When deprecated."

    def test_extended_summary(self):
        doc = pydocstring.parse_google("Summary.\n\nExtended description here.")
        assert doc.extended_summary is not None
        assert doc.extended_summary.text == "Extended description here."

    def test_deprecation(self):
        doc = pydocstring.parse_google("Summary.\n\n.. deprecated:: 1.6.0\n    Use new_func instead.")
        dep = doc.deprecation
        assert dep is not None
        assert dep.version.text == "1.6.0"
        assert dep.description is not None
        assert dep.description.text == "Use new_func instead."
        assert doc.extended_summary is None

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.deps = []

            def enter_google_deprecation(self, dep, ctx):
                self.deps.append(dep)

        deps = pydocstring.walk(doc, Collector()).deps
        assert len(deps) == 1
        assert deps[0].version.text == "1.6.0"
        assert repr(deps[0]) == 'GoogleDeprecation("1.6.0")'

    def test_paragraphs_between_sections(self):
        text = "Summary.\n\nArgs:\n    a: desc.\n\nstray one\nstray two\n\nstray three\n\nReturns:\n    int: result.\n"
        doc = pydocstring.parse_google(text)
        # Stray prose lines between sections are PARAGRAPH text blocks: lines
        # separated only by a newline form one paragraph, a blank line splits.
        paragraphs = doc.paragraphs
        assert [p.logical_text for p in paragraphs] == ["stray one\nstray two", "stray three"]
        assert [line.text for line in paragraphs[0].lines] == ["stray one", "stray two"]
        # The deprecated ``stray_lines`` alias was removed in 0.3.0.
        assert not hasattr(doc, "stray_lines")

    def test_body_text_section(self):
        doc = pydocstring.parse_google("Summary.\n\nNotes:\n    Some free text.")
        section = doc.sections[0]
        assert section.section_kind == pydocstring.GoogleSectionKind.NOTES

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.sections = []

            def enter_google_section(self, sec, ctx):
                self.sections.append(sec)

        sections = pydocstring.walk(doc, Collector()).sections
        assert len(sections) == 1
        assert sections[0].section_kind == pydocstring.GoogleSectionKind.NOTES

    def test_references(self):
        doc = pydocstring.parse_google(
            'Summary.\n\nReferences:\n    .. [1] Author A, "Title A", 2020.\n    Plain reference line.'
        )
        section = doc.sections[0]
        assert section.section_kind == pydocstring.GoogleSectionKind.REFERENCES

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.refs = []

            def enter_google_reference(self, ref, ctx):
                self.refs.append(ref)

        refs = pydocstring.walk(doc, Collector()).refs
        assert len(refs) == 2
        assert refs[0].directive_marker.text == ".."
        assert refs[0].label.text == "1"
        assert refs[0].content.text == 'Author A, "Title A", 2020.'
        assert refs[1].directive_marker is None
        assert refs[1].label is None
        assert refs[1].content.text == "Plain reference line."

    def test_pretty_print(self):
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x: Desc.")
        output = doc.pretty_print()
        assert "DOCUMENT" in output
        assert "SUMMARY" in output

    def test_source(self):
        text = "Summary.\n\nArgs:\n    x: Desc."
        doc = pydocstring.parse_google(text)
        assert doc.source == text

    def test_no_summary(self):
        doc = pydocstring.parse_google("")
        assert doc.summary is None

    def test_yields_is_optional(self):
        doc = pydocstring.parse_google("Summary.\n\nYields:\n    int: The next value.")
        section = doc.sections[0]
        assert section.section_kind == pydocstring.GoogleSectionKind.YIELDS

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.yld = None

            def enter_google_yield(self, yld, ctx):
                self.yld = yld

        yld = pydocstring.walk(doc, Collector()).yld
        assert yld is not None
        assert yld.return_type.text == "int"

    def test_section_kind_repr(self):
        assert repr(pydocstring.GoogleSectionKind.ARGS) == "GoogleSectionKind.ARGS"
        assert repr(pydocstring.GoogleSectionKind.RETURNS) == "GoogleSectionKind.RETURNS"

    def test_range_on_token(self):
        doc = pydocstring.parse_google("Summary.")
        assert doc.summary is not None
        r = doc.summary.range
        assert r.start == 0
        assert r.end == 8


class TestParseNumPy:
    def test_summary(self):
        doc = pydocstring.parse_numpy("Summary line.")
        assert doc.summary is not None
        assert doc.summary.text == "Summary line."

    def test_parameters(self):
        doc = pydocstring.parse_numpy(
            "Summary.\n\nParameters\n----------\nx : int\n    The first.\ny : str\n    The second."
        )
        sections = doc.sections
        assert len(sections) == 1
        assert sections[0].section_kind == pydocstring.NumPySectionKind.PARAMETERS

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.params = []

            def enter_numpy_parameter(self, prm, ctx):
                self.params.append(prm)

        params = pydocstring.walk(doc, Collector()).params
        assert len(params) == 2
        assert [n.text for n in params[0].names] == ["x"]
        # ``name`` is the first-name convenience (parity with GoogleArg).
        assert params[0].name is not None
        assert params[0].name.text == "x"
        assert params[0].type.text == "int"
        assert params[0].description.text == "The first."
        assert [n.text for n in params[1].names] == ["y"]

    def test_parameter_multiple_names_first_name(self):
        doc = pydocstring.parse_numpy("Summary.\n\nParameters\n----------\nx1, x2 : int\n    The values.")

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.params = []

            def enter_numpy_parameter(self, prm, ctx):
                self.params.append(prm)

        params = pydocstring.walk(doc, Collector()).params
        assert [n.text for n in params[0].names] == ["x1", "x2"]
        assert params[0].name.text == "x1"

    def test_paragraphs_property(self):
        # Parity with ``GoogleDocstring.paragraphs``. The NumPy grammar lets
        # the extended summary and section bodies absorb stray prose, so the
        # list is typically empty — but the accessor exists with the same type.
        doc = pydocstring.parse_numpy("Summary.\n\nParameters\n----------\nx : int\n    Desc.\n")
        assert doc.paragraphs == []
        assert not hasattr(doc, "stray_lines")

    def test_returns(self):
        doc = pydocstring.parse_numpy("Summary.\n\nReturns\n-------\nbool\n    True if successful.")
        section = doc.sections[0]
        assert section.section_kind == pydocstring.NumPySectionKind.RETURNS

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.returns = []

            def enter_numpy_returns(self, rtn, ctx):
                self.returns.append(rtn)

        returns = pydocstring.walk(doc, Collector()).returns
        assert len(returns) == 1
        assert returns[0].return_type.text == "bool"
        assert returns[0].description.text == "True if successful."

    def test_raises(self):
        doc = pydocstring.parse_numpy("Summary.\n\nRaises\n------\nValueError\n    If x is negative.")
        section = doc.sections[0]
        assert section.section_kind == pydocstring.NumPySectionKind.RAISES

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.excepts = []

            def enter_numpy_exception(self, exc, ctx):
                self.excepts.append(exc)

        excepts = pydocstring.walk(doc, Collector()).excepts
        assert len(excepts) == 1
        assert excepts[0].type.text == "ValueError"

    def test_pretty_print(self):
        doc = pydocstring.parse_numpy("Summary.\n\nParameters\n----------\nx : int\n    Desc.")
        output = doc.pretty_print()
        assert "DOCUMENT" in output

    def test_source(self):
        text = "Summary.\n\nParameters\n----------\nx : int\n    Desc."
        doc = pydocstring.parse_numpy(text)
        assert doc.source == text

    def test_section_kind_repr(self):
        assert repr(pydocstring.NumPySectionKind.PARAMETERS) == "NumPySectionKind.PARAMETERS"
        assert repr(pydocstring.NumPySectionKind.RETURNS) == "NumPySectionKind.RETURNS"


class TestToken:
    def test_text_and_range(self):
        doc = pydocstring.parse_google("Summary.")
        assert doc.summary is not None
        token = doc.summary.lines[0]
        assert token.text == "Summary."
        assert token.range.start == 0
        assert token.range.end == 8

    def test_repr(self):
        doc = pydocstring.parse_google("Hello.")
        assert doc.summary is not None
        assert repr(doc.summary.lines[0]) == 'Token("Hello.")'

    def test_no_kind_field(self):
        doc = pydocstring.parse_google("Summary.")
        assert doc.summary is not None
        token = doc.summary.lines[0]
        assert not hasattr(token, "kind"), "Token must not expose a 'kind' field"


class TestTextBlock:
    def test_text_is_raw_slice(self):
        doc = pydocstring.parse_google("Summary.")
        block = doc.summary
        assert block is not None
        assert block.text == "Summary."
        assert block.range.start == 0
        assert block.range.end == 8

    def test_repr(self):
        doc = pydocstring.parse_google("Hello.")
        assert repr(doc.summary) == 'TextBlock("Hello.")'

    def test_lines_one_token_per_content_line(self):
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x: First line.\n        Cont.")
        descriptions = []

        class V(pydocstring.Visitor):
            def enter_google_arg(self, arg, ctx):
                descriptions.append(arg.description)

        pydocstring.walk(doc, V())
        block = descriptions[0]
        assert [line.text for line in block.lines] == ["First line.", "Cont."]
        # Raw text keeps the interior newline and indentation.
        assert block.text == "First line.\n        Cont."

    def test_logical_text_dedents_continuation(self):
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x: First line.\n        Cont.")
        descriptions = []

        class V(pydocstring.Visitor):
            def enter_google_arg(self, arg, ctx):
                descriptions.append(arg.description)

        pydocstring.walk(doc, V())
        assert descriptions[0].logical_text == "First line.\nCont."

    def test_multiline_summary_text_matches_source_slice(self):
        doc = pydocstring.parse_plain("First summary line.\nSecond summary line.")
        assert doc.summary is not None
        assert doc.summary.text == "First summary line.\nSecond summary line."
        assert [line.text for line in doc.summary.lines] == [
            "First summary line.",
            "Second summary line.",
        ]

    def test_missing_description_block(self):
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x (int):")
        descriptions = []

        class V(pydocstring.Visitor):
            def enter_google_arg(self, arg, ctx):
                descriptions.append(arg.description)

        pydocstring.walk(doc, V())
        block = descriptions[0]
        assert block is not None
        assert block.is_missing()
        assert block.range.is_empty()
        assert block.text == ""
        assert block.lines == []

    def test_is_missing_false_for_present_token(self):
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x (int): Desc.")
        args = []

        class V(pydocstring.Visitor):
            def enter_google_arg(self, arg, ctx):
                args.append(arg)

        pydocstring.walk(doc, V())
        assert not args[0].type.is_missing()

    def test_is_missing_true_for_empty_parens(self):
        # "x ():" — brackets present but type content is absent
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x (): Desc.")
        args = []

        class V(pydocstring.Visitor):
            def enter_google_arg(self, arg, ctx):
                args.append(arg)

        pydocstring.walk(doc, V())
        assert args[0].type.is_missing()


class TestTextRange:
    def test_range_repr(self):
        doc = pydocstring.parse_google("Summary.")
        assert doc.summary is not None
        r = doc.summary.range
        assert repr(r) == "TextRange(0..8)"

    def test_section_range(self):
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x: Desc.")
        section = doc.sections[0]
        r = section.range
        assert r.start < r.end

    def test_is_empty_false_for_normal_range(self):
        doc = pydocstring.parse_google("Summary.")
        assert doc.summary is not None
        assert not doc.summary.range.is_empty()

    def test_is_empty_true_for_missing_token(self):
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x (): Desc.")
        args = []

        class V(pydocstring.Visitor):
            def enter_google_arg(self, arg, ctx):
                args.append(arg)

        pydocstring.walk(doc, V())
        assert args[0].type.range.is_empty()


class TestLineColumn:
    def test_summary_start(self):
        doc = pydocstring.parse_plain("Summary.")
        result = []

        class V(pydocstring.Visitor):
            def enter_plain_docstring(self, node, ctx):
                result.append(ctx.line_col(node.summary.range.start))

        pydocstring.walk(doc, V())
        assert result[0].lineno == 1
        assert result[0].col == 0

    def test_extended_summary_start(self):
        doc = pydocstring.parse_plain("Summary.\n\nExtended.")
        result = []

        class V(pydocstring.Visitor):
            def enter_plain_docstring(self, node, ctx):
                result.append(ctx.line_col(node.extended_summary.range.start))

        pydocstring.walk(doc, V())
        assert result[0].lineno == 3
        assert result[0].col == 0

    def test_repr(self):
        doc = pydocstring.parse_plain("Summary.")
        result = []

        class V(pydocstring.Visitor):
            def enter_plain_docstring(self, node, ctx):
                result.append(ctx.line_col(0))

        pydocstring.walk(doc, V())
        assert repr(result[0]) == "LineColumn(lineno=1, col=0)"


class TestModelTypes:
    def test_parameter_construction(self):
        p = pydocstring.Parameter(["x"], type_annotation="int", description="The value.")
        assert p.names == ["x"]
        assert p.type_annotation == "int"
        assert p.description == "The value."
        assert p.is_optional is False
        assert p.default_value is None

    def test_parameter_mutability(self):
        p = pydocstring.Parameter(["x"])
        p.names = ["x", "y"]
        p.type_annotation = "str"
        p.is_optional = True
        assert p.names == ["x", "y"]
        assert p.type_annotation == "str"
        assert p.is_optional is True

    def test_parameter_construction_validates_names(self):
        with pytest.raises(TypeError):
            pydocstring.Parameter([1, 2])  # ty: ignore[invalid-argument-type]

    def test_parameter_names_setter_validates(self):
        p = pydocstring.Parameter(["x"])
        with pytest.raises(TypeError):
            p.names = [1, 2]  # ty: ignore[invalid-assignment]
        assert p.names == ["x"]

    def test_parameter_repr(self):
        p = pydocstring.Parameter(["x", "y"])
        assert repr(p) == "Parameter(names=['x', 'y'])"

    def test_return_construction(self):
        r = pydocstring.Return(type_annotation="int", description="The result.")
        assert r.name is None
        assert r.type_annotation == "int"
        assert r.description == "The result."

    def test_exception_entry_construction(self):
        e = pydocstring.ExceptionEntry("ValueError", description="If x is negative.")
        assert e.type_name == "ValueError"
        assert e.description == "If x is negative."

    def test_directive_construction(self):
        d = pydocstring.Directive("deprecated", argument="1.6.0", description="Use new_func instead.")
        assert d.name == "deprecated"
        assert d.argument == "1.6.0"
        assert d.description == "Use new_func instead."

    def test_directive_defaults_and_repr(self):
        d = pydocstring.Directive("versionadded")
        assert d.argument is None
        assert d.description is None
        assert repr(d) == 'Directive("versionadded")'

    def test_see_also_entry_construction_validates_names(self):
        with pytest.raises(TypeError):
            pydocstring.SeeAlsoEntry(names=[1])  # ty: ignore[invalid-argument-type]

    def test_attribute_construction(self):
        a = pydocstring.Attribute("name", type_annotation="str", description="The name.")
        assert a.name == "name"
        assert a.type_annotation == "str"

    def test_method_construction(self):
        m = pydocstring.Method("run", description="Run the task.")
        assert m.name == "run"
        assert m.type_annotation is None
        assert m.description == "Run the task."

    def test_see_also_entry_construction(self):
        s = pydocstring.SeeAlsoEntry(["foo", "bar"], description="Related functions.")
        assert s.names == ["foo", "bar"]
        assert s.description == "Related functions."

    def test_see_also_entry_names_setter_validates(self):
        s = pydocstring.SeeAlsoEntry(["foo"])
        with pytest.raises(TypeError):
            s.names = [1]  # ty: ignore[invalid-assignment]
        assert s.names == ["foo"]

    def test_reference_construction(self):
        r = pydocstring.Reference(label="1", content="Doe et al. 2020")
        assert r.label == "1"
        assert r.content == "Doe et al. 2020"


class TestSection:
    def test_parameters_section(self):
        p = pydocstring.Parameter(["x"], type_annotation="int", description="Value.")
        sec = pydocstring.Section(pydocstring.SectionKind.PARAMETERS, parameters=[p])
        assert sec.kind == pydocstring.SectionKind.PARAMETERS
        params = sec.parameters
        assert params is not None
        assert len(params) == 1
        assert params[0].names == ["x"]
        assert params[0].type_annotation == "int"

    def test_returns_section(self):
        r = pydocstring.Return(type_annotation="bool", description="Success.")
        sec = pydocstring.Section(pydocstring.SectionKind.RETURNS, returns=[r])
        assert sec.kind == pydocstring.SectionKind.RETURNS
        rets = sec.returns
        assert rets is not None
        assert len(rets) == 1
        assert rets[0].type_annotation == "bool"

    def test_raises_section(self):
        e = pydocstring.ExceptionEntry("ValueError", description="Bad value.")
        sec = pydocstring.Section(pydocstring.SectionKind.RAISES, exceptions=[e])
        assert sec.kind == pydocstring.SectionKind.RAISES
        exceptions = sec.exceptions
        assert exceptions is not None
        assert len(exceptions) == 1
        assert exceptions[0].type_name == "ValueError"

    def test_free_text_section(self):
        sec = pydocstring.Section(pydocstring.SectionKind.NOTES, body="Some notes here.")
        assert sec.kind == pydocstring.SectionKind.NOTES
        assert sec.body == "Some notes here."

    def test_empty_accessors(self):
        sec = pydocstring.Section(pydocstring.SectionKind.PARAMETERS, parameters=[])
        assert sec.returns is None
        assert sec.exceptions is None
        assert sec.body is None

    def test_unknown_section_requires_name(self):
        with pytest.raises(ValueError, match="unknown_name"):
            pydocstring.Section(pydocstring.SectionKind.UNKNOWN)
        sec = pydocstring.Section(pydocstring.SectionKind.UNKNOWN, unknown_name="Custom", body="text")
        assert sec.kind == pydocstring.SectionKind.UNKNOWN
        assert sec.unknown_name == "Custom"
        assert sec.body == "text"

    def test_wrong_kind_kwarg_rejected(self):
        with pytest.raises(TypeError):
            pydocstring.Section(
                pydocstring.SectionKind.PARAMETERS,
                returns=[pydocstring.Return()],
            )

    def test_variant_constructors_are_suppressed(self):
        # PyO3 complex-enum variant constructors would bypass __init__'s
        # validation, so they are removed from the class surface.
        for variant in [
            "Parameters",
            "KeywordParameters",
            "OtherParameters",
            "Receives",
            "Returns",
            "Yields",
            "Raises",
            "Warns",
            "Attributes",
            "Methods",
            "SeeAlso",
            "References",
            "FreeText",
        ]:
            assert not hasattr(pydocstring.Section, variant), variant


class TestDocstringModel:
    def test_construction(self):
        doc = pydocstring.Docstring(summary="Brief summary.")
        assert doc.summary == "Brief summary."
        assert doc.extended_summary is None
        assert doc.directives == []
        assert doc.deprecation is None
        assert doc.sections == []

    def test_mutability(self):
        doc = pydocstring.Docstring(summary="Old.")
        doc.summary = "New."
        assert doc.summary == "New."

    def test_repr_renders_summary(self):
        assert repr(pydocstring.Docstring(summary="Hi.")) == 'Docstring(summary="Hi.")'
        assert repr(pydocstring.Docstring()) == "Docstring(summary=None)"

    def test_with_sections(self):
        p = pydocstring.Parameter(["x"], type_annotation="int")
        sec = pydocstring.Section(pydocstring.SectionKind.PARAMETERS, parameters=[p])
        doc = pydocstring.Docstring(summary="Brief.", sections=[sec])
        assert len(doc.sections) == 1
        assert doc.sections[0].kind == pydocstring.SectionKind.PARAMETERS

        params = doc.sections[0].parameters
        assert params is not None
        params[0].description = "foo"
        assert params[0].description == "foo"

    def test_with_directives(self):
        dep = pydocstring.Directive("deprecated", argument="2.0", description="Removed.")
        doc = pydocstring.Docstring(directives=[dep])
        assert [d.name for d in doc.directives] == ["deprecated"]
        assert doc.deprecation is not None
        assert doc.deprecation.argument == "2.0"
        assert doc.deprecation.description == "Removed."

    def test_deprecation_is_computed_first_match(self):
        doc = pydocstring.Docstring()
        assert doc.deprecation is None
        doc.directives = [
            pydocstring.Directive("versionadded", argument="1.0"),
            pydocstring.Directive("deprecated", argument="2.0"),
            pydocstring.Directive("deprecated", argument="3.0"),
        ]
        assert doc.deprecation is not None
        assert doc.deprecation.argument == "2.0"

    def test_deprecation_is_read_only(self):
        doc = pydocstring.Docstring()
        with pytest.raises(AttributeError):
            doc.deprecation = pydocstring.Directive("deprecated")  # ty: ignore[invalid-assignment]

    def test_deprecation_kwarg_removed(self):
        with pytest.raises(TypeError):
            pydocstring.Docstring(deprecation=pydocstring.Directive("deprecated"))  # ty: ignore[unknown-argument]

    def test_directives_validated(self):
        with pytest.raises(TypeError):
            pydocstring.Docstring(directives=["not a directive"])  # ty: ignore[invalid-argument-type]
        doc = pydocstring.Docstring()
        with pytest.raises(TypeError):
            doc.directives = ["not a directive"]  # ty: ignore[invalid-assignment]

    def test_sections_setter_validated(self):
        doc = pydocstring.Docstring()
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
        params = model.sections[0].parameters
        assert params is not None
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
        params = model.sections[0].parameters
        assert params is not None
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
        exceptions = model.sections[0].exceptions
        assert exceptions is not None
        assert exceptions[0].type_name == "ValueError"

    def test_google_to_model_returns(self):
        doc = pydocstring.parse_google("Summary.\n\nReturns:\n    int: The result.\n")
        model = doc.to_model()
        assert model.sections[0].kind == pydocstring.SectionKind.RETURNS
        rets = model.sections[0].returns
        assert rets is not None
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
        doc = pydocstring.Docstring(
            summary="Summary.",
            directives=[pydocstring.Directive("deprecated", argument="2.0", description="Gone.")],
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
        doc = pydocstring.parse_plain("Summary line.")
        assert doc.summary is not None
        assert doc.summary.text == "Summary line."
        assert doc.extended_summary is None

    def test_empty(self):
        doc = pydocstring.parse_plain("")
        assert doc.summary is None
        assert doc.extended_summary is None

    def test_extended_summary(self):
        doc = pydocstring.parse_plain("Summary.\n\nMore details here.\nContinued.")
        assert doc.summary is not None
        assert doc.summary.text == "Summary."
        assert doc.extended_summary is not None
        assert "More details here." in doc.extended_summary.text

    def test_no_sections(self):
        doc = pydocstring.parse_plain("Summary.\n\n:param x: A value.\n:returns: Something.")
        model = doc.to_model()
        assert model.sections == []

    def test_source(self):
        text = "Summary.\n\nExtended."
        doc = pydocstring.parse_plain(text)
        assert doc.source == text

    def test_pretty_print(self):
        doc = pydocstring.parse_plain("Summary.\n\nExtended.")
        output = doc.pretty_print()
        assert "DOCUMENT" in output
        assert "SUMMARY" in output
        assert "EXTENDED_SUMMARY" in output

    def test_repr(self):
        doc = pydocstring.parse_plain("Summary.")
        assert repr(doc) == "PlainDocstring(...)"

    def test_line_col_summary(self):
        doc = pydocstring.parse_plain("Summary.")
        result = []

        class V(pydocstring.Visitor):
            def enter_plain_docstring(self, node, ctx):
                result.append(ctx.line_col(node.summary.range.start))

        pydocstring.walk(doc, V())
        assert result[0].lineno == 1
        assert result[0].col == 0

    def test_line_col_extended_summary(self):
        doc = pydocstring.parse_plain("Summary.\n\nExtended.")
        result = []

        class V(pydocstring.Visitor):
            def enter_plain_docstring(self, node, ctx):
                result.append(ctx.line_col(node.extended_summary.range.start))

        pydocstring.walk(doc, V())
        assert result[0].lineno == 3
        assert result[0].col == 0

    def test_detect_style_dispatches_to_plain(self):
        assert pydocstring.detect_style("Just a summary.") == pydocstring.Style.PLAIN
        assert pydocstring.detect_style("Summary.\n\n:param x: value.") == pydocstring.Style.PLAIN

    def test_style_property(self):
        doc = pydocstring.parse_plain("Summary.")
        assert doc.style == pydocstring.Style.PLAIN

    def test_no_node_attribute(self):
        doc = pydocstring.parse_plain("Summary.")
        assert not hasattr(doc, "node"), "Docstring must not expose a 'node' attribute"


class TestParse:
    """Tests for the unified parse() entry point."""

    def test_google_returns_google_docstring(self):
        doc = pydocstring.parse("Summary.\n\nArgs:\n    x (int): Value.")
        assert isinstance(doc, pydocstring.GoogleDocstring)
        assert doc.style == pydocstring.Style.GOOGLE

    def test_numpy_returns_numpy_docstring(self):
        doc = pydocstring.parse("Summary.\n\nParameters\n----------\nx : int\n    Value.")
        assert isinstance(doc, pydocstring.NumPyDocstring)
        assert doc.style == pydocstring.Style.NUMPY

    def test_plain_returns_plain_docstring(self):
        doc = pydocstring.parse("Just a summary.")
        assert isinstance(doc, pydocstring.PlainDocstring)
        assert doc.style == pydocstring.Style.PLAIN

    def test_empty_returns_plain_docstring(self):
        doc = pydocstring.parse("")
        assert isinstance(doc, pydocstring.PlainDocstring)
        assert doc.style == pydocstring.Style.PLAIN

    def test_sphinx_returns_plain_docstring(self):
        doc = pydocstring.parse("Summary.\n\n:param x: A value.\n:returns: Something.")
        assert isinstance(doc, pydocstring.PlainDocstring)
        assert doc.style == pydocstring.Style.PLAIN

    def test_google_style_property(self):
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x: Desc.")
        assert doc.style == pydocstring.Style.GOOGLE

    def test_numpy_style_property(self):
        doc = pydocstring.parse_numpy("Summary.\n\nParameters\n----------\nx : int\n    Desc.")
        assert doc.style == pydocstring.Style.NUMPY

    def test_parse_google_summary(self):
        doc = pydocstring.parse("Summary.\n\nArgs:\n    x (int): Value.")
        assert doc.summary is not None
        assert doc.summary.text == "Summary."

    def test_parse_numpy_summary(self):
        doc = pydocstring.parse("Summary.\n\nParameters\n----------\nx : int\n    Value.")
        assert doc.summary is not None
        assert doc.summary.text == "Summary."


class TestWalk:
    def test_google_walk_collects_args(self):
        source = "Summary.\n\nArgs:\n    x (int): The x value.\n    y (str): The y value."
        doc = pydocstring.parse_google(source)

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.arg_names = []

            def enter_google_arg(self, arg, ctx):
                self.arg_names.append(arg.name.text)

        collector = Collector()
        pydocstring.walk(doc, collector)
        assert collector.arg_names == ["x", "y"]

    def test_numpy_walk_collects_parameters(self):
        source = "Summary.\n\nParameters\n----------\nx : int\n    Desc x.\ny : str\n    Desc y."
        doc = pydocstring.parse_numpy(source)

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.names = []

            def enter_numpy_parameter(self, param, ctx):
                self.names.append(param.names[0].text)

        collector = Collector()
        pydocstring.walk(doc, collector)
        assert collector.names == ["x", "y"]

    def test_walk_plain_dispatches_plain_docstring(self):
        doc = pydocstring.parse_plain("Just a summary.")

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.called = False

            def enter_plain_docstring(self, plain_doc, ctx):
                self.called = True
                assert plain_doc.summary is not None
                assert plain_doc.summary.text == "Just a summary."

        collector = Collector()
        pydocstring.walk(doc, collector)
        assert collector.called

    def test_walk_plain_no_google_numpy_dispatch(self):
        doc = pydocstring.parse_plain("Just a summary.")

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.called = False

            def enter_google_arg(self, arg, ctx):
                self.called = True

            def enter_numpy_parameter(self, param, ctx):
                self.called = True

        collector = Collector()
        pydocstring.walk(doc, collector)
        assert not collector.called

    def test_walk_rejects_wrong_type(self):
        with pytest.raises(TypeError):
            pydocstring.walk("not a docstring", object())  # ty: ignore[invalid-argument-type]

    def test_walk_via_parse_google(self):
        """walk() dispatches correctly when doc comes from auto-detect parse()."""
        source = "Summary.\n\nArgs:\n    z (float): A float."
        doc = pydocstring.parse(source)
        assert isinstance(doc, pydocstring.GoogleDocstring)

        names = []

        class V(pydocstring.Visitor):
            def enter_google_arg(self, arg, ctx):
                names.append(arg.name.text)

        pydocstring.walk(doc, V())
        assert names == ["z"]

    def test_walk_via_parse_numpy(self):
        """walk() dispatches correctly when doc comes from auto-detect parse()."""
        source = "Summary.\n\nParameters\n----------\na : int\n    Desc."
        doc = pydocstring.parse(source)
        assert isinstance(doc, pydocstring.NumPyDocstring)

        names = []

        class V(pydocstring.Visitor):
            def enter_numpy_parameter(self, param, ctx):
                names.append(param.names[0].text)

        pydocstring.walk(doc, V())
        assert names == ["a"]

    def test_walk_visitor_without_methods_is_safe(self):
        """A Visitor with no overrides should not raise."""
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x: Desc.")
        pydocstring.walk(doc, pydocstring.Visitor())

    def test_walk_non_visitor_raises_type_error(self):
        """Passing a non-Visitor object raises TypeError."""
        doc = pydocstring.parse_google("Summary.")
        with pytest.raises(TypeError, match="must subclass pydocstring.Visitor"):
            pydocstring.walk(doc, object())  # ty: ignore[invalid-argument-type]

    def test_walk_returns_visitor(self):
        """walk() returns the visitor object."""
        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x (int): Desc.")

        class V(pydocstring.Visitor):
            def enter_google_arg(self, arg, ctx):
                pass

        v = V()
        result = pydocstring.walk(doc, v)
        assert result is v

    def test_ctx_line_col_google(self):
        """ctx.line_col() returns correct LineColumn for a given offset."""
        source = "Summary.\n\nArgs:\n    x (int): The value."
        doc = pydocstring.parse_google(source)

        line_cols = []

        class V(pydocstring.Visitor):
            def enter_google_arg(self, arg, ctx):
                lc = ctx.line_col(arg.range.start)
                line_cols.append((lc.lineno, lc.col))

        pydocstring.walk(doc, V())
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

        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x: Desc.")
        pydocstring.walk(doc, V())
        assert called == []

    def test_visitor_overridden_method_is_dispatched(self):
        """An overridden Visitor method is called during walk()."""
        names = []

        class V(pydocstring.Visitor):
            def enter_google_arg(self, node, ctx):
                names.append(node.name.text)

        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x: Desc.\n    y: Desc.")
        pydocstring.walk(doc, V())
        assert names == ["x", "y"]

    def test_visitor_only_overridden_methods_dispatched(self):
        """Only overridden methods fire; base no-ops are silent."""
        events = []

        class V(pydocstring.Visitor):
            def enter_google_arg(self, node, ctx):
                events.append(("enter_arg", node.name.text))

            # exit_google_arg intentionally NOT overridden

        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    x: Desc.")
        pydocstring.walk(doc, V())
        assert events == [("enter_arg", "x")]

    def test_visitor_duck_typing_raises_type_error(self):
        """Non-Visitor objects raise TypeError."""

        class Duck:
            def enter_google_arg(self, node, ctx):
                pass

        doc = pydocstring.parse_google("Summary.\n\nArgs:\n    z: Desc.")
        with pytest.raises(TypeError, match="must subclass pydocstring.Visitor"):
            pydocstring.walk(doc, Duck())  # ty: ignore[invalid-argument-type]


class TestMissingDescription:
    def test_numpy_exception_missing_description_is_exposed(self):
        doc = pydocstring.parse_numpy("Summary.\n\nRaises\n------\nValueError:\n")

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.excepts = []

            def enter_numpy_exception(self, exc, ctx):
                self.excepts.append(exc)

        excepts = pydocstring.walk(doc, Collector()).excepts
        assert len(excepts) == 1
        assert excepts[0].description is not None
        assert excepts[0].description.is_missing()

    def test_numpy_exception_continuation_replaces_placeholder(self):
        doc = pydocstring.parse_numpy("Summary.\n\nRaises\n------\nValueError:\n    Later description.\n")

        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.excepts = []

            def enter_numpy_exception(self, exc, ctx):
                self.excepts.append(exc)

        excepts = pydocstring.walk(doc, Collector()).excepts
        assert excepts[0].description.text == "Later description."
        assert not excepts[0].description.is_missing()


def _first_node(doc, method: str):
    """Walk ``doc`` and return the first node dispatched to ``method``."""
    nodes = []
    visitor_cls = type(
        "_Collector",
        (pydocstring.Visitor,),
        {method: lambda self, node, ctx: nodes.append(node)},
    )
    pydocstring.walk(doc, visitor_cls())
    assert nodes, f"no node dispatched to {method}"
    return nodes[0]


def _missingness(value) -> str:
    if value is None:
        return "none"
    return "missing" if value.is_missing() else "present"


# LAW (cross-style missing-ness parity): for the same role and the same
# semantic omission, the Google and NumPy wrappers must expose the same
# missing-value category — a zero-length placeholder when the syntax marker
# is present without content, None when the parser emits no placeholder.
# Mirrors the Rust cross-style slot-kind parity law (tests/unified.rs).
MISSINGNESS_CASES = [
    pytest.param(
        "Summary.\n\nRaises:\n    ValueError:\n",
        "Summary.\n\nRaises\n------\nValueError:\n",
        ("enter_google_exception", "enter_numpy_exception"),
        "description",
        "missing",
        id="raises-description-colon-no-text",
    ),
    pytest.param(
        "Summary.\n\nRaises:\n    ValueError\n",
        "Summary.\n\nRaises\n------\nValueError\n",
        ("enter_google_exception", "enter_numpy_exception"),
        "description",
        "none",
        id="raises-description-no-colon",
    ),
    pytest.param(
        "Summary.\n\nWarns:\n    UserWarning:\n",
        "Summary.\n\nWarns\n-----\nUserWarning:\n",
        ("enter_google_warning", "enter_numpy_warning"),
        "description",
        "missing",
        id="warns-description-colon-no-text",
    ),
    pytest.param(
        "Summary.\n\nMethods:\n    run:\n",
        "Summary.\n\nMethods\n-------\nrun:\n",
        ("enter_google_method", "enter_numpy_method"),
        "description",
        "missing",
        id="methods-description-colon-no-text",
    ),
    pytest.param(
        "Summary.\n\nSee Also:\n    other_func:\n",
        "Summary.\n\nSee Also\n--------\nother_func :\n",
        ("enter_google_see_also_item", "enter_numpy_see_also_item"),
        "description",
        "missing",
        id="see-also-description-colon-no-text",
    ),
    pytest.param(
        "Summary.\n\nArgs:\n    x (): Desc.\n",
        "Summary.\n\nParameters\n----------\nx :\n    Desc.\n",
        ("enter_google_arg", "enter_numpy_parameter"),
        "type",
        "missing",
        id="parameter-type-marker-no-text",
    ),
    pytest.param(
        "Summary.\n\nArgs:\n    x: Desc.\n",
        "Summary.\n\nParameters\n----------\nx\n    Desc.\n",
        ("enter_google_arg", "enter_numpy_parameter"),
        "type",
        "none",
        id="parameter-type-no-marker",
    ),
    pytest.param(
        "Summary.\n\nAttributes:\n    attr ():\n",
        "Summary.\n\nAttributes\n----------\nattr :\n",
        ("enter_google_attribute", "enter_numpy_attribute"),
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
        ("enter_google_return", "enter_numpy_returns"),
        "description",
        "none",
        id="returns-description-absent-stays-none",
    ),
    pytest.param(
        "Summary.\n\nYields:\n    int:\n",
        "Summary.\n\nYields\n------\nint\n",
        ("enter_google_yield", "enter_numpy_yields"),
        "description",
        "none",
        id="yields-description-absent-stays-none",
    ),
]


class TestMissingnessParity:
    """Google/NumPy parity for description/type missing-ness across roles."""

    @pytest.mark.parametrize(("google_src", "numpy_src", "methods", "field", "expected"), MISSINGNESS_CASES)
    def test_parity(self, google_src, numpy_src, methods, field, expected):
        google_method, numpy_method = methods
        google_node = _first_node(pydocstring.parse_google(google_src), google_method)
        numpy_node = _first_node(pydocstring.parse_numpy(numpy_src), numpy_method)

        google_cat = _missingness(getattr(google_node, field))
        numpy_cat = _missingness(getattr(numpy_node, field))

        assert google_cat == numpy_cat, f"{field}: google={google_cat}, numpy={numpy_cat}"
        assert google_cat == expected


class TestMissingnessGrammarAsymmetries:
    """Slots where google/numpy missingness legitimately differs by grammar.

    These are NOT parity violations: numpy attribute descriptions live on
    continuation lines only (there is no "marker present, content absent"
    spelling), so numpy yields None where google yields a missing placeholder.
    NumPyMethod has no type slot at all (grammar has none), so the
    method-type column is google-only. Documented in the .pyi conventions.
    """

    def test_attribute_description_missingness_differs_by_grammar(self):
        class Collector(pydocstring.Visitor):
            def __init__(self):
                self.attrs = []

            def enter_google_attribute(self, a, ctx):
                self.attrs.append(a)

            def enter_numpy_attribute(self, a, ctx):
                self.attrs.append(a)

        g = pydocstring.walk(pydocstring.parse_google("Summary.\n\nAttributes:\n    x (int):\n"), Collector()).attrs[0]
        assert g.description is not None and g.description.is_missing()

        n = pydocstring.walk(
            pydocstring.parse_numpy("Summary.\n\nAttributes\n----------\nx : int\n"), Collector()
        ).attrs[0]
        assert n.description is None
