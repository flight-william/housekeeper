//! A staged file whose name starts with `-` reaches each tool as a file, never as one
//! of its options: `-dash.swift` is linted, and `--output=Sources/A.swift` (the file
//! `A.swift` in a folder named `--output=Sources`) is linted too, rather than making
//! the tool write its report over the tracked `Sources/A.swift`. Each test runs the
//! real tool through `house check` and passes with a note when it is not installed.

mod common;

use house::adapter::{Adapter, Stage};
use house::adapters::{
    cargo_deny::CargoDeny, cargo_fmt::CargoFmt, clippy::Clippy, r3::R3, ruff::Ruff,
    ruff::RuffFormat, semgrep::Semgrep, shellcheck::Shellcheck, swift_format::SwiftFormat,
    swiftformat::Swiftformat, swiftlint::SwiftLint,
};
use house::cli;
use house::reply::Report;
use std::path::{Path, PathBuf};
use std::process::Command;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn installed(program: &str, args: &[&str]) -> bool {
    let found = Command::new(program)
        .args(args)
        .output()
        .is_ok_and(|o| o.status.success());
    if !found {
        eprintln!("note: {program} is not installed; skipping the run on a repo");
    }
    found
}

/// The two names every tool is tried with, then any the tool's own options make
/// dangerous, each with extension `ext`.
fn names(ext: &str, extra: &[&str]) -> Vec<String> {
    ["-dash", "--output=Sources/A"]
        .iter()
        .chain(extra)
        .map(|n| format!("{n}.{ext}"))
        .collect()
}

/// A repo with `setup` and a clean `Sources/A.<ext>` committed, then `bad` staged
/// under every name in `staged`.
fn repo(name: &str, ext: &str, setup: &[(&str, &str)], staged: &[String], bad: &str) -> PathBuf {
    let root = common::repo(name);
    for (path, text) in setup {
        write(&root, path, text);
    }
    write(&root, &format!("Sources/A.{ext}"), CLEAN);
    common::git(&root, &["add", "-A"]);
    common::git(&root, &["commit", "-q", "-m", "clean"]);
    for path in staged {
        write(&root, path, bad);
    }
    common::git(&root, &["add", "-A"]);
    root
}

/// What the committed `Sources/A.<ext>` holds: no tool may write over it.
const CLEAN: &str = "keep\n";

/// The adapter's run: no cannot-check, a finding on every name in `want`, and no
/// tracked file changed.
fn linted_as_files(root: &Path, adapter: Box<dyn Adapter>, want: &[String]) {
    let report: Report = cli::check(root, Stage::Commit, vec![adapter]);
    assert!(report.cannots().is_empty(), "{}", report.text());
    let mut places: Vec<String> = report
        .findings()
        .iter()
        .map(|f| house::reply::place_text(&f.place).to_string())
        .collect();
    places.sort();
    places.dedup();
    let mut want = want.to_vec();
    want.sort();
    assert_eq!(places, want, "{}", report.text());
    let changed = Command::new("git")
        .args(["diff", "--name-only"])
        .current_dir(root)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&changed.stdout),
        "",
        "a tracked file was written"
    );
}

const SWIFT_BAD: &str = "let value = 1;\n";

#[test]
fn swiftlint_lints_files_named_like_options() {
    if !installed("swiftlint", &["version"]) {
        return;
    }
    let staged = names("swift", &["--write-baseline=Sources/A"]);
    let root = repo("dash-swiftlint", "swift", &[], &staged, SWIFT_BAD);
    linted_as_files(&root, Box::new(SwiftLint), &staged);
}

#[test]
fn swift_format_lints_files_named_like_options() {
    if !installed("swift", &["format", "--version"]) {
        return;
    }
    let staged = names("swift", &[]);
    let root = repo("dash-swift-format", "swift", &[], &staged, SWIFT_BAD);
    linted_as_files(&root, Box::new(SwiftFormat), &staged);
}

#[test]
fn swiftformat_lints_files_named_like_options() {
    if !installed("swiftformat", &["--version"]) {
        return;
    }
    let staged = names("swift", &[]);
    let setup = [(".house.toml", "[tools]\ncommit = [\"swiftformat\"]\n")];
    let root = repo("dash-swiftformat", "swift", &setup, &staged, SWIFT_BAD);
    linted_as_files(&root, Box::new(Swiftformat), &staged);
}

#[test]
fn shellcheck_checks_files_named_like_options() {
    if !installed("shellcheck", &["--version"]) {
        return;
    }
    let staged = names("sh", &[]);
    let root = repo(
        "dash-shellcheck",
        "sh",
        &[],
        &staged,
        "#!/bin/sh\necho $1\n",
    );
    linted_as_files(&root, Box::new(Shellcheck), &staged);
}

#[test]
fn ruff_checks_files_named_like_options() {
    if !installed("ruff", &["--version"]) {
        return;
    }
    let staged = names("py", &["--output-file=Sources/A"]);
    let root = repo("dash-ruff", "py", &[], &staged, "import os\n");
    linted_as_files(&root, Box::new(Ruff), &staged);
}

#[test]
fn ruff_format_checks_files_named_like_options() {
    if !installed("ruff", &["--version"]) {
        return;
    }
    let staged = names("py", &["--output-file=Sources/A"]);
    let root = repo("dash-ruff-format", "py", &[], &staged, "x=1\n");
    linted_as_files(&root, Box::new(RuffFormat), &staged);
}

#[test]
fn semgrep_scans_files_named_like_options() {
    if !installed("semgrep", &["--version"]) {
        return;
    }
    let staged = names("py", &[]);
    let root = repo("dash-semgrep", "py", &[], &staged, "x = 1  # noqa\n");
    linted_as_files(&root, Box::new(Semgrep), &staged);
}

#[test]
fn r3_counts_files_named_like_options() {
    if !installed("python3", &["--version"]) {
        return;
    }
    let staged = names("py", &["-h", "--help"]);
    let long = "x = 1\n".repeat(401);
    let root = repo("dash-r3", "py", &[], &staged, &long);
    linted_as_files(&root, Box::new(R3), &staged);
}

/// A Rust crate in a folder named `-ws`, with one line unformatted and clippy findings.
fn dash_crate(name: &str) -> PathBuf {
    let root = common::repo(name);
    write(&root, ".gitignore", "target/\n");
    write(
        &root,
        "-ws/Cargo.toml",
        "[package]\nname = \"dashws\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
         license = \"MIT\"\npublish = false\n",
    );
    write(&root, "-ws/deny.toml", "[licenses]\nallow = [\"MIT\"]\n");
    write(
        &root,
        "-ws/src/main.rs",
        "fn main() {\n    let v = vec![1];\n    println!(\"{}\",   v.len() == 0);\n}\n",
    );
    let lock = Command::new("cargo")
        .args([
            "generate-lockfile",
            "--offline",
            "--manifest-path",
            "./-ws/Cargo.toml",
        ])
        .current_dir(&root)
        .status()
        .unwrap();
    assert!(lock.success());
    common::git(&root, &["add", "-A"]);
    root
}

fn rust_findings(root: &Path, adapter: Box<dyn Adapter>) -> Vec<String> {
    let report = cli::check(root, Stage::Commit, vec![adapter]);
    assert!(report.cannots().is_empty(), "{}", report.text());
    report
        .findings()
        .iter()
        .map(|f| house::reply::place_text(&f.place).to_string())
        .collect()
}

#[test]
fn cargo_fmt_checks_a_workspace_in_a_folder_named_like_an_option() {
    let root = dash_crate("dash-cargo-fmt");
    let found = rust_findings(&root, Box::new(CargoFmt));
    assert_eq!(found, ["-ws/src/main.rs"]);
}

#[test]
fn clippy_checks_a_workspace_in_a_folder_named_like_an_option() {
    let root = dash_crate("dash-clippy");
    let found = rust_findings(&root, Box::new(Clippy));
    assert!(!found.is_empty());
    assert!(found.iter().all(|f| f == "-ws/src/main.rs"), "{found:?}");
}

#[test]
fn cargo_deny_checks_a_workspace_in_a_folder_named_like_an_option() {
    if !installed("cargo-deny", &["--version"]) {
        return;
    }
    let root = dash_crate("dash-cargo-deny");
    assert!(rust_findings(&root, Box::new(CargoDeny)).is_empty());
}
