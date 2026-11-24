//! A secret in a file `.gitattributes` keeps out of git's diffs (`-diff`, `binary`, or
//! a diff driver whose textconv prints something else) is found at every stage:
//! gitleaks reads `git diff` and `git log -p`, which print "Binary files differ" for
//! such a file, so house runs it with no attributes from the tree.

mod common;
#[path = "common/leaks.rs"]
mod leaks;

use common::{git, repo};
use house::adapter::Stage;
use house::adapters::gitleaks::Gitleaks;
use house::cli;
use std::path::{Path, PathBuf};

const CONFIG: &str = "[tools]\ncommit = [\"gitleaks\"]\npush = [\"gitleaks\"]\n";

/// The ways a tree's attributes hide a file's text from a diff.
const HIDDEN: [&str; 3] = ["-diff", "binary", "diff=blank"];

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn places(report: &house::reply::Report) -> Vec<String> {
    assert!(report.cannots().is_empty(), "{}", report.text());
    report
        .findings()
        .iter()
        .map(|f| house::reply::place_text(&f.place).to_string())
        .collect()
}

/// A repo whose config is committed, then src/creds.txt holding a token staged with a
/// `.gitattributes` line giving it `attribute`. Gives the repo and its first commit.
fn hidden(name: &str, attribute: &str) -> (PathBuf, String) {
    let root = repo(name);
    // A diff driver that prints nothing for the file.
    git(&root, &["config", "diff.blank.textconv", "true"]);
    write(&root, ".house.toml", CONFIG);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "config"]);
    let base = rev(&root);
    write(
        &root,
        ".gitattributes",
        &format!("src/creds.txt {attribute}\n"),
    );
    write(&root, "src/creds.txt", &leaks::token_line());
    git(&root, &["add", "-A"]);
    (root, base)
}

fn rev(root: &Path) -> String {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn a_secret_in_a_file_kept_out_of_diffs_is_found_at_commit() {
    if !leaks::installed() {
        return;
    }
    for (i, attribute) in HIDDEN.iter().enumerate() {
        let (root, _) = hidden(&format!("attr-commit-{i}"), attribute);
        let report = cli::check(&root, Stage::Commit, vec![Box::new(Gitleaks)]);
        assert_eq!(places(&report), ["src/creds.txt"], "{attribute}");
    }
}

#[test]
fn a_secret_in_a_file_kept_out_of_diffs_is_found_by_all() {
    if !leaks::installed() {
        return;
    }
    for (i, attribute) in HIDDEN.iter().enumerate() {
        let (root, _) = hidden(&format!("attr-all-{i}"), attribute);
        git(&root, &["commit", "-qm", "creds"]);
        let report = cli::check_all(&root, vec![Box::new(Gitleaks)]);
        assert_eq!(places(&report), ["src/creds.txt"], "{attribute}");
    }
}

#[test]
fn a_secret_in_a_file_kept_out_of_diffs_is_found_at_push() {
    if !leaks::installed() {
        return;
    }
    for (i, attribute) in HIDDEN.iter().enumerate() {
        let (root, base) = hidden(&format!("attr-push-{i}"), attribute);
        git(&root, &["commit", "-qm", "creds"]);
        git(&root, &["update-ref", "refs/remotes/origin/main", &base]);
        let log = common::fresh(&format!("attr-push-{i}-log"));
        let (code, text) = leaks::pushed(&root, &rev(&root), &log);
        assert_eq!(code, Some(1), "{attribute}: {text}");
        assert!(text.contains("R2 src/creds.txt:1"), "{attribute}: {text}");
    }
}

/// A SHA-256 repo has its own empty tree; git refuses SHA-1's there.
#[test]
fn a_secret_kept_out_of_diffs_is_found_in_a_sha256_repo() {
    if !leaks::installed() {
        return;
    }
    let root = common::fresh("attr-sha256");
    git(&root, &["init", "-q", "--object-format=sha256"]);
    write(&root, ".house.toml", CONFIG);
    write(&root, ".gitattributes", "src/creds.txt -diff\n");
    write(&root, "src/creds.txt", &leaks::token_line());
    git(&root, &["add", "-A"]);
    let report = cli::check(&root, Stage::Commit, vec![Box::new(Gitleaks)]);
    assert_eq!(places(&report), ["src/creds.txt"], "{}", report.text());
}
