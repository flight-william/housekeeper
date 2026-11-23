//! Shared by the `house review` tests: the binary over a record folder, the made-up
//! records in tests/fixtures/review, and reading the report's sections.

use crate::common;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/review")
}

/// `house review <args>` over the records in `log`, from `cwd`: (exit, stdout).
pub fn review_in(cwd: &Path, log: &Path, args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_house"))
        .arg("review")
        .args(args)
        .current_dir(cwd)
        .env("HOUSE_LOG_DIR", log)
        .env_remove("CLAUDECODE")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

pub fn review(args: &[&str]) -> (i32, String) {
    review_in(&common::fresh("review-cwd"), &fixtures(), args)
}

pub const NOVEMBER: [&str; 4] = ["--from", "2026-11-01", "--to", "2026-11-30"];

/// A month's record file holding `lines`, in a fresh record folder.
pub fn folder_with(name: &str, lines: &[&str]) -> PathBuf {
    let log = common::fresh(name);
    std::fs::write(log.join("2026-11.jsonl"), lines.join("\n") + "\n").unwrap();
    log
}

/// The indented lines under `heading`.
pub fn section<'a>(text: &'a str, heading: &str) -> Vec<&'a str> {
    text.lines()
        .skip_while(|l| *l != heading)
        .skip(1)
        .take_while(|l| l.starts_with("  "))
        .collect()
}
