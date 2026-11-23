//! `house review`: one plain report over the run records in `$HOUSE_LOG_DIR`. The
//! records in tests/fixtures/review are made up: four November runs (three in
//! /work/shop, one in /work/blog), one October run, two lines another program wrote
//! and one broken line.

mod common;
mod review_support;

use review_support::{NOVEMBER, folder_with, review, review_in, section};
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn the_header_names_the_period_and_the_runs_per_stage() {
    let (code, text) = review(&NOVEMBER);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.starts_with(
            "house review: 2026-11-01 to 2026-11-30, 4 runs: 2 commit, 1 push, 1 all\n"
        ),
        "{text}"
    );
}

#[test]
fn the_period_is_inclusive_and_read_from_each_records_start() {
    let (_, text) = review(&["--from", "2026-11-03", "--to", "2026-11-05"]);
    assert!(
        text.starts_with("house review: 2026-11-03 to 2026-11-05, 2 runs: 1 commit, 1 push\n"),
        "{text}"
    );
    let (_, text) = review(&["--to", "2026-10-31"]);
    assert!(
        text.starts_with("house review: 2026-10-15 to 2026-10-31, 1 run: 1 commit\n"),
        "{text}"
    );
}

#[test]
fn lines_that_are_not_house_records_are_skipped_and_counted_in_one_line() {
    // Two lines with "schema_version" and no "tools", and one cut-off line; notes.json
    // is not a month's record file and is not read.
    let (code, text) = review(&NOVEMBER);
    assert_eq!(code, 0, "{text}");
    let skipped: Vec<&str> = text.lines().filter(|l| l.contains("skipped")).collect();
    assert_eq!(
        skipped,
        ["3 lines skipped: not house run records"],
        "{text}"
    );
}

#[test]
fn each_rule_has_its_findings_excused_and_cannot_checks() {
    let (_, text) = review(&NOVEMBER);
    let block = section(&text, "per rule:");
    assert_eq!(
        block,
        [
            "  R2: 2 findings, 0 excused, 1 cannot check",
            "  R3: 3 findings, 6 excused, 0 cannot check",
            "  R4: 2 findings, 0 excused, 0 cannot check",
            // Known from October's record, silent in November.
            "  R8: 0 findings, 0 excused, 0 cannot check",
        ],
        "{text}"
    );
}

#[test]
fn each_tool_has_its_runs_findings_and_seconds_total_and_median() {
    let (_, text) = review(&NOVEMBER);
    let block = section(&text, "per tool:");
    assert_eq!(
        block,
        [
            // Skipped on the first run: two runs.
            "  clippy: 2 runs, 1 finding, 0 excused, 0 cannot check, 17.0 s total, 8.5 s median",
            "  gitleaks: 4 runs, 0 findings, 0 excused, 0 cannot check, 1.9 s total, 0.5 s median",
            "  r3: 3 runs, 3 findings, 6 excused, 0 cannot check, 0.7 s total, 0.2 s median",
            "  ruff: 4 runs, 3 findings, 0 excused, 0 cannot check, 3.5 s total, 0.9 s median",
            "  semgrep: 1 run, 0 findings, 0 excused, 1 cannot check, 0.0 s total, 0.0 s median",
        ],
        "{text}"
    );
}

const OLD_RUN: &str = r#"{"start":"2026-11-02T09:00:00Z","repo":"/work/shop","stage":"commit","seconds":1.0,"exit":0,"tools":[{"name":"r3","seconds":0.2,"skipped":false,"findings":{},"cannot_check":null}],"findings":[],"excused":2}"#;

#[test]
fn excuses_in_a_record_without_a_tools_excused_map_are_counted_apart() {
    // Records written before house 0.3.0 keep only the run's excused total.
    let log = folder_with("review-old", &[OLD_RUN]);
    let (code, text) = review_in(&log, &log, &NOVEMBER);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.contains("\n2 excused in records that do not say which rule (before house 0.3.0)\n"),
        "{text}"
    );
    assert!(
        text.contains("  r3: 1 run, 0 findings, 0 excused,"),
        "{text}"
    );
}

#[test]
fn a_place_that_failed_on_more_than_one_run_is_listed_most_frequent_first() {
    let (_, text) = review(&NOVEMBER);
    // Two repos in the period: each place names its repo.
    assert_eq!(
        section(&text, "repeated places (failed on more than one run):"),
        [
            "  R3 src/big.py [r3 file] in shop: 3 runs",
            "  R4 src/score.py [ruff C901] in shop: 2 runs",
        ],
        "{text}"
    );
    // October's R4 on the same place is outside the period.
    let (_, text) = review(&["--from", "2026-11-03", "--to", "2026-11-30"]);
    assert_eq!(
        section(&text, "repeated places (failed on more than one run):"),
        ["  R3 src/big.py [r3 file] in shop: 2 runs"],
        "{text}"
    );
}

#[test]
fn a_place_found_twice_in_one_run_is_not_repeated() {
    let run = r#"{"start":"2026-11-02T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{"name":"ruff","seconds":1.0,"findings":{"R4":2},"excused":{}}],"findings":[{"rule":"R4","place":"src/a.py","line":3,"tool_rule":"ruff C901"},{"rule":"R4","place":"src/a.py","line":9,"tool_rule":"ruff C901"}],"excused":0}"#;
    let log = folder_with("review-once", &[run]);
    let (_, text) = review_in(&log, &log, &NOVEMBER);
    assert!(text.contains("\nrepeated places: none\n"), "{text}");
}

#[test]
fn repo_takes_a_folder_name_or_a_path_and_names_no_repo_on_the_places() {
    let (_, text) = review(&[&NOVEMBER[..], &["--repo", "shop"]].concat());
    assert!(
        text.starts_with("house review: 2026-11-01 to 2026-11-30, 3 runs: 2 commit, 1 push\n"),
        "{text}"
    );
    assert_eq!(
        section(&text, "repeated places (failed on more than one run):"),
        [
            "  R3 src/big.py [r3 file]: 3 runs",
            "  R4 src/score.py [ruff C901]: 2 runs",
        ],
        "{text}"
    );
    let (_, text) = review(&[&NOVEMBER[..], &["--repo", "/work/blog"]].concat());
    assert!(
        text.starts_with("house review: 2026-11-01 to 2026-11-30, 1 run: 1 all\n"),
        "{text}"
    );
    // A path is its repo's top folder, from anywhere inside it.
    let repo = common::repo("review-path");
    std::fs::create_dir_all(repo.join("sub")).unwrap();
    let run = OLD_RUN.replace("/work/shop", &repo.display().to_string());
    let log = folder_with("review-path-log", &[&run, OLD_RUN]);
    let (_, text) = review_in(
        &repo.join("sub"),
        &log,
        &[&NOVEMBER[..], &["--repo", ".."]].concat(),
    );
    assert!(text.contains(", 1 run: 1 commit\n"), "{text}");
    let (_, text) = review_in(&repo, &log, &[&NOVEMBER[..], &["--repo", "./sub"]].concat());
    assert!(text.contains(", 1 run: 1 commit\n"), "{text}");
}

#[test]
fn json_holds_the_same_report_as_one_object() {
    let (code, text) = review(&[&NOVEMBER[..], &["--json"]].concat());
    assert_eq!(code, 0, "{text}");
    assert_eq!(text.lines().count(), 1, "{text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["from"], "2026-11-01");
    assert_eq!(v["to"], "2026-11-30");
    assert_eq!(v["runs"], 4);
    assert_eq!(
        v["stages"],
        serde_json::json!({"commit": 2, "push": 1, "all": 1})
    );
    assert_eq!(v["skipped_lines"], 3);
    assert_eq!(
        v["rules"][1],
        serde_json::json!({"rule": "R3", "findings": 3, "excused": 6, "cannot_check": 0})
    );
    assert_eq!(
        v["tools"][0],
        serde_json::json!({"tool": "clippy", "runs": 2, "findings": 1, "excused": 0, "cannot_check": 0, "seconds_total": 17.0, "seconds_median": 8.5})
    );
    assert_eq!(v["excused_unattributed"], 0);
    assert_eq!(
        v["repeated"][0],
        serde_json::json!({"repo": "/work/shop", "rule": "R3", "place": "src/big.py", "tool_rule": "r3 file", "runs": 3})
    );
    assert_eq!(v["repeated"].as_array().unwrap().len(), 2);
    assert_eq!(v["suggestions"].as_array().unwrap().len(), 5);
    assert_eq!(
        v["suggestions"][4],
        serde_json::json!({"kind": "slow", "subject": "clippy", "text": "clippy takes most of the time: 17.0 of 23.1 s (74%); run it at push only, or hand it fewer files"})
    );
    assert_eq!(v["suggestions"][0]["kind"], "looks_noisy");
}

#[test]
fn broken_lines_are_skipped_and_counted_never_a_crash() {
    let log = folder_with(
        "review-broken",
        &[
            "[1, 2]",
            "42",
            "not json at all",
            r#"{"start":"2026-11-02T09:00:00Z","stage":"commit","tools":{"name":"ruff"}}"#,
            r#"{"start":"yesterday","stage":"commit","tools":[]}"#,
            r#"{"start":"2026-02-30T09:00:00Z","stage":"commit","tools":[]}"#,
            r#"{"start":"2026-11-02T09:00:00Z","stage":"commit","tools":[{"seconds":1}]}"#,
            r#"{"schema_version":1,"start":"2026-11-02T09:00:00Z","stage":"commit"}"#,
            OLD_RUN,
        ],
    );
    let mut bytes = std::fs::read(log.join("2026-11.jsonl")).unwrap();
    bytes.extend_from_slice(b"{\"start\":\"2026-11-02\xff\xfe\n");
    std::fs::write(log.join("2026-11.jsonl"), bytes).unwrap();
    let (code, text) = review_in(&log, &log, &NOVEMBER);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains(", 1 run: 1 commit\n"), "{text}");
    assert!(
        text.contains("\n9 lines skipped: not house run records\n"),
        "{text}"
    );
}

#[test]
fn a_record_folder_that_does_not_exist_holds_no_runs() {
    let missing = common::fresh("review-missing").join("runs");
    let (code, text) = review_in(missing.parent().unwrap(), &missing, &NOVEMBER);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.starts_with("house review: 2026-11-01 to 2026-11-30, 0 runs\n"),
        "{text}"
    );
    assert!(!text.contains("skipped"), "{text}");
    assert!(text.ends_with("\nsuggestions: none\n"), "{text}");
}

#[test]
fn a_bad_date_or_an_unknown_option_is_a_usage_error() {
    for args in [
        &["--from", "2026-13-01"][..],
        &["--to", "today"],
        &["--from", "2026-11-30", "--to", "2026-11-01"],
        &["--repo"],
        &["--since", "2026-11-01"],
    ] {
        let (code, text) = review(args);
        assert_eq!(code, 2, "{args:?}: {text}");
    }
}

#[test]
fn without_to_the_review_runs_to_today() {
    // Every fixture run is in the past, so each one is covered.
    let (code, text) = review(&["--from", "2026-10-01"]);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.contains(", 5 runs: 3 commit, 1 push, 1 all\n"),
        "{text}"
    );
}

#[test]
fn a_tool_with_no_runs_in_the_period_is_left_out() {
    // cargo-deny was skipped on the one run: not a tool of the period.
    let run = r#"{"start":"2026-11-02T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{"name":"ruff","seconds":1.0,"findings":{"R4":1},"excused":{}},{"name":"cargo-deny","seconds":0.0,"skipped":true,"findings":{},"excused":{}}],"findings":[{"rule":"R4","place":"src/a.py","tool_rule":"ruff C901"}],"excused":0}"#;
    let log = folder_with("review-no-runs", &[run]);
    let (_, text) = review_in(&log, &log, &NOVEMBER);
    assert_eq!(
        section(&text, "per tool:"),
        ["  ruff: 1 run, 1 finding, 0 excused, 0 cannot check, 1.0 s total, 1.0 s median"],
        "{text}"
    );
    assert!(!text.contains("cargo-deny"), "{text}");
    let (_, json) = review_in(&log, &log, &[&NOVEMBER[..], &["--json"]].concat());
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["tools"].as_array().unwrap().len(), 1, "{json}");
}

#[test]
fn a_total_never_prints_negative_zero() {
    let run = r#"{"start":"2026-11-02T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{"name":"ruff","seconds":-0.0,"findings":{"R4":1},"excused":{}}],"findings":[{"rule":"R4","place":"src/a.py","tool_rule":"ruff C901"}],"excused":0}"#;
    let log = folder_with("review-negative-zero", &[run]);
    let (_, text) = review_in(&log, &log, &NOVEMBER);
    assert!(!text.contains("-0.0"), "{text}");
    assert!(text.contains(", 0.0 s total, 0.0 s median\n"), "{text}");
    let (_, json) = review_in(&log, &log, &[&NOVEMBER[..], &["--json"]].concat());
    assert!(!json.contains("-0.0"), "{json}");
}

#[test]
fn a_start_with_an_offset_is_filed_under_its_utc_day() {
    // house writes each start in UTC (Z); another offset counts by its UTC day.
    let at = |start: &str| OLD_RUN.replace("2026-11-02T09:00:00Z", start);
    let lines = [
        at("2026-11-22T20:00:00-10:00"), // 2026-11-23 in UTC
        at("2026-11-01T00:00:01+14:00"), // 2026-10-31 in UTC
        at("2026-11-15T12:00:00Z"),
        at("2026-11-15T12:00:00.250+05:30"),
        at("2026-11-15T25:00:00Z"), // no such time: not a house record
    ];
    let log = folder_with(
        "review-offsets",
        &lines.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let (_, text) = review_in(&log, &log, &["--from", "2026-11-01", "--to", "2026-11-22"]);
    assert!(text.contains(", 2 runs: 2 commit\n"), "{text}");
    assert!(
        text.contains("\n1 line skipped: not house run records\n"),
        "{text}"
    );
    let (_, text) = review_in(&log, &log, &["--from", "2026-10-31", "--to", "2026-10-31"]);
    assert!(text.contains(", 1 run: 1 commit\n"), "{text}");
}

#[test]
fn without_to_the_last_day_is_today_in_utc_whatever_the_time_zone() {
    // Honolulu is 10 hours behind UTC and Kiritimati 14 ahead: at any moment one of
    // them is on another calendar day than UTC.
    let log = common::fresh("review-utc-today");
    for tz in ["Pacific/Honolulu", "Pacific/Kiritimati"] {
        let utc = || {
            let out = Command::new("date").args(["-u", "+%F"]).output().unwrap();
            String::from_utf8(out.stdout).unwrap().trim().to_string()
        };
        let before = utc();
        let out = Command::new(env!("CARGO_BIN_EXE_house"))
            .args(["review", "--json"])
            .env("HOUSE_LOG_DIR", &log)
            .env("TZ", tz)
            .output()
            .unwrap();
        let after = utc();
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let to = v["to"].as_str().unwrap_or_default().to_string();
        assert!(to == before || to == after, "{tz}: to {to}, UTC {before}");
    }
}

// `--repo` matches a repository, not a folder: runs in two worktrees of one
// repository are its runs. The records in tests/fixtures/review_repo are made up:
// shop's runs in two worktrees and in its main checkout (an older record that names
// only the folder), and blog's in its main checkout and one worktree.

fn repo_fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/review_repo")
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

/// A run record of `folder` on 2026-11-`day`, with `root` as its root commit, or
/// none (a record from before 0.3.1, which named only the folder).
fn run(day: u32, folder: &Path, root: Option<&str>) -> String {
    let id = match root {
        Some(root) => format!(r#","root_commit":"{root}","main_checkout":null"#),
        None => String::new(),
    };
    format!(
        r#"{{"start":"2026-11-{day:02}T09:00:00Z","repo":"{}"{id},"stage":"commit","tools":[{{"name":"ruff","seconds":1.0,"findings":{{}},"excused":{{}}}}],"findings":[],"excused":0}}"#,
        folder.display()
    )
}

fn root_of(repo: &Path) -> String {
    let out = Command::new("git")
        .args(["rev-list", "--max-parents=0", "HEAD"])
        .current_dir(repo)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// A repo with one commit and a worktree of it: (main checkout, worktree).
fn repo_with_worktree(name: &str) -> (PathBuf, PathBuf) {
    let main = common::repo(name);
    std::fs::write(main.join("a.txt"), "a\n").unwrap();
    common::git(&main, &["add", "a.txt"]);
    common::git(&main, &["commit", "-q", "-m", "a"]);
    let tree = common::fresh(&format!("{name}-trees")).join("feature");
    let tree_arg = tree.display().to_string();
    common::git(
        &main,
        &["worktree", "add", "-q", "-b", "feature", &tree_arg],
    );
    (main, tree.canonicalize().unwrap())
}

#[test]
fn a_path_counts_the_runs_of_every_worktree_of_its_repository() {
    let (main, tree) = repo_with_worktree("review-repo-path");
    let root = root_of(&main);
    let other = "c".repeat(40);
    let log = folder_with(
        "review-repo-path-log",
        &[
            &run(2, &main, Some(&root)),
            &run(3, &tree, Some(&root)),
            // A worktree since removed: still the repository's, by its root.
            &run(4, Path::new("/work/gone"), Some(&root)),
            // An unrelated repository with a different root, in the same folder.
            &run(5, &tree, Some(&other)),
            // An older record: no root commit, matched by its folder.
            &run(6, &tree, None),
            &run(7, &main, None),
        ],
    );
    let (code, text) = review_in(&tree, &log, &[&NOVEMBER[..], &["--repo", "."]].concat());
    assert_eq!(code, 0, "{text}");
    assert_eq!(
        first_line(&text),
        "house review: 2026-11-01 to 2026-11-30, 4 runs: 4 commit",
        "{text}"
    );
    // From the main checkout: the same root, and the older record of its folder.
    let main_arg = main.display().to_string();
    let (_, text) = review_in(
        &tree,
        &log,
        &[&NOVEMBER[..], &["--repo", &main_arg]].concat(),
    );
    assert_eq!(
        first_line(&text),
        "house review: 2026-11-01 to 2026-11-30, 4 runs: 4 commit",
        "{text}"
    );
}

#[test]
fn a_name_matches_a_records_folder_or_its_main_checkout() {
    let cwd = common::fresh("review-repo-name");
    let (code, text) = review_in(
        &cwd,
        &repo_fixtures(),
        &[&NOVEMBER[..], &["--repo", "shop"]].concat(),
    );
    assert_eq!(code, 0, "{text}");
    // feature-a and feature-b by their main checkout, /work/shop by its folder.
    assert_eq!(
        first_line(&text),
        "house review: 2026-11-01 to 2026-11-30, 3 runs: 2 commit, 1 push",
        "{text}"
    );
    // A worktree's own folder name is a name too.
    let (_, text) = review_in(
        &cwd,
        &repo_fixtures(),
        &[&NOVEMBER[..], &["--repo", "draft"]].concat(),
    );
    assert_eq!(
        first_line(&text),
        "house review: 2026-11-01 to 2026-11-30, 1 run: 1 commit",
        "{text}"
    );
}
