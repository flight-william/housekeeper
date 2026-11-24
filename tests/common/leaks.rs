//! Helpers for runs of the real gitleaks: whether it is installed, a synthetic
//! token, and `house check --stage push` on a new branch's tip.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn installed() -> bool {
    let found = Command::new("gitleaks").arg("version").output().is_ok();
    if !found {
        eprintln!("note: gitleaks is not installed; skipping the run on a repo");
    }
    found
}

/// A ghp_-shaped line. SYNTHETIC, NOT A CREDENTIAL: generated at run time from a fixed
/// formula, never issued by anyone.
pub fn token_line() -> String {
    let alphabet: Vec<char> = ('a'..='z').chain('A'..='Z').chain('0'..='9').collect();
    let body: String = (0..36).map(|i| alphabet[(i * 23 + 11) % 62]).collect();
    format!("token = \"ghp_{body}\"\n")
}

/// `house check --stage push` of `tip` as a new branch, logging to `log`: the exit and
/// the reply.
pub fn pushed(root: &Path, tip: &str, log: &Path) -> (Option<i32>, String) {
    let zero = "0".repeat(40);
    let mut child = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "push"])
        .current_dir(root)
        .env("HOUSE_LOG_DIR", log)
        .env_remove("CLAUDECODE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let line = format!("refs/heads/feature {tip} refs/heads/feature {zero}\n");
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(line.as_bytes()).unwrap();
    drop(stdin);
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    (out.status.code(), text)
}
