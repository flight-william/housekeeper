//! R8 "deterministic tests": proves patterns/r8.yml on the planted fixtures in
//! tests/fixtures/semgrep_r8/. The scan runs only when semgrep is installed
//! (captured with semgrep 1.186.0); otherwise it prints a note and passes.

mod semgrep_common;

use semgrep_common::{Hit, fixtures, pattern_file, rule_ids, scan, semgrep_installed};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

fn patterns() -> PathBuf {
    pattern_file("r8.yml")
}

fn fixture() -> PathBuf {
    fixtures("semgrep_r8")
}

/// One scan for the whole file; None when semgrep is not installed.
fn found() -> Option<&'static [Hit]> {
    static FOUND: OnceLock<Option<Vec<Hit>>> = OnceLock::new();
    FOUND
        .get_or_init(|| {
            if semgrep_installed() {
                Some(scan(&fixture(), &patterns(), "r8"))
            } else {
                eprintln!("note: semgrep is not installed; skipping the R8 pattern scan");
                None
            }
        })
        .as_deref()
}

/// The wrapper that turns a tests/ file into the inline module of the same code.
const INLINE_HEAD: &str = "#[cfg(test)]\nmod tests {\n";
const INLINE_TAIL: &str = "}\n";
const INLINE_OFFSET: u64 = 2;

/// The inline-module twin of a rule that reads tests/ files.
fn twin(rule: &str) -> Option<String> {
    if rule == "test-machine-path" {
        return Some("rust-test-mod-machine-path".to_string());
    }
    let what = rule.strip_prefix("rust-test-")?;
    (!what.starts_with("mod-")).then(|| format!("rust-test-mod-{what}"))
}

#[test]
fn every_rule_is_r8_with_a_warning_message_and_fix() {
    let text = fs::read_to_string(patterns()).expect("read patterns/r8.yml");
    let ids = rule_ids(&patterns());
    assert!(!ids.is_empty(), "no rules in patterns/r8.yml");
    for id in &ids {
        // The adapter reports an id that starts with r9- as R9.
        assert!(
            !id.starts_with("r9-"),
            "rule id {id} would be reported as R9"
        );
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
fn semgrep_finds_every_planted_case_and_nothing_in_clean_code() {
    let Some(found) = found() else { return };
    let lines: Vec<String> = found.iter().map(Hit::text).collect();
    assert_eq!(lines, semgrep_common::expected(&fixture()));
    semgrep_common::assert_every_rule_proven(&rule_ids(&patterns()), found);
    // Clean test code, and production code outside every test path.
    let clean = [
        "python/tests/test_clean.py",
        "python/src/app.py",
        "swift/AppTests/CleanTests.swift",
        "swift/App/Clock.swift",
        "rust/tests/clean.rs",
        "rust/src/inline_clean.rs",
        "rust/src/prod.rs",
    ];
    for name in clean {
        assert!(
            !found.iter().any(|f| f.path == name),
            "{name} must give no finding"
        );
    }
}

/// Each rule's near miss: the call inside a string or a comment, a seeded or
/// fixed value, a stubbed network, HOME set for a child, a path only mentioned,
/// or the same call in production code.
#[test]
fn every_rule_has_a_not_matched_case() {
    let Some(found) = found() else { return };
    semgrep_common::assert_every_rule_has_a_not_matched_case(
        &fixture(),
        &rule_ids(&patterns()),
        found,
    );
}

/// Each `rust-test-mod-*` rule repeats its tests/ rule's patterns for an inline
/// `mod tests`. The inline fixtures are the tests/ fixtures wrapped in that
/// module, and each twin must hit exactly the lines its tests/ rule hits.
#[test]
fn inline_test_module_rules_hit_what_their_tests_folder_rules_hit() {
    let ids = rule_ids(&patterns());
    for id in ids.iter().filter(|id| id.starts_with("rust-test-mod-")) {
        assert!(
            ids.iter()
                .any(|base| twin(base).as_deref() == Some(id.as_str())),
            "{id} has no tests/ rule"
        );
    }
    let rust = fixture().join("rust");
    for name in ["planted", "clean"] {
        let folder = fs::read_to_string(rust.join(format!("tests/{name}.rs"))).expect("tests/");
        let inline = fs::read_to_string(rust.join(format!("src/inline_{name}.rs"))).expect("src/");
        assert_eq!(
            inline,
            format!("{INLINE_HEAD}{folder}{INLINE_TAIL}"),
            "src/inline_{name}.rs must be tests/{name}.rs inside `mod tests`"
        );
    }
    let Some(found) = found() else { return };
    let mut from_folder: Vec<(String, u64)> = found
        .iter()
        .filter(|f| f.path == "rust/tests/planted.rs")
        .map(|f| {
            (
                twin(&f.rule).expect("a rule with a twin"),
                f.line + INLINE_OFFSET,
            )
        })
        .collect();
    let mut from_inline: Vec<(String, u64)> = found
        .iter()
        .filter(|f| f.path == "rust/src/inline_planted.rs")
        .map(|f| (f.rule.clone(), f.line))
        .collect();
    from_folder.sort();
    from_inline.sort();
    assert!(!from_folder.is_empty(), "no hit in rust/tests/planted.rs");
    assert_eq!(from_inline, from_folder);
}
