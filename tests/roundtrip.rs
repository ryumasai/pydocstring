//! Round-trip law tests over the corpus.
//!
//! `emit_*` is a normalizer, so `emit(parse(s))` is not `s` in general.
//! The laws that *must* hold, checked here for every corpus input:
//!
//! 1. **Idempotence** — normalizing a normalized docstring is a fixed point:
//!    `emit(parse(emit(parse(s)))) == emit(parse(s))`
//! 2. **Model stability** — emitting loses no model information:
//!    `to_model(parse(emit(m))) == m` where `m = to_model(parse(s))`
//! 3. **Cross-style conversion** — the model survives being emitted in the
//!    *other* style and parsed back: for `m` from a NumPy docstring,
//!    `to_model(parse_google(emit_google(m))) == m`, and vice versa.
//!
//! A violation means parse and emit disagree about the normal form; every
//! violation is a bug in the parser, the emitter, or both. Known violations
//! are tracked in the `KNOWN_*` lists with the issue that covers them; a test
//! fails when a new violation appears *or* when a listed one starts passing
//! (stale entry). Shrink the lists by fixing bugs, never by relaxing a law.

mod common;

use common::{collect_inputs, corpus_name, diff, style_dirs};
use pydocstring::model::Docstring;

const KNOWN_IDEMPOTENCE_FAILURES: &[&str] = &[];
const KNOWN_MODEL_STABILITY_FAILURES: &[&str] = &[];
/// Entries are `"<from>-><to>: <corpus path>"`, e.g. `"numpy->google: numpy/returns/yields_basic.txt"`.
const KNOWN_CONVERSION_FAILURES: &[&str] = &[
    // Fundamental NumPy ambiguity: a description-only Return has no
    // unambiguous NumPy form — the bare line re-parses as the type
    // (prefer_type, see the #26 discussion).
    "google->numpy: google/returns/returns_without_type.txt",
    // emit_google/parse_google do not round-trip the deprecation directive.
    "numpy->google: numpy/edge_cases/indented_with_deprecation.txt",
    "numpy->google: numpy/sections/deprecation_directive.txt",
    // Google parser reads References as free text, losing the structured
    // Reference entries (number + content) — issue #55. The google->numpy
    // direction of the same asymmetry hits the References section inside
    // freetext_sections.txt.
    "google->numpy: google/freetext/freetext_sections.txt",
    "numpy->google: numpy/freetext/references_directive_markers.txt",
    "numpy->google: numpy/freetext/references_parsing.txt",
    "numpy->google: numpy/freetext/references_unclosed_bracket.txt",
    // Google parser does not split comma-separated parameter names.
    "numpy->google: numpy/parameters/multiple_parameter_names.txt",
    // default_value is lost on the Google round trip.
    "numpy->google: numpy/parameters/enum_type_with_default.txt",
    // Named / multiple / type-only Returns and Yields entries do not survive
    // the Google round trip (Google's entry syntax cannot express all of
    // them, and emit_google/parse_google disagree on the rest).
    "numpy->google: numpy/regressions/issue26_rst_roles.txt",
    "numpy->google: numpy/returns/parse_named_returns.txt",
    "numpy->google: numpy/returns/returns_multiline_description.txt",
    "numpy->google: numpy/returns/returns_no_spaces_around_colon.txt",
    "numpy->google: numpy/returns/yields_multiple.txt",
    "numpy->google: numpy/returns/yields_named.txt",
];

fn model_for(style: &str, input: &str) -> Option<Docstring> {
    match style {
        "google" => pydocstring::parse::google::to_model::to_model(&pydocstring::parse::google::parse_google(input)),
        "numpy" => pydocstring::parse::numpy::to_model::to_model(&pydocstring::parse::numpy::parse_numpy(input)),
        // Plain has no emitter, so it cannot participate in round trips.
        "plain" => None,
        other => panic!("unknown corpus style directory: {other}"),
    }
}

fn emit_in(style: &str, model: &Docstring) -> String {
    match style {
        "google" => pydocstring::emit::google::emit_google(model, 0),
        "numpy" => pydocstring::emit::numpy::emit_numpy(model, 0),
        other => panic!("no emitter for style: {other}"),
    }
}

/// Runs `law` over every google/numpy corpus input, then reconciles the
/// observed violations against the `known` list.
fn check_law(law_name: &str, known: &[&str], law: impl Fn(&str, &str, &str) -> Vec<(String, String)>) {
    let mut failures = Vec::new();
    let mut passed_known: Vec<&str> = known.to_vec();
    let mut checked = 0;

    for style_dir in style_dirs() {
        let style = style_dir.file_name().unwrap().to_str().unwrap().to_owned();
        if style == "plain" {
            continue;
        }
        for txt_path in collect_inputs(&style_dir) {
            checked += 1;
            let input = std::fs::read_to_string(&txt_path).unwrap();
            for (case, detail) in law(&style, &corpus_name(&txt_path), &input) {
                if let Some(pos) = passed_known.iter().position(|k| *k == case) {
                    passed_known.remove(pos);
                } else {
                    failures.push(format!("{case}:\n{detail}"));
                }
            }
        }
    }

    assert!(checked > 0, "no corpus inputs exercised the {law_name} law");
    assert!(
        failures.is_empty(),
        "{} new {law_name} violation(s):\n\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        passed_known.is_empty(),
        "these KNOWN failures of the {law_name} law now pass — remove the stale entries:\n  {}",
        passed_known.join("\n  ")
    );
}

#[test]
fn emit_parse_is_idempotent() {
    check_law("idempotence", KNOWN_IDEMPOTENCE_FAILURES, |style, name, input| {
        let Some(model) = model_for(style, input) else {
            return Vec::new();
        };
        let first = emit_in(style, &model);
        let second = emit_in(style, &model_for(style, &first).expect("emitted output must parse"));
        if first == second {
            Vec::new()
        } else {
            vec![(name.to_owned(), diff(&first, &second))]
        }
    });
}

#[test]
fn emit_preserves_model() {
    check_law(
        "model stability",
        KNOWN_MODEL_STABILITY_FAILURES,
        |style, name, input| {
            let Some(model) = model_for(style, input) else {
                return Vec::new();
            };
            let reparsed = model_for(style, &emit_in(style, &model)).expect("emitted output must parse");
            if reparsed == model {
                Vec::new()
            } else {
                vec![(name.to_owned(), diff(&format!("{model:#?}"), &format!("{reparsed:#?}")))]
            }
        },
    );
}

#[test]
fn cross_style_conversion_preserves_model() {
    check_law(
        "cross-style conversion",
        KNOWN_CONVERSION_FAILURES,
        |style, name, input| {
            let Some(model) = model_for(style, input) else {
                return Vec::new();
            };
            let mut violations = Vec::new();
            for target in ["google", "numpy"] {
                if target == style {
                    continue;
                }
                let reparsed = model_for(target, &emit_in(target, &model)).expect("emitted output must parse");
                if reparsed != model {
                    violations.push((
                        format!("{style}->{target}: {name}"),
                        diff(&format!("{model:#?}"), &format!("{reparsed:#?}")),
                    ));
                }
            }
            violations
        },
    );
}
