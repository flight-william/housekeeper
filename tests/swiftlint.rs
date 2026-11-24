//! SwiftLint keeps to the repo's own `excluded:`: the stage's Swift files are named
//! to it with `--force-exclude`, so it never walks the tree. A named file skips
//! `included:` (SwiftLint 0.65.1, Configuration+LintableFiles.swift), a stated
//! limit. Output captured from SwiftLint 0.65.1.

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

/// With the repo's config, the stage's Swift files are named, with --force-exclude
/// so its `excluded:` still holds; a file the stage did not touch is not linted.
#[test]
fn with_the_repos_config_the_stage_files_are_named_and_force_excluded() {
    let root = common::fresh("swiftlint-config");
    write(&root, ".swiftlint.yml", "excluded:\n  - Generated\n");
    write(&root, "Sources/Untouched.swift", "let old = 0;\n");
    let cmd = SwiftLint
        .command(&ctx(&root, &["Sources/A.swift", "Generated/G.swift"]))
        .unwrap();
    assert_eq!(
        cmd.args.join(" "),
        "lint --reporter json --quiet --force-exclude -- Sources/A.swift Generated/G.swift"
    );
}

#[test]
fn without_a_config_the_files_are_named() {
    let root = common::fresh("swiftlint-no-config");
    let cmd = SwiftLint
        .command(&ctx(&root, &["a.py", "Sources/A.swift"]))
        .unwrap();
    assert_eq!(
        cmd.args.join(" "),
        "lint --reporter json --quiet --force-exclude -- Sources/A.swift"
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
fn a_stage_whose_swift_files_are_all_excluded_is_nothing_to_fix() {
    let root = PathBuf::from("/repo");
    let found = SwiftLint
        .parse(
            &ctx(&root, &["Generated/Made.swift"]),
            &out(1, "", NO_LINTABLE),
        )
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

fn places(report: &house::reply::Report) -> Vec<String> {
    report
        .findings()
        .iter()
        .map(|f| house::reply::place_text(&f.place).to_string())
        .collect()
}

/// A repo with a `.swiftlint.yml` excluding `Generated`, the same problem in a
/// source file and a generated one, and a tracked symlink to an ancestor folder,
/// on which SwiftLint 0.65.1 crashes when it walks the tree.
fn scoped_repo(name: &str) -> PathBuf {
    let root = common::repo(name);
    write(&root, ".swiftlint.yml", "excluded:\n  - Generated\n");
    write(&root, "Sources/Main.swift", "let value = 1;\n");
    write(&root, "Generated/Made.swift", "let made = 2;\n");
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::os::unix::fs::symlink("../..", root.join("a/b/root")).unwrap();
    root
}

/// The real tool: the staged source file has its finding, the staged file in
/// `excluded:` has none, and the symlink does not crash the run.
#[test]
fn the_repos_excluded_scope_is_kept_and_a_symlink_to_an_ancestor_is_no_crash() {
    if !installed() {
        return;
    }
    let root = scoped_repo("swiftlint-scope");
    common::git(&root, &["add", "-A"]);
    let report = cli::check(&root, Stage::Commit, vec![Box::new(SwiftLint)]);
    assert!(report.cannots().is_empty(), "{}", report.text());
    assert_eq!(places(&report), ["Sources/Main.swift"], "{}", report.text());
}

/// The real tool: a stage whose only Swift file is in `excluded:` checks nothing.
#[test]
fn a_stage_of_only_excluded_files_is_nothing_to_fix() {
    if !installed() {
        return;
    }
    let root = scoped_repo("swiftlint-only-excluded");
    common::git(&root, &["add", "a", ".swiftlint.yml", "Generated"]);
    let report = cli::check(&root, Stage::Commit, vec![Box::new(SwiftLint)]);
    assert!(report.cannots().is_empty(), "{}", report.text());
    assert!(report.findings().is_empty(), "{}", report.text());
}

/// The real tool, a stated limit: SwiftLint lints a named file whatever the
/// config's `included:` says, so a staged probe outside it has its finding.
#[test]
fn a_staged_file_outside_included_is_linted_a_known_limit() {
    if !installed() {
        return;
    }
    let root = common::repo("swiftlint-included");
    write(&root, ".swiftlint.yml", "included:\n  - Sources\n");
    write(&root, "Probes/Spike.swift", "let other = 2;\n");
    common::git(&root, &["add", "-A"]);
    let report = cli::check(&root, Stage::Commit, vec![Box::new(SwiftLint)]);
    assert!(report.cannots().is_empty(), "{}", report.text());
    assert_eq!(places(&report), ["Probes/Spike.swift"], "{}", report.text());
}
