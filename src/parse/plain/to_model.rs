//! Convert a plain-style AST into the style-independent [`Docstring`] model.

use crate::model::Docstring;
use crate::parse::plain::nodes::PlainDocstring;
use crate::syntax::Parsed;

/// Build a [`Docstring`] from a plain-style [`Parsed`] result.
///
/// Returns `None` if the docstring was not parsed as
/// [`Style::Plain`](crate::parse::Style::Plain).
pub fn to_model(parsed: &Parsed) -> Option<Docstring> {
    if parsed.style() != crate::parse::Style::Plain {
        return None;
    }
    let root = PlainDocstring::cast(parsed, parsed.root())?;

    Some(Docstring {
        summary: root.summary().map(|t| t.text().to_owned()),
        extended_summary: root.extended_summary().map(|t| t.text().to_owned()),
        directives: Vec::new(),
        sections: Vec::new(),
    })
}
