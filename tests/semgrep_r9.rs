//! R9 "nothing silenced": proves patterns/r9.yml on the planted fixtures in
//! tests/fixtures/semgrep_r9/. The scan runs only when semgrep is installed
//! (captured with semgrep 1.186.0); otherwise it prints a note and passes.

mod semgrep_common;

use semgrep_common::{Hit, fixtures, pattern_file, rule_ids, scan, semgrep_installed};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

fn patterns() -> PathBuf {
    pattern_file("r9.yml")
}

fn fixture() -> PathBuf {
    fixtures("semgrep_r9")
}

/// One scan for the whole file; None when semgrep is not installed.
fn found() -> Option<&'static [Hit]> {
    static FOUND: OnceLock<Option<Vec<Hit>>> = OnceLock::new();
    FOUND
        .get_or_init(|| {
            if semgrep_installed() {
                Some(scan(&fixture(), &patterns(), "r9"))
            } else {
                eprintln!("note: semgrep is not installed; skipping the R9 pattern scan");
                None
            }
        })
        .as_deref()
}

#[test]
fn every_rule_has_the_r9_prefix_warning_message_and_fix() {
    let text = fs::read_to_string(patterns()).expect("read patterns/r9.yml");
    let ids = rule_ids(&patterns());
    assert!(!ids.is_empty(), "no rules in patterns/r9.yml");
    for id in &ids {
        assert!(id.starts_with("r9-"), "rule id {id} lacks the r9- prefix");
    }
    let count = |needle: &str| {
        text.lines()
            .filter(|l| l.trim().starts_with(needle))
            .count()
    };
    assert_eq!(
        count("severity: WARNING"),
        ids.len(),
        "one WARNING per rule"
    );
    assert_eq!(count("message:"), ids.len(), "one message per rule");
    assert_eq!(count("fix:"), ids.len(), "one metadata.fix per rule");
}

#[test]
fn semgrep_finds_every_planted_silencer_and_nothing_in_clean_code() {
    let Some(found) = found() else { return };
    let lines: Vec<String> = found.iter().map(Hit::text).collect();
    assert_eq!(lines, semgrep_common::expected(&fixture()));
    semgrep_common::assert_every_rule_proven(&rule_ids(&patterns()), found);
    let clean = [
        "ContentView.swift",
        "CleanTests.swift",
        "CleanupTests.swift",
        "RemoveTests.swift",
        "test_clean.py",
        "clean.rs",
        "cleanup.rs",
        "exempt.rs",
        "clean.sh",
    ];
    for name in clean {
        assert!(
            !found.iter().any(|f| f.path.ends_with(name)),
            "{name} must give no finding"
        );
    }
}

/// Each rule's near miss: a silencer's text inside a string or a comment, a
/// SwiftUI `.disabled`, and cleanup code (Drop, defer, deinit, teardown).
#[test]
fn every_rule_has_a_not_matched_case() {
    let Some(found) = found() else { return };
    semgrep_common::assert_every_rule_has_a_not_matched_case(
        &fixture(),
        &rule_ids(&patterns()),
        found,
    );
}
