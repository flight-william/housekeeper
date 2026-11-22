//! The tools read the working tree, so house checks only when the
//! working tree holds what is committed: a staged file with unstaged changes on
//! top is cannot-check, naming the file.

mod common;

use std::process::Command;

#[test]
fn a_staged_file_with_unstaged_changes_cannot_check_and_is_named() {
    let dir = common::repo("partial-stage");
    std::fs::write(dir.join(".house.toml"), "[tools]\ncommit = []\n").unwrap();
    common::git(&dir, &["add", "-A"]);
    common::git(&dir, &["commit", "-q", "-m", "fixture"]);
    std::fs::write(dir.join("bad.py"), "x=1\n").unwrap();
    std::fs::write(dir.join("good.py"), "y = 1\n").unwrap();
    common::git(&dir, &["add", "bad.py", "good.py"]);
    // Only the working copy is fixed; the staged bad content would be committed.
    std::fs::write(dir.join("bad.py"), "x = 1\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "commit"])
        .current_dir(&dir)
        .env("HOUSE_LOG_DIR", common::fresh("partial-stage-log"))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(2), "{text}");
    assert!(text.contains("bad.py has unstaged changes"), "{text}");
    assert!(!text.contains("good.py"), "{text}");
    assert!(text.contains("stage or stash the rest"), "{text}");
}
