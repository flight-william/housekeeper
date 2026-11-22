//! `house check --stage commit|push [--json]`, `house check --all [--json]`,
//! `house commit-msg <file>`,
//! `house init [--owner @user]`, `house --version`. Arguments are parsed by hand.
//! The cli is the only place that reads the clock for the excuse list's "today".

mod excuse;
mod message;
mod stage;

pub use excuse::{excuse, excuse_all};
pub use message::{commit_facts, judge_message};
pub use stage::{check, check_all};

use crate::adapter::Stage;
use crate::excuses::Date;
use crate::reply::{Report, stage_name};
use crate::{adapters, files, init, record, run, runner};
use message::commit_msg;
use serde_json::Value;
use stage::{Mode, check_from};
use std::path::Path;
use std::time::SystemTime;

pub const USAGE: &str = "usage: house check --stage commit|push [--json] | house check --all [--json] | house commit-msg <message file> | house init [--owner @user] | house --version";

#[derive(Debug, PartialEq, Eq)]
pub enum Cmd {
    Check { stage: Stage, json: bool },
    CheckAll { json: bool },
    CommitMsg { file: String },
    Init,
    InitOwner { owner: String },
    Version,
}

pub fn parse(args: &[String]) -> Result<Cmd, String> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["--version"] | ["-V"] | ["version"] => Ok(Cmd::Version),
        ["init"] => Ok(Cmd::Init),
        ["init", "--owner", owner] if owner.starts_with('@') && owner.len() > 1 => {
            Ok(Cmd::InitOwner {
                owner: owner.to_string(),
            })
        }
        ["commit-msg", file] => Ok(Cmd::CommitMsg {
            file: file.to_string(),
        }),
        ["check", rest @ ..] => parse_check(rest),
        _ => Err(USAGE.into()),
    }
}

fn parse_check(rest: &[&str]) -> Result<Cmd, String> {
    let mut stage = None;
    let mut json = false;
    let mut all = false;
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        match *arg {
            "--json" => json = true,
            "--all" => all = true,
            "--stage" => {
                stage = Some(match it.next() {
                    Some(&"commit") => Stage::Commit,
                    Some(&"push") => Stage::Push,
                    _ => return Err(USAGE.into()),
                })
            }
            _ => return Err(USAGE.into()),
        }
    }
    match (stage, all) {
        (None, true) => Ok(Cmd::CheckAll { json }),
        (Some(stage), false) => Ok(Cmd::Check { stage, json }),
        _ => Err(USAGE.into()),
    }
}

/// Run house with `args` (without the program name) in `cwd`; returns the exit code.
pub fn main(args: &[String], cwd: &Path) -> i32 {
    match parse(args) {
        Err(usage) => {
            eprintln!("{usage}");
            2
        }
        Ok(Cmd::Version) => {
            println!("house {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Ok(Cmd::Init) => run_init(cwd, None),
        Ok(Cmd::InitOwner { owner }) => run_init(cwd, Some(&owner)),
        Ok(Cmd::CommitMsg { file }) => commit_msg(cwd, &file),
        Ok(Cmd::Check { stage, json }) => check_and_record(cwd, Mode::Stage(stage), json),
        Ok(Cmd::CheckAll { json }) => check_and_record(cwd, Mode::All, json),
    }
}

fn run_init(cwd: &Path, owner: Option<&str>) -> i32 {
    match init::init_with_owner(cwd, adapters::all(), owner) {
        Ok(lines) => {
            lines.iter().for_each(|l| println!("{l}"));
            0
        }
        Err(c) => {
            println!("house init: cannot: {}. fix: {}", c.what, c.fix);
            2
        }
    }
}

fn check_and_record(cwd: &Path, mode: Mode, json: bool) -> i32 {
    if let Some(line) = runner::refusal(&|key| std::env::var(key).ok()) {
        println!("{line}");
        return 2;
    }
    let start = SystemTime::now();
    let label = match mode {
        Mode::Stage(stage) => stage_name(stage),
        Mode::All => "all",
    };
    let Some(root) = root_or_say(cwd, label) else {
        return 2;
    };
    let checked = check_from(&root, mode, adapters::all());
    let mut report = checked.report;
    let base = checked.base.as_deref();
    let excused = match mode {
        Mode::Stage(_) => excuse(&mut report, &root, today(), base),
        Mode::All => excuse_all(&mut report, &root, today(), base),
    };
    if json {
        let mut value = report.json();
        if let Some(m) = value.as_object_mut() {
            m.insert("stage".into(), label.into());
            m.insert("excused".into(), excused.into());
        }
        println!("{value}");
    } else {
        print!("{}", report.text_as(label, excused));
    }
    let tests = serde_json::to_value(&checked.failed_tests).unwrap_or_default();
    write_record(
        &root,
        start,
        &report,
        label,
        excused,
        &[("test_output", tests)],
    );
    report.exit()
}

/// Today in UTC, from the system clock.
fn today() -> Date {
    let now = record::rfc3339(SystemTime::now());
    Date::parse(&now[..10]).expect("rfc3339 starts with a real date")
}

pub(crate) fn root_or_say(cwd: &Path, label: &str) -> Option<std::path::PathBuf> {
    match files::repo_root(cwd) {
        Ok(root) => Some(root),
        Err(c) => {
            println!("house {label}: cannot check: {}. fix: {}", c.what, c.fix);
            None
        }
    }
}

/// Append the run's record line, its stage shown as `label`, with the excused count
/// and the `extra` fields (a check's failed test commands and their output's tails).
pub(crate) fn write_record(
    root: &Path,
    start: SystemTime,
    report: &Report,
    label: &str,
    excused: usize,
    extra: &[(&str, Value)],
) {
    let info = record::RunInfo {
        start,
        root,
        head: run::git(root, &["rev-parse", "--verify", "-q", "HEAD"])
            .ok()
            .map(|o| o.stdout.trim().to_string())
            .filter(|h| !h.is_empty()),
        user: std::env::var("USER").unwrap_or_default(),
    };
    let mut line = record::line(&info, report);
    if let Value::Object(m) = &mut line {
        m.insert("stage".into(), label.into());
        m.insert("excused".into(), excused.into());
        for (key, value) in extra {
            m.insert((*key).into(), value.clone());
        }
    }
    match record::log_dir() {
        Some(dir) => {
            if let Err(e) = record::append(&dir, start, &line) {
                eprintln!("house: record not written to {}: {e}", dir.display());
            }
        }
        None => eprintln!("house: record not written: neither HOUSE_LOG_DIR nor HOME is set"),
    }
}
