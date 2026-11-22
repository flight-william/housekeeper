//! `house check --all`: every tool on every tracked file, the push stage's tools and
//! tests, and the only run that reports stale excuses.

mod common;
mod wire_run;

use house::adapter::{Adapter, CannotCheck, Command, Context, Output, Stage};
use house::cli;
use house::excuses::Date;
use house::finding::{Finding, Place};
use house::reply::{Report, ToolRun};
use std::path::{Path, PathBuf};
use wire_run::{house, records};

fn parse(args: &[&str]) -> Result<cli::Cmd, String> {
    cli::parse(&args.iter().map(|a| a.to_string()).collect::<Vec<_>>())
}

#[test]
fn check_all_parses_alone_or_with_json_and_not_with_a_stage() {
    assert_eq!(
        parse(&["check", "--all"]),
        Ok(cli::Cmd::CheckAll { json: false })
    );
    assert_eq!(
        parse(&["check", "--json", "--all"]),
        Ok(cli::Cmd::CheckAll { json: true })
    );
    assert!(parse(&["check", "--all", "--stage", "commit"]).is_err());
    assert!(cli::USAGE.contains("--all"), "{}", cli::USAGE);
}

/// An adapter whose one finding names the files and stage it was given.
struct Lister;

impl Adapter for Lister {
    fn name(&self) -> &'static str {
        "lister"
    }
    fn program(&self) -> &'static str {
        "true"
    }
    fn brew_formula(&self) -> &'static str {
        "coreutils"
    }
    fn command(&self, ctx: &Context) -> Option<Command> {
        Some(Command {
            program: "echo".into(),
            args: vec![format!("{:?} {}", ctx.stage, ctx.files.join(","))],
        })
    }
    fn parse(&self, _: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        Ok(vec![Finding {
            rule: "R2".into(),
            place: Place::Repo,
            line: None,
            what: out.stdout.trim().into(),
            fix: "x".into(),
            tool_rule: "lister".into(),
            measure: None,
        }])
    }
}

/// A repo with `toml` as `.house.toml` and `files` committed; nothing staged.
fn committed(name: &str, toml: &str, files: &[&str]) -> PathBuf {
    let dir = common::repo(name);
    std::fs::write(dir.join(".house.toml"), toml).unwrap();
    for f in files {
        let path = dir.join(f);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "x\n").unwrap();
    }
    common::git(&dir, &["add", "-A"]);
    common::git(&dir, &["commit", "-q", "--no-verify", "-m", "files"]);
    dir
}

fn listed(root: &Path) -> String {
    let r = cli::check_all(root, vec![Box::new(Lister)]);
    r.findings()[0].what.clone()
}

#[test]
fn check_all_hands_every_tracked_file_to_the_tools_with_nothing_staged() {
    let root = committed(
        "all-tracked",
        "[paths]\nexclude = [\"vendor/\"]\n",
        &["a.py", "src/b.rs", "vendor/c.py"],
    );
    assert_eq!(listed(&root), "Push .house.toml,a.py,src/b.rs");
}

#[test]
fn check_all_skips_tracked_files_gone_from_the_working_copy_and_links() {
    let root = committed("all-gone", "", &["a.py", "b.py"]);
    std::fs::remove_file(root.join("b.py")).unwrap();
    std::os::unix::fs::symlink("a.py", root.join("link.py")).unwrap();
    common::git(&root, &["add", "link.py"]);
    assert_eq!(listed(&root), "Push .house.toml,a.py");
}

#[test]
fn check_all_runs_the_push_tools_and_the_push_tests() {
    let toml = "[tools]\ncommit = []\npush = [\"lister\"]\n[tests]\ncommit = [[\"true\"]]\npush = [[\"false\"]]\n";
    let root = committed("all-push", toml, &["a.py"]);
    let r = cli::check_all(&root, vec![Box::new(Lister)]);
    let whats: Vec<&str> = r.findings().iter().map(|f| f.what.as_str()).collect();
    assert_eq!(
        whats,
        ["Push .house.toml,a.py", "test command failed: false"]
    );
}

const LIVE: &str = "[[excuse]]\nrule = \"R3\"\nfile = \"src/old.py\"\ndeadline = \"2099-12-31\"\nreason = \"split in v2\"\n";
const STALE: &str = "[[excuse]]\nrule = \"R4\"\nfile = \"src/gone.py\"\ndeadline = \"2099-12-31\"\nreason = \"was refactored\"\n";
const EXPIRED_UNUSED: &str = "[[excuse]]\nrule = \"R4\"\nfile = \"src/done.py\"\ndeadline = \"2026-11-01\"\nreason = \"was refactored\"\n";

fn report(findings: Vec<Finding>, cannot: Option<CannotCheck>) -> Report {
    Report {
        stage: Stage::Push,
        seconds: 0.1,
        tools: vec![ToolRun {
            name: "t".into(),
            seconds: 0.1,
            exit: Some(1),
            skipped: false,
            findings,
            cannot,
            unread: Vec::new(),
        }],
    }
}

fn too_long(file: &str) -> Finding {
    Finding {
        rule: "R3".into(),
        place: Place::File(file.into()),
        line: Some(3),
        what: "too long".into(),
        fix: "split it".into(),
        tool_rule: "r3".into(),
        measure: None,
    }
}

fn day() -> Date {
    Date::parse("2026-11-23").unwrap()
}

#[test]
fn a_full_run_reports_a_stale_excuse_and_keeps_the_used_one() {
    let root = committed("all-stale", "", &[]);
    std::fs::write(root.join(".house-excuses.toml"), format!("{LIVE}{STALE}")).unwrap();
    common::git(&root, &["add", "-A"]);
    common::git(&root, &["commit", "-q", "--no-verify", "-m", "list"]);
    let mut r = report(vec![too_long("src/old.py")], None);
    assert_eq!(cli::excuse_all(&mut r, &root, day(), Some("HEAD")), 1);
    let whats: Vec<&str> = r.findings().iter().map(|f| f.what.as_str()).collect();
    assert_eq!(whats, ["stale excuse: R4 src/gone.py matches no finding"]);
}

#[test]
fn a_full_run_does_not_call_an_expired_entry_stale_too() {
    let root = committed("all-expired", "", &[]);
    std::fs::write(root.join(".house-excuses.toml"), EXPIRED_UNUSED).unwrap();
    common::git(&root, &["add", "-A"]);
    common::git(&root, &["commit", "-q", "--no-verify", "-m", "list"]);
    let mut r = report(vec![], None);
    cli::excuse_all(&mut r, &root, day(), Some("HEAD"));
    let whats: Vec<&str> = r.findings().iter().map(|f| f.what.as_str()).collect();
    assert_eq!(whats.len(), 1, "{whats:?}");
    assert!(whats[0].starts_with("excuse expired"), "{whats:?}");
}

#[test]
fn a_full_run_with_a_tool_that_could_not_check_judges_no_staleness() {
    let root = committed("all-stale-cannot", "", &[]);
    std::fs::write(root.join(".house-excuses.toml"), STALE).unwrap();
    common::git(&root, &["add", "-A"]);
    common::git(&root, &["commit", "-q", "--no-verify", "-m", "list"]);
    let cannot = CannotCheck {
        what: "ruff is not installed".into(),
        fix: "brew install ruff".into(),
    };
    let mut r = report(vec![], Some(cannot));
    cli::excuse_all(&mut r, &root, day(), Some("HEAD"));
    assert!(!r.text().contains("stale excuse"), "{}", r.text());
}

#[test]
fn the_binary_reports_stale_excuses_at_all_but_not_at_commit() {
    let root = committed("all-binary", "[tools]\ncommit = []\npush = []\n", &[]);
    std::fs::write(root.join(".house-excuses.toml"), STALE).unwrap();
    common::git(&root, &["add", "-A"]);
    common::git(&root, &["commit", "-q", "--no-verify", "-m", "list"]);
    let log = common::fresh("all-binary-log");
    let (code, text) = house(&root, &log, &["check", "--all"], &[]);
    assert_eq!(code, 1, "{text}");
    assert!(text.starts_with("house all: 1 to fix"), "{text}");
    assert!(text.contains("stale excuse: R4 src/gone.py"), "{text}");
    let (code, json) = house(&root, &log, &["check", "--all", "--json"], &[]);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!((code, value["stage"].as_str()), (1, Some("all")), "{json}");
    std::fs::write(root.join("a.txt"), "a\n").unwrap();
    common::git(&root, &["add", "a.txt"]);
    let (code, text) = house(&root, &log, &["check", "--stage", "commit"], &[]);
    assert_eq!(code, 0, "{text}");
    let stages: Vec<String> = records(&log)
        .iter()
        .map(|r| r["stage"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(stages, ["all", "all", "commit"]);
}
