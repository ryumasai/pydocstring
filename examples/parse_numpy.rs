//! Example: Parsing NumPy-style docstrings
//!
//! Shows the raw docstring text, then the detailed parsed AST.

use pydocstring::parse::numpy::NumPyDocstring;
use pydocstring::parse::numpy::parse_numpy;

fn main() {
    let docstring = r#"
Calculate the area of a rectangle.

This function takes the width and height of a rectangle
and returns its area.

Parameters
----------
width : float
    The width of the rectangle.
height : float
    The height of the rectangle.

Returns
-------
float
    The area of the rectangle.

Raises
------
ValueError
    If width or height is negative.

Examples
--------
>>> calculate_area(5.0, 3.0)
15.0
"#;

    let parsed = parse_numpy(docstring);
    let doc = NumPyDocstring::cast(parsed.root()).unwrap();

    println!("╔══════════════════════════════════════════════════╗");
    println!("║          NumPy-style Docstring Example           ║");
    println!("╚══════════════════════════════════════════════════╝");

    println!();

    // Display: raw source text
    println!("── raw text ────────────────────────────────────────");
    println!("{}", doc.syntax().range().source_text(parsed.source()));

    println!();

    // pretty_print: structured AST
    println!("── parsed AST ──────────────────────────────────────");
    print!("{}", parsed.pretty_print());
}
