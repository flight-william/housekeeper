//! R9 "nothing silenced": proves patterns/r9.yml on the planted fixtures in
//! tests/fixtures/semgrep_r9/. The scan runs only when semgrep is installed
//! (captured with semgrep 1.186.0); otherwise it prints a note and passes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn pattern_file() -> PathBuf {
    root().join("patterns/r9.yml")
}

fn fixtures() -> PathBuf {
    root().join("tests/fixtures/semgrep_r9")
}

/// The `- id:` values in the pattern file, in order.
fn rule_ids() -> Vec<String> {
    let text = fs::read_to_string(pattern_file()).expect("read patterns/r9.yml");
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("- id:"))
        .map(|id| id.trim().to_string())
        .collect()
}

#[test]
fn every_rule_has_the_r9_prefix_warning_message_and_fix() {
    let text = fs::read_to_string(pattern_file()).expect("read patterns/r9.yml");
    let ids = rule_ids();
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

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create dir");
    for entry in fs::read_dir(from).expect("read dir") {
        let entry = entry.expect("dir entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

fn semgrep_installed() -> bool {
    Command::new("semgrep")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Runs semgrep on a copy of the fixtures outside the repo (the repo's
/// .semgrepignore skips tests/fixtures/) and returns "rule path:line" lines.
fn scan_fixtures() -> Vec<String> {
    let work = std::env::temp_dir().join(format!("house-semgrep-r9-{}", std::process::id()));
    let target = work.join("target");
    let home = work.join("home");
    copy_dir(&fixtures(), &target);
    fs::remove_file(target.join("expected.txt")).expect("drop expected.txt from the copy");
    // Without a .semgrepignore, semgrep's built-in default skips tests/ folders.
    fs::write(target.join(".semgrepignore"), "# scan everything\n").expect("write ignore");
    fs::create_dir_all(&home).expect("create home");
    let output = Command::new("semgrep")
        .args(["scan", "--metrics=off", "--json", "--quiet", "--config"])
        .arg(pattern_file())
        .arg(".")
        .current_dir(&target)
        .env("HOME", &home)
        .env("SEMGREP_ENABLE_VERSION_CHECK", "0")
        .output()
        .expect("run semgrep");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("semgrep JSON");
    fs::remove_dir_all(&work).expect("remove work dir");
    assert_eq!(json["errors"], serde_json::json!([]), "semgrep errors");
    let mut found = Vec::new();
    for result in json["results"].as_array().expect("results") {
        let full_id = result["check_id"].as_str().expect("check_id");
        let id = full_id.rsplit('.').next().expect("rule id");
        let extra = &result["extra"];
        assert_eq!(extra["severity"], "WARNING", "{id} severity");
        assert!(
            extra["metadata"]["fix"].is_string(),
            "{id} has no metadata.fix"
        );
        let path = result["path"].as_str().expect("path");
        let line = result["start"]["line"].as_u64().expect("line");
        found.push((path.to_string(), line, id.to_string()));
    }
    found.sort();
    found
        .into_iter()
        .map(|(path, line, id)| format!("{id} {path}:{line}"))
        .collect()
}

fn expected() -> Vec<String> {
    fs::read_to_string(fixtures().join("expected.txt"))
        .expect("read expected.txt")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(str::to_string)
        .collect()
}

#[test]
fn semgrep_finds_every_planted_silencer_and_nothing_in_clean_code() {
    if !semgrep_installed() {
        eprintln!("note: semgrep is not installed; skipping the R9 pattern scan");
        return;
    }
    let found = scan_fixtures();
    assert_eq!(found, expected());
    for id in rule_ids() {
        assert!(
            found.iter().any(|f| f.starts_with(&format!("{id} "))),
            "rule {id} is not proven by any planted fixture"
        );
    }
    let clean = [
        "ContentView.swift",
        "CleanTests.swift",
        "test_clean.py",
        "clean.rs",
    ];
    for name in clean {
        assert!(
            !found.iter().any(|f| f.contains(name)),
            "{name} must give no finding"
        );
    }
}
