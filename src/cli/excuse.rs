//! The excuse list (`.house-excuses.toml`) applied to a stage's report.

use super::stage::cannot_run;
use crate::adapter::CannotCheck;
use crate::excuses::{self, Date};
use crate::finding::Finding;
use crate::reply::{Report, ToolRun};
use std::path::Path;

/// Apply `.house-excuses.toml` to `report`: excused findings leave the reply (their
/// count is returned), the list's own problems join it as tool "excuses". "Stale
/// excuse" problems are always dropped: commit and push check changed files only, and
/// only a full run over every file the list names can judge staleness. The list may
/// only shrink: it is compared with the list at `base`, `HEAD` at commit and the push
/// base (the part of `push_range` before `..`) at push; `None` is no base to compare
/// with (a first push without origin/main), so every entry is growth. A base that holds
/// no list is adoption: no entry is growth, but each deadline is still capped at 56 days
/// from today, and the protected file still needs the owner's approval at commit-msg.
pub fn excuse(report: &mut Report, root: &Path, today: Date, base: Option<&str>) -> usize {
    apply_list(report, root, today, base, false)
}

/// `excuse` for `house check --all`, the one run over every tracked file: it also
/// reports each live entry that matches no finding as a stale excuse, unless a tool
/// could not check (its findings are missing, so nothing can be judged stale). An
/// expired entry is reported as expired only.
pub fn excuse_all(report: &mut Report, root: &Path, today: Date, base: Option<&str>) -> usize {
    apply_list(report, root, today, base, true)
}

fn apply_list(
    report: &mut Report,
    root: &Path,
    today: Date,
    base: Option<&str>,
    full: bool,
) -> usize {
    let list = match std::fs::read_to_string(root.join(excuses::FILE)) {
        Ok(text) => excuses::parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return 0,
        Err(e) => Err(CannotCheck {
            what: format!("{} cannot be read: {e}", excuses::FILE),
            fix: format!("make {} readable", excuses::FILE),
        }),
    };
    let list = match list {
        Ok(list) => list,
        Err(c) => {
            report.tools.push(cannot_run("excuses", c));
            return 0;
        }
    };
    let base = match base.map_or(Ok(Some(Vec::new())), |rev| excuses::at_rev(root, rev)) {
        Ok(base) => base,
        Err(c) => {
            report.tools.push(cannot_run("excuses", c));
            return 0;
        }
    };
    let all: Vec<Finding> = report
        .tools
        .iter()
        .flat_map(|t| t.findings.iter().cloned())
        .collect();
    let stale = if full && report.cannots().is_empty() {
        let live: Vec<_> = list
            .iter()
            .filter(|e| e.deadline >= today)
            .cloned()
            .collect();
        excuses::stale(&live, &all)
    } else {
        Vec::new()
    };
    let applied = excuses::apply(all, &list, today);
    for t in &mut report.tools {
        t.findings.retain(|f| !applied.excused.contains(f));
    }
    let mut problems = match &base {
        Some(base) => excuses::grew(base, &list),
        None => Vec::new(),
    };
    problems.extend(excuses::too_far(
        base.as_deref().unwrap_or(&[]),
        &list,
        today,
    ));
    problems.extend(
        applied
            .problems
            .into_iter()
            .filter(|p| p.tool_rule != "house excuse-stale"),
    );
    problems.extend(stale);
    if !problems.is_empty() {
        report.tools.push(ToolRun {
            name: "excuses".into(),
            seconds: 0.0,
            exit: None,
            skipped: false,
            findings: problems,
            cannot: None,
            unread: Vec::new(),
        });
    }
    applied.excused.len()
}
