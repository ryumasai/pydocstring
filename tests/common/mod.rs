//! Helpers shared by the corpus-driven test harnesses
//! (`tests/snapshots.rs`, `tests/roundtrip.rs`, `tests/trivia.rs`).
//!
//! Each test binary compiles its own copy of this module and not every
//! binary uses every helper, so unused-code lints are silenced.
#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// Root of the corpus: `tests/corpus`.
pub fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("corpus")
}

/// The style directories under the corpus root, sorted.
pub fn style_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(corpus_dir())
        .expect("tests/corpus directory missing")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// Every corpus input as a `(style, txt_path)` pair, sorted by path.
///
/// Two layouts coexist under `tests/corpus`:
///
/// * plain first-party style dirs — `google/`, `numpy/`, `plain/`, where the
///   top-level directory names the parser style, and
/// * the third-party subtree — `third_party/<lib>/<style>/`, where the style
///   is the directory one level below the library (each `<lib>/` also holds a
///   `LICENSE` file, which is skipped since only `.txt` inputs are collected).
pub fn corpus_cases() -> Vec<(String, PathBuf)> {
    let mut cases = Vec::new();
    for style_dir in style_dirs() {
        let name = style_dir.file_name().unwrap().to_str().unwrap();
        if name == "third_party" {
            // third_party/<lib>/<style>/*.txt — style is the inner dir.
            for lib_dir in read_sorted_dirs(&style_dir) {
                for inner in read_sorted_dirs(&lib_dir) {
                    let style = inner.file_name().unwrap().to_str().unwrap().to_owned();
                    for txt in collect_inputs(&inner) {
                        cases.push((style.clone(), txt));
                    }
                }
            }
        } else {
            // Plain style dir — the directory names the style.
            let style = name.to_owned();
            for txt in collect_inputs(&style_dir) {
                cases.push((style.clone(), txt));
            }
        }
    }
    cases.sort_by(|a, b| a.1.cmp(&b.1));
    cases
}

/// Immediate subdirectories of `dir`, sorted.
fn read_sorted_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// Collects every `.txt` file under `dir` recursively, sorted.
pub fn collect_inputs(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "txt") {
                out.push(path);
            }
        }
    }
    let mut inputs = Vec::new();
    walk(dir, &mut inputs);
    inputs.sort();
    inputs
}

/// Corpus-relative display name for a corpus input path.
pub fn corpus_name(path: &Path) -> String {
    path.strip_prefix(corpus_dir()).unwrap().display().to_string()
}

/// A minimal line diff: everything from the first to the last differing line,
/// prefixed with `-` (expected) / `+` (actual).
pub fn diff(expected: &str, actual: &str) -> String {
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
