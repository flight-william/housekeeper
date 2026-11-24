//! Helpers for `house check --stage merge`: repos with a base commit, commits made
//! without the hooks, and the binary run as a runner runs it, the ref line on stdin.

#[path = "../common/mod.rs"]
mod common;

pub use common::{fresh, git, repo};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const ZERO: &str = "0000000000000000000000000000000000000000";
pub const R3_ONLY: &str = "[tools]\npush = [\"r3\"]\n";
/// A message every R17 check passes.
pub const GOOD: &str = "app: add the file\n\nWhy: w.\nProof: p.\nUndo: u.\n";

pub fn sha(dir: &Path, rev: &str) -> String {
    let out = Command::new("git")
        .args(["rev-parse", rev])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// Write `files`, stage everything and commit with `message`, no hooks.
pub fn commit(dir: &Path, files: &[(&str, &str)], message: &str) -> String {
    for (path, text) in files {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(
        dir,
        &[
            "commit",
            "-q",
            "--no-verify",
            "--allow-empty",
            "--cleanup=verbatim",
            "-m",
            message,
        ],
    );
    sha(dir, "HEAD")
}

/// A repo whose base commit holds `base_files` and a README; returns it and the base.
pub fn based(name: &str, base_files: &[(&str, &str)]) -> (PathBuf, String) {
    let dir = repo(name);
    let mut files = vec![("README.md", "base\n")];
    files.extend_from_slice(base_files);
    let base = commit(&dir, &files, GOOD);
    (dir, base)
}

/// The ref line a runner feeds: the branch at `local`, the base as the remote.
pub fn line(local: &str, remote: &str) -> String {
    format!("refs/heads/feature {local} refs/heads/feature {remote}\n")
}

/// What one run gave.
pub struct Out {
    pub code: i32,
    pub out: String,
    pub err: String,
    /// The run record's text.
    pub record: String,
}

/// `house check --stage <stage> <args>`, `stdin` piped, `env` added after GIT_DIR
/// is pointed nowhere and the Actions variables and agent markers are removed.
pub fn run(dir: &Path, stage: &str, args: &[&str], stdin: &str, env: &[(&str, &str)]) -> Out {
    use std::io::Write;
    let log = fresh("merge-log");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_house"));
    cmd.args(["check", "--stage", stage])
        .args(args)
        .current_dir(dir)
        .env("HOUSE_LOG_DIR", &log)
        .env("GIT_DIR", "/nonexistent")
        .env_remove("CLAUDECODE")
        .env_remove("GITHUB_ACTIONS")
        .env_remove("RUNNER_ENVIRONMENT")
        .env_remove("GITHUB_EVENT_PATH")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in env {
        cmd.env(key, value);
    }
    let mut child = cmd.spawn().unwrap();
    let mut pipe = child.stdin.take().unwrap();
    pipe.write_all(stdin.as_bytes()).unwrap();
    drop(pipe);
    let out = child.wait_with_output().unwrap();
    let record = std::fs::read_dir(&log)
        .map(|dir| {
            dir.filter_map(Result::ok)
                .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
                .collect()
        })
        .unwrap_or_default();
    Out {
        code: out.status.code().unwrap_or(-1),
        out: String::from_utf8_lossy(&out.stdout).into_owned(),
        err: String::from_utf8_lossy(&out.stderr).into_owned(),
        record,
    }
}

/// The runner's call: `--stage merge --config-from <base>`, the branch at `local`.
pub fn merge(dir: &Path, base: &str, local: &str) -> Out {
    run(
        dir,
        "merge",
        &["--config-from", base],
        &line(local, base),
        &[],
    )
}

/// The findings' lines of a reply, the header dropped.
pub fn findings(out: &str) -> Vec<&str> {
    out.lines().skip(1).collect()
}
