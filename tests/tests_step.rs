//! The test step: `.house.toml` `[tests]` commands run from the repo root after the
//! adapters; a failing one is one R7 finding, its output's tail kept in the record.

mod common;
mod wire_run;

use house::adapter::{Adapter, CannotCheck, Command, Context, Output, Stage};
use house::finding::{Finding, Place};
use house::reply::Report;
use house::{cli, config};
use std::path::{Path, PathBuf};
use wire_run::{house, records};

/// A repo with `.house.toml` = `toml` and one staged file (or none).
fn repo_with(name: &str, toml: &str, stage_a_file: bool) -> PathBuf {
    let dir = common::repo(name);
    std::fs::write(dir.join(".house.toml"), toml).unwrap();
    common::git(&dir, &["add", ".house.toml"]);
    common::git(&dir, &["commit", "-q", "--no-verify", "-m", "config"]);
    if stage_a_file {
        std::fs::write(dir.join("a.txt"), "a\n").unwrap();
        common::git(&dir, &["add", "a.txt"]);
    }
    dir
}

fn tests_toml(commands: &str) -> String {
    format!("[tools]\ncommit = []\n[tests]\ncommit = [{commands}]\n")
}

fn check(root: &Path) -> Report {
    cli::check(root, Stage::Commit, Vec::new())
}

#[test]
fn a_failing_test_command_is_one_r7_finding_for_the_repo() {
    let root = repo_with(
        "tests-fail",
        &tests_toml(r#"["sh", "-c", "echo boom; exit 3"]"#),
        true,
    );
    let r = check(&root);
    let found = r.findings();
    assert_eq!(found.len(), 1, "{}", r.text());
    let f = found[0];
    assert_eq!(
        (f.rule.as_str(), &f.place, f.what.as_str(), f.fix.as_str()),
        (
            "R7",
            &Place::Repo,
            "test command failed: sh -c echo boom; exit 3",
            "run it and fix the failures"
        )
    );
    assert_eq!(r.exit(), 1);
    assert!(
        !r.text().contains("boom\n"),
        "output stays out of the reply"
    );
}

#[test]
fn a_passing_test_command_is_nothing_to_fix() {
    let root = repo_with("tests-pass", &tests_toml(r#"["true"]"#), true);
    let r = check(&root);
    assert_eq!(r.exit(), 0, "{}", r.text());
    assert!(
        r.tools
            .iter()
            .any(|t| t.name == "tests" && t.exit == Some(0))
    );
}

#[test]
fn each_command_is_run_and_judged_on_its_own() {
    let root = repo_with(
        "tests-each",
        &tests_toml(r#"["false"], ["true"], ["sh", "-c", "exit 1"]"#),
        true,
    );
    let r = check(&root);
    let whats: Vec<&str> = r.findings().iter().map(|f| f.what.as_str()).collect();
    assert_eq!(
        whats,
        [
            "test command failed: false",
            "test command failed: sh -c exit 1"
        ]
    );
}

#[test]
fn test_commands_run_in_the_repo_root_with_no_shell() {
    let root = repo_with(
        "tests-root",
        &tests_toml(
            r#"["test", "-f", ".house.toml"], ["sh", "-c", "test \"$0\" = '$HOME'", "$HOME"]"#,
        ),
        true,
    );
    let r = check(&root);
    assert_eq!(r.exit(), 0, "{}", r.text());
}

#[test]
fn a_missing_test_program_is_cannot_check() {
    let root = repo_with(
        "tests-missing",
        &tests_toml(r#"["house-no-such-program", "x"]"#),
        true,
    );
    let r = check(&root);
    let cannots = r.cannots();
    assert_eq!(cannots.len(), 1, "{}", r.text());
    let (tool, c) = cannots[0];
    assert_eq!(tool, "tests");
    assert_eq!(c.what, "house-no-such-program is not installed");
    assert!(c.fix.contains("[tests]"), "{}", c.fix);
    assert_eq!(r.exit(), 2);
}

#[test]
fn with_nothing_to_check_the_tests_are_skipped() {
    let root = repo_with("tests-nothing", &tests_toml(r#"["false"]"#), false);
    let r = check(&root);
    assert_eq!(r.exit(), 0, "{}", r.text());
    assert!(r.tools.iter().any(|t| t.name == "tests" && t.skipped));
}

struct Named;

impl Adapter for Named {
    fn name(&self) -> &'static str {
        "named"
    }
    fn program(&self) -> &'static str {
        "true"
    }
    fn brew_formula(&self) -> &'static str {
        "coreutils"
    }
    fn command(&self, _: &Context) -> Option<Command> {
        Some(Command {
            program: "true".into(),
            args: Vec::new(),
        })
    }
    fn parse(&self, _: &Context, _: &Output) -> Result<Vec<Finding>, CannotCheck> {
        Ok(Vec::new())
    }
}

#[test]
fn the_tests_run_after_the_adapters() {
    let toml = "[tools]\ncommit = [\"named\"]\n[tests]\ncommit = [[\"true\"]]\n";
    let root = repo_with("tests-order", toml, true);
    let r = cli::check(&root, Stage::Commit, vec![Box::new(Named)]);
    let names: Vec<&str> = r.tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["named", "tests"]);
}

#[test]
fn tests_config_push_falls_back_to_commit_and_an_empty_command_is_refused() {
    let cfg = config::parse("[tests]\ncommit = [[\"a\", \"b\"]]\n").unwrap();
    let want = vec![vec!["a".to_string(), "b".to_string()]];
    assert_eq!(config::tests_for(&cfg, Stage::Commit), want.as_slice());
    assert_eq!(config::tests_for(&cfg, Stage::Push), want.as_slice());
    let cfg = config::parse("[tests]\ncommit = [[\"a\"]]\npush = []\n").unwrap();
    assert!(config::tests_for(&cfg, Stage::Push).is_empty());
    assert!(config::tests_for(&config::Config::default(), Stage::Commit).is_empty());
    let what = config::parse("[tests]\ncommit = [[]]\n").unwrap_err().what;
    assert!(what.contains("empty command"), "{what}");
    assert!(config::parse("[tests]\ncomit = []\n").is_err());
}

#[test]
fn the_record_keeps_the_last_20_lines_of_a_failing_commands_output() {
    let root = repo_with(
        "tests-record",
        &tests_toml(r#"["sh", "-c", "seq 1 30; echo oops >&2; exit 1"]"#),
        true,
    );
    let log = common::fresh("tests-record-log");
    let (code, text) = house(&root, &log, &["check", "--stage", "commit"], &[]);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("R7 repo: test command failed: sh -c seq 1 30; echo oops >&2; exit 1. fix: run it and fix the failures."),
        "{text}"
    );
    assert!(
        !text.contains("\n30\n") && !text.contains("oops\n"),
        "{text}"
    );
    let rec = records(&log);
    let failed = &rec[0]["test_output"][0];
    assert_eq!(failed["command"], "sh -c seq 1 30; echo oops >&2; exit 1");
    let stdout: Vec<String> = (11..=30).map(|n| n.to_string()).collect();
    assert_eq!(failed["stdout_tail"], serde_json::json!(stdout));
    assert_eq!(failed["stderr_tail"], serde_json::json!(["oops"]));
}

#[test]
fn a_test_command_that_hangs_is_killed_by_the_guard_and_cannot_check() {
    let root = common::fresh("tests-hang");
    let hang = vec![vec!["sleep".to_string(), "5".to_string()]];
    let runs = house::tests_step::run(&root, &hang, std::time::Duration::from_millis(200));
    let c = runs[0].tool.cannot.as_ref().expect("cannot check");
    assert_eq!(c.what, "sleep did not finish in 0 s");
    assert!(runs[0].tool.findings.is_empty());
}
