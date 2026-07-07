//! Example: Auto-detecting docstring style with `parse()`
//!
//! Demonstrates the unified `parse()` entry point, which detects the style
//! automatically and returns a `Parsed` result. The root node kind is always
//! the style-neutral `DOCUMENT`; `Parsed::style()` reports the detected style:
//!
//! - `Style::Google` — Google style (section headers ending with `:`)
//! - `Style::NumPy`  — NumPy style (section headers with `---` underlines)
//! - `Style::Plain`  — no recognised section markers (summary/extended
//!   summary only, or unrecognised styles such as Sphinx)

use pydocstring::parse::Style;
use pydocstring::parse::parse;

fn show(label: &str, input: &str) {
    let parsed = parse(input);
    let style_label = match parsed.style() {
        Style::Google => "Google",
        Style::NumPy => "NumPy",
        Style::Plain => "Plain",
    };

    println!(
        "── {} → {} ──────────────────────────────────────────",
        label, style_label
    );
    print!("{}", parsed.pretty_print());
    println!();
}

fn main() {
    println!("╔══════════════════════════════════════════════════╗");
    println!("║        Auto-detecting Docstring Style            ║");
    println!("╚══════════════════════════════════════════════════╝");
    println!();

    show(
        "Google",
        r#"
Calculate the area of a rectangle.

Args:
    width (float): The width of the rectangle.
    height (float): The height of the rectangle.

Returns:
    float: The area of the rectangle.
"#,
    );

    show(
        "NumPy",
        r#"
Calculate the area of a rectangle.

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
"#,
    );

    show("Plain (summary only)", "Calculate the area of a rectangle.");

    show(
        "Plain (summary + extended)",
        r#"
Calculate the area of a rectangle.

Takes width and height as arguments and returns their product.
Negative values will raise a ValueError.
"#,
    );
}
