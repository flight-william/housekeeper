//! The review's suggestions, for a person to weigh: a rule or tool that never fires,
//! one that looks noisy, and a tool whose seconds dominate the runs.

use super::{Count, Review};
use serde::Serialize;

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
    /// tool that takes most of the time.
    pub fn suggestions(&self) -> Vec<Suggestion> {
        let mut out = Vec::new();
        for (rule, count) in self.rules_in_order() {
            let runs = self.rule_checked_runs(rule);
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
            out.extend(noisy(rule, count, self.rule_repeats(rule), "rule"));
        }
        for (tool, stat) in &self.tools {
            out.extend(noisy(tool, &stat.count, self.tool_repeats(tool), "tool"));
        }
        out.extend(self.slow());
        out
    }

    /// The covered runs in which a tool that names `rule` checked: a run where they
    /// were all skipped or could not check says nothing about the rule.
    fn rule_checked_runs(&self, rule: &str) -> usize {
        let Some(tools) = self.rule_tools.get(rule) else {
            return 0;
        };
        self.checked_sets
            .iter()
            .filter(|(checked, _)| !checked.is_disjoint(tools))
            .map(|(_, runs)| runs)
            .sum()
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

    /// The runs a rule failed on places found on more than one run.
    fn rule_repeats(&self, rule: &str) -> u64 {
        self.places
            .iter()
            .filter(|(p, runs)| **runs > 1 && p.rule == rule)
            .map(|(_, runs)| *runs as u64)
            .sum()
    }

    /// The runs a tool found places that were found on more than one run.
    fn tool_repeats(&self, tool: &str) -> u64 {
        let repeated = |p| self.places.get(p).is_some_and(|runs| *runs > 1);
        self.place_tools
            .iter()
            .filter(|((p, t), _)| t == tool && repeated(p))
            .map(|(_, runs)| *runs as u64)
            .sum()
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
