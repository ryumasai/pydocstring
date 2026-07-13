//! Example: Parsing Google-style docstrings
//!
//! Shows the raw docstring text, then the detailed parsed AST.

use pydocstring::parse::parse_google;

fn main() {
    let docstring = r#"
Calculate the area of a rectangle.

This function takes the width and height of a rectangle
and returns its area.

Args:
    width (float): The width of the rectangle.
    height (float): The height of the rectangle.

Returns:
    float: The area of the rectangle.

Raises:
    ValueError: If width or height is negative.
"#;

    let parsed = parse_google(docstring);

    println!("╔══════════════════════════════════════════════════╗");
    println!("║          Google-style Docstring Example          ║");
    println!("╚══════════════════════════════════════════════════╝");

    println!();

    // Display: raw source text
    println!("── raw text ────────────────────────────────────────");
    println!("{}", parsed.source());

    println!();

    // pretty_print: structured AST
    println!("── parsed AST ──────────────────────────────────────");
    print!("{}", parsed.pretty_print());
}
