"""Model IR — the normalized, position-free view of a docstring.

The model is what `Docstring.to_model()` produces: owned, interpreted data with
the source positions dropped. Dropping positions is what lets this layer apply
context-dependent semantics (such as merging consecutive lines into one
`Block.Paragraph`) that the tree cannot express without breaking edit locality.

Because it has no positions, the model is a one-way projection: use it to
inspect, transform, and re-emit (`emit_google` / `emit_numpy` / `emit_sphinx`),
not to edit in place. For editing, use the position-preserving unified view
(`pydocstring.Document`) together with `Edits`.

This mirrors the Rust crate's `model` module. Both layers define a `Section` and
a `Directive`; keeping the model in its own namespace is what lets the top level
carry the unified view under those names.
"""

from ._pydocstring.model import Attribute
from ._pydocstring.model import Block
from ._pydocstring.model import Directive
from ._pydocstring.model import Docstring
from ._pydocstring.model import ExceptionEntry
from ._pydocstring.model import Method
from ._pydocstring.model import Parameter
from ._pydocstring.model import Reference
from ._pydocstring.model import Return
from ._pydocstring.model import Section
from ._pydocstring.model import SectionKind
from ._pydocstring.model import SeeAlsoEntry

__all__ = [
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
    "SectionKind",
]
