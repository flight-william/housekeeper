//! The test step at push runs whenever the push changes anything: a push that only
//! deletes a module leaves no file for the tools, but a suite that imports the module
//! must still run, and fail. A push with no change at all still skips the tests.

mod common;

use common::{git, repo};
use std::path::Path;
use std::process::{Command, Stdio};

const CONFIG: &str = "[tools]\npush = []\n\n[paths]\nexclude = [\"fixtures/\"]\n\n[tests]\ncommit = [[\"python3\", \"-c\", \"import app\"]]\n";

fn head(dir: &Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn commit(dir: &Path, message: &str) -> String {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--no-verify", "-m", message]);
    head(dir)
}

/// The binary at push, judged by `base`'s rules: (exit, text, the record's tools).
fn push(dir: &Path, base: &str, local: &str) -> (i32, String, serde_json::Value) {
    use std::io::Write;
    let log = common::fresh("tests-on-delete-log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "push", "--config-from", base])
        .current_dir(dir)
        .env("HOUSE_LOG_DIR", &log)
        .env_remove("CLAUDECODE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let line = format!("refs/heads/f {local} refs/heads/f {base}\n");
    let mut pipe = child.stdin.take().unwrap();
    pipe.write_all(line.as_bytes()).unwrap();
    drop(pipe);
    let out = child.wait_with_output().unwrap();
    let entry = std::fs::read_dir(&log).unwrap().next().unwrap().unwrap();
    let record = std::fs::read_to_string(entry.path()).unwrap();
    let last: serde_json::Value = serde_json::from_str(record.lines().last().unwrap()).unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        last["tools"].clone(),
    )
}

/// A repo with the rules, app.py and a fixture; the base commit's id.
fn base_repo(name: &str) -> (std::path::PathBuf, String) {
    let dir = repo(name);
    std::fs::write(dir.join(".house.toml"), CONFIG).unwrap();
    std::fs::write(dir.join("app.py"), "X = 1\n").unwrap();
    std::fs::create_dir_all(dir.join("fixtures")).unwrap();
    std::fs::write(dir.join("fixtures/sample.txt"), "a\n").unwrap();
    let base = commit(&dir, "base");
    (dir, base)
}

fn tests_skipped(tools: &serde_json::Value) -> bool {
    tools
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"] == "tests" && t["skipped"] == true)
}

#[test]
fn a_push_that_only_deletes_a_module_runs_the_tests() {
    let (dir, base) = base_repo("tests-on-delete");
    std::fs::remove_file(dir.join("app.py")).unwrap();
    let local = commit(&dir, "delete app.py, nothing else");
    let (code, text, tools) = push(&dir, &base, &local);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("R7"), "{text}");
    assert!(text.contains("python3 -c import app"), "{text}");
    assert!(!tests_skipped(&tools), "{tools}");
}

#[test]
fn a_push_that_only_changes_excluded_files_runs_the_tests() {
    let (dir, base) = base_repo("tests-on-excluded");
    std::fs::write(dir.join("fixtures/sample.txt"), "b\n").unwrap();
    let local = commit(&dir, "fixture only");
    let (code, text, tools) = push(&dir, &base, &local);
    assert_eq!(code, 0, "app.py is there, the suite passes: {text}");
    assert!(!tests_skipped(&tools), "{tools}");
}

#[test]
fn a_push_with_no_change_still_skips_the_tests() {
    let (dir, base) = base_repo("tests-on-nothing");
    git(
        &dir,
        &[
            "commit",
            "-q",
            "--no-verify",
            "--allow-empty",
            "-m",
            "empty",
        ],
    );
    let local = head(&dir);
    let (code, text, tools) = push(&dir, &base, &local);
    assert_eq!(code, 0, "{text}");
    assert!(tests_skipped(&tools), "{tools}");
}
