//! The review's suggestions, for a person to weigh: a rule or tool that never fires,
//! one that looks noisy, and a tool whose seconds dominate the runs.

use super::{Count, Review};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// The fewest runs in the period, of the review or of the tool, after which a rule
/// or tool with no findings is said to never fire: fewer runs are too few to tell.
pub const NEVER_FIRES_MIN_RUNS: usize = 5;

/// One suggestion: its kind (`never_fires`, `looks_noisy`, `slow`), the rule or tool
/// it is about, and the plain sentence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    pub kind: &'static str,
    pub subject: String,
    pub text: String,
}

impl Review {
    /// Never fires (rules, then tools), looks noisy (rules, then tools), then the
    /// tool that takes most of the time. Every total is made in one pass over the
    /// places and the checked runs, then looked up per rule and tool, so the time
    /// grows with the records, not with rules times places.
    pub fn suggestions(&self) -> Vec<Suggestion> {
        let checked = self.rule_checked_runs();
        let (rule_repeats, tool_repeats) = self.repeats();
        let mut out = Vec::new();
        for (rule, count) in self.rules_in_order() {
            let runs = checked.get(rule).copied().unwrap_or(0);
            if runs >= NEVER_FIRES_MIN_RUNS && silent(count) && !self.rule_excused_unknown(rule) {
                out.push(never_fires(rule, runs, RULE_CHECK));
            }
        }
        for (tool, stat) in &self.tools {
            if stat.checked >= NEVER_FIRES_MIN_RUNS && silent(&stat.count) && !stat.excused_unknown
            {
                out.push(never_fires(tool, stat.checked, TOOL_CHECK));
            }
        }
        for (rule, count) in self.rules_in_order() {
            let repeats = rule_repeats.get(rule).copied().unwrap_or(0);
            out.extend(noisy(rule, count, repeats, "rule"));
        }
        for (tool, stat) in &self.tools {
            let repeats = tool_repeats.get(tool.as_str()).copied().unwrap_or(0);
            out.extend(noisy(tool, &stat.count, repeats, "tool"));
        }
        out.extend(self.slow());
        out
    }

    /// For each rule, the covered runs in which a tool that names it checked: a run
    /// where they were all skipped or could not check says nothing about the rule.
    /// Each set of checked tools adds its runs once to every rule its tools name.
    fn rule_checked_runs(&self) -> BTreeMap<&str, usize> {
        let mut tool_rules: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (rule, tools) in &self.rule_tools {
            for tool in tools {
                tool_rules.entry(tool).or_default().push(rule);
            }
        }
        let mut runs: BTreeMap<&str, usize> = BTreeMap::new();
        for (checked, n) in &self.checked_sets {
            let rules: BTreeSet<&str> = checked
                .iter()
                .filter_map(|t| tool_rules.get(t.as_str()))
                .flatten()
                .copied()
                .collect();
            for rule in rules {
                *runs.entry(rule).or_default() += n;
            }
        }
        runs
    }

    /// A tool that names `rule` ran where excuses were not tied to a rule (records
    /// written before 0.3.0), so the rule may have had excused findings.
    fn rule_excused_unknown(&self, rule: &str) -> bool {
        self.rule_tools.get(rule).is_some_and(|tools| {
            tools
                .iter()
                .any(|t| self.tools.get(t).is_some_and(|s| s.excused_unknown))
        })
    }

    /// The findings on places found on more than one run, every finding counted, by
    /// rule and by the tool that found them. A sum is only compared, and shown capped
    /// at the rule's or tool's own total, so a saturated sum gives the same
    /// suggestion as the exact one.
    fn repeats(&self) -> (BTreeMap<&str, u64>, BTreeMap<&str, u64>) {
        let mut by_rule: BTreeMap<&str, u64> = BTreeMap::new();
        let mut by_tool: BTreeMap<&str, u64> = BTreeMap::new();
        for (place, stat) in self.places.iter().filter(|(_, s)| s.runs > 1) {
            let total = by_rule.entry(place.rule.as_str()).or_default();
            *total = total.saturating_add(stat.findings);
            for (tool, n) in &stat.by_tool {
                let total = by_tool.entry(tool.as_str()).or_default();
                *total = total.saturating_add(*n);
            }
        }
        (by_rule, by_tool)
    }

    /// The tool with more than half of all the tools' seconds, when two or more ran.
    fn slow(&self) -> Option<Suggestion> {
        let total: f64 = self.tools.values().map(|t| t.total_seconds()).sum();
        let timed = self
            .tools
            .values()
            .filter(|t| t.total_seconds() > 0.0)
            .count();
        let (tool, stat) = self
            .tools
            .iter()
            .max_by(|a, b| a.1.total_seconds().total_cmp(&b.1.total_seconds()))?;
        let seconds = stat.total_seconds();
        (timed > 1 && seconds * 2.0 > total).then(|| Suggestion {
            kind: "slow",
            subject: tool.clone(),
            text: format!(
                "{tool} takes most of the time: {seconds:.1} of {total:.1} s ({:.0}%); run it at push only, or hand it fewer files",
                seconds / total * 100.0
            ),
        })
    }
}

const RULE_CHECK: &str = "check that a tool still checks it, or drop it";
const TOOL_CHECK: &str = "check that it is handed the files it should read";

fn silent(count: &Count) -> bool {
    count.findings == 0 && count.excused == 0
}

fn never_fires(subject: &str, runs: usize, check: &str) -> Suggestion {
    let runs = if runs == 1 {
        "1 run".into()
    } else {
        format!("{runs} runs")
    };
    Suggestion {
        kind: "never_fires",
        subject: subject.to_string(),
        text: format!("{subject} never fires: no findings in {runs}; {check}"),
    }
}

/// Looks noisy when most of its findings were excused, or most of those left to fix
/// were on places that failed on another run too, so were not fixed in between.
fn noisy(subject: &str, count: &Count, repeats: u64, what: &str) -> Option<Suggestion> {
    // In u128: two u64 counts add, and a count doubles, without overflow.
    let all = u128::from(count.findings) + u128::from(count.excused);
    let repeats = repeats.min(count.findings);
    let mut reasons = Vec::new();
    if count.excused > count.findings {
        reasons.push(format!("{} of {all} findings excused", count.excused));
    }
    if u128::from(repeats) * 2 > u128::from(count.findings) {
        reasons.push(format!(
            "{repeats} of {} left to fix on places that failed before",
            count.findings
        ));
    }
    (!reasons.is_empty()).then(|| Suggestion {
        kind: "looks_noisy",
        subject: subject.to_string(),
        text: format!(
            "{subject} looks noisy: {}; fix that code, or tune the {what}",
            reasons.join(", ")
        ),
    })
}
