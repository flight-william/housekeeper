//! The core's reply side: reply shape and exit, the unread count, and the record line.

#[path = "common/temp.rs"]
mod common;

use house::adapter::{CannotCheck, Stage};
use house::finding::{Finding, Measure, Place};
use house::record;
use house::reply::{Report, ToolRun};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

fn finding(
    rule: &str,
    place: Place,
    line: Option<u32>,
    what: &str,
    fix: &str,
    tool_rule: &str,
) -> Finding {
    Finding {
        rule: rule.into(),
        place,
        line,
        what: what.into(),
        fix: fix.into(),
        tool_rule: tool_rule.into(),
        measure: None,
    }
}

fn tool(name: &str, findings: Vec<Finding>, cannot: Option<CannotCheck>) -> ToolRun {
    ToolRun {
        name: name.into(),
        seconds: 0.5,
        exit: Some(1),
        skipped: false,
        findings,
        cannot,
        unread: Vec::new(),
    }
}

fn sample() -> Report {
    let file = |p: &str| Place::File(p.into());
    let mut r4 = finding(
        "R4",
        file("src/score.py"),
        Some(88),
        "complexity 14, limit 10",
        "split the branches into helpers",
        "ruff C901",
    );
    r4.measure = Some(Measure {
        value: 14.0,
        unit: "complexity".into(),
        limit: 10.0,
    });
    Report {
        stage: Stage::Commit,
        seconds: 4.1,
        tools: vec![
            tool(
                "coverage",
                vec![finding(
                    "R7",
                    Place::Repo,
                    None,
                    "coverage 71 %, limit 75 %",
                    "add tests for the changed lines",
                    "coverage fail-under",
                )],
                None,
            ),
            tool(
                "ruff",
                vec![
                    r4,
                    finding("R10", file("src/a.rs"), None, "ten", "fix ten", "t10"),
                    finding("R2", file("src/a.rs"), None, "two", "fix two", "t2"),
                ],
                None,
            ),
            tool(
                "swiftlint",
                vec![],
                Some(CannotCheck {
                    what: "swiftlint is not installed".into(),
                    fix: "brew install swiftlint".into(),
                }),
            ),
        ],
    }
}

#[test]
fn reply_has_the_design_shape_sorted_by_file_line_rule() {
    assert_eq!(
        sample().text(),
        "house commit: 4 to fix, 1 cannot check (4.1 s)\n\
         R2 src/a.rs: two. fix: fix two. [t2]\n\
         R10 src/a.rs: ten. fix: fix ten. [t10]\n\
         R4 src/score.py:88: complexity 14, limit 10. fix: split the branches into helpers. [ruff C901]\n\
         R7 repo: coverage 71 %, limit 75 %. fix: add tests for the changed lines. [coverage fail-under]\n\
         R2 cannot check: swiftlint is not installed. fix: brew install swiftlint [swiftlint]\n"
    );
}

#[test]
fn exit_two_wins_then_one_then_zero() {
    assert_eq!(sample().exit(), 2);
    let mut only_findings = sample();
    only_findings.tools.pop();
    assert_eq!(only_findings.exit(), 1);
    assert!(
        only_findings
            .text()
            .starts_with("house commit: 4 to fix (4.1 s)\n")
    );
    let clean = Report {
        stage: Stage::Commit,
        seconds: 1.2,
        tools: vec![tool("ruff", vec![], None)],
    };
    assert_eq!(clean.exit(), 0);
    assert_eq!(clean.text(), "house commit: nothing to fix (1.2 s)\n");
}

/// Files Semgrep's own parser could not read are Semgrep's
/// limit, not findings and not cannot-check; the header counts them.
#[test]
fn files_a_tool_could_not_read_are_counted_in_the_header_not_findings() {
    let mut semgrep = tool("semgrep", vec![], None);
    semgrep.unread = vec!["Tests/A.swift".into(), "Tests/B.swift".into()];
    let mut r = Report {
        stage: Stage::Commit,
        seconds: 1.2,
        tools: vec![semgrep],
    };
    assert_eq!(r.exit(), 0);
    assert_eq!(
        r.text(),
        "house commit: nothing to fix (1.2 s) (2 files semgrep could not read)\n"
    );
    r.tools[0].unread.pop();
    assert_eq!(
        r.text(),
        "house commit: nothing to fix (1.2 s) (1 file semgrep could not read)\n"
    );
}

#[test]
fn record_line_keeps_the_unread_count_and_files() {
    let info = record::RunInfo {
        start: UNIX_EPOCH,
        root: Path::new("/r"),
        head: None,
        identity: Default::default(),
        user: "u".into(),
    };
    let mut semgrep = tool("semgrep", vec![], None);
    semgrep.unread = vec!["Tests/A.swift".into(), "Tests/B.swift".into()];
    let mut r = sample();
    r.tools.push(semgrep);
    let line = record::line(&info, &r);
    assert_eq!(line["tools"][0]["unread"], 0);
    assert_eq!(line["tools"][0]["unread_files"], serde_json::json!([]));
    assert_eq!(line["tools"][3]["name"], "semgrep");
    assert_eq!(line["tools"][3]["unread"], 2);
    assert_eq!(
        line["tools"][3]["unread_files"],
        serde_json::json!(["Tests/A.swift", "Tests/B.swift"])
    );
}

#[test]
fn json_holds_findings_and_cannot_checks() {
    let v = sample().json();
    assert_eq!(v["exit"], 2);
    assert_eq!(v["stage"], "commit");
    assert_eq!(v["findings"].as_array().unwrap().len(), 4);
    assert_eq!(v["findings"][2]["place"]["file"], "src/score.py");
    assert_eq!(v["findings"][3]["place"], "repo");
    assert_eq!(v["cannot_check"][0]["tool"], "swiftlint");
    assert_eq!(v["cannot_check"][0]["rule"], "R2");
}

#[test]
fn record_line_has_run_tools_and_findings_but_no_bodies() {
    let start = UNIX_EPOCH + Duration::from_secs(1_795_000_000);
    let info = record::RunInfo {
        start,
        root: Path::new("/r"),
        head: None,
        identity: Default::default(),
        user: "u".into(),
    };
    let line = record::line(&info, &sample());
    assert_eq!(line["start"], record::rfc3339(start));
    assert_eq!(line["repo"], "/r");
    assert!(line["head"].is_null());
    assert_eq!(line["exit"], 2);
    assert_eq!(line["tools"][1]["name"], "ruff");
    assert_eq!(line["tools"][1]["findings"]["R4"], 1);
    assert_eq!(
        line["tools"][2]["cannot_check"],
        "swiftlint is not installed. fix: brew install swiftlint"
    );
    assert_eq!(line["findings"][2]["tool_rule"], "ruff C901");
    assert_eq!(line["findings"][2]["line"], 88);
    let text = line.to_string();
    assert!(
        !text.contains("split the branches"),
        "no fix or message bodies: {text}"
    );
}

#[test]
fn record_appends_one_line_per_run_in_the_month_file() {
    let dir = common::fresh("record").join("runs");
    let start = UNIX_EPOCH + Duration::from_secs(1_795_000_000);
    let path = record::append(&dir, start, &serde_json::json!({"a": 1})).unwrap();
    record::append(&dir, start, &serde_json::json!({"a": 2})).unwrap();
    assert_eq!(path.file_name().unwrap(), "2026-11.jsonl");
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "{\"a\":1}\n{\"a\":2}\n"
    );
}

#[test]
fn rfc3339_known_times() {
    assert_eq!(record::rfc3339(UNIX_EPOCH), "1970-01-01T00:00:00Z");
    assert_eq!(
        record::rfc3339(UNIX_EPOCH + Duration::from_secs(951_782_400)),
        "2000-02-29T00:00:00Z"
    );
    assert_eq!(
        record::rfc3339(UNIX_EPOCH + Duration::from_secs(1_795_000_000)),
        "2026-11-18T11:06:40Z"
    );
}
