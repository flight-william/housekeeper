//! The wiring of an adapter's several commands, and Semgrep with both pattern files.

mod common;

use house::adapter::{Adapter, CannotCheck, Command, Context, Output, Stage};
use house::adapters::semgrep::Semgrep;
use house::cli;
use house::finding::{Finding, Place};
use std::path::Path;
use std::process::Command as Proc;

fn found(rule: &str, file: &str, tool_rule: &str) -> Finding {
    Finding {
        rule: rule.into(),
        place: Place::File(file.into()),
        line: Some(3),
        what: "too long".into(),
        fix: "split it".into(),
        tool_rule: tool_rule.into(),
        measure: None,
    }
}
// ---- several commands per adapter ----

/// An adapter with one `echo <word>` per word; each output is one finding, and the
/// word `missing` names a program that is not installed.
struct Many(&'static [&'static str]);

impl Adapter for Many {
    fn name(&self) -> &'static str {
        "many"
    }
    fn program(&self) -> &'static str {
        "echo"
    }
    fn brew_formula(&self) -> &'static str {
        "many-formula"
    }
    fn command(&self, _: &Context) -> Option<Command> {
        None
    }
    fn commands(&self, ctx: &Context) -> Vec<Command> {
        if ctx.files.is_empty() {
            return Vec::new();
        }
        self.0
            .iter()
            .map(|w| Command {
                program: if *w == "missing" {
                    "house-no-such-tool"
                } else {
                    "echo"
                }
                .into(),
                args: vec![(*w).into()],
            })
            .collect()
    }
    fn parse(&self, _: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        Ok(vec![found("R2", out.stdout.trim(), "many")])
    }
}

/// An adapter that runs `echo` and reads every output as clean, with one file its
/// own parser could not read.
struct Unread;

impl Adapter for Unread {
    fn name(&self) -> &'static str {
        "unread"
    }
    fn program(&self) -> &'static str {
        "echo"
    }
    fn brew_formula(&self) -> &'static str {
        "unread-formula"
    }
    fn command(&self, _: &Context) -> Option<Command> {
        Some(Command {
            program: "echo".into(),
            args: vec!["x".into()],
        })
    }
    fn parse(&self, _: &Context, _: &Output) -> Result<Vec<Finding>, CannotCheck> {
        Ok(vec![])
    }
    fn unread(&self, _: &Context, _: &Output) -> Vec<String> {
        vec!["Tests/A.swift".into()]
    }
}

#[test]
fn the_files_an_adapter_could_not_read_reach_the_report() {
    let repo = common::repo("unread");
    std::fs::write(repo.join("a.txt"), "x").unwrap();
    common::git(&repo, &["add", "a.txt"]);
    let r = cli::check(&repo, Stage::Commit, vec![Box::new(Unread)]);
    assert_eq!(r.tools[0].unread, ["Tests/A.swift"]);
    assert!(r.tools[0].findings.is_empty());
    assert_eq!(r.exit(), 0);
}

#[test]
fn every_command_runs_and_findings_concatenate() {
    let repo = common::repo("many");
    std::fs::write(repo.join("a.txt"), "x").unwrap();
    common::git(&repo, &["add", "a.txt"]);
    let r = cli::check(&repo, Stage::Commit, vec![Box::new(Many(&["one", "two"]))]);
    let places: Vec<_> = r.tools[0]
        .findings
        .iter()
        .map(|f| f.place.clone())
        .collect();
    assert_eq!(
        places,
        [Place::File("one".into()), Place::File("two".into())]
    );
    assert_eq!(r.tools[0].exit, Some(0));
    let r = cli::check(
        &repo,
        Stage::Commit,
        vec![Box::new(Many(&["one", "missing", "two"]))],
    );
    assert_eq!(r.tools[0].findings.len(), 2, "the others still ran");
    assert_eq!(
        r.tools[0].cannot.as_ref().unwrap().what,
        "house-no-such-tool is not installed"
    );
    assert_eq!(r.exit(), 2);
}

#[test]
fn the_same_finding_from_two_commands_is_kept_once() {
    let repo = common::repo("many-dedup");
    std::fs::write(repo.join("a.txt"), "x").unwrap();
    common::git(&repo, &["add", "a.txt"]);
    let r = cli::check(
        &repo,
        Stage::Commit,
        vec![Box::new(Many(&["one", "two", "one"]))],
    );
    let places: Vec<_> = r.tools[0]
        .findings
        .iter()
        .map(|f| f.place.clone())
        .collect();
    assert_eq!(
        places,
        [Place::File("one".into()), Place::File("two".into())]
    );
}

/// One command whose output holds two findings with the same rule, place, line and
/// tool rule (two undefined names on one line, two long functions in one file).
struct OneCommandTwoFindings;

impl Adapter for OneCommandTwoFindings {
    fn name(&self) -> &'static str {
        "two"
    }
    fn program(&self) -> &'static str {
        "echo"
    }
    fn brew_formula(&self) -> &'static str {
        "two-formula"
    }
    fn command(&self, _: &Context) -> Option<Command> {
        Some(Command {
            program: "echo".into(),
            args: vec!["x".into()],
        })
    }
    fn parse(&self, _: &Context, _: &Output) -> Result<Vec<Finding>, CannotCheck> {
        let mut second = found("R3", "a.py", "r3 function_lines");
        second.what = "function other has 90 lines".into();
        Ok(vec![found("R3", "a.py", "r3 function_lines"), second])
    }
}

#[test]
fn findings_of_one_command_are_all_kept() {
    let repo = common::repo("one-two");
    std::fs::write(repo.join("a.txt"), "x").unwrap();
    common::git(&repo, &["add", "a.txt"]);
    let r = cli::check(&repo, Stage::Commit, vec![Box::new(OneCommandTwoFindings)]);
    assert_eq!(r.tools[0].findings.len(), 2);
}

#[test]
fn no_commands_is_skipped() {
    let repo = common::repo("many-skip");
    let r = cli::check(&repo, Stage::Commit, vec![Box::new(Many(&["one"]))]);
    assert!(r.tools[0].skipped);
}

// ---- Semgrep with r8.yml and r9.yml ----

fn semgrep_ctx(root: &Path, files: &[&str]) -> Context {
    Context {
        root: root.to_path_buf(),
        stage: Stage::Commit,
        files: files.iter().map(|f| f.to_string()).collect(),
        push_range: None,
    }
}

#[test]
fn semgrep_gets_both_pattern_files_as_two_config_flags() {
    let cmd = Semgrep
        .command(&semgrep_ctx(Path::new("/r"), &["tests/test_a.py"]))
        .unwrap();
    let configs: Vec<&String> = cmd
        .args
        .iter()
        .zip(cmd.args.iter().skip(1))
        .filter(|(flag, _)| *flag == "--config")
        .map(|(_, file)| file)
        .collect();
    assert_eq!(configs.len(), 2, "{:?}", cmd.args);
    assert!(configs[0].ends_with("/patterns/r8.yml"), "{configs:?}");
    assert!(configs[1].ends_with("/patterns/r9.yml"), "{configs:?}");
    for c in configs {
        assert!(Path::new(c).is_file(), "{c} exists");
    }
    assert_eq!(cmd.args.last().unwrap(), "tests/test_a.py");
}

#[test]
fn semgrep_reports_r8_and_r9_findings_from_one_run() {
    if Proc::new("semgrep").arg("--version").output().is_err() {
        eprintln!("note: semgrep is not installed; skipping the two-pattern scan");
        return;
    }
    let work = common::fresh("semgrep-both");
    let home = work.join("home");
    let root = work.join("proj");
    std::fs::create_dir_all(root.join("tests")).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    std::fs::copy(
        fixtures.join("semgrep/planted/tests/test_timing.py"),
        root.join("tests/test_timing.py"),
    )
    .unwrap();
    std::fs::copy(
        fixtures.join("semgrep_r9/python/test_planted.py"),
        root.join("tests/test_planted.py"),
    )
    .unwrap();
    let ctx = semgrep_ctx(&root, &["tests/test_timing.py", "tests/test_planted.py"]);
    let cmd = Semgrep.command(&ctx).unwrap();
    let out = Proc::new(&cmd.program)
        .args(&cmd.args)
        .current_dir(&root)
        .env("HOME", &home)
        .env("SEMGREP_ENABLE_VERSION_CHECK", "0")
        .output()
        .unwrap();
    let out = Output {
        exit: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    };
    let findings = Semgrep.parse(&ctx, &out).unwrap();
    let r8 = findings
        .iter()
        .find(|f| f.rule == "R8")
        .expect("an R8 finding");
    let r9 = findings
        .iter()
        .find(|f| f.rule == "R9")
        .expect("an R9 finding");
    assert_eq!(r8.place, Place::File("tests/test_timing.py".into()));
    assert!(r8.tool_rule.starts_with("semgrep python-test-"), "{r8:?}");
    assert_eq!(r9.place, Place::File("tests/test_planted.py".into()));
    assert!(r9.tool_rule.starts_with("semgrep r9-python-"), "{r9:?}");
    assert!(
        findings
            .iter()
            .all(|f| f.rule == "R9" || !f.tool_rule.contains(" r9-")),
        "every r9- rule reports R9"
    );
}
