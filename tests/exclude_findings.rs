//! `[paths] exclude` reaches a tool that reads more than the files it is handed:
//! gitleaks reads the git history, so a finding on an excluded path is dropped
//! after the tool runs, at every stage and at `--all`.

mod common;

use common::{git, repo};
use house::adapter::{Adapter, CannotCheck, Command, Context, Output, Stage};
use house::adapters::gitleaks::Gitleaks;
use house::cli;
use house::finding::{Finding, Place};
use std::path::Path;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn places(report: &house::reply::Report) -> Vec<String> {
    assert!(report.cannots().is_empty(), "{}", report.text());
    report
        .findings()
        .iter()
        .map(|f| house::reply::place_text(&f.place).to_string())
        .collect()
}

/// Reports one finding on each of `PATHS`, whatever files it is handed, as a tool
/// that reads the history does.
struct History;

const PATHS: [&str; 3] = ["vendor/lib.c", "src/main.c", "vendored/x.c"];

impl Adapter for History {
    fn name(&self) -> &'static str {
        "history"
    }
    fn program(&self) -> &'static str {
        "true"
    }
    fn brew_formula(&self) -> &'static str {
        "coreutils"
    }
    fn command(&self, _: &Context) -> Option<Command> {
        Some(Command {
            program: "true".into(),
            args: Vec::new(),
        })
    }
    fn parse(&self, _: &Context, _: &Output) -> Result<Vec<Finding>, CannotCheck> {
        let finding = |file: &str| Finding {
            rule: "R2".into(),
            place: Place::File(file.into()),
            line: Some(1),
            what: "found".into(),
            fix: "none".into(),
            tool_rule: "history".into(),
            measure: None,
        };
        let mut found: Vec<Finding> = PATHS.iter().map(|p| finding(p)).collect();
        found.push(Finding {
            place: Place::Repo,
            ..finding("")
        });
        Ok(found)
    }
}

#[test]
fn a_finding_on_an_excluded_path_is_dropped() {
    let root = repo("exclude-findings");
    write(&root, ".house.toml", "[paths]\nexclude = [\"vendor/\"]\n");
    write(&root, "src/main.c", "int main;\n");
    git(&root, &["add", "-A"]);
    let report = cli::check(&root, Stage::Commit, vec![Box::new(History)]);
    assert_eq!(places(&report), ["src/main.c", "vendored/x.c", "repo"]);
}

fn gitleaks_installed() -> bool {
    let found = std::process::Command::new("gitleaks")
        .arg("version")
        .output()
        .is_ok();
    if !found {
        eprintln!("note: gitleaks is not installed; skipping the run on a repo");
    }
    found
}

/// A committed vendored file holding a token-shaped string, scanned by `--all`.
fn leak_report(config: &str) -> Vec<String> {
    let root = repo(&format!("exclude-gitleaks-{}", config.len()));
    write(&root, ".house.toml", config);
    // SYNTHETIC, NOT A CREDENTIAL: a ghp_-shaped string generated at run time from a
    // fixed formula (as in tests/gitleaks.rs), never issued by anyone.
    let alphabet: Vec<char> = ('a'..='z').chain('A'..='Z').chain('0'..='9').collect();
    let body: String = (0..36).map(|i| alphabet[(i * 17 + 5) % 62]).collect();
    write(
        &root,
        "vendor/blob.txt",
        &format!("token = \"ghp_{body}\"\n"),
    );
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "base"]);
    places(&cli::check_all(&root, vec![Box::new(Gitleaks)]))
}

#[test]
fn gitleaks_reports_nothing_in_an_excluded_vendored_file() {
    if !gitleaks_installed() {
        return;
    }
    assert_eq!(
        leak_report("[tools]\npush = [\"gitleaks\"]\n"),
        ["vendor/blob.txt"]
    );
    let excluded = "[tools]\npush = [\"gitleaks\"]\n[paths]\nexclude = [\"vendor/\"]\n";
    assert!(leak_report(excluded).is_empty());
}
