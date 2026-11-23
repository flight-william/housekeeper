//! `house review --repo` matches a repository, not a folder: runs in two worktrees
//! of one repository are its runs. The records in tests/fixtures/review_repo are made
//! up: shop's runs in two worktrees and in its main checkout (an older record that
//! names only the folder), and blog's in its main checkout and one worktree.

mod common;
mod review_support;

use review_support::{NOVEMBER, folder_with, review, review_in, section};
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/review_repo")
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

/// A run record of `folder` on 2026-11-`day`, with `root` as its root commit, or
/// none (a record from before 0.3.1, which named only the folder).
fn run(day: u32, folder: &Path, root: Option<&str>) -> String {
    let id = match root {
        Some(root) => format!(r#","root_commit":"{root}","main_checkout":null"#),
        None => String::new(),
    };
    format!(
        r#"{{"start":"2026-11-{day:02}T09:00:00Z","repo":"{}"{id},"stage":"commit","tools":[{{"name":"ruff","seconds":1.0,"findings":{{}},"excused":{{}}}}],"findings":[],"excused":0}}"#,
        folder.display()
    )
}

fn root_of(repo: &Path) -> String {
    let out = Command::new("git")
        .args(["rev-list", "--max-parents=0", "HEAD"])
        .current_dir(repo)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// A repo with one commit and a worktree of it: (main checkout, worktree).
fn repo_with_worktree(name: &str) -> (PathBuf, PathBuf) {
    let main = common::repo(name);
    // main: the default branch whose root names the repository.
    common::git(&main, &["checkout", "-q", "-b", "main"]);
    std::fs::write(main.join("a.txt"), "a\n").unwrap();
    common::git(&main, &["add", "a.txt"]);
    common::git(&main, &["commit", "-q", "-m", "a"]);
    let tree = common::fresh(&format!("{name}-trees")).join("feature");
    let tree_arg = tree.display().to_string();
    common::git(
        &main,
        &["worktree", "add", "-q", "-b", "feature", &tree_arg],
    );
    (main, tree.canonicalize().unwrap())
}

#[test]
fn a_path_counts_the_runs_of_every_worktree_of_its_repository() {
    let (main, tree) = repo_with_worktree("review-repo-path");
    let root = root_of(&main);
    let other = "c".repeat(40);
    let log = folder_with(
        "review-repo-path-log",
        &[
            &run(2, &main, Some(&root)),
            &run(3, &tree, Some(&root)),
            // A worktree since removed: still the repository's, by its root.
            &run(4, Path::new("/work/gone"), Some(&root)),
            // An unrelated repository with a different root, in the same folder.
            &run(5, &tree, Some(&other)),
            // An older record: no root commit, matched by its folder.
            &run(6, &tree, None),
            &run(7, &main, None),
        ],
    );
    let (code, text) = review_in(&tree, &log, &[&NOVEMBER[..], &["--repo", "."]].concat());
    assert_eq!(code, 0, "{text}");
    assert_eq!(
        first_line(&text),
        "house review: 2026-11-01 to 2026-11-30, 4 runs: 4 commit",
        "{text}"
    );
    // From the main checkout: the same root, and the older record of its folder.
    let main_arg = main.display().to_string();
    let (_, text) = review_in(
        &tree,
        &log,
        &[&NOVEMBER[..], &["--repo", &main_arg]].concat(),
    );
    assert_eq!(
        first_line(&text),
        "house review: 2026-11-01 to 2026-11-30, 4 runs: 4 commit",
        "{text}"
    );
}

#[test]
fn a_name_matches_a_records_folder_or_its_main_checkout() {
    let cwd = common::fresh("review-repo-name");
    let (code, text) = review_in(
        &cwd,
        &repo_fixtures(),
        &[&NOVEMBER[..], &["--repo", "shop"]].concat(),
    );
    assert_eq!(code, 0, "{text}");
    // feature-a and feature-b by their main checkout, /work/shop by its folder.
    assert_eq!(
        first_line(&text),
        "house review: 2026-11-01 to 2026-11-30, 3 runs: 2 commit, 1 push",
        "{text}"
    );
    // A worktree's own folder name is a name too.
    let (_, text) = review_in(
        &cwd,
        &repo_fixtures(),
        &[&NOVEMBER[..], &["--repo", "draft"]].concat(),
    );
    assert_eq!(
        first_line(&text),
        "house review: 2026-11-01 to 2026-11-30, 1 run: 1 commit",
        "{text}"
    );
}

fn second_line(text: &str) -> &str {
    text.lines().nth(1).unwrap_or("")
}

#[test]
fn the_header_names_the_repo_and_the_folders_its_runs_came_from() {
    let cwd = common::fresh("review-repo-header");
    let (_, text) = review_in(
        &cwd,
        &repo_fixtures(),
        &[&NOVEMBER[..], &["--repo", "shop"]].concat(),
    );
    assert_eq!(
        second_line(&text),
        "repo: shop, root commit aaaaaaaaaaaa, runs from 3 folders",
        "{text}"
    );
    // A path names its repository's main checkout and its root commit.
    let (main, tree) = repo_with_worktree("review-repo-header-path");
    let root = root_of(&main);
    let log = folder_with(
        "review-repo-header-log",
        &[
            &run(2, &main, Some(&root)),
            &run(3, &tree, Some(&root)),
            &run(4, &tree, None),
        ],
    );
    let (_, text) = review_in(&tree, &log, &[&NOVEMBER[..], &["--repo", "."]].concat());
    let name = main.file_name().unwrap().to_str().unwrap();
    assert_eq!(
        second_line(&text),
        format!(
            "repo: {name}, root commit {}, runs from 2 folders",
            &root[..12]
        ),
        "{text}"
    );
    let (_, json) = review_in(
        &tree,
        &log,
        &[&NOVEMBER[..], &["--repo", ".", "--json"]].concat(),
    );
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        v["repo"],
        serde_json::json!({
            "name": name,
            "root_commit": root,
            "folders": [main.display().to_string(), tree.display().to_string()],
        })
    );
    // No --repo: no repo line, and no repo in the JSON.
    let (_, text) = review_in(&cwd, &repo_fixtures(), &NOVEMBER);
    assert_eq!(second_line(&text), "per rule:", "{text}");
    let (_, json) = review_in(
        &cwd,
        &repo_fixtures(),
        &[&NOVEMBER[..], &["--json"]].concat(),
    );
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(v["repo"].is_null(), "{json}");
}

#[test]
fn a_name_that_matches_two_repositories_says_so() {
    let log = folder_with(
        "review-repo-two",
        &[
            &run(2, Path::new("/work/shop"), Some(&"a".repeat(40))),
            &run(3, Path::new("/old/shop"), Some(&"d".repeat(40))),
        ],
    );
    let (_, text) = review_in(&log, &log, &[&NOVEMBER[..], &["--repo", "shop"]].concat());
    assert_eq!(
        second_line(&text),
        "repo: shop, runs from 2 folders, 2 root commits: the name matches more than one repository; give a path to pick one",
        "{text}"
    );
}

#[test]
fn each_tool_counts_the_runs_of_every_folder_of_the_repository() {
    let cwd = common::fresh("review-repo-tools");
    let (_, text) = review_in(
        &cwd,
        &repo_fixtures(),
        &[&NOVEMBER[..], &["--repo", "shop"]].concat(),
    );
    assert_eq!(
        section(&text, "per tool:"),
        ["  ruff: 3 runs, 0 findings, 0 excused, 0 cannot check, 3.0 s total, 1.0 s median"],
        "{text}"
    );
}

#[test]
fn records_from_before_0_3_1_are_matched_and_named_by_their_folder() {
    // tests/fixtures/review: records with no root commit, three in /work/shop.
    let (_, text) = review(&[&NOVEMBER[..], &["--repo", "shop"]].concat());
    assert_eq!(
        text.lines().take(2).collect::<Vec<_>>(),
        [
            "house review: 2026-11-01 to 2026-11-30, 3 runs: 2 commit, 1 push",
            "repo: shop, runs from 1 folder",
        ],
        "{text}"
    );
}
