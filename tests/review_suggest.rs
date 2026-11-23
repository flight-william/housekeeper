//! `house review`'s suggestions: what never fires, what looks noisy, the slow tool.
//! The fixture records are tests/fixtures/review (see tests/review.rs); the others are
//! written to temp folders.

#[path = "common/temp.rs"]
mod common;
mod review_support;

use review_support::{NOVEMBER, folder_with, review, review_in, section};
use std::path::PathBuf;

const SUGGESTIONS: &str = "suggestions (to weigh, not orders):";

#[test]
fn suggestions_name_what_looks_noisy_and_the_slow_tool() {
    // R8 and gitleaks find nothing, but four runs are too few to say they never fire.
    let (_, text) = review(&NOVEMBER);
    assert_eq!(
        section(&text, SUGGESTIONS),
        [
            "  suggestion: R3 looks noisy: 6 of 9 findings excused, 3 of 3 left to fix on places that failed before; fix that code, or tune the rule",
            "  suggestion: R4 looks noisy: 2 of 2 left to fix on places that failed before; fix that code, or tune the rule",
            "  suggestion: r3 looks noisy: 6 of 9 findings excused, 3 of 3 left to fix on places that failed before; fix that code, or tune the tool",
            "  suggestion: ruff looks noisy: 2 of 3 left to fix on places that failed before; fix that code, or tune the tool",
            "  suggestion: clippy takes most of the time: 17.0 of 23.1 s (74%); run it at push only, or hand it fewer files",
        ],
        "{text}"
    );
}

#[test]
fn a_rule_mostly_excused_looks_noisy_without_repeating() {
    let run = r#"{"start":"2026-11-02T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{"name":"r3","seconds":0.1,"findings":{"R3":1},"excused":{"R3":3}},{"name":"ruff","seconds":0.1,"findings":{},"excused":{}}],"findings":[{"rule":"R3","place":"src/a.py","tool_rule":"r3 file"}],"excused":3}"#;
    let log = folder_with("review-excused", &[run]);
    let (_, text) = review_in(&log, &log, &NOVEMBER);
    let lines = section(&text, SUGGESTIONS);
    assert!(
        lines.contains(&"  suggestion: R3 looks noisy: 3 of 4 findings excused; fix that code, or tune the rule"),
        "{text}"
    );
}

#[test]
fn nothing_to_suggest_says_so() {
    // One tool, one run, nothing excused or repeated.
    let run = r#"{"start":"2026-11-02T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{"name":"ruff","seconds":1.0,"findings":{"R4":1},"excused":{}}],"findings":[{"rule":"R4","place":"src/a.py","tool_rule":"ruff C901"}],"excused":0}"#;
    let log = folder_with("review-quiet", &[run]);
    let (_, text) = review_in(&log, &log, &NOVEMBER);
    assert!(text.ends_with("\nsuggestions: none\n"), "{text}");
}

/// `n` November runs of ruff and gitleaks with nothing found, after an October run
/// whose R4 finding makes the rule known.
fn quiet_runs(name: &str, n: usize) -> PathBuf {
    let october = r#"{"start":"2026-10-30T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{"name":"ruff","seconds":0.1,"findings":{"R4":1},"excused":{}}],"findings":[{"rule":"R4","place":"src/a.py","tool_rule":"ruff C901"}],"excused":0}"#;
    let quiet = r#"{"start":"2026-11-02T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{"name":"ruff","seconds":0.1,"findings":{},"excused":{}},{"name":"gitleaks","seconds":0.1,"findings":{},"excused":{}}],"findings":[],"excused":0}"#;
    let mut lines = vec![october];
    lines.extend(std::iter::repeat_n(quiet, n));
    folder_with(name, &lines)
}

#[test]
fn never_fires_needs_at_least_five_runs_in_the_period() {
    assert_eq!(house::review::NEVER_FIRES_MIN_RUNS, 5);
    let log = quiet_runs("review-four", 4);
    let (_, text) = review_in(&log, &log, &NOVEMBER);
    assert!(text.contains(", 4 runs: 4 commit\n"), "{text}");
    assert!(text.ends_with("\nsuggestions: none\n"), "{text}");
    let log = quiet_runs("review-five", 5);
    let (_, text) = review_in(&log, &log, &NOVEMBER);
    assert_eq!(
        section(&text, SUGGESTIONS),
        [
            "  suggestion: R4 never fires: no findings in 5 runs; check that a tool still checks it, or drop it",
            "  suggestion: gitleaks never fires: no findings in 5 runs; check that it is handed the files it should read",
            "  suggestion: ruff never fires: no findings in 5 runs; check that it is handed the files it should read",
        ],
        "{text}"
    );
}

#[test]
fn a_tool_is_judged_by_its_own_runs_a_rule_by_its_tools_runs() {
    // Five runs, gitleaks skipped on one: ruff, and R4 that ruff checks, have five;
    // gitleaks has four.
    let skipped = r#"{"start":"2026-11-03T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{"name":"ruff","seconds":0.0,"findings":{},"excused":{}},{"name":"gitleaks","seconds":0.0,"skipped":true,"findings":{},"excused":{}}],"findings":[],"excused":0}"#;
    let log = quiet_runs("review-tool-four", 4);
    let path = log.join("2026-11.jsonl");
    let text = std::fs::read_to_string(&path).unwrap() + skipped + "\n";
    std::fs::write(&path, text).unwrap();
    let (_, text) = review_in(&log, &log, &NOVEMBER);
    assert!(text.contains(", 5 runs: 5 commit\n"), "{text}");
    assert_eq!(
        section(&text, SUGGESTIONS),
        [
            "  suggestion: R4 never fires: no findings in 5 runs; check that a tool still checks it, or drop it",
            "  suggestion: ruff never fires: no findings in 5 runs; check that it is handed the files it should read",
        ],
        "{text}"
    );
}

/// `n` copies of `line` as November's records, in a fresh folder.
fn repeated_runs(name: &str, line: &str, n: usize) -> PathBuf {
    folder_with(name, &vec![line; n])
}

#[test]
fn a_tool_whose_old_records_excused_findings_is_not_said_to_never_fire() {
    // Before 0.3.0 a record kept only the run's excused total: these ten excuses
    // may be r3's, the run's only tool.
    let old = r#"{"start":"2026-11-02T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{"name":"r3","seconds":0.2,"findings":{}}],"findings":[],"excused":2}"#;
    let log = repeated_runs("review-old-excused", old, 5);
    let (_, text) = review_in(&log, &log, &NOVEMBER);
    assert!(
        text.contains("\n10 excused in records that do not say which rule"),
        "{text}"
    );
    assert!(text.ends_with("\nsuggestions: none\n"), "{text}");
}

#[test]
fn a_rule_whose_tool_never_could_check_is_not_said_to_never_fire() {
    // R2 is named only by semgrep's cannot-check: nothing was ever checked for it.
    let cannot = r#"{"start":"2026-11-02T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{"name":"semgrep","seconds":0.0,"findings":{},"cannot_check":"semgrep is not installed. fix: brew install semgrep","excused":{}}],"findings":[],"excused":0}"#;
    let log = repeated_runs("review-cannot-only", cannot, 5);
    let (_, text) = review_in(&log, &log, &NOVEMBER);
    assert!(
        text.contains("  R2: 0 findings, 0 excused, 5 cannot check\n"),
        "{text}"
    );
    assert!(text.ends_with("\nsuggestions: none\n"), "{text}");
}

#[test]
fn a_rule_counts_only_the_runs_its_tools_checked() {
    // Five runs, ruff skipped on one: R4, which only ruff names, was checked four times.
    let skipped = r#"{"start":"2026-11-03T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{"name":"ruff","seconds":0.0,"skipped":true,"findings":{},"excused":{}},{"name":"gitleaks","seconds":0.0,"findings":{},"excused":{}}],"findings":[],"excused":0}"#;
    let log = quiet_runs("review-rule-four", 4);
    let path = log.join("2026-11.jsonl");
    let text = std::fs::read_to_string(&path).unwrap() + skipped + "\n";
    std::fs::write(&path, text).unwrap();
    let (_, text) = review_in(&log, &log, &NOVEMBER);
    assert!(text.contains(", 5 runs: 5 commit\n"), "{text}");
    assert_eq!(
        section(&text, SUGGESTIONS),
        [
            "  suggestion: gitleaks never fires: no findings in 5 runs; check that it is handed the files it should read"
        ],
        "{text}"
    );
}
