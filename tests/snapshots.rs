//! Corpus snapshot tests.
//!
//! Every `.txt` file under `tests/corpus/<style>/` (any nesting depth) is a
//! docstring input. It is parsed with the parser named by its top-level
//! directory (`google`, `numpy`, or `plain`), and the resulting CST — plus,
//! for `google` and `numpy`, the output of the model round-trip `to_model`
//! → `emit_*` — is compared byte-for-byte against the sibling `<name>.snap`
//! file. See `tests/corpus/README.md` for the directory layout.
//!
//! - To add a test (e.g. an issue reproducer): drop a `.txt` file into the
//!   corpus directory for its style, then bless.
//! - To bless (create or update) snapshots:
//!   `UPDATE_SNAPSHOTS=1 cargo test --test snapshots`
//! - Input files are read verbatim: a trailing newline in the file is a
//!   trailing newline in the docstring input.

use std::fs;
use std::path::{Path, PathBuf};

use pydocstring::syntax::Parsed;

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("corpus")
}

/// Renders the snapshot text for one input: CST shape, then (for styles with
/// an emitter) the normalized output of the model round-trip.
fn render_snapshot(style: &str, input: &str) -> String {
    let (parsed, emitted): (Parsed, Option<String>) = match style {
        "google" => {
            let parsed = pydocstring::parse::google::parse_google(input);
            let emitted = pydocstring::parse::google::to_model::to_model(&parsed)
                .map(|model| pydocstring::emit::google::emit_google(&model, 0));
            (parsed, emitted)
        }
        "numpy" => {
            let parsed = pydocstring::parse::numpy::parse_numpy(input);
            let emitted = pydocstring::parse::numpy::to_model::to_model(&parsed)
                .map(|model| pydocstring::emit::numpy::emit_numpy(&model, 0));
            (parsed, emitted)
        }
        "plain" => (pydocstring::parse::plain::parse_plain(input), None),
        other => panic!("unknown corpus style directory: {other}"),
    };

    let mut snap = String::new();
    snap.push_str("=== CST ===\n");
    snap.push_str(&parsed.pretty_print());
    if !snap.ends_with('\n') {
        snap.push('\n');
    }
    if let Some(emitted) = emitted {
        snap.push_str("=== EMIT ===\n");
        snap.push_str(&emitted);
        if !snap.ends_with('\n') {
            snap.push('\n');
        }
    }
    snap
}

/// A minimal line diff: everything from the first to the last differing line,
/// prefixed with `-` (expected) / `+` (actual).
fn diff(expected: &str, actual: &str) -> String {
    let exp: Vec<&str> = expected.lines().collect();
    let act: Vec<&str> = actual.lines().collect();
    let common = exp.len().min(act.len());
    let first = (0..common).find(|&i| exp[i] != act[i]).unwrap_or(common);
    let mut tail = 0;
    while tail < common - first && exp[exp.len() - 1 - tail] == act[act.len() - 1 - tail] {
        tail += 1;
    }
    let mut out = String::new();
    for line in &exp[first..exp.len() - tail] {
        out.push_str("  - ");
        out.push_str(line);
        out.push('\n');
    }
    for line in &act[first..act.len() - tail] {
        out.push_str("  + ");
        out.push_str(line);
        out.push('\n');
    }
    format!("  (first difference at line {})\n{out}", first + 1)
}

/// Collects every `.txt` file under `dir`, recursively.
fn collect_inputs(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_inputs(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "txt") {
            out.push(path);
        }
    }
}

#[test]
fn corpus_snapshots() {
    let update = std::env::var_os("UPDATE_SNAPSHOTS").is_some();
    let mut failures = Vec::new();
    let mut checked = 0;

    let mut style_dirs: Vec<PathBuf> = fs::read_dir(corpus_dir())
        .expect("tests/corpus directory missing")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    style_dirs.sort();

    for style_dir in style_dirs {
        let style = style_dir.file_name().unwrap().to_str().unwrap().to_owned();
        let mut inputs = Vec::new();
        collect_inputs(&style_dir, &mut inputs);
        inputs.sort();

        for txt_path in inputs {
            checked += 1;
            let input = fs::read_to_string(&txt_path).unwrap();
            let actual = render_snapshot(&style, &input);
            let snap_path = txt_path.with_extension("snap");
            let expected = fs::read_to_string(&snap_path).ok();

            if expected.as_deref() == Some(actual.as_str()) {
                continue;
            }
            if update {
                fs::write(&snap_path, &actual).unwrap();
                eprintln!("blessed {}", snap_path.display());
            } else {
                let name = txt_path.strip_prefix(corpus_dir()).unwrap().display().to_string();
                match expected {
                    None => failures.push(format!("{name}: snapshot file missing")),
                    Some(expected) => failures.push(format!("{name}:\n{}", diff(&expected, &actual))),
                }
            }
        }
    }

    assert!(checked > 0, "no corpus input files found under tests/corpus");
    assert!(
        failures.is_empty(),
        "{} snapshot mismatch(es):\n\n{}\n\
         Run `UPDATE_SNAPSHOTS=1 cargo test --test snapshots` to bless.",
        failures.len(),
        failures.join("\n")
    );
}
