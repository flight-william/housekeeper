//! Each run's record names the repository it ran in, not only the folder: the root
//! commit of the repository's default branch and its main checkout, so runs in any
//! worktree of one repository, whatever its HEAD, are known to be one repository's.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

/// A commit of `file` in `repo` dated `date`, its hash.
fn commit_at(repo: &Path, file: &str, date: &str) -> String {
    std::fs::write(repo.join(file), "x\n").unwrap();
    common::git(repo, &["add", file]);
    let status = Command::new("git")
        .args(["commit", "-q", "-m", file])
        .current_dir(repo)
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .status()
        .unwrap();
    assert!(status.success());
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// A fresh repo whose first branch is `branch`, with one commit: (repo, commit).
fn repo_on(name: &str, branch: &str) -> (PathBuf, String) {
    let repo = common::repo(name);
    common::git(&repo, &["checkout", "-q", "-b", branch]);
    let root = commit_at(&repo, "a.txt", "2020-01-01T00:00:00Z");
    (repo, root)
}

/// `house check --stage commit` in `dir`, its record appended to `log`; exit 0.
fn check(dir: &Path, log: &Path) {
    let out = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "commit"])
        .current_dir(dir)
        .env("HOUSE_LOG_DIR", log)
        .env_remove("GIT_DIR")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

fn records(log: &Path) -> Vec<serde_json::Value> {
    let mut lines = Vec::new();
    for entry in std::fs::read_dir(log).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        lines.extend(text.lines().map(|l| serde_json::from_str(l).unwrap()));
    }
    lines
}

/// The record of one check in `dir`.
fn check_record(dir: &Path, name: &str) -> serde_json::Value {
    let log = common::fresh(name);
    check(dir, &log);
    records(&log).remove(0)
}

/// The number of runs `house review --repo <repo>` counts over `log`.
fn review_runs(log: &Path, repo: &Path) -> u64 {
    let out = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["review", "--json", "--repo", &repo.display().to_string()])
        .current_dir(repo)
        .env("HOUSE_LOG_DIR", log)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    v["runs"].as_u64().unwrap()
}

#[test]
fn a_run_in_a_worktree_records_the_oldest_root_of_main_and_the_main_checkout() {
    // main has two roots: its own newer one as first parent, and an older,
    // unrelated history merged in.
    let (main, old_root) = repo_on("record-repo-main", "old-line");
    common::git(&main, &["checkout", "-q", "--orphan", "main"]);
    common::git(&main, &["rm", "-q", "-r", "--cached", "."]);
    std::fs::remove_file(main.join("a.txt")).unwrap();
    let new_root = commit_at(&main, "new.txt", "2021-01-01T00:00:00Z");
    common::git(
        &main,
        &[
            "merge",
            "-q",
            "--allow-unrelated-histories",
            "-m",
            "merge",
            "old-line",
        ],
    );
    assert_ne!(old_root, new_root);
    let tree = common::fresh("record-repo-trees").join("feature");
    let tree_arg = tree.display().to_string();
    common::git(
        &main,
        &["worktree", "add", "-q", "-b", "feature", &tree_arg, "main"],
    );
    let tree = tree.canonicalize().unwrap();

    let line = check_record(&tree, "record-repo-log");
    assert_eq!(line["repo"], tree.display().to_string());
    assert_eq!(line["root_commit"], old_root);
    assert_eq!(line["main_checkout"], main.display().to_string());
}

#[test]
fn a_worktree_on_an_orphan_branch_is_counted_with_its_repo() {
    let (main, root) = repo_on("record-repo-orphan", "main");
    let tree = common::fresh("record-repo-orphan-trees").join("orphan");
    let tree_arg = tree.display().to_string();
    common::git(
        &main,
        &[
            "worktree", "add", "-q", "--orphan", "-b", "orphan", &tree_arg,
        ],
    );
    let tree = tree.canonicalize().unwrap();
    let own_root = commit_at(&tree, "b.txt", "2021-01-01T00:00:00Z");
    assert_ne!(own_root, root);

    let log = common::fresh("record-repo-orphan-log");
    check(&main, &log);
    check(&tree, &log);
    let lines = records(&log);
    assert_eq!(lines[0]["root_commit"], root);
    assert_eq!(lines[1]["root_commit"], root);
    assert_eq!(review_runs(&log, &main), 2);
    assert_eq!(review_runs(&log, &tree), 2);
}

#[test]
fn origin_head_names_the_default_branch_before_a_local_main() {
    let (source, root) = repo_on("record-repo-trunk", "trunk");
    let clone = common::fresh("record-repo-trunk-clone").join("clone");
    let url = format!("file://{}", source.display());
    let clone_arg = clone.display().to_string();
    common::git(&source, &["clone", "-q", &url, &clone_arg]);
    common::git(&clone, &["config", "user.email", "test@example.com"]);
    common::git(&clone, &["config", "user.name", "test"]);
    // A local main with a root of its own: not the default branch.
    common::git(&clone, &["checkout", "-q", "--orphan", "main"]);
    let main_root = commit_at(&clone, "b.txt", "2021-01-01T00:00:00Z");
    assert_ne!(main_root, root);

    let line = check_record(&clone, "record-repo-trunk-log");
    assert_eq!(line["root_commit"], root);
}

#[test]
fn a_shallow_clone_records_no_root_commit() {
    let (source, _) = repo_on("record-repo-deep", "main");
    commit_at(&source, "b.txt", "2021-01-01T00:00:00Z");
    commit_at(&source, "c.txt", "2022-01-01T00:00:00Z");
    let shallow = common::fresh("record-repo-shallow").join("shallow");
    let url = format!("file://{}", source.display());
    let shallow_arg = shallow.display().to_string();
    common::git(
        &source,
        &["clone", "-q", "--depth", "1", &url, &shallow_arg],
    );

    let line = check_record(&shallow, "record-repo-shallow-log");
    assert!(line["root_commit"].is_null(), "{line}");
}

#[test]
fn a_repo_with_no_default_branch_records_no_root_commit() {
    let (repo, _) = repo_on("record-repo-work", "work");
    let line = check_record(&repo, "record-repo-work-log");
    assert!(line["root_commit"].is_null(), "{line}");
}

#[test]
fn a_repo_with_no_commit_records_no_root_commit() {
    let repo = common::repo("record-repo-empty");
    let line = check_record(&repo, "record-repo-empty-log");
    assert!(line["root_commit"].is_null(), "{line}");
    assert_eq!(line["main_checkout"], repo.display().to_string());
}
