//! `house check --stage`: the stage's files through the chosen adapters.

use crate::adapter::{Adapter, CannotCheck, Context, Stage};
use crate::finding::Finding;
use crate::reply::{Report, ToolRun};
use crate::{config, files, run};
use std::path::Path;
use std::time::Instant;

/// Run the stage's adapters in `root` and collect the report.
pub fn check(root: &Path, stage: Stage, all: Vec<Box<dyn Adapter>>) -> Report {
    check_from(root, stage, all).0
}

/// `check`, and the commit the excuse list is compared with: HEAD at commit, the
/// push range's base at push, HEAD when nothing is pushed, and `None` (no prior
/// list) on an unbounded push, a first push without origin/main.
pub(super) fn check_from(
    root: &Path,
    stage: Stage,
    all: Vec<Box<dyn Adapter>>,
) -> (Report, Option<String>) {
    let mut base = Some("HEAD".to_string());
    let start = Instant::now();
    let mut tools = Vec::new();
    let (chosen, exclude, config_cannot) = match config::load(root) {
        Ok(cfg) => {
            let (chosen, unknown) = config::select(&cfg, stage, all);
            (chosen, cfg.paths.exclude, unknown)
        }
        Err(c) => (all, Vec::new(), vec![c]),
    };
    for c in config_cannot {
        tools.push(cannot_run("house", c));
    }
    match files::for_stage_excluding(root, stage, &exclude) {
        Ok(files::Scope {
            files,
            push_range,
            unbounded,
        }) => {
            if let Some((from, _)) = push_range.as_deref().and_then(|r| r.split_once("..")) {
                base = Some(from.to_string());
            }
            if unbounded {
                base = None;
            }
            let ctx = Context {
                root: root.to_path_buf(),
                stage,
                files,
                push_range,
            };
            tools.extend(chosen.iter().map(|a| run_one(a.as_ref(), &ctx)));
        }
        Err(c) => tools.push(cannot_run("git", c)),
    }
    let report = Report {
        stage,
        seconds: start.elapsed().as_secs_f64(),
        tools,
    };
    (report, base)
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
