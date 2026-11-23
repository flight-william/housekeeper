//! Each run's record names the repository it ran in, not only the folder: the
//! repository's root commit and its main checkout, so runs in two worktrees of one
//! repository are known to be one repository's runs.

mod common;

use std::path::Path;
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

fn check_records(dir: &Path, log: &Path) -> serde_json::Value {
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
    let entry = std::fs::read_dir(log).unwrap().next().unwrap().unwrap();
    let text = std::fs::read_to_string(entry.path()).unwrap();
    serde_json::from_str(text.lines().next().unwrap()).unwrap()
}

#[test]
fn a_run_in_a_worktree_records_the_oldest_root_commit_and_the_main_checkout() {
    // Two roots: the older one, then an unrelated newer history merged in as the
    // first parent, so the first-parent root is the newer one.
    let main = common::repo("record-repo-main");
    let old_root = commit_at(&main, "old.txt", "2020-01-01T00:00:00Z");
    common::git(&main, &["checkout", "-q", "--orphan", "other"]);
    common::git(&main, &["rm", "-q", "-r", "--cached", "."]);
    std::fs::remove_file(main.join("old.txt")).unwrap();
    let new_root = commit_at(&main, "new.txt", "2021-01-01T00:00:00Z");
    common::git(
        &main,
        &[
            "merge",
            "-q",
            "--allow-unrelated-histories",
            "-m",
            "merge",
            &old_root,
        ],
    );
    assert_ne!(old_root, new_root);
    let tree = common::fresh("record-repo-trees").join("feature");
    let tree_arg = tree.display().to_string();
    common::git(
        &main,
        &["worktree", "add", "-q", "-b", "feature", &tree_arg],
    );
    let tree = tree.canonicalize().unwrap();

    let line = check_records(&tree, &common::fresh("record-repo-log"));
    assert_eq!(line["repo"], tree.display().to_string());
    assert_eq!(line["root_commit"], old_root);
    assert_eq!(line["main_checkout"], main.display().to_string());
}

#[test]
fn a_repo_with_no_commit_records_no_root_commit() {
    let repo = common::repo("record-repo-empty");
    let line = check_records(&repo, &common::fresh("record-repo-empty-log"));
    assert!(line["root_commit"].is_null(), "{line}");
    assert_eq!(line["main_checkout"], repo.display().to_string());
}
