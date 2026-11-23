//! `house check --stage` and `--all`: the files through the chosen adapters, then
//! the test step.

use crate::adapter::{Adapter, CannotCheck, Context, Stage};
use crate::finding::Finding;
use crate::reply::{Report, ToolRun};
use crate::rules::Rules;
use crate::tests_step::{self, FailedOutput, TestRun};
use crate::{approval, config, files, lints, run};
use std::path::Path;
use std::time::{Duration, Instant};

/// Run the stage's adapters, then its test commands, in `root` and collect the report.
pub fn check(root: &Path, stage: Stage, all: Vec<Box<dyn Adapter>>) -> Report {
    let rules = Rules::working_tree(root);
    check_from(root, Mode::Stage(stage), &rules, all).report
}

/// `house check --all`: the push stage's adapters and tests on every tracked file.
pub fn check_all(root: &Path, all: Vec<Box<dyn Adapter>>) -> Report {
    check_from(root, Mode::All, &Rules::working_tree(root), all).report
}

/// What a check runs on: a stage's files, or every tracked file (`--all`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    Stage(Stage),
    All,
}

impl Mode {
    /// The stage whose tools and tests run: `--all` takes push's.
    fn stage(self) -> Stage {
        match self {
            Mode::Stage(stage) => stage,
            Mode::All => Stage::Push,
        }
    }
}

/// What a check gives the cli beyond the report.
pub(super) struct Checked {
    pub report: Report,
    /// The commit the excuse list is compared with: HEAD at commit and at `--all`, the
    /// push range's base at push, HEAD when nothing is pushed, and `None` (no prior
    /// list) on an unbounded push, a first push without origin/main.
    pub base: Option<String>,
    /// The test commands that failed, with their output's tails, for the record.
    pub failed_tests: Vec<FailedOutput>,
}

/// Run by `rules`' `.house.toml`: its tools, excluded paths and test commands.
pub(super) fn check_from(
    root: &Path,
    mode: Mode,
    rules: &Rules,
    all: Vec<Box<dyn Adapter>>,
) -> Checked {
    let stage = mode.stage();
    let start = Instant::now();
    let mut checked = Checked {
        report: Report {
            stage,
            seconds: 0.0,
            tools: Vec::new(),
        },
        base: Some("HEAD".to_string()),
        failed_tests: Vec::new(),
    };
    let tools = &mut checked.report.tools;
    let (chosen, exclude, tests, config_cannot) = match &rules.config {
        Ok(cfg) => {
            let tests = (
                config::tests_for(cfg, stage).to_vec(),
                config::test_limit(cfg),
            );
            let (chosen, unknown) = config::select(cfg, stage, all);
            (chosen, cfg.paths.exclude.clone(), tests, unknown)
        }
        Err(c) => (
            all,
            Vec::new(),
            (Vec::new(), config::TEST_LIMIT),
            vec![c.clone()],
        ),
    };
    for c in config_cannot {
        tools.push(cannot_run("house", c));
    }
    let scope = match mode {
        Mode::Stage(stage) => files::for_stage_excluding(root, stage, &exclude),
        Mode::All => every_file(root, &exclude),
    };
    match scope {
        Ok(scope) => {
            if let Some((from, _)) = scope.push_range.as_deref().and_then(|r| r.split_once("..")) {
                checked.base = Some(from.to_string());
            }
            if scope.unbounded {
                checked.base = None;
            }
            let scope_changed = scope.changed;
            let tests_due = scope.run_tests;
            let ctx = Context {
                root: root.to_path_buf(),
                stage,
                languages: files::languages(root, &scope.files),
                files: scope.files,
                push_range: scope.push_range,
            };
            tools.extend(chosen.iter().map(|a| run_one(a.as_ref(), &ctx)));
            tools.extend(lints::run(&ctx));
            tools.extend(settings_case(&scope_changed, rules));
            for test in run_tests(&ctx, &tests.0, tests.1, tests_due) {
                tools.push(test.tool);
                checked.failed_tests.extend(test.failed);
            }
        }
        Err(c) => tools.push(cannot_run("git", c)),
    }
    checked.report.seconds = start.elapsed().as_secs_f64();
    checked
}

/// The wrong-case settings names among `changed`, against the built-in protected
/// list and the rules' `[protected] extra`; `None` when there are none.
fn settings_case(changed: &[String], rules: &Rules) -> Option<ToolRun> {
    let mut protected: Vec<String> = approval::PROTECTED.iter().map(|g| g.to_string()).collect();
    if let Ok(config) = &rules.config {
        protected.extend(config.protected.extra.iter().cloned());
    }
    let findings = approval::wrong_case(changed, &protected);
    (!findings.is_empty()).then(|| ToolRun {
        name: "settings-case".into(),
        seconds: 0.0,
        exit: None,
        skipped: false,
        findings,
        cannot: None,
        unread: Vec::new(),
    })
}

/// Every tracked file still in the working copy, links and `exclude` dropped: what
/// `--all` hands the tools, with no push range (gitleaks then reads all history).
/// Every tracked path, links and all, is what the policy checks judge.
fn every_file(root: &Path, exclude: &[String]) -> Result<files::Scope, CannotCheck> {
    let changed = files::tracked(root)?;
    let tracked = files::without_links(root, changed.clone());
    let present = tracked
        .into_iter()
        .filter(|f| root.join(f).is_file())
        .collect();
    let files = files::without_excluded(present, exclude);
    Ok(files::Scope {
        run_tests: !files.is_empty(),
        files,
        changed,
        push_range: None,
        unbounded: false,
    })
}

/// The test step after the adapters: skipped unless `due` (`Scope::run_tests`): at push
/// when the push changes any path, at commit and `--all` when there is a file to check.
/// Each command may run for `limit` (`[tests] timeout_seconds`).
fn run_tests(ctx: &Context, tests: &[Vec<String>], limit: Duration, due: bool) -> Vec<TestRun> {
    if tests.is_empty() {
        Vec::new()
    } else if !due {
        vec![tests_step::skipped()]
    } else {
        tests_step::run(&ctx.root, tests, limit)
    }
}

/// Two findings are one when rule, place, line and tool rule match.
fn same_place(a: &Finding, b: &Finding) -> bool {
    (&a.rule, &a.place, a.line, &a.tool_rule) == (&b.rule, &b.place, b.line, &b.tool_rule)
}

/// Run every command the adapter gives (one per nested workspace for some), parse
/// each output with the same adapter: findings concatenate, a finding an earlier
/// command already gave (`same_place`) is dropped, the first cannot-check wins,
/// seconds add up, the exit is the first non-zero one. Findings of one command are
/// all kept: two undefined names on one line share rule, place, line and tool rule.
fn run_one(adapter: &dyn Adapter, ctx: &Context) -> ToolRun {
    let mut tool = ToolRun {
        name: adapter.name().to_string(),
        seconds: 0.0,
        exit: None,
        skipped: false,
        findings: Vec::new(),
        cannot: None,
        unread: Vec::new(),
    };
    let commands = adapter.commands(ctx);
    if commands.is_empty() {
        tool.skipped = true;
        return tool;
    }
    for cmd in &commands {
        let ran = run::run(cmd, &ctx.root, adapter.brew_formula(), run::HANG_GUARD);
        tool.seconds += ran.seconds;
        let parsed = ran.result.and_then(|out| {
            if tool.exit.is_none_or(|e| e == 0) {
                tool.exit = Some(out.exit);
            }
            for file in adapter.unread(ctx, &out) {
                if !tool.unread.contains(&file) {
                    tool.unread.push(file);
                }
            }
            adapter.parse(ctx, &out)
        });
        match parsed {
            Ok(findings) => {
                let earlier = tool.findings.len();
                for f in findings {
                    if !tool.findings[..earlier].iter().any(|e| same_place(e, &f)) {
                        tool.findings.push(f);
                    }
                }
            }
            Err(c) => {
                tool.cannot.get_or_insert(c);
            }
        }
    }
    tool
}

pub(super) fn cannot_run(name: &str, c: CannotCheck) -> ToolRun {
    ToolRun {
        name: name.into(),
        seconds: 0.0,
        exit: None,
        skipped: false,
        findings: Vec::new(),
        cannot: Some(c),
        unread: Vec::new(),
    }
}
