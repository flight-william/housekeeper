//! The run record keeps each tool's excused findings by rule, beside the run's total,
//! so a review of the records can tell which rule and tool the excuses go to.

mod common;
mod wire_run;

use wire_run::{house, records};

const PACKAGE: &str = "[package]\nname = \"a\"\nversion = \"0.1.0\"\n\n";

#[test]
fn the_record_counts_each_tools_excused_findings_by_rule() {
    let dir = common::repo("record-excused");
    let log = common::fresh("record-excused-log");
    std::fs::write(dir.join(".house.toml"), "[tools]\ncommit = []\n").unwrap();
    let list = "[[excuse]]\nrule = \"R9\"\nfile = \"Cargo.toml\"\nsymbol = \"too_many_lines\"\ndeadline = \"2999-12-31\"\nreason = \"old\"\n";
    std::fs::write(dir.join(".house-excuses.toml"), list).unwrap();
    let manifest =
        format!("{PACKAGE}[lints.clippy]\ntoo_many_lines = \"allow\"\nfloat_cmp = \"allow\"\n");
    std::fs::write(dir.join("Cargo.toml"), &manifest).unwrap();
    common::git(&dir, &["add", "-A"]);
    common::git(&dir, &["commit", "-q", "--no-verify", "-m", "x"]);
    std::fs::write(dir.join("Cargo.toml"), format!("{manifest}\n")).unwrap();
    common::git(&dir, &["add", "-A"]);

    let (code, text) = house(&dir, &log, &["check", "--stage", "commit"], &[]);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("1 to fix, 1 excused"), "{text}");
    let lines = records(&log);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["excused"], 1);
    let tool = &lines[0]["tools"][0];
    assert_eq!(tool["name"], "cargo-lints");
    assert_eq!(tool["findings"], serde_json::json!({"R9": 1}));
    assert_eq!(tool["excused"], serde_json::json!({"R9": 1}));
}

#[test]
fn a_tool_with_nothing_excused_records_an_empty_map() {
    let dir = common::repo("record-excused-none");
    let log = common::fresh("record-excused-none-log");
    std::fs::write(dir.join(".house.toml"), "[tools]\ncommit = []\n").unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        format!("{PACKAGE}[lints.clippy]\nfloat_cmp = \"allow\"\n"),
    )
    .unwrap();
    common::git(&dir, &["add", "-A"]);

    let (code, text) = house(&dir, &log, &["check", "--stage", "commit"], &[]);
    assert_eq!(code, 1, "{text}");
    let lines = records(&log);
    assert_eq!(lines[0]["excused"], 0);
    assert_eq!(lines[0]["tools"][0]["excused"], serde_json::json!({}));
}
