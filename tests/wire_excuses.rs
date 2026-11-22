//! The wiring of the excuse list after the adapters.

mod common;
mod wire_run;

use house::adapter::{CannotCheck, Stage};
use house::cli;
use house::excuses::Date;
use house::finding::{Finding, Place};
use house::reply::{Report, ToolRun};
use std::path::PathBuf;
use wire_run::{house, records};

fn found(rule: &str, file: &str, tool_rule: &str) -> Finding {
    Finding {
        rule: rule.into(),
        place: Place::File(file.into()),
        line: Some(3),
        what: "too long".into(),
        fix: "split it".into(),
        tool_rule: tool_rule.into(),
        measure: None,
    }
}

fn report(stage: Stage, findings: Vec<Finding>, cannot: Option<CannotCheck>) -> Report {
    Report {
        stage,
        seconds: 0.1,
        tools: vec![ToolRun {
            name: "t".into(),
            seconds: 0.1,
            exit: Some(1),
            skipped: false,
            findings,
            cannot,
            unread: Vec::new(),
        }],
    }
}

/// A repo whose HEAD holds `list` as its excuse list, the working copy the same.
fn excuses_repo(name: &str, list: &str) -> PathBuf {
    let dir = common::repo(name);
    std::fs::write(dir.join(".house-excuses.toml"), list).unwrap();
    common::git(&dir, &["add", "-A"]);
    common::git(&dir, &["commit", "-q", "--no-verify", "-m", "list"]);
    dir
}

const LIVE: &str = "[[excuse]]\nrule = \"R3\"\nfile = \"src/old.py\"\ndeadline = \"2026-12-31\"\nreason = \"split in v2\"\n";
const STALE: &str = "[[excuse]]\nrule = \"R4\"\nfile = \"src/gone.py\"\ndeadline = \"2026-12-31\"\nreason = \"was refactored\"\n";
const EXPIRED: &str = "[[excuse]]\nrule = \"R3\"\nfile = \"src/old.py\"\ndeadline = \"2026-11-01\"\nreason = \"split in v2\"\n";

fn day(text: &str) -> Date {
    Date::parse(text).unwrap()
}

#[test]
fn an_excused_finding_leaves_the_reply_and_is_counted_in_the_header() {
    let root = excuses_repo("ex-live", LIVE);
    let mut r = report(
        Stage::Commit,
        vec![
            found("R3", "src/old.py", "r3 file"),
            found("R3", "src/new.py", "r3 file"),
        ],
        None,
    );
    let excused = cli::excuse(&mut r, &root, day("2026-11-22"), Some("HEAD"));
    assert_eq!(excused, 1);
    assert_eq!(r.exit(), 1);
    let text = r.text_as("commit", excused);
    assert!(
        text.starts_with("house commit: 1 to fix, 1 excused ("),
        "{text}"
    );
    assert!(text.contains("R3 src/new.py:3"), "{text}");
    assert!(!text.contains("src/old.py"), "{text}");
}

#[test]
fn every_finding_excused_is_nothing_to_fix_exit_zero() {
    let root = excuses_repo("ex-all", LIVE);
    let mut r = report(Stage::Commit, vec![found("R3", "src/old.py", "x")], None);
    let excused = cli::excuse(&mut r, &root, day("2026-12-31"), Some("HEAD"));
    assert_eq!((excused, r.exit()), (1, 0));
    assert!(
        r.text_as("commit", excused)
            .starts_with("house commit: nothing to fix, 1 excused (")
    );
}

// A push is a changed-file run, so an untouched, still-needed excuse looked stale. Only a
// full run can judge staleness; commit and push never do.
#[test]
fn a_stale_excuse_is_dropped_at_commit_and_at_push() {
    let root = excuses_repo("ex-stale", STALE);
    for stage in [Stage::Commit, Stage::Push] {
        let mut r = report(stage, vec![], None);
        assert_eq!(
            cli::excuse(&mut r, &root, day("2026-11-22"), Some("HEAD")),
            0
        );
        assert_eq!(r.exit(), 0, "{:?}: {}", stage, r.text());
        assert!(!r.text().contains("stale excuse"), "{}", r.text());
    }
}

#[test]
fn a_stale_excuse_is_not_judged_when_a_tool_could_not_check() {
    let root = excuses_repo("ex-stale-cannot", STALE);
    let cannot = CannotCheck {
        what: "ruff is not installed".into(),
        fix: "brew install ruff".into(),
    };
    let mut r = report(Stage::Push, vec![], Some(cannot));
    cli::excuse(&mut r, &root, day("2026-11-22"), Some("HEAD"));
    assert!(!r.text().contains("stale excuse"), "{}", r.text());
}

#[test]
fn an_expired_excuse_excuses_nothing_and_is_a_finding_at_commit() {
    let root = excuses_repo("ex-expired", EXPIRED);
    let mut r = report(Stage::Commit, vec![found("R3", "src/old.py", "x")], None);
    assert_eq!(
        cli::excuse(&mut r, &root, day("2026-11-22"), Some("HEAD")),
        0
    );
    let text = r.text();
    assert!(text.starts_with("house commit: 2 to fix ("), "{text}");
    assert!(
        text.contains("excuse expired: R3 src/old.py, deadline 2026-11-01"),
        "{text}"
    );
}

#[test]
fn a_broken_excuse_list_cannot_check() {
    let root = excuses_repo("ex-broken", "[[excuse]]\nrule = \"R3\"\n");
    let mut r = report(Stage::Commit, vec![], None);
    cli::excuse(&mut r, &root, day("2026-11-22"), Some("HEAD"));
    assert_eq!(r.exit(), 2);
    assert!(
        r.text().contains("[excuses]\n")
            && r.text().contains("R2 cannot check: .house-excuses.toml"),
        "{}",
        r.text()
    );
}

// Regression: an entry added beside new bad code must not pass the check.
#[test]
fn an_entry_added_since_head_is_the_list_growing() {
    let root = common::repo("ex-grew-new");
    common::git(
        &root,
        &[
            "commit",
            "-q",
            "--allow-empty",
            "--no-verify",
            "-m",
            "start",
        ],
    );
    std::fs::write(root.join(".house-excuses.toml"), LIVE).unwrap();
    let mut r = report(Stage::Commit, vec![found("R3", "src/old.py", "x")], None);
    assert_eq!(
        cli::excuse(&mut r, &root, day("2026-11-22"), Some("HEAD")),
        1
    );
    assert_eq!(r.exit(), 1, "{}", r.text());
    assert!(
        r.text().contains("R9 .house-excuses.toml: excuse list grew: R3 src/old.py. fix: remove it, or ask the owner. [house excuse-grew]\n"),
        "{}",
        r.text()
    );
}

#[test]
fn a_deadline_moved_later_than_heads_is_the_list_growing() {
    let root = excuses_repo("ex-grew-later", LIVE);
    std::fs::write(
        root.join(".house-excuses.toml"),
        LIVE.replace("2026-12-31", "2027-06-30"),
    )
    .unwrap();
    let mut r = report(Stage::Commit, vec![], None);
    cli::excuse(&mut r, &root, day("2026-11-22"), Some("HEAD"));
    assert!(
        r.text().contains("excuse list grew: R3 src/old.py."),
        "{}",
        r.text()
    );
}

#[test]
fn in_a_repo_with_no_commit_yet_every_entry_is_new() {
    let root = common::repo("ex-grew-unborn");
    std::fs::write(root.join(".house-excuses.toml"), LIVE).unwrap();
    let mut r = report(Stage::Commit, vec![], None);
    cli::excuse(&mut r, &root, day("2026-11-22"), Some("HEAD"));
    assert!(
        r.text().contains("excuse list grew: R3 src/old.py."),
        "{}",
        r.text()
    );
}

#[test]
fn a_broken_list_at_head_cannot_check() {
    let root = excuses_repo("ex-grew-broken", "[[excuse]]\nrule = \"R3\"\n");
    std::fs::write(root.join(".house-excuses.toml"), LIVE).unwrap();
    let mut r = report(Stage::Commit, vec![], None);
    cli::excuse(&mut r, &root, day("2026-11-22"), Some("HEAD"));
    assert_eq!(r.exit(), 2, "{}", r.text());
    assert!(r.text().contains("at HEAD"), "{}", r.text());
}

// At push the base is the push base, so a list grown in a pushed commit (made with
// --no-verify) is still the list growing, though HEAD holds it.
#[test]
fn at_push_the_list_is_compared_with_the_push_base() {
    let root = excuses_repo("ex-grew-push", LIVE);
    let out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()
        .unwrap();
    let base = String::from_utf8(out.stdout).unwrap().trim().to_string();
    let grown = format!("{LIVE}{}", STALE);
    std::fs::write(root.join(".house-excuses.toml"), grown).unwrap();
    common::git(&root, &["commit", "-q", "-a", "--no-verify", "-m", "grow"]);
    let mut r = report(Stage::Push, vec![], None);
    cli::excuse(&mut r, &root, day("2026-11-22"), Some(&base));
    assert!(
        r.text().contains("excuse list grew: R4 src/gone.py."),
        "{}",
        r.text()
    );
    let mut r = report(Stage::Push, vec![], None);
    cli::excuse(&mut r, &root, day("2026-11-22"), Some("HEAD"));
    assert!(!r.text().contains("grew"), "{}", r.text());
}

#[test]
fn no_excuse_list_changes_nothing() {
    let root = common::fresh("ex-none");
    let mut r = report(Stage::Push, vec![found("R3", "a.py", "x")], None);
    assert_eq!(
        cli::excuse(&mut r, &root, day("2026-11-22"), Some("HEAD")),
        0
    );
    assert_eq!(r.tools.len(), 1);
    assert_eq!(r.findings().len(), 1);
}

#[test]
fn the_binary_applies_the_list_by_the_system_clock_and_records_the_count() {
    let repo = common::repo("ex-bin");
    let log = common::fresh("ex-bin-log");
    // Deadline far ahead: live, matches nothing.
    std::fs::write(
        repo.join(".house-excuses.toml"),
        STALE.replace("2026-12-31", "2999-12-31"),
    )
    .unwrap();
    common::git(&repo, &["add", "-A"]);
    common::git(&repo, &["commit", "-q", "--no-verify", "-m", "list"]);
    let (code, text) = house(&repo, &log, &["check", "--stage", "commit"], &[]);
    assert_eq!(code, 0, "stale is not judged at commit: {text}");
    let (code, text) = house(&repo, &log, &["check", "--stage", "push"], &[]);
    assert_eq!(code, 0, "nor at push: {text}");
    // Deadline long past: expired by the current date, at commit too.
    std::fs::write(
        repo.join(".house-excuses.toml"),
        STALE.replace("2026-12-31", "2000-01-01"),
    )
    .unwrap();
    let (code, text) = house(&repo, &log, &["check", "--stage", "commit", "--json"], &[]);
    assert_eq!(code, 1, "{text}");
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["excused"], 0);
    assert_eq!(json["findings"][0]["tool_rule"], "house excuse-expired");
    let lines = records(&log);
    assert_eq!(lines.len(), 3);
    assert!(lines.iter().all(|l| l["excused"] == 0));
    assert_eq!(lines[1]["stage"], "push");
}
