//! Entries whose words overlap: one finding can fit two entries of the same rule and
//! file. Each finding goes to the most specific entry that fits, and an entry earlier
//! in the file must not use up findings a later entry was written for.

use house::excuses::{Date, Excuse, apply, parse};
use house::finding::{Finding, Place};

fn found(rule: &str, file: &str, what: &str) -> Finding {
    Finding {
        rule: rule.to_string(),
        place: Place::File(file.to_string()),
        line: None,
        what: what.to_string(),
        fix: "fix it".to_string(),
        tool_rule: "tool rule".to_string(),
        measure: None,
    }
}

fn today() -> Date {
    Date::parse("2026-11-22").unwrap()
}

fn list(text: &str) -> Vec<Excuse> {
    parse(text).unwrap()
}

fn stale(out: &house::excuses::Applied) -> Vec<&str> {
    out.problems.iter().map(|p| p.what.as_str()).collect()
}

/// A formatter whose every message starts with the same word, and a second entry
/// for one kind of its findings, written later in the file.
const FORMAT_THEN_LINE: &str = r#"
[[excuse]]
rule = "R1"
file = "Sources/View.swift"
symbol = "format"
count = 2
deadline = "2026-12-31"
reason = "never formatted; the format commit removes these"

[[excuse]]
rule = "R1"
file = "Sources/View.swift"
symbol = "LineLength"
count = 1
deadline = "2026-12-31"
reason = "one long line"
"#;

#[test]
fn an_earlier_entry_whose_word_overlaps_does_not_starve_a_later_one() {
    let findings = vec![
        found(
            "R1",
            "Sources/View.swift",
            "format: [LineLength] line is too long",
        ),
        found(
            "R1",
            "Sources/View.swift",
            "format: [Indentation] replace 2 spaces with 4",
        ),
        found(
            "R1",
            "Sources/View.swift",
            "format: [Indentation] replace 2 spaces with 4",
        ),
    ];
    let out = apply(findings, &list(FORMAT_THEN_LINE), today());
    assert_eq!(out.excused.len(), 3, "to fix: {:?}", out.to_fix);
    assert!(out.to_fix.is_empty());
    assert!(stale(&out).is_empty(), "{:?}", stale(&out));
}

/// A whole-file entry first, then one naming the function.
const FILE_THEN_SYMBOL: &str = r#"
[[excuse]]
rule = "R3"
file = "old.py"
deadline = "2026-12-31"
reason = "the file is long"

[[excuse]]
rule = "R3"
file = "old.py"
symbol = "run"
deadline = "2026-12-31"
reason = "run is long"
"#;

#[test]
fn a_finding_goes_to_the_entry_that_names_it_over_one_that_names_only_the_file() {
    let findings = vec![found(
        "R3",
        "old.py",
        "function run has 90 lines, limit is 60",
    )];
    let out = apply(findings, &list(FILE_THEN_SYMBOL), today());
    assert_eq!(out.excused.len(), 1);
    assert_eq!(
        stale(&out),
        ["stale excuse: R3 old.py matches no finding"],
        "the entry naming `run` takes it; the file-only entry is left unused"
    );
}

const SHORT_THEN_LONG: &str = r#"
[[excuse]]
rule = "R2"
file = "a.py"
symbol = "E501"
deadline = "2026-12-31"
reason = "a long line"

[[excuse]]
rule = "R2"
file = "a.py"
symbol = "E501 line too long"
deadline = "2026-12-31"
reason = "the same long line, named in full"
"#;

#[test]
fn of_two_named_entries_the_longer_name_takes_the_finding() {
    let findings = vec![found("R2", "a.py", "E501 line too long (120 > 100)")];
    let out = apply(findings, &list(SHORT_THEN_LONG), today());
    assert_eq!(out.excused.len(), 1);
    assert_eq!(
        stale(&out),
        ["stale excuse: R2 a.py E501 matches no finding"]
    );
}

#[test]
fn a_full_specific_entry_passes_the_next_finding_to_a_general_one() {
    let text = FILE_THEN_SYMBOL.replace(
        "reason = \"the file is long\"",
        "reason = \"the file is long\"\ncount = 1",
    );
    let findings = vec![
        found("R3", "old.py", "function run has 90 lines, limit is 60"),
        found("R3", "old.py", "function run has 70 lines, limit is 60"),
        found("R3", "old.py", "function run has 65 lines, limit is 60"),
    ];
    let out = apply(findings, &list(&text), today());
    assert_eq!(out.excused.len(), 2, "one entry each, count 1 each");
    assert_eq!(out.to_fix.len(), 1);
    assert!(stale(&out).is_empty());
}

#[test]
fn findings_keep_their_order_whichever_entry_excuses_them() {
    let a = found("R3", "old.py", "file has 500 lines, limit is 400");
    let b = found("R3", "old.py", "function run has 90 lines, limit is 60");
    let text = FILE_THEN_SYMBOL.replace(
        "reason = \"the file is long\"",
        "reason = \"the file is long\"\ncount = 1",
    );
    let out = apply(vec![a.clone(), b.clone()], &list(&text), today());
    assert_eq!(out.excused, [a, b]);
}
