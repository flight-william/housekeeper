//! The excuse list (`.house-excuses.toml`) applied to a stage's report.

use super::stage::cannot_run;
use crate::excuses::{self, Date};
use crate::finding::Finding;
use crate::record::Excused;
use crate::reply::{Report, ToolRun};
use crate::rules::Rules;
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
    let rules = Rules::working_tree(root);
    apply_list(report, root, today, base, &rules, false).total
}

/// `excuse` for `house check --all`, the one run over every tracked file: it also
/// reports each live entry that matches no finding as a stale excuse, unless a tool
/// could not check (its findings are missing, so nothing can be judged stale). An
/// expired entry is reported as expired only.
pub fn excuse_all(report: &mut Report, root: &Path, today: Date, base: Option<&str>) -> usize {
    let rules = Rules::working_tree(root);
    apply_list(report, root, today, base, &rules, true).total
}

/// `excuse` (`full`: `excuse_all`) with the list from `rules`, telling which tool and
/// rule each excused finding was. With rules read at a revision, that list excuses
/// the findings, while the pushed list (the working tree's) is what the growth,
/// deadline and expiry checks judge.
pub(super) fn apply_list(
    report: &mut Report,
    root: &Path,
    today: Date,
    base: Option<&str>,
    rules: &Rules,
    full: bool,
) -> Excused {
    let lists = rules.excuses.clone().and_then(|applied| {
        let pushed = match rules.rev {
            Some(_) => excuses::in_tree(root)?,
            None => applied.clone(),
        };
        Ok((applied, pushed))
    });
    let (applied, pushed) = match lists {
        Ok((None, None)) => return Excused::default(),
        Ok((applied, pushed)) => (applied.unwrap_or_default(), pushed.unwrap_or_default()),
        // A revision that cannot be read is reported once, by the check's config.
        Err(c) if rules.config.as_ref().err() == Some(&c) => return Excused::default(),
        Err(c) => {
            report.tools.push(cannot_run("excuses", c));
            return Excused::default();
        }
    };
    let base = match base.map_or(Ok(Some(Vec::new())), |rev| excuses::at_rev(root, rev)) {
        Ok(base) => base,
        Err(c) => {
            report.tools.push(cannot_run("excuses", c));
            return Excused::default();
        }
    };
    let all: Vec<Finding> = report
        .tools
        .iter()
        .flat_map(|t| t.findings.iter().cloned())
        .collect();
    let stale = if full && report.cannots().is_empty() {
        let live: Vec<_> = applied
            .iter()
            .filter(|e| e.deadline >= today)
            .cloned()
            .collect();
        excuses::stale(&live, &all)
    } else {
        Vec::new()
    };
    let sorted = excuses::apply(all, &applied, today);
    let mut excused = Excused {
        total: sorted.excused.len(),
        by_tool: Vec::new(),
    };
    for t in &mut report.tools {
        let mut by_rule = std::collections::BTreeMap::new();
        t.findings.retain(|f| {
            let keep = !sorted.excused.contains(f);
            if !keep {
                *by_rule.entry(f.rule.clone()).or_insert(0) += 1;
            }
            keep
        });
        excused.by_tool.push(by_rule);
    }
    let mut problems = match &base {
        Some(base) => excuses::grew(base, &pushed),
        None => Vec::new(),
    };
    problems.extend(excuses::too_far(
        base.as_deref().unwrap_or(&[]),
        &pushed,
        today,
    ));
    problems.extend(excuses::expired(&pushed, today));
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
    excused
}
