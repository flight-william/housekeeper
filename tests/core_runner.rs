//! The core's run side: config choosing tools, the runner and its hang guard, check,
//! the cli parser, and init.

mod common;

use house::adapter::{Adapter, CannotCheck, Command, Context, Output, Stage};
use house::finding::{Finding, Place};
use house::{cli, config, init, run};
use std::path::Path;
use std::time::Duration;

struct Fake(&'static str, &'static str);

impl Adapter for Fake {
    fn name(&self) -> &'static str {
        self.0
    }
    fn program(&self) -> &'static str {
        self.1
    }
    fn brew_formula(&self) -> &'static str {
        "fake-formula"
    }
    fn command(&self, ctx: &Context) -> Option<Command> {
        (!ctx.files.is_empty()).then(|| Command {
            program: self.1.into(),
            args: vec!["fake".into()],
        })
    }
    fn parse(&self, _: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        Ok(vec![Finding {
            rule: "R2".into(),
            place: Place::Repo,
            line: None,
            what: out.stdout.trim().into(),
            fix: "x".into(),
            tool_rule: "fake".into(),
            measure: None,
        }])
    }
}

fn fakes() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(Fake("a", "echo")),
        Box::new(Fake("b", "echo")),
        Box::new(Fake("c", "echo")),
    ]
}

#[test]
fn config_absent_runs_all_present_runs_named_unknown_is_cannot_check() {
    let (all, bad) = config::select(&config::Config::default(), Stage::Commit, fakes());
    assert_eq!(all.len(), 3);
    assert!(bad.is_empty());
    let cfg = config::parse("[tools]\ncommit = [\"c\", \"a\", \"zz\"]\n").unwrap();
    let (chosen, bad) = config::select(&cfg, Stage::Commit, fakes());
    assert_eq!(
        chosen.iter().map(|a| a.name()).collect::<Vec<_>>(),
        ["c", "a"]
    );
    assert_eq!(bad[0].what, ".house.toml names an unknown tool zz");
    let (push, _) = config::select(&cfg, Stage::Push, fakes());
    assert_eq!(push.len(), 2, "push falls back to the commit list");
    assert!(config::parse("[tools\n").is_err());
}

#[test]
fn run_captures_output_and_exit() {
    let cmd = Command {
        program: "sh".into(),
        args: vec!["-c".into(), "echo out; echo err >&2; exit 3".into()],
    };
    let out = run::run(&cmd, Path::new("/"), "sh", run::HANG_GUARD)
        .result
        .unwrap();
    assert_eq!(
        (out.exit, out.stdout.as_str(), out.stderr.as_str()),
        (3, "out\n", "err\n")
    );
}

#[test]
fn run_missing_program_says_brew_install() {
    let cmd = Command {
        program: "house-no-such-tool".into(),
        args: vec![],
    };
    let c = run::run(&cmd, Path::new("/"), "nosuch", run::HANG_GUARD)
        .result
        .unwrap_err();
    assert_eq!(c.what, "house-no-such-tool is not installed");
    assert_eq!(c.fix, "brew install nosuch");
}

#[test]
fn run_hang_guard_kills_the_child() {
    let cmd = Command {
        program: "sleep".into(),
        args: vec!["30".into()],
    };
    let ran = run::run(&cmd, Path::new("/"), "sleep", Duration::from_millis(200));
    assert!(ran.seconds < 5.0);
    assert_eq!(ran.result.unwrap_err().what, "sleep did not finish in 0 s");
}

/// The tool exits at once but a child it started keeps stdout and
/// stderr open; the guard must cover reading them, not only the tool's exit.
#[test]
fn run_hang_guard_covers_draining_the_output() {
    let cmd = Command {
        program: "sh".into(),
        args: vec!["-c".into(), "sleep 30 & echo started".into()],
    };
    let ran = run::run(&cmd, Path::new("/"), "sh", Duration::from_millis(200));
    assert!(ran.seconds < 5.0, "took {} s", ran.seconds);
    assert_eq!(ran.result.unwrap_err().what, "sh did not finish in 0 s");
}

#[test]
fn check_runs_staged_files_through_adapters() {
    let repo = common::repo("check");
    let report = cli::check(&repo, Stage::Commit, fakes());
    assert_eq!(report.exit(), 0, "nothing staged: every adapter skips");
    assert!(report.tools.iter().all(|t| t.skipped));
    std::fs::write(repo.join("a.txt"), "x").unwrap();
    common::git(&repo, &["add", "a.txt"]);
    let report = cli::check(
        &repo,
        Stage::Commit,
        vec![
            Box::new(Fake("a", "echo")),
            Box::new(Fake("m", "house-no-such-tool")),
        ],
    );
    assert_eq!(report.exit(), 2);
    let text = report.text();
    assert!(text.contains("R2 repo: fake. fix: x. [fake]\n"), "{text}");
    assert!(text.contains("R2 cannot check: house-no-such-tool is not installed. fix: brew install fake-formula [m]\n"), "{text}");
}

#[test]
fn cli_parses_by_hand() {
    let a = |s: &[&str]| cli::parse(&s.iter().map(|x| x.to_string()).collect::<Vec<_>>());
    assert_eq!(
        a(&["check", "--stage", "commit"]),
        Ok(cli::Cmd::Check {
            stage: Stage::Commit,
            json: false
        })
    );
    assert_eq!(
        a(&["check", "--json", "--stage", "push"]),
        Ok(cli::Cmd::Check {
            stage: Stage::Push,
            json: true
        })
    );
    assert_eq!(a(&["init"]), Ok(cli::Cmd::Init));
    assert_eq!(a(&["--version"]), Ok(cli::Cmd::Version));
    assert!(a(&["check"]).is_err());
    assert!(a(&["check", "--stage", "nope"]).is_err());
    assert!(a(&[]).is_err());
}

#[test]
fn init_writes_hook_and_config_only_when_absent() {
    let repo = common::repo("init");
    std::fs::write(repo.join("a.py"), "x = 1\n").unwrap();
    common::git(&repo, &["add", "a.py"]);
    let said = init::init(&repo, house::adapters::all()).unwrap();
    let hook = repo.join(".git/hooks/pre-commit");
    assert_eq!(std::fs::read_to_string(&hook).unwrap(), init::HOOK);
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&hook).unwrap().permissions().mode() & 0o777,
        0o755
    );
    let cfg = std::fs::read_to_string(repo.join(".house.toml")).unwrap();
    assert!(
        // Every adapter in all(), including r3: a Python
        // repo gets ruff, ruff-format, the any-language gitleaks and semgrep, and
        // r3 (it counts .py file and function length).
        cfg.contains("commit = [\"ruff\", \"ruff-format\", \"gitleaks\", \"semgrep\", \"r3\"]"),
        "{cfg}"
    );
    config::parse(&cfg).unwrap();
    assert!(said[0].contains("wrote"), "{said:?}");

    std::fs::write(&hook, "#!/bin/sh\necho mine\n").unwrap();
    let said = init::init(&repo, house::adapters::all()).unwrap();
    assert_eq!(
        std::fs::read_to_string(&hook).unwrap(),
        "#!/bin/sh\necho mine\n"
    );
    assert!(
        said[0].ends_with("add this line to it: house check --stage commit || true"),
        "{said:?}"
    );
    assert_eq!(
        std::fs::read_to_string(repo.join(".house.toml")).unwrap(),
        cfg
    );
}

#[test]
fn a_staged_symbolic_link_is_not_a_file_to_check() {
    let repo = common::repo("links");
    std::fs::write(repo.join("a.py"), "x = 1\n").unwrap();
    std::os::unix::fs::symlink("a.py", repo.join("b.py")).unwrap();
    common::git(&repo, &["add", "a.py", "b.py"]);
    assert_eq!(
        house::files::staged(&repo).unwrap(),
        vec!["a.py".to_string()]
    );
}
