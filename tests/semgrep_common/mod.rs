//! Shared by tests/semgrep_r8.rs and tests/semgrep_r9.rs: runs semgrep with one
//! house pattern file on a copy of a planted fixture folder, and reads the
//! folder's expected.txt and not_matched.txt.
//!
//! The pattern files come from `$HOUSE_PATTERNS_DIR` when it is set (as for the
//! adapter), else this crate's patterns/, so an older pattern file can be run
//! against the same fixtures to see a test fail.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The two files a fixture folder holds next to its planted code; never scanned.
const NOTES: [&str; 2] = ["expected.txt", "not_matched.txt"];

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn pattern_file(name: &str) -> PathBuf {
    let dir = std::env::var_os("HOUSE_PATTERNS_DIR")
        .map_or_else(|| root().join("patterns"), PathBuf::from);
    dir.join(name)
}

pub fn fixtures(name: &str) -> PathBuf {
    root().join("tests/fixtures").join(name)
}

/// The `- id:` values in a pattern file, in order.
pub fn rule_ids(pattern: &Path) -> Vec<String> {
    let text = fs::read_to_string(pattern).expect("read the pattern file");
    text.lines()
        .filter_map(|line| line.strip_prefix("  - id:"))
        .map(|id| id.trim().to_string())
        .collect()
}

pub fn semgrep_installed() -> bool {
    Command::new("semgrep")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// One result: the rule id without its config prefix, the path inside the
/// fixture folder, and the 1-based line.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hit {
    pub path: String,
    pub line: u64,
    pub rule: String,
}

impl Hit {
    /// "rule path:line", the form expected.txt holds.
    pub fn text(&self) -> String {
        format!("{} {}:{}", self.rule, self.path, self.line)
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create dir");
    for entry in fs::read_dir(from).expect("read dir") {
        let entry = entry.expect("dir entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else if !NOTES.iter().any(|n| entry.file_name() == *n) {
            fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

/// Runs semgrep with `pattern` on a copy of `fixture` outside the repo (the
/// repo's .semgrepignore skips tests/fixtures/), checks that every result is a
/// WARNING with a metadata.fix, and returns the hits sorted by path and line.
pub fn scan(fixture: &Path, pattern: &Path, tag: &str) -> Vec<Hit> {
    let work = std::env::temp_dir().join(format!("house-semgrep-{tag}-{}", std::process::id()));
    let target = work.join("target");
    let home = work.join("home");
    if work.exists() {
        fs::remove_dir_all(&work).expect("clear the work dir");
    }
    copy_dir(fixture, &target);
    // Without a .semgrepignore, semgrep's built-in default skips tests/ folders.
    fs::write(target.join(".semgrepignore"), "# scan everything\n").expect("write ignore");
    fs::create_dir_all(&home).expect("create home");
    let output = Command::new("semgrep")
        .args(["scan", "--metrics=off", "--json", "--quiet", "--config"])
        .arg(pattern)
        .arg(".")
        .current_dir(&target)
        .env("HOME", &home)
        .env("SEMGREP_ENABLE_VERSION_CHECK", "0")
        .output()
        .expect("run semgrep");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("semgrep JSON");
    fs::remove_dir_all(&work).expect("remove work dir");
    // Semgrep 1.186.0 cannot parse Swift's #expect and #require macros: it reads
    // the rest of such a file and warns that it read it in part. That warning is
    // Semgrep's limit, not a broken rule or fixture; any other error fails.
    let errors: Vec<&serde_json::Value> = json["errors"]
        .as_array()
        .expect("errors")
        .iter()
        .filter(|e| !swift_macro_partial_parse(e))
        .collect();
    assert!(errors.is_empty(), "semgrep errors: {errors:?}");
    let mut found = Vec::new();
    for result in json["results"].as_array().expect("results") {
        let full_id = result["check_id"].as_str().expect("check_id");
        let rule = full_id.rsplit('.').next().expect("rule id").to_string();
        let extra = &result["extra"];
        assert_eq!(extra["severity"], "WARNING", "{rule} severity");
        assert!(
            extra["metadata"]["fix"].is_string(),
            "{rule} has no metadata.fix"
        );
        found.push(Hit {
            path: result["path"].as_str().expect("path").to_string(),
            line: result["start"]["line"].as_u64().expect("line"),
            rule,
        });
    }
    found.sort();
    found
}

/// A warning that a Swift file was read in part, at a `#expect` or `#require`.
fn swift_macro_partial_parse(error: &serde_json::Value) -> bool {
    let message = error["message"].as_str().unwrap_or_default();
    error["type"][0] == "PartialParsing"
        && error["level"] == "warn"
        && (message.contains("`#expect` was unexpected")
            || message.contains("`#require` was unexpected"))
}

/// The non-comment lines of the fixture folder's expected.txt.
pub fn expected(fixture: &Path) -> Vec<String> {
    fs::read_to_string(fixture.join("expected.txt"))
        .expect("read expected.txt")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// Every rule's planted case must be found: each id gives at least one hit.
pub fn assert_every_rule_proven(ids: &[String], found: &[Hit]) {
    for id in ids {
        assert!(
            found.iter().any(|f| &f.rule == id),
            "rule {id} is not proven by any planted fixture"
        );
    }
}

/// Every rule names a not-matched case in not_matched.txt: a line of
/// `<rule> <path>: <the whole line, trimmed>`. That line must be in the file
/// exactly once and give no finding of any rule.
pub fn assert_every_rule_has_a_not_matched_case(fixture: &Path, ids: &[String], found: &[Hit]) {
    let text = fs::read_to_string(fixture.join("not_matched.txt")).expect("read not_matched.txt");
    let cases: Vec<(&str, &str, &str)> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let (rule, rest) = l.split_once(' ').expect("rule then path");
            let (path, line) = rest.split_once(": ").expect("path: line");
            (rule, path, line)
        })
        .collect();
    for id in ids {
        assert!(
            cases.iter().any(|(rule, _, _)| rule == id),
            "rule {id} has no not-matched case in not_matched.txt"
        );
    }
    for (rule, path, wanted) in cases {
        assert!(ids.iter().any(|id| id == rule), "{rule} is not a rule");
        let source = fs::read_to_string(fixture.join(path)).expect("read the not-matched file");
        let at: Vec<u64> = (1..)
            .zip(source.lines())
            .filter(|(_, l)| l.trim() == wanted)
            .map(|(n, _)| n)
            .collect();
        assert_eq!(at.len(), 1, "{rule}: `{wanted}` must be one line of {path}");
        assert!(
            !found.iter().any(|f| f.path == path && f.line == at[0]),
            "{rule}: {path}:{} must give no finding",
            at[0]
        );
    }
}
