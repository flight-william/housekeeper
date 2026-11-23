//! A Swift file's size has one owner, house's own counter (r3, 400 lines for every
//! language): SwiftLint's `file_length` is not reported, so one long file is one
//! finding, with a measure, whatever the repo's SwiftLint limit is.

use house::adapter::{Adapter, Context, Output, Stage};
use house::adapters::r3::R3;
use house::adapters::swiftlint::SwiftLint;
use std::path::PathBuf;

fn ctx() -> Context {
    Context {
        root: PathBuf::from("/repo"),
        stage: Stage::Commit,
        files: vec!["Sources/Long.swift".to_string()],
        push_range: None,
        languages: Default::default(),
    }
}

fn out(exit: i32, stdout: &str) -> Output {
    Output {
        exit,
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

fn violation(rule_id: &str, severity: &str, reason: &str) -> String {
    format!(
        r#"{{"character": 1, "file": "/repo/Sources/Long.swift", "line": 12, "reason": "{reason}", "rule_id": "{rule_id}", "severity": "{severity}", "type": "x"}}"#
    )
}

#[test]
fn swiftlint_does_not_report_a_files_length() {
    let json = format!(
        "[{}, {}]",
        violation(
            "file_length",
            "Warning",
            "File should contain 400 lines or less: currently contains 512"
        ),
        violation(
            "type_body_length",
            "Warning",
            "Type body should span 250 lines or less: currently spans 300 lines"
        ),
    );
    let found = SwiftLint.parse(&ctx(), &out(0, &json)).unwrap();
    let rules: Vec<&str> = found.iter().map(|f| f.tool_rule.as_str()).collect();
    assert_eq!(rules, ["swiftlint type_body_length"]);
}

#[test]
fn a_run_whose_only_serious_violation_is_file_length_is_nothing_to_fix() {
    let json = format!(
        "[{}]",
        violation(
            "file_length",
            "Error",
            "File should contain 1000 lines or less: currently contains 1200"
        )
    );
    let found = SwiftLint.parse(&ctx(), &out(2, &json)).unwrap();
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn r3_reports_the_swift_file_once_with_its_measure() {
    let line =
        r#"{"file": "Sources/Long.swift", "kind": "file", "name": "", "lines": 512, "limit": 400}"#;
    let found = R3.parse(&ctx(), &out(0, line)).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule, "R3");
    assert_eq!(found[0].tool_rule, "r3 file_lines");
    assert_eq!(found[0].measure.as_ref().map(|m| m.value), Some(512.0));
}
