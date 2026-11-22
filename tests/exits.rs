//! Every adapter validates its tool's exit code against its own table.
//! A signal (exit -1) or an exit the tool does not document is cannot-check even when
//! the output parses; a "found" exit with no findings is cannot-check.

use house::adapter::{Adapter, Context, Output, Stage};
use house::adapters;

fn ctx() -> Context {
    Context {
        root: "/repo".into(),
        stage: Stage::Commit,
        files: vec![],
        push_range: None,
    }
}

/// Output each tool gives when it ran and found nothing.
fn clean_output(tool: &str) -> &'static str {
    match tool {
        "ruff" | "ruff-format" | "swiftlint" | "gitleaks" => "[]",
        "shellcheck" => r#"{"comments":[]}"#,
        "semgrep" => r#"{"results":[],"errors":[]}"#,
        "cargo-fmt" | "clippy" | "swift-format" | "r3" => "",
        other => panic!("no clean output for {other}: add it here"),
    }
}

/// The exit each tool gives when it found problems; `None` when it has none.
fn found_exit(tool: &str) -> Option<i32> {
    match tool {
        "ruff" | "ruff-format" | "cargo-fmt" | "swift-format" | "shellcheck" | "gitleaks"
        | "semgrep" => Some(1),
        "swiftlint" => Some(2),
        "clippy" => Some(101),
        "r3" => None,
        other => panic!("no found exit for {other}: add it here"),
    }
}

fn out(exit: i32, stdout: &str) -> Output {
    Output {
        exit,
        stdout: stdout.into(),
        stderr: "tool failed after emitting report".into(),
    }
}

#[test]
fn clean_output_with_exit_zero_is_clean_for_every_adapter() {
    for a in adapters::all() {
        let got = a.parse(&ctx(), &out(0, clean_output(a.name())));
        assert_eq!(got, Ok(vec![]), "{}", a.name());
    }
}

/// Regression: exit -1 (a signal) plus valid empty output must not become house exit 0.
#[test]
fn a_signal_with_valid_empty_output_is_cannot_check_for_every_adapter() {
    let passed: Vec<_> = adapters::all()
        .into_iter()
        .filter(|a| a.parse(&ctx(), &out(-1, clean_output(a.name()))).is_ok())
        .map(|a| a.name())
        .collect();
    assert_eq!(passed, Vec::<&str>::new());
}

#[test]
fn an_undocumented_exit_with_valid_output_is_cannot_check_for_every_adapter() {
    let passed: Vec<_> = adapters::all()
        .into_iter()
        .filter(|a| a.parse(&ctx(), &out(42, clean_output(a.name()))).is_ok())
        .map(|a| a.name())
        .collect();
    assert_eq!(passed, Vec::<&str>::new());
}

/// Regression: ruff exit 1 plus [] was clean; a "found" exit with no findings is
/// cannot-check, for every tool.
#[test]
fn a_found_exit_with_no_findings_is_cannot_check_for_every_adapter() {
    let passed: Vec<_> = adapters::all()
        .into_iter()
        .filter(|a| {
            found_exit(a.name())
                .is_some_and(|exit| a.parse(&ctx(), &out(exit, clean_output(a.name()))).is_ok())
        })
        .map(|a| a.name())
        .collect();
    assert_eq!(passed, Vec::<&str>::new());
}

#[test]
fn semgrep_without_the_results_list_is_cannot_check() {
    let semgrep = adapters::semgrep::Semgrep;
    assert!(semgrep.parse(&ctx(), &out(0, "{}")).is_err());
}
