//! `house review` on hostile record folders: what is not a regular month file is
//! skipped and counted, never opened in a way that can block or follow a link.

#[path = "common/temp.rs"]
mod common;

use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// The longest a review may take here. It is a guard against a hang, not a
/// timing: a passing run is done in well under a second and never waits on it.
const HANG_GUARD: Duration = Duration::from_secs(20);

/// `house review --from 2026-11-01 --to 2026-11-30` over `log`: (exit, stdout), or
/// `None` when it was still running at the guard and was killed.
fn review_guarded(log: &Path, extra: &[&str]) -> Option<(i32, String)> {
    let child = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["review", "--from", "2026-11-01", "--to", "2026-11-30"])
        .args(extra)
        .current_dir(log)
        .env("HOUSE_LOG_DIR", log)
        .env_remove("CLAUDECODE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id().to_string();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || tx.send(child.wait_with_output().unwrap()));
    match rx.recv_timeout(HANG_GUARD) {
        Ok(out) => Some((
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        )),
        Err(_) => {
            Command::new("kill").args(["-9", &pid]).status().unwrap();
            None
        }
    }
}

const RUN: &str = r#"{"start":"2026-11-02T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{"name":"ruff","seconds":1.0,"findings":{},"excused":{}}],"findings":[],"excused":0}"#;

fn mkfifo(path: &Path) {
    let status = Command::new("mkfifo").arg(path).status().unwrap();
    assert!(status.success(), "mkfifo {}", path.display());
}

#[test]
fn a_fifo_named_like_a_month_is_skipped_and_counted_never_opened_to_block() {
    let log = common::fresh("review-fifo");
    mkfifo(&log.join("2026-11.jsonl"));
    std::fs::write(log.join("2026-10.jsonl"), format!("{RUN}\n")).unwrap();
    let Some((code, text)) = review_guarded(&log, &[]) else {
        panic!("house review blocked on a FIFO");
    };
    assert_eq!(code, 0, "{text}");
    assert!(
        text.contains("\n1 record file skipped: not a regular file\n"),
        "{text}"
    );
}

#[test]
fn links_and_folders_named_like_a_month_are_skipped_and_counted() {
    let log = common::fresh("review-links");
    let outside = common::fresh("review-links-outside");
    mkfifo(&outside.join("pipe"));
    std::fs::write(outside.join("records.jsonl"), format!("{RUN}\n")).unwrap();
    // A link to a FIFO, a link to a real record file, a dangling link, a folder.
    symlink(outside.join("pipe"), log.join("2026-08.jsonl")).unwrap();
    symlink(outside.join("records.jsonl"), log.join("2026-09.jsonl")).unwrap();
    symlink(outside.join("gone.jsonl"), log.join("2026-10.jsonl")).unwrap();
    std::fs::create_dir(log.join("2026-07.jsonl")).unwrap();
    std::fs::write(log.join("2026-11.jsonl"), format!("{RUN}\n")).unwrap();
    let Some((code, text)) = review_guarded(&log, &["--json"]) else {
        panic!("house review blocked on a link to a FIFO");
    };
    assert_eq!(code, 0, "{text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    // Only the regular file's run: the linked record file is not read.
    assert_eq!(v["runs"], 1, "{text}");
    assert_eq!(v["skipped_files"], 4, "{text}");
}
