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

#[test]
fn a_line_longer_than_the_limit_is_skipped_once_even_a_valid_record() {
    let log = common::fresh("review-long-line");
    // A real record padded past 1 MiB with a field the review does not read.
    let padded = RUN.replacen('{', &format!("{{\"pad\":\"{}\",", "x".repeat(1 << 20)), 1);
    std::fs::write(log.join("2026-11.jsonl"), format!("{padded}\n{RUN}\n")).unwrap();
    let Some((code, text)) = review_guarded(&log, &["--json"]) else {
        panic!("house review did not finish");
    };
    assert_eq!(code, 0, "{text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        (v["runs"].clone(), v["skipped_lines"].clone()),
        (1.into(), 1.into()),
        "{text}"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn a_huge_line_is_counted_without_holding_it_in_memory() {
    let log = common::fresh("review-huge-line");
    let mut huge = vec![b'x'; 64 << 20];
    huge.push(b'\n');
    std::fs::write(log.join("2026-11.jsonl"), huge).unwrap();
    // macOS time -l reports the peak resident memory, in bytes, on stderr.
    let out = Command::new("/usr/bin/time")
        .args([
            "-l",
            env!("CARGO_BIN_EXE_house"),
            "review",
            "--to",
            "2026-11-30",
        ])
        .env("HOUSE_LOG_DIR", &log)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stdout.contains("\n1 line skipped: not house run records\n"),
        "{stdout}"
    );
    let peak: u64 = stderr
        .lines()
        .find(|l| l.contains("maximum resident set size"))
        .and_then(|l| l.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no peak memory in: {stderr}"));
    assert!(
        peak < 32 << 20,
        "peak memory {peak} bytes for a 64 MiB line"
    );
}

#[test]
fn more_distinct_places_than_the_limit_stop_the_review_with_a_clear_error() {
    let log = common::fresh("review-many-places");
    // 201 lines of 500 distinct places each: 100 500 places, past the limit.
    let mut lines = String::new();
    for line in 0..201 {
        let findings: Vec<String> = (0..500)
            .map(|n| {
                format!(r#"{{"rule":"R3","place":"src/f{line}_{n}.py","tool_rule":"r3 file"}}"#)
            })
            .collect();
        lines.push_str(&format!(
            r#"{{"start":"2026-11-02T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{{"name":"r3","seconds":0.1,"findings":{{"R3":500}},"excused":{{}}}}],"findings":[{}],"excused":0}}"#,
            findings.join(",")
        ));
        lines.push('\n');
    }
    std::fs::write(log.join("2026-11.jsonl"), lines).unwrap();
    let Some((code, text)) = review_guarded(&log, &[]) else {
        panic!("house review did not finish");
    };
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("more than 100000 distinct places"), "{text}");
    assert!(
        text.contains("fix: review a shorter period or one repo"),
        "{text}"
    );
}

#[test]
fn a_line_that_is_not_utf8_is_skipped_never_repaired_into_a_record() {
    let log = common::fresh("review-not-utf8");
    let mut bytes = RUN.replace("/work/shop", "/work/\u{1}shop").into_bytes();
    let at = bytes.iter().position(|b| *b == 1).unwrap();
    bytes[at] = 0xff;
    bytes.push(b'\n');
    bytes.extend_from_slice(format!("{RUN}\n").as_bytes());
    std::fs::write(log.join("2026-11.jsonl"), bytes).unwrap();
    let Some((code, text)) = review_guarded(&log, &["--json"]) else {
        panic!("house review did not finish");
    };
    assert_eq!(code, 0, "{text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        (v["runs"].clone(), v["skipped_lines"].clone()),
        (1.into(), 1.into()),
        "{text}"
    );
}

#[test]
fn a_blank_line_is_not_a_record_and_is_counted_like_any_other() {
    let log = common::fresh("review-blank");
    std::fs::write(
        log.join("2026-11.jsonl"),
        format!("\n \t\nnot json\n{RUN}\n"),
    )
    .unwrap();
    let Some((code, text)) = review_guarded(&log, &["--json"]) else {
        panic!("house review did not finish");
    };
    assert_eq!(code, 0, "{text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["skipped_lines"], 3, "{text}");
}
