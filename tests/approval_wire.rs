//! `house commit-msg` and the approval check on real staged changes: every changed
//! path counts (deletes, both ends of a rename, links), protected names count
//! anywhere and in any case, and only the message git keeps can approve.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

const HOUSE: &str = env!("CARGO_BIN_EXE_house");

const GOOD: &str =
    "review: demonstrate behavior\n\nWhy: review\nProof: isolated fixture\nUndo: remove fixture\n";

/// A repo with `.house.toml` and `ruff.toml` committed, so later changes to them
/// are the only staged changes.
fn repo(name: &str) -> PathBuf {
    let dir = common::repo(name);
    std::fs::write(
        dir.join(".house.toml"),
        "[tools]\ncommit = []\npush = []\n[commit]\nauthor = \"test\"\nmodels = [\"claude-opus-5-5\"]\n",
    )
    .unwrap();
    std::fs::write(dir.join("ruff.toml"), "line-length = 88\n").unwrap();
    common::git(&dir, &["add", "-A"]);
    common::git(&dir, &["commit", "-q", "-m", "fixture"]);
    dir
}

/// `house commit-msg MSG` with `text` as the message: (exit, stdout).
fn commit_msg(dir: &Path, text: &str) -> (i32, String) {
    std::fs::write(dir.join("MSG"), text).unwrap();
    let out = Command::new(HOUSE)
        .args(["commit-msg", "MSG"])
        .current_dir(dir)
        .env("HOUSE_LOG_DIR", common::fresh("approval-wire-log"))
        .env_remove("CLAUDECODE")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

fn needs_approval(dir: &Path, path: &str) {
    let (code, text) = commit_msg(dir, GOOD);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains(&format!(
            "R9 {path}: protected file changed without the owner's approval line"
        )),
        "{text}"
    );
}

#[test]
fn deleting_a_protected_file_needs_approval() {
    let dir = repo("approval-delete");
    common::git(&dir, &["rm", "-q", "ruff.toml"]);
    needs_approval(&dir, "ruff.toml");
}

#[test]
fn renaming_a_protected_file_away_needs_approval() {
    let dir = repo("approval-rename");
    common::git(&dir, &["mv", "ruff.toml", "ordinary.txt"]);
    needs_approval(&dir, "ruff.toml");
}

#[test]
fn replacing_a_protected_file_with_a_link_needs_approval() {
    let dir = repo("approval-link");
    std::fs::remove_file(dir.join("ruff.toml")).unwrap();
    std::fs::write(dir.join("ordinary.txt"), "line-length = 999\n").unwrap();
    std::os::unix::fs::symlink("ordinary.txt", dir.join("ruff.toml")).unwrap();
    common::git(&dir, &["add", "-A"]);
    needs_approval(&dir, "ruff.toml");
}

#[test]
fn globs_the_house_toml_lists_under_protected_extra_need_approval() {
    let dir = repo("approval-extra");
    let config = std::fs::read_to_string(dir.join(".house.toml")).unwrap();
    std::fs::write(
        dir.join(".house.toml"),
        format!("{config}[protected]\nextra = [\"policy/*.yml\"]\n"),
    )
    .unwrap();
    common::git(&dir, &["commit", "-q", "-am", "extra"]);
    std::fs::create_dir(dir.join("policy")).unwrap();
    std::fs::write(dir.join("policy/rules.yml"), "a: 1\n").unwrap();
    common::git(&dir, &["add", "-A"]);
    needs_approval(&dir, "policy/rules.yml");
}

/// Git drops everything from the scissors line on, so an approval
/// line written below it never reaches the commit.
#[test]
fn an_approval_line_below_the_scissors_line_does_not_approve() {
    let dir = repo("approval-scissors");
    std::fs::write(dir.join("ruff.toml"), "line-length = 999\n").unwrap();
    common::git(&dir, &["add", "ruff.toml"]);
    let (code, text) = commit_msg(
        &dir,
        &format!(
            "{GOOD}# ------------------------ >8 ------------------------\nOwner-approved: 2026-11-22 \"yes\"\n"
        ),
    );
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("R9 ruff.toml: protected file changed"),
        "{text}"
    );
    // Above the scissors line, the same approval counts.
    let (code, text) = commit_msg(&dir, &format!("{GOOD}Owner-approved: 2026-11-22 \"yes\"\n"));
    assert_eq!(code, 0, "{text}");
}
