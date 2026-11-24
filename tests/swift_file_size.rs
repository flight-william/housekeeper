//! A Swift file's size has exactly one owner, whatever `[tools]` names: house's own
//! counter (r3, 400 lines for every language) when r3 runs for the stage, so
//! SwiftLint's `file_length` is then dropped and one long file is one finding with a
//! measure; SwiftLint's `file_length` when r3 is not named, so the rule is still kept.

mod common;

use house::adapter::{Adapter, Context, Output, Stage};
use house::adapters::r3::R3;
use house::adapters::swiftlint::SwiftLint;
use house::cli;
use std::path::{Path, PathBuf};

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

/// The parse keeps `file_length`; it is r3's only when r3 runs (`yields_to`).
#[test]
fn swiftlint_gives_a_files_length_up_to_r3() {
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
    assert_eq!(
        rules,
        ["swiftlint file_length", "swiftlint type_body_length"]
    );
    assert_eq!(SwiftLint.yields_to(), [("swiftlint file_length", "r3")]);
}

#[test]
fn a_run_whose_only_serious_violation_is_file_length_is_not_cannot_check() {
    let json = format!(
        "[{}]",
        violation(
            "file_length",
            "Error",
            "File should contain 1000 lines or less: currently contains 1200"
        )
    );
    let found = SwiftLint.parse(&ctx(), &out(2, &json)).unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
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

/// The tool rules `house check --stage commit` reports for a staged 501-line Swift
/// file, with `[tools] commit` naming `tools`.
fn long_file_rules(name: &str, tools: &str) -> Vec<String> {
    let root = common::repo(name);
    let toml = format!("[tools]\ncommit = [{tools}]\n");
    std::fs::write(root.join(".house.toml"), toml).unwrap();
    let body: String = (0..501).map(|i| format!("let value{i} = {i}\n")).collect();
    std::fs::write(root.join("Long.swift"), body).unwrap();
    common::git(&root, &["add", "-A"]);
    rules(&root)
}

fn rules(root: &Path) -> Vec<String> {
    let report = cli::check(root, Stage::Commit, house::adapters::all());
    assert!(report.cannots().is_empty(), "{}", report.text());
    report
        .findings()
        .iter()
        .map(|f| format!("{} {}", f.rule, f.tool_rule))
        .collect()
}

#[test]
fn without_r3_named_swiftlint_still_reports_a_long_swift_file() {
    let got = long_file_rules("swift-size-swiftlint-only", "\"swiftlint\"");
    assert_eq!(got, ["R3 swiftlint file_length"]);
}

#[test]
fn with_r3_named_the_long_swift_file_is_r3s_alone() {
    let got = long_file_rules("swift-size-with-r3", "\"swiftlint\", \"r3\"");
    assert_eq!(got, ["R3 r3 file_lines"]);
}
