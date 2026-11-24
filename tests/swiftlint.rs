//! SwiftLint keeps to the repo's own scope: with a `.swiftlint.yml` at the root,
//! SwiftLint picks the files itself (its `included:` and `excluded:`), and house
//! keeps the findings on the files the stage checks. Output captured from
//! SwiftLint 0.65.1.

mod common;

use house::adapter::{Adapter, Context, Output, Stage};
use house::adapters::swiftlint::SwiftLint;
use house::cli;
use house::finding::Place;
use std::path::{Path, PathBuf};

const SCOPE: &str = include_str!("fixtures/swiftlint/scope.json");
const NO_LINTABLE: &str = include_str!("fixtures/swiftlint/no_lintable.stderr");

fn ctx(root: &Path, files: &[&str]) -> Context {
    Context {
        root: root.to_path_buf(),
        stage: Stage::Commit,
        files: files.iter().map(|f| f.to_string()).collect(),
        push_range: None,
        languages: Default::default(),
    }
}

fn out(exit: i32, stdout: &str, stderr: &str) -> Output {
    Output {
        exit,
        stdout: stdout.into(),
        stderr: stderr.into(),
    }
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn with_the_repos_config_swiftlint_picks_the_files_itself() {
    let root = common::fresh("swiftlint-config");
    write(&root, ".swiftlint.yml", "included:\n  - Sources\n");
    let cmd = SwiftLint
        .command(&ctx(&root, &["Sources/A.swift", "Probes/P.swift"]))
        .unwrap();
    assert_eq!(cmd.args.join(" "), "lint --reporter json --quiet");
}

#[test]
fn without_a_config_the_files_are_named() {
    let root = common::fresh("swiftlint-no-config");
    let cmd = SwiftLint
        .command(&ctx(&root, &["a.py", "Sources/A.swift"]))
        .unwrap();
    assert_eq!(
        cmd.args.join(" "),
        "lint --reporter json --quiet Sources/A.swift"
    );
    assert!(SwiftLint.command(&ctx(&root, &["a.py"])).is_none());
}

/// A.swift has a serious violation (exit 2), but only B.swift is checked: B's
/// warning is the one finding, and exit 2 is not a cannot-check.
#[test]
fn only_findings_on_the_checked_files_are_kept() {
    let root = PathBuf::from("/repo");
    let found = SwiftLint
        .parse(&ctx(&root, &["Sources/B.swift"]), &out(2, SCOPE, ""))
        .unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].place, Place::File("Sources/B.swift".into()));
    assert_eq!(found[0].tool_rule, "swiftlint trailing_semicolon");
    let both = ["Sources/A.swift", "Sources/B.swift"];
    let found = SwiftLint
        .parse(&ctx(&root, &both), &out(2, SCOPE, ""))
        .unwrap();
    assert_eq!(found.len(), 2);
}

#[test]
fn a_scope_with_no_swift_file_is_nothing_to_fix() {
    let root = PathBuf::from("/repo");
    let found = SwiftLint
        .parse(&ctx(&root, &["Probes/P.swift"]), &out(1, "", NO_LINTABLE))
        .unwrap();
    assert!(found.is_empty());
    for (exit, stderr) in [
        (2, NO_LINTABLE),
        (1, "Error: Could not read configuration\n"),
        (1, "Error: No lintable files found\n"),
    ] {
        let got = SwiftLint.parse(&ctx(&root, &["A.swift"]), &out(exit, "", stderr));
        assert!(got.is_err(), "exit {exit}, {stderr:?}: {got:?}");
    }
}

fn installed() -> bool {
    let found = std::process::Command::new("swiftlint")
        .arg("version")
        .output()
        .is_ok();
    if !found {
        eprintln!("note: swiftlint is not installed; skipping the run on a repo");
    }
    found
}

/// The real tool: a probe file the repo's `included:` leaves out gives nothing,
/// the same problem in a source file is a finding.
#[test]
fn a_file_outside_the_repos_scope_gives_no_finding() {
    if !installed() {
        return;
    }
    let root = common::repo("swiftlint-scope");
    write(&root, ".swiftlint.yml", "included:\n  - Sources\n");
    write(&root, "Sources/Main.swift", "let value = 1;\n");
    write(&root, "Probes/Spike.swift", "let other = 2;\n");
    common::git(&root, &["add", "-A"]);
    let report = cli::check(&root, Stage::Commit, vec![Box::new(SwiftLint)]);
    assert!(report.cannots().is_empty(), "{}", report.text());
    let places: Vec<String> = report
        .findings()
        .iter()
        .map(|f| house::reply::place_text(&f.place).to_string())
        .collect();
    assert_eq!(places, ["Sources/Main.swift"], "{}", report.text());
}
