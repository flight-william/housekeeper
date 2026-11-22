//! The test step: the stage's `[tests]` commands (`.house.toml`), each a program and
//! its arguments, run from the repo root with no shell under the hang guard, after
//! the adapters. A non-zero exit is one R7 finding for the repo; the last lines of
//! the command's output go to the run record, never to the reply.

use crate::adapter::{CannotCheck, Command};
use crate::config;
use crate::finding::{Finding, Place};
use crate::reply::ToolRun;
use crate::run;
use serde::Serialize;
use std::path::Path;
use std::time::Duration;

/// The tool name test commands report under.
pub const NAME: &str = "tests";

/// How many lines of each output stream a failed command keeps in the record.
pub const TAIL_LINES: usize = 20;

/// One command's run, and for a failed one what the record keeps of its output.
#[derive(Debug)]
pub struct TestRun {
    pub tool: ToolRun,
    pub failed: Option<FailedOutput>,
}

/// A failed command and the last `TAIL_LINES` lines of its stdout and of its stderr.
#[derive(Debug, Serialize)]
pub struct FailedOutput {
    pub command: String,
    pub stdout_tail: Vec<String>,
    pub stderr_tail: Vec<String>,
}

/// Run each of `commands` in `root`, in order, each killed after `guard`.
pub fn run(root: &Path, commands: &[Vec<String>], guard: Duration) -> Vec<TestRun> {
    commands.iter().map(|c| run_one(root, c, guard)).collect()
}

/// One skipped run, for a stage with no file to check.
pub fn skipped() -> TestRun {
    let mut tool = tool_run();
    tool.skipped = true;
    TestRun { tool, failed: None }
}

fn run_one(root: &Path, argv: &[String], guard: Duration) -> TestRun {
    let shown = argv.join(" ");
    let mut test = TestRun {
        tool: tool_run(),
        failed: None,
    };
    let Some((program, args)) = argv.split_first() else {
        test.tool.cannot = Some(fix_config("a [tests] command is empty".into()));
        return test;
    };
    let cmd = Command {
        program: program.clone(),
        args: args.to_vec(),
    };
    let ran = run::run(&cmd, root, program, guard);
    test.tool.seconds = ran.seconds;
    match ran.result {
        Err(c) => test.tool.cannot = Some(fix_config(c.what)),
        Ok(out) => {
            test.tool.exit = Some(out.exit);
            if out.exit != 0 {
                test.tool.findings.push(Finding {
                    rule: "R7".into(),
                    place: Place::Repo,
                    line: None,
                    what: format!("test command failed: {shown}"),
                    fix: "run it and fix the failures".into(),
                    tool_rule: format!("house tests exit {}", out.exit),
                    measure: None,
                });
                test.failed = Some(FailedOutput {
                    command: shown,
                    stdout_tail: tail(&out.stdout),
                    stderr_tail: tail(&out.stderr),
                });
            }
        }
    }
    test
}

fn fix_config(what: String) -> CannotCheck {
    CannotCheck {
        what,
        fix: format!(
            "install the program, or correct [tests] in {}",
            config::FILE
        ),
    }
}

fn tool_run() -> ToolRun {
    ToolRun {
        name: NAME.into(),
        seconds: 0.0,
        exit: None,
        skipped: false,
        findings: Vec::new(),
        cannot: None,
        unread: Vec::new(),
    }
}

/// The last `TAIL_LINES` lines of `text`.
fn tail(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let from = lines.len().saturating_sub(TAIL_LINES);
    lines[from..].iter().map(|l| l.to_string()).collect()
}
