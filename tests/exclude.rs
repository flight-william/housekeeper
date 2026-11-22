//! `.house.toml` `[paths] exclude`: excluded files never reach the adapters, at
//! commit or at push; the policy checks (approval) still see every change.

mod common;

use common::{git, repo};
use house::adapter::{Adapter, CannotCheck, Command, Context, Output, Stage};
use house::finding::{Finding, Place};
use house::{cli, config, files};
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

/// Gives one finding per file it is handed, so the report shows what it saw.
struct Seen;

impl Adapter for Seen {
    fn name(&self) -> &'static str {
        "seen"
    }
    fn program(&self) -> &'static str {
        "true"
    }
    fn brew_formula(&self) -> &'static str {
        "coreutils"
    }
    fn command(&self, ctx: &Context) -> Option<Command> {
        (!ctx.files.is_empty()).then(|| Command {
            program: "true".into(),
            args: Vec::new(),
        })
    }
    fn parse(&self, ctx: &Context, _out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        Ok(ctx
            .files
            .iter()
            .map(|f| Finding {
                rule: "R1".into(),
                place: Place::File(f.clone()),
                line: None,
                what: "seen".into(),
                fix: "none".into(),
                tool_rule: "seen".into(),
                measure: None,
            })
            .collect())
    }
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn seen_files(root: &Path) -> Vec<String> {
    let report = cli::check(root, Stage::Commit, vec![Box::new(Seen)]);
    assert!(report.cannots().is_empty(), "{}", report.text());
    report
        .findings()
        .iter()
        .map(|f| house::reply::place_text(&f.place).to_string())
        .collect()
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn config_reads_paths_exclude_and_defaults_to_none() {
    let cfg = config::parse("[paths]\nexclude = [\"tests/fixtures/\", \"vendor\"]\n").unwrap();
    assert_eq!(cfg.paths.exclude, ["tests/fixtures/", "vendor"]);
    assert!(config::parse("").unwrap().paths.exclude.is_empty());
    assert!(config::parse("[paths]\nexclude = \"vendor\"\n").is_err());
}

#[test]
fn an_entry_excludes_by_leading_folders_with_globs() {
    let exclude = strings(&["tests/fixtures/", "vendor", "gen/*_pb2.py", "**/build/"]);
    let kept = files::without_excluded(
        strings(&[
            "tests/fixtures/semgrep/planted/a.py",
            "tests/fixtures.rs",
            "tests/a.rs",
            "vendor",
            "vendor/lib/x.py",
            "vendorx/y.py",
            "gen/a_pb2.py",
            "gen/a.py",
            "app/build/out.rs",
            "build.rs",
        ]),
        &exclude,
    );
    assert_eq!(
        kept,
        [
            "tests/fixtures.rs",
            "tests/a.rs",
            "vendorx/y.py",
            "gen/a.py",
            "build.rs"
        ]
    );
}

#[test]
fn no_exclude_keeps_every_file() {
    let all = strings(&["a.py", "tests/fixtures/b.py"]);
    assert_eq!(files::without_excluded(all.clone(), &[]), all);
}

#[test]
fn commit_drops_excluded_files_before_the_adapters() {
    let root = repo("exclude-commit");
    write(
        &root,
        ".house.toml",
        "[paths]\nexclude = [\"tests/fixtures/\"]\n",
    );
    write(&root, "src/a.py", "x = 1\n");
    write(&root, "tests/fixtures/planted.py", "x=1\n");
    git(&root, &["add", "-A"]);
    assert_eq!(seen_files(&root), [".house.toml", "src/a.py"]);
}

#[test]
fn an_unstaged_change_to_an_excluded_file_does_not_stop_the_commit() {
    let root = repo("exclude-unstaged");
    write(
        &root,
        ".house.toml",
        "[paths]\nexclude = [\"tests/fixtures/\"]\n",
    );
    write(&root, "tests/fixtures/planted.py", "x=1\n");
    git(&root, &["add", "-A"]);
    write(&root, "tests/fixtures/planted.py", "x=2\n");
    assert_eq!(seen_files(&root), [".house.toml"]);
}

#[test]
fn approval_still_sees_an_excluded_protected_file() {
    let root = repo("exclude-approval");
    let cfg = "[paths]\nexclude = [\"policy/\"]\n[protected]\nextra = [\"policy/**\"]\n";
    write(&root, ".house.toml", cfg);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "base"]);
    write(&root, "policy/rules.yml", "a: 1\n");
    git(&root, &["add", "-A"]);
    write(&root, "msg.txt", "policy: a rule\n");
    let findings = cli::judge_message(&root, &root.join("msg.txt")).unwrap();
    assert!(
        findings
            .iter()
            .any(|f| f.tool_rule == "approval"
                && f.place == Place::File("policy/rules.yml".into())),
        "{findings:?}"
    );
}

/// A pushed 401-line Python file under an excluded folder: r3 (python3) is not
/// even run for it, so the push is clean; the same push without the entry is R3.
fn push_with(config: &str) -> std::process::Output {
    let root = repo(&format!("exclude-push-{}", config.len()));
    write(&root, ".house.toml", config);
    write(&root, "vendor/big.py", &"x = 1\n".repeat(401));
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "base"]);
    let head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()
        .unwrap();
    let head = String::from_utf8(head.stdout).unwrap();
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "push"])
        .current_dir(&root)
        .env("HOUSE_LOG_DIR", root.join("log"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let zero = "0".repeat(40);
    let line = format!("refs/heads/f {} refs/heads/f {zero}\n", head.trim());
    child
        .stdin
        .take()
        .unwrap()
        .write_all(line.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn push_drops_excluded_files_before_the_adapters() {
    let tools = "[tools]\npush = [\"r3\"]\n";
    let out = push_with(&format!("{tools}[paths]\nexclude = [\"vendor/\"]\n"));
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{text}");
    let out = push_with(tools);
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(
        text.contains("R3 vendor/big.py: file has 401 lines"),
        "{text}"
    );
}
