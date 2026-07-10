//! Visitor tests for the generic directive hook (#84).
//!
//! Every `DIRECTIVE` node fires the generic `visit_*_directive` hook, whatever
//! its name. A `deprecated`-named directive is a specialization: it fires the
//! generic hook *and* the deprecation hook, in that order.

use pydocstring::parse::google::GoogleDeprecation;
use pydocstring::parse::google::GoogleDirective;
use pydocstring::parse::google::parse_google;
use pydocstring::parse::numpy::NumPyDeprecation;
use pydocstring::parse::numpy::NumPyDirective;
use pydocstring::parse::numpy::parse_numpy;
use pydocstring::parse::visitor::DocstringVisitor;
use pydocstring::parse::visitor::walk;
use pydocstring::syntax::Parsed;

/// Records, in order, the visitor events it receives as `"kind:name"`.
#[derive(Default)]
struct Recorder {
    events: Vec<String>,
}

impl DocstringVisitor for Recorder {
    type Error = std::convert::Infallible;

    fn visit_google_directive(&mut self, _p: &Parsed, dir: &GoogleDirective<'_>) -> Result<(), Self::Error> {
        self.events.push(format!("google_directive:{}", dir.name().text()));
        Ok(())
    }

    fn visit_google_deprecation(&mut self, _p: &Parsed, dep: &GoogleDeprecation<'_>) -> Result<(), Self::Error> {
        self.events.push(format!("google_deprecation:{}", dep.version().text()));
        Ok(())
    }

    fn visit_numpy_directive(&mut self, _p: &Parsed, dir: &NumPyDirective<'_>) -> Result<(), Self::Error> {
        self.events.push(format!("numpy_directive:{}", dir.name().text()));
        Ok(())
    }

    fn visit_numpy_deprecation(&mut self, _p: &Parsed, dep: &NumPyDeprecation<'_>) -> Result<(), Self::Error> {
        self.events.push(format!("numpy_deprecation:{}", dep.version().text()));
        Ok(())
    }
}

fn record_google(src: &str) -> Vec<String> {
    let parsed = parse_google(src);
    let mut rec = Recorder::default();
    walk(&parsed, parsed.root(), &mut rec).unwrap();
    rec.events
}

fn record_numpy(src: &str) -> Vec<String> {
    let parsed = parse_numpy(src);
    let mut rec = Recorder::default();
    walk(&parsed, parsed.root(), &mut rec).unwrap();
    rec.events
}

/// The generic hook fires for a non-deprecated directive; the deprecation hook
/// does not.
#[test]
fn generic_hook_fires_for_non_deprecated_directive() {
    assert_eq!(
        record_google("Summary.\n\n.. versionadded:: 2.0\n\nArgs:\n    x: v\n"),
        vec!["google_directive:versionadded"]
    );
    assert_eq!(
        record_numpy("Summary.\n\n.. versionadded:: 2.0\n\nParameters\n----------\nx : int\n    Desc.\n"),
        vec!["numpy_directive:versionadded"]
    );
}

/// A deprecated directive fires BOTH hooks — generic first, then deprecation.
#[test]
fn deprecated_directive_fires_generic_then_deprecation() {
    assert_eq!(
        record_google("Summary.\n\n.. deprecated:: 1.6.0\n\nArgs:\n    x: v\n"),
        vec!["google_directive:deprecated", "google_deprecation:1.6.0"]
    );
    assert_eq!(
        record_numpy("Summary.\n\n.. deprecated:: 1.6.0\n\nParameters\n----------\nx : int\n    Desc.\n"),
        vec!["numpy_directive:deprecated", "numpy_deprecation:1.6.0"]
    );
}

/// Each directive in a consecutive run fires the generic hook once.
#[test]
fn generic_hook_fires_per_directive_in_a_run() {
    assert_eq!(
        record_numpy(
            "Summary.\n\n.. deprecated:: 1.6.0\n.. versionadded:: 2.0\n\nParameters\n----------\nx : int\n    Desc.\n"
        ),
        vec![
            "numpy_directive:deprecated",
            "numpy_deprecation:1.6.0",
            "numpy_directive:versionadded",
        ]
    );
}
