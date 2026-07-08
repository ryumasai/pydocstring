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
        # The deprecated ``stray_lines`` alias still works; its items are now
        # TextBlocks (one per paragraph), not per-line tokens.
        assert [p.text for p in doc.stray_lines] == [p.text for p in paragraphs]

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
        assert refs[0].number.text == "1"
        assert refs[0].content.text == 'Author A, "Title A", 2020.'
        assert refs[1].directive_marker is None
        assert refs[1].number is None
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
        assert params[0].type.text == "int"
        assert params[0].description.text == "The first."
        assert [n.text for n in params[1].names] == ["y"]

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

    def test_return_construction(self):
        r = pydocstring.Return(type_annotation="int", description="The result.")
        assert r.name is None
        assert r.type_annotation == "int"
        assert r.description == "The result."

    def test_exception_entry_construction(self):
        e = pydocstring.ExceptionEntry("ValueError", description="If x is negative.")
        assert e.type_name == "ValueError"
        assert e.description == "If x is negative."

    def test_deprecation_construction(self):
        d = pydocstring.Deprecation("1.6.0", description="Use new_func instead.")
        assert d.version == "1.6.0"
        assert d.description == "Use new_func instead."

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

    def test_reference_construction(self):
        r = pydocstring.Reference(number="1", content="Doe et al. 2020")
        assert r.number == "1"
        assert r.content == "Doe et al. 2020"


class TestSection:
    def test_parameters_section(self):
        p = pydocstring.Parameter(["x"], type_annotation="int", description="Value.")
        sec = pydocstring.Section(pydocstring.SectionKind.PARAMETERS, parameters=[p])
        assert sec.kind == pydocstring.SectionKind.PARAMETERS
        params = sec.parameters
        assert len(params) == 1
        assert params[0].names == ["x"]
        assert params[0].type_annotation == "int"

    def test_returns_section(self):
        r = pydocstring.Return(type_annotation="bool", description="Success.")
        sec = pydocstring.Section(pydocstring.SectionKind.RETURNS, returns=[r])
        assert sec.kind == pydocstring.SectionKind.RETURNS
        rets = sec.returns
        assert len(rets) == 1
        assert rets[0].type_annotation == "bool"

    def test_raises_section(self):
        e = pydocstring.ExceptionEntry("ValueError", description="Bad value.")
        sec = pydocstring.Section(pydocstring.SectionKind.RAISES, exceptions=[e])
        assert sec.kind == pydocstring.SectionKind.RAISES
        assert len(sec.exceptions) == 1
        assert sec.exceptions[0].type_name == "ValueError"

    def test_free_text_section(self):
        sec = pydocstring.Section(pydocstring.SectionKind.NOTES, body="Some notes here.")
        assert sec.kind == pydocstring.SectionKind.NOTES
        assert sec.body == "Some notes here."

    def test_empty_accessors(self):
        sec = pydocstring.Section(pydocstring.SectionKind.PARAMETERS, parameters=[])
        assert sec.returns is None
        assert sec.exceptions is None
        assert sec.body is None


class TestDocstringModel:
    def test_construction(self):
        doc = pydocstring.Docstring(summary="Brief summary.")
        assert doc.summary == "Brief summary."
        assert doc.extended_summary is None
        assert doc.deprecation is None
        assert doc.sections == []

    def test_mutability(self):
        doc = pydocstring.Docstring(summary="Old.")
        doc.summary = "New."
        assert doc.summary == "New."

    def test_with_sections(self):
        p = pydocstring.Parameter(["x"], type_annotation="int")
        sec = pydocstring.Section(pydocstring.SectionKind.PARAMETERS, parameters=[p])
        doc = pydocstring.Docstring(summary="Brief.", sections=[sec])
        assert len(doc.sections) == 1
        assert doc.sections[0].kind == pydocstring.SectionKind.PARAMETERS

        doc.sections[0].parameters[0].description = "foo"
        assert doc.sections[0].parameters[0].description == "foo"

    def test_with_deprecation(self):
        dep = pydocstring.Deprecation("2.0", description="Removed.")
        doc = pydocstring.Docstring(deprecation=dep)
        assert doc.deprecation is not None
        assert doc.deprecation.version == "2.0"


class TestToModel:
    def test_google_to_model(self):
        docstr = "Summary.\n\nArgs:\n    x (int): The value.\n"
        doc = pydocstring.parse_google(docstr)
        model = doc.to_model()
        assert model.summary == "Summary."
        assert len(model.sections) == 1
        assert model.sections[0].kind == pydocstring.SectionKind.PARAMETERS
        params = model.sections[0].parameters
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
        assert model.sections[0].exceptions[0].type_name == "ValueError"

    def test_google_to_model_returns(self):
        doc = pydocstring.parse_google("Summary.\n\nReturns:\n    int: The result.\n")
        model = doc.to_model()
        assert model.sections[0].kind == pydocstring.SectionKind.RETURNS
        rets = model.sections[0].returns
        assert len(rets) == 1
        assert rets[0].type_annotation == "int"

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
