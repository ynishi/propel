use arch_lint::rules::{NoErrorSwallowing, NoSilentResultDrop};
use arch_lint::{Analyzer, Severity};

/// Runs AL003 (no-error-swallowing) and AL013 (no-silent-result-drop) against
/// the workspace source tree. Violations in test code are excluded.
#[test]
fn arch_lint_al003_al013() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");

    let analyzer = Analyzer::builder()
        .root(root)
        .exclude("**/target/**")
        .exclude("**/tests/**")
        .exclude("examples/**")
        .exclude("crates/propel-sdk/**")
        .rule(NoErrorSwallowing::new())
        .rule(NoSilentResultDrop::new())
        .build()
        .expect("build analyzer");

    let result = analyzer.analyze().expect("analyze");

    if result.has_violations_at(Severity::Warning) {
        let report = result.format_test_report(Severity::Warning);
        panic!("{report}");
    }
}
