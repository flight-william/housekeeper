//! The house binary end to end, without needing any checking tool installed.

mod common;

use std::process::Command;

fn house(
    repo: &std::path::Path,
    log: &std::path::Path,
    path_env: Option<&str>,
) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_house"));
    cmd.args(["check", "--stage", "commit"])
        .current_dir(repo)
        .env("HOUSE_LOG_DIR", log);
    // A GIT_DIR from a hook must not leak into house's own git calls.
    cmd.env("GIT_DIR", "/nonexistent");
    if let Some(p) = path_env {
        cmd.env("PATH", p);
    }
    cmd.output().unwrap()
}

fn record_lines(log: &std::path::Path) -> Vec<serde_json::Value> {
    let mut lines = Vec::new();
    for entry in std::fs::read_dir(log).unwrap() {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        lines.extend(text.lines().map(|l| serde_json::from_str(l).unwrap()));
    }
    lines
}

#[test]
fn clean_repo_exits_zero_and_records_one_line() {
    let repo = common::repo("bin-clean");
    let log = common::fresh("bin-clean-log");
    let out = house(&repo, &log, None);
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.starts_with("house commit: nothing to fix ("), "{text}");
    let lines = record_lines(&log);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["stage"], "commit");
    assert_eq!(lines[0]["exit"], 0);
    assert_eq!(lines[0]["repo"], repo.display().to_string());
}

#[test]
fn missing_ruff_is_cannot_check_exit_two() {
    let repo = common::repo("bin-noruff");
    std::fs::write(repo.join("a.py"), "import os\n").unwrap();
    common::git(&repo, &["add", "a.py"]);
    let log = common::fresh("bin-noruff-log");
    let out = house(&repo, &log, Some("/usr/bin:/bin"));
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(2), "{text}");
    assert!(
        text.contains("R2 cannot check: ruff is not installed. fix: brew install ruff [ruff]\n"),
        "{text}"
    );
    assert!(
        text.contains(
            "R1 cannot check: ruff is not installed. fix: brew install ruff [ruff-format]\n"
        ),
        "{text}"
    );
    assert_eq!(record_lines(&log)[0]["exit"], 2);
}

#[test]
fn version_and_bad_usage() {
    let out = Command::new(env!("CARGO_BIN_EXE_house"))
        .arg("--version")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        format!("house {}\n", env!("CARGO_PKG_VERSION"))
    );
    let out = Command::new(env!("CARGO_BIN_EXE_house"))
        .arg("nope")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}
