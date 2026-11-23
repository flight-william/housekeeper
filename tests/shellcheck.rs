//! ShellCheck follows the files a script sources (`--external-sources`), looked up
//! from the script's own folder (`--source-path=SCRIPTDIR`), so a verdict does not
//! depend on which other files the stage hands it. Run with ShellCheck 0.11.0.

mod common;

use common::{git, repo};
use house::adapter::Stage;
use house::adapters::shellcheck::Shellcheck;
use house::cli;
use std::path::Path;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn installed() -> bool {
    let found = std::process::Command::new("shellcheck")
        .arg("--version")
        .output()
        .is_ok();
    if !found {
        eprintln!("note: shellcheck is not installed; skipping the run on a repo");
    }
    found
}

/// Only main.sh is staged; the lib.sh it sources sits beside it, in a folder
/// that is not the repo root. Neither SC1091 (not following) nor SC2154 (a
/// variable lib.sh assigns) is reported, and a source the repo lacks still is.
#[test]
fn a_sourced_file_beside_the_script_is_followed() {
    if !installed() {
        return;
    }
    let root = repo("shellcheck-source");
    write(
        &root,
        "scripts/lib.sh",
        "# shellcheck shell=sh\ngreeting=hi\n",
    );
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "base"]);
    let main = "#!/bin/sh\n. ./lib.sh\necho \"$greeting\"\n. ./missing.sh\n";
    write(&root, "scripts/main.sh", main);
    git(&root, &["add", "-A"]);
    let report = cli::check(&root, Stage::Commit, vec![Box::new(Shellcheck)]);
    assert!(report.cannots().is_empty(), "{}", report.text());
    let found: Vec<(Option<u32>, &str)> = report
        .findings()
        .iter()
        .map(|f| (f.line, f.tool_rule.as_str()))
        .collect();
    assert_eq!(found, [(Some(4), "shellcheck SC1091")], "{}", report.text());
}
