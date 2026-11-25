//! Approval at merge, commit by commit as the commit-msg hook judges it: each commit
//! that changes a protected path carries its own `Owner-approved:` line. And the
//! committer is held to the base's `[commit] author` as the author is.

mod merge_support;

use house::approval;
use merge_support::{
    GOOD, R3_ONLY, ZERO, based, commit, findings, git, line, merge, repo, run, sha,
};
use std::process::Command;

fn approved(words: &str) -> String {
    format!("{GOOD}Owner-approved: 2026-11-25 \"{words}\"\n")
}

#[test]
fn one_commits_approval_does_not_cover_another() {
    let (dir, base) = based("ma-other", &[(".house.toml", R3_ONLY)]);
    commit(
        &dir,
        &[("ruff.toml", "line-length = 90\n")],
        &approved("ok"),
    );
    let second = commit(&dir, &[(".semgrepignore", "x/\n")], GOOD);
    let out = merge(&dir, &base, &second);
    assert_eq!(out.code, 1, "{}", out.out);
    assert!(
        out.out
            .contains(&format!("R9 .semgrepignore: commit {}", &second[..12])),
        "{}",
        out.out
    );
    assert!(!out.out.contains("R9 ruff.toml"), "{}", out.out);
    assert_eq!(findings(&out.out).len(), 1, "{}", out.out);
}

// A root commit's own paths are its whole tree.
#[test]
fn a_root_commit_adding_a_protected_file_needs_approval() {
    let dir = repo("ma-root");
    let first = commit(&dir, &[(".house.toml", R3_ONLY)], GOOD);
    let out = run(&dir, "merge", &[], &line(&first, ZERO), &[]);
    assert_eq!(out.code, 1, "{}{}", out.out, out.err);
    assert!(out.out.contains("R9 .house.toml: commit"), "{}", out.record);
}

#[test]
fn an_empty_commit_carrying_an_approval_approves_nothing() {
    let (dir, base) = based("ma-empty", &[(".house.toml", R3_ONLY)]);
    commit(&dir, &[(".semgrepignore", "x/\n")], GOOD);
    let local = commit(&dir, &[], &approved("yes"));
    let out = merge(&dir, &base, &local);
    assert_eq!(out.code, 1, "{}", out.out);
    assert!(out.out.contains("R9 .semgrepignore"), "{}", out.out);
}

// The net diff shows nothing, but the first commit lands on main with a merge.
#[test]
fn a_protected_change_put_back_still_needs_approval() {
    let (dir, base) = based("ma-revert", &[(".house.toml", R3_ONLY)]);
    commit(&dir, &[(".githooks/pre-push", "exit 0\n")], GOOD);
    std::fs::remove_dir_all(dir.join(".githooks")).unwrap();
    let local = commit(&dir, &[], GOOD);
    let out = merge(&dir, &base, &local);
    assert_eq!(out.code, 1, "{}", out.out);
    assert_eq!(
        out.out.matches("R9 .githooks/pre-push").count(),
        2,
        "{}",
        out.out
    );
}

fn symlinked(name: &str) -> (std::path::PathBuf, String) {
    let (dir, base) = based(name, &[(".house.toml", R3_ONLY)]);
    std::fs::create_dir_all(dir.join("build/cfg")).unwrap();
    std::fs::write(dir.join("build/cfg/config.toml"), "[build]\n").unwrap();
    std::os::unix::fs::symlink("build/cfg", dir.join(".cargo")).unwrap();
    (dir, base)
}

// A symlink named as a protected folder reaches the files in it.
#[test]
fn a_symlinked_protected_folder_is_protected() {
    let (dir, _) = symlinked("ma-link-unit");
    let protected: Vec<String> = approval::PROTECTED.iter().map(|g| g.to_string()).collect();
    let found = approval::check_in(&dir, &[".cargo".into()], &protected, "m");
    assert_eq!(found.len(), 1, "{found:?}");
    // A plain file of that name is not a folder of the glob, nor is any other link.
    std::os::unix::fs::symlink("build/cfg", dir.join("other")).unwrap();
    assert!(approval::check_in(&dir, &["other".into()], &protected, "m").is_empty());
    let (dir, base) = symlinked("ma-link-merge");
    let local = commit(&dir, &[], GOOD);
    let out = merge(&dir, &base, &local);
    assert_eq!(out.code, 1, "{}", out.out);
    assert!(out.out.contains("R9 .cargo: commit"), "{}", out.out);
}

#[test]
fn the_committer_is_held_to_the_base_author() {
    let rules = "[tools]\npush = [\"r3\"]\n[commit]\nauthor = \"test\"\n";
    let (dir, base) = based("ma-committer", &[(".house.toml", rules)]);
    std::fs::write(dir.join("a.py"), "x = 1\n").unwrap();
    git(&dir, &["add", "-A"]);
    let status = Command::new("git")
        .args(["commit", "-q", "--no-verify", "-m", GOOD])
        .current_dir(&dir)
        .env("GIT_COMMITTER_NAME", "Mallory")
        .status()
        .unwrap();
    assert!(status.success());
    let out = merge(&dir, &base, &sha(&dir, "HEAD"));
    assert_eq!(out.code, 1, "{}", out.out);
    assert!(
        out.out
            .contains("committer is not the installed author: Mallory"),
        "{}",
        out.out
    );
}

// A link that only an earlier commit holds is read from that commit's tree, not the
// disk: the commit that added it is judged though the tip has none.
#[test]
fn a_link_only_an_earlier_commit_holds_is_protected() {
    let (dir, base) = symlinked("ma-link-gone");
    let added = commit(&dir, &[], GOOD);
    std::fs::remove_file(dir.join(".cargo")).unwrap();
    let local = commit(&dir, &[], GOOD);
    let out = merge(&dir, &base, &local);
    assert_eq!(out.code, 1, "{}", out.out);
    let want = format!("R9 .cargo: commit {}", &added[..12]);
    assert!(out.out.contains(&want), "{}", out.out);
}
