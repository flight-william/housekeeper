//! Test helpers: fresh temp folders (never removed) and temp git repos.
//! A test file that needs only `fresh` includes `common/temp.rs` as `common`.

use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "temp.rs"]
mod temp;

pub fn fresh(name: &str) -> PathBuf {
    temp::fresh(name)
}

pub fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_WORK_TREE")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// A fresh git repo with a local identity.
pub fn repo(name: &str) -> PathBuf {
    let dir = fresh(name);
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.email", "test@example.com"]);
    git(&dir, &["config", "user.name", "test"]);
    dir
}
