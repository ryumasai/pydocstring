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

use common::corpus_cases;
use common::corpus_name;
use common::diff;
use pydocstring::model::Docstring;

// Real bugs flushed out by the realworld corpus ingest — each entry below is
// an emit/parse disagreement, NOT a representational limit. Clusters:
//
// (SA-indent) numpy emit_see_also writes a multi-line description raw
// (src/emit/numpy.rs), so continuation lines land at entry indent and
// re-parse as new name-only SeeAlso entries.
//
// (SA-role) emitters collapse `name` + description to one line `name : desc`
// (numpy) / `name: desc` (google); when the name starts with an rST role
// (`:func:`x``), find_term_colon's leading-colon guard (src/parse/utils.rs)
// rejects the whole line, so the re-parse keeps it as a single name (and
// comma-splits the description into extra names).
//
// (DEP-indent) the `.. deprecated::` body is stored with its continuation
// indent NOT dedented, and numpy emit re-indents by 4 on top — the indent
// grows by 4 each emit/parse cycle.
//
// (RET-flat) google emit_return writes a description-only Return's
// continuation lines raw at column 0 (src/emit/google.rs), dedenting them
// out of the Returns section; the re-parse silently drops every line after
// the first.
const KNOWN_IDEMPOTENCE_FAILURES: &[&str] = &[
    // (RET-flat)
    "third_party/fire/google/fire.txt",
    // (SA-indent) — the comma-split names lose their trailing comma.
    "third_party/numpy/numpy/einsum.txt",
    // (DEP-indent)
    "third_party/scipy/numpy/interpolate_pade.txt",
];
const KNOWN_MODEL_STABILITY_FAILURES: &[&str] = &[
    // (RET-flat)
    "third_party/fire/google/fire.txt",
    // (SA-indent)
    "third_party/numpy/numpy/convolve.txt",
    "third_party/numpy/numpy/einsum.txt",
    "third_party/numpy/numpy/linspace.txt",
    "third_party/numpy/numpy/ndarray.txt",
    "third_party/numpy/numpy/outer.txt",
    "third_party/numpy/numpy/packbits.txt",
    "third_party/numpy/numpy/roll.txt",
    "third_party/numpy/numpy/split.txt",
    "third_party/scipy/numpy/optimize_curve_fit.txt",
    "third_party/scipy/numpy/optimize_minimize.txt",
    // (SA-role)
    "third_party/scipy/numpy/integrate_simpson.txt",
    "third_party/scipy/numpy/interpolate_cubicspline.txt",
    "third_party/scipy/numpy/interpolate_interp1d.txt",
    "third_party/scipy/numpy/ndimage_label.txt",
    "third_party/scipy/numpy/signal_butter.txt",
    "third_party/scipy/numpy/signal_hilbert.txt",
    "third_party/scipy/numpy/signal_medfilt.txt",
    "third_party/scipy/numpy/signal_welch.txt",
    "third_party/scipy/numpy/stats_linregress.txt",
    // (DEP-indent)
    "third_party/scipy/numpy/interpolate_pade.txt",
];
/// Entries are `"<from>-><to>: <corpus path>"`, e.g. `"numpy->google: numpy/returns/yields_basic.txt"`.
const KNOWN_CONVERSION_FAILURES: &[&str] = &[
    // Fundamental NumPy ambiguity: a description-only Return has no
    // unambiguous NumPy form — the bare line re-parses as the type
    // (prefer_type, see the #26 discussion).
    "google->numpy: google/returns/returns_without_type.txt",
    // Representational limits of the Google format (#58): both sides are
    // individually faithful to napoleon, so these are permanent — a lossless
    // encoding would be a deliberate spec departure (v2 material, see #48).
    //
    // (a) Google Returns/Yields is a SINGLE entry — later `type:` lines fold
    // into the description (pinned by test_returns_multiple_lines), so
    // multiple numpy entries cannot survive.
    "numpy->google: numpy/regressions/issue26_rst_roles.txt",
    "numpy->google: numpy/returns/parse_named_returns.txt",
    "numpy->google: numpy/returns/yields_multiple.txt",
    // (b) Google has no named-return syntax — Return::name is dropped by
    // emit_return (a `name (type): desc` spelling would re-parse as pure
    // description, matching napoleon). Also contributes to the (a) cases.
    "numpy->google: numpy/returns/returns_multiline_description.txt",
    "numpy->google: numpy/returns/returns_no_spaces_around_colon.txt",
    "numpy->google: numpy/returns/yields_named.txt",
    //
    // ---- realworld corpus ----
    //
    // Description-only Returns/Yields (prefer_type ambiguity, same as
    // returns_without_type.txt above). fire/google/fire.txt is aggravated by
    // the (RET-flat) bug (see KNOWN_MODEL_STABILITY_FAILURES): its multi-line
    // description becomes one bare numpy line PER LINE, i.e. many entries.
    "google->numpy: third_party/absl/google/flags_define.txt",
    "google->numpy: third_party/absl/google/flags_define_enum.txt",
    "google->numpy: third_party/absl/google/flags_define_multi.txt",
    "google->numpy: third_party/absl/google/flags_flag_dict_to_args.txt",
    "google->numpy: third_party/absl/google/flags_text_wrap.txt",
    "google->numpy: third_party/absl/google/flags_validator.txt",
    "google->numpy: third_party/absl/google/logging_skip_log_prefix.txt",
    "google->numpy: third_party/fire/google/completion_membervisible.txt",
    "google->numpy: third_party/fire/google/decorators_setparsefns.txt",
    "google->numpy: third_party/fire/google/fire.txt",
    // Named and/or multiple NumPy returns — permanent limits (a)/(b) above.
    // (numpydoc's `name : type` return convention is near-universal in
    // real numpy/scipy docstrings, hence the breadth of this cluster.)
    "numpy->google: third_party/numpy/numpy/bincount.txt",
    "numpy->google: third_party/numpy/numpy/broadcast.txt",
    "numpy->google: third_party/numpy/numpy/busday_count.txt",
    "numpy->google: third_party/numpy/numpy/clip.txt",
    "numpy->google: third_party/numpy/numpy/convolve.txt",
    "numpy->google: third_party/numpy/numpy/diff.txt",
    "numpy->google: third_party/numpy/numpy/einsum.txt",
    "numpy->google: third_party/numpy/numpy/fft_fft.txt",
    "numpy->google: third_party/numpy/numpy/fft_fftfreq.txt",
    "numpy->google: third_party/numpy/numpy/fromfunction.txt",
    "numpy->google: third_party/numpy/numpy/fromiter.txt",
    "numpy->google: third_party/numpy/numpy/fromstring.txt",
    "numpy->google: third_party/numpy/numpy/histogram.txt",
    "numpy->google: third_party/numpy/numpy/interp.txt",
    "numpy->google: third_party/numpy/numpy/isclose.txt",
    "numpy->google: third_party/numpy/numpy/linalg_solve.txt",
    "numpy->google: third_party/numpy/numpy/linalg_svd.txt",
    "numpy->google: third_party/numpy/numpy/linspace.txt",
    "numpy->google: third_party/numpy/numpy/ma_masked_where.txt",
    "numpy->google: third_party/numpy/numpy/nanmean.txt",
    "numpy->google: third_party/numpy/numpy/outer.txt",
    "numpy->google: third_party/numpy/numpy/packbits.txt",
    "numpy->google: third_party/numpy/numpy/repeat.txt",
    "numpy->google: third_party/numpy/numpy/reshape.txt",
    "numpy->google: third_party/numpy/numpy/roll.txt",
    "numpy->google: third_party/numpy/numpy/searchsorted.txt",
    "numpy->google: third_party/numpy/numpy/split.txt",
    "numpy->google: third_party/numpy/numpy/stack.txt",
    "numpy->google: third_party/numpy/numpy/tile.txt",
    "numpy->google: third_party/scipy/numpy/linalg_expm.txt",
    "numpy->google: third_party/scipy/numpy/linalg_solve_triangular.txt",
    "numpy->google: third_party/scipy/numpy/optimize_curve_fit.txt",
    "numpy->google: third_party/scipy/numpy/optimize_minimize.txt",
    // (SA-role) through the google side — real bug, see
    // KNOWN_MODEL_STABILITY_FAILURES. Most of these ALSO hit the named-return
    // limits above, so fixing (SA-role) alone will not clear them.
    "numpy->google: third_party/scipy/numpy/integrate_simpson.txt",
    "numpy->google: third_party/scipy/numpy/interpolate_cubicspline.txt",
    "numpy->google: third_party/scipy/numpy/interpolate_interp1d.txt",
    "numpy->google: third_party/scipy/numpy/ndimage_label.txt",
    "numpy->google: third_party/scipy/numpy/signal_butter.txt",
    "numpy->google: third_party/scipy/numpy/signal_hilbert.txt",
    "numpy->google: third_party/scipy/numpy/signal_medfilt.txt",
    "numpy->google: third_party/scipy/numpy/signal_welch.txt",
    "numpy->google: third_party/scipy/numpy/stats_linregress.txt",
    // Free-text fidelity through the google round trip (real bugs):
    // numpy/where — a `::` literal block inside Notes loses its 4-space base
    // indent on google re-parse (plus the named-return limit).
    // numpy/dtype — a numpy unknown section whose header is a signature line
    // (`dtype(...)` underlined with `--` in the real docstring) has no valid
    // google header form; its google spelling re-parses as summary text.
    // scipy/interpolate_pade — (DEP-indent) directive-body indent drift
    // (plus the named-return limit).
    "numpy->google: third_party/numpy/numpy/where.txt",
    "numpy->google: third_party/numpy/numpy/dtype.txt",
    "numpy->google: third_party/scipy/numpy/interpolate_pade.txt",
    //
    // ---- scverse corpus (anndata / scanpy — the #26 reporters' ecosystem) ----
    //
    // ALL scverse conversion failures are the same #58 named/multiple-return
    // and prefer_type limits already documented above — NO new bug clusters,
    // and zero new idempotence/model-stability failures (the within-style
    // laws hold on every scverse input).
    //
    // The canonical #26 "Sets the following fields:" pattern (pca/umap/leiden/
    // tsne/neighbors/rank_genes_groups/regress_out/score_genes_cell_cycle):
    // its Returns definition list parses FAITHFULLY into named Return entries
    // — NAME=the backtick field path (`.obsm['X_pca' | key_added]`),
    // TYPE=the `:class:` role classifier, DESCRIPTION=the indented line — while
    // each prose intro line above it becomes a bare-TYPE entry (prefer_type).
    // Google has no named/multiple-return syntax (#58), so the round trip
    // folds them all into one description; the numpy parse itself is sound.
    //
    // Description-only returns (to_df/log1p/normalize_total/read_loom/
    // obs_vector/sparse_dataset/filter_cells/concat) are the prefer_type
    // ambiguity (a bare Returns line re-parses as the type, +trailing `:`),
    // same as returns_without_type.txt.
    //
    // concat additionally carries a `.. warning::` block whose 4-space body
    // indent is dropped on the google side — the same free-text-fidelity
    // limit as numpy/where above, not a new cluster.
    "numpy->google: third_party/anndata/numpy/concat.txt",
    "numpy->google: third_party/anndata/numpy/obs_vector.txt",
    "numpy->google: third_party/anndata/numpy/read_loom.txt",
    "numpy->google: third_party/anndata/numpy/sparse_dataset.txt",
    "numpy->google: third_party/anndata/numpy/to_df.txt",
    "numpy->google: third_party/scanpy/numpy/filter_cells.txt",
    "numpy->google: third_party/scanpy/numpy/leiden.txt",
    "numpy->google: third_party/scanpy/numpy/log1p.txt",
    "numpy->google: third_party/scanpy/numpy/neighbors.txt",
    "numpy->google: third_party/scanpy/numpy/normalize_total.txt",
    "numpy->google: third_party/scanpy/numpy/pca.txt",
    "numpy->google: third_party/scanpy/numpy/rank_genes_groups.txt",
    "numpy->google: third_party/scanpy/numpy/regress_out.txt",
    "numpy->google: third_party/scanpy/numpy/score_genes_cell_cycle.txt",
    "numpy->google: third_party/scanpy/numpy/tsne.txt",
    "numpy->google: third_party/scanpy/numpy/umap.txt",
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
        "google" => pydocstring::emit::google::emit_google(model, &pydocstring::emit::EmitOptions::default()),
        "numpy" => pydocstring::emit::numpy::emit_numpy(model, &pydocstring::emit::EmitOptions::default()),
        other => panic!("no emitter for style: {other}"),
    }
}

/// Runs `law` over every google/numpy corpus input, then reconciles the
/// observed violations against the `known` list.
fn check_law(law_name: &str, known: &[&str], law: impl Fn(&str, &str, &str) -> Vec<(String, String)>) {
    let mut failures = Vec::new();
    let mut passed_known: Vec<&str> = known.to_vec();
    let mut checked = 0;

    for (style, txt_path) in corpus_cases() {
        if style == "plain" {
            continue;
        }
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
