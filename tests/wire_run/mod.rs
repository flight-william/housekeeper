//! Shared by the wiring tests: the house binary run in a repo, and its run records.

use std::path::Path;
use std::process::Command as Proc;

pub const HOUSE: &str = env!("CARGO_BIN_EXE_house");

/// The house binary in `repo` with `args`, records in `log`, no Claude session,
/// and a hook's GIT_DIR that must not leak into house's own git calls.
pub fn house(repo: &Path, log: &Path, args: &[&str], env: &[(&str, &str)]) -> (i32, String) {
    let mut cmd = Proc::new(HOUSE);
    cmd.args(args)
        .current_dir(repo)
        .env("HOUSE_LOG_DIR", log)
        .env("GIT_DIR", "/nonexistent")
        .env_remove("CLAUDECODE");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

pub fn records(log: &Path) -> Vec<serde_json::Value> {
    let mut lines = Vec::new();
    for entry in std::fs::read_dir(log).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        lines.extend(text.lines().map(|l| serde_json::from_str(l).unwrap()));
    }
    lines
}
