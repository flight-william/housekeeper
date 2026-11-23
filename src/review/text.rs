//! The review as plain text, one report, or as one JSON object (`--json`).

use super::{Count, Review};
use serde_json::{Value, json};

/// The stages in the order a change meets them; any other name follows, by name.
const STAGES: [&str; 4] = ["commit", "commit-msg", "push", "all"];

impl Review {
    /// The plain report: the header, then the skipped lines when there are any.
    pub fn text(&self) -> String {
        let mut out = self.header();
        out.push('\n');
        if self.skipped_lines > 0 {
            let lines = plural(self.skipped_lines, "line", "lines");
            out.push_str(&format!("{lines} skipped: not house run records\n"));
        }
        if self.skipped_files > 0 {
            let files = plural(self.skipped_files, "record file", "record files");
            let what = if self.skipped_files == 1 {
                "not a regular file"
            } else {
                "not regular files"
            };
            out.push_str(&format!("{files} skipped: {what}\n"));
        }
        out.push_str("per rule:\n");
        for (rule, count) in self.rules_in_order() {
            out.push_str(&format!("  {rule}: {}\n", counts(count)));
        }
        if self.excused_unattributed > 0 {
            out.push_str(&format!(
                "{} excused in records that do not say which rule (before house 0.3.0)\n",
                self.excused_unattributed
            ));
        }
        out.push_str("per tool:\n");
        for (tool, stat) in &self.tools {
            out.push_str(&format!(
                "  {tool}: {}, {}, {:.1} s total, {:.1} s median\n",
                plural(stat.runs, "run", "runs"),
                counts(&stat.count),
                stat.total_seconds(),
                stat.median_seconds()
            ));
        }
        out.push_str(&self.repeated_text());
        let suggestions = self.suggestions();
        if suggestions.is_empty() {
            out.push_str("suggestions: none\n");
        } else {
            out.push_str("suggestions (to weigh, not orders):\n");
            for s in suggestions {
                out.push_str(&format!("  suggestion: {}\n", s.text));
            }
        }
        out
    }

    /// The repeated places; each names its repo's folder when the runs span repos.
    fn repeated_text(&self) -> String {
        let repeated = self.repeated();
        if repeated.is_empty() {
            return "repeated places: none\n".into();
        }
        let mut out = String::from("repeated places (failed on more than one run):\n");
        for (p, runs) in repeated {
            let repo = if self.repos.len() > 1 {
                format!(" in {}", folder_name(&p.repo))
            } else {
                String::new()
            };
            out.push_str(&format!(
                "  {} {} [{}]{repo}: {runs} runs\n",
                p.rule, p.place, p.tool_rule
            ));
        }
        out
    }

    /// `--json`: the same report as one object; an open end of the period is `null`
    /// when no run is covered.
    pub fn json(&self) -> Value {
        let (from, to) = self.period();
        let stages: serde_json::Map<String, Value> = self
            .stage_order()
            .into_iter()
            .map(|(s, n)| (s.to_string(), n.into()))
            .collect();
        let rules: Vec<Value> = self
            .rules_in_order()
            .into_iter()
            .map(|(rule, c)| {
                json!({"rule": rule, "findings": c.findings, "excused": c.excused, "cannot_check": c.cannot_check})
            })
            .collect();
        let tools: Vec<Value> = self
            .tools
            .iter()
            .map(|(tool, t)| {
                json!({
                    "tool": tool,
                    "runs": t.runs,
                    "findings": t.count.findings,
                    "excused": t.count.excused,
                    "cannot_check": t.count.cannot_check,
                    "seconds_total": round(t.total_seconds()),
                    "seconds_median": round(t.median_seconds()),
                })
            })
            .collect();
        let repeated: Vec<Value> = self
            .repeated()
            .into_iter()
            .map(|(p, runs)| {
                json!({"repo": p.repo, "rule": p.rule, "place": p.place, "tool_rule": p.tool_rule, "runs": runs})
            })
            .collect();
        json!({
            "from": from.map(|d| d.to_string()),
            "to": to.map(|d| d.to_string()),
            "runs": self.runs,
            "stages": stages,
            "skipped_lines": self.skipped_lines,
            "skipped_files": self.skipped_files,
            "rules": rules,
            "tools": tools,
            "excused_unattributed": self.excused_unattributed,
            "repeated": repeated,
            "suggestions": self.suggestions(),
        })
    }

    /// The period: each end as asked, else the first or last run covered.
    fn period(&self) -> (Option<crate::excuses::Date>, Option<crate::excuses::Date>) {
        (
            self.filter.from.or(self.first),
            self.filter.to.or(self.last),
        )
    }

    /// `house review: <from> to <to>, N runs: 2 commit, 1 push`.
    fn header(&self) -> String {
        let (from, to) = self.period();
        let from = from.map_or("the first record".into(), |d| d.to_string());
        let to = to.map_or("the last record".into(), |d| d.to_string());
        let mut out = format!(
            "house review: {from} to {to}, {}",
            plural(self.runs, "run", "runs")
        );
        let stages = self.stage_order();
        if !stages.is_empty() {
            let parts: Vec<String> = stages.iter().map(|(s, n)| format!("{n} {s}")).collect();
            out.push_str(&format!(": {}", parts.join(", ")));
        }
        out
    }

    fn stage_order(&self) -> Vec<(&str, usize)> {
        let known = STAGES
            .iter()
            .filter_map(|s| self.stages.get(*s).map(|n| (*s, *n)));
        let others = self
            .stages
            .iter()
            .filter(|(s, _)| !STAGES.contains(&s.as_str()))
            .map(|(s, n)| (s.as_str(), *n));
        known.chain(others).collect()
    }
}

fn folder_name(repo: &str) -> &str {
    repo.rsplit('/').find(|s| !s.is_empty()).unwrap_or(repo)
}

/// `2 findings, 1 excused, 0 cannot check`.
fn counts(c: &Count) -> String {
    format!(
        "{}, {} excused, {} cannot check",
        plural(c.findings as usize, "finding", "findings"),
        c.excused,
        c.cannot_check
    )
}

/// Seconds to the millisecond, as the record keeps them.
fn round(seconds: f64) -> f64 {
    (seconds * 1000.0).round() / 1000.0
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
