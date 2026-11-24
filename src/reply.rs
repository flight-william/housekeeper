//! The one reply agents read, and house's exit code.
//!
//! ```text
//! house commit: 3 to fix, 1 cannot check (4.1 s)
//! R4 src/score.py:88: complexity 14, limit 10. fix: split the branches into helpers. [ruff C901]
//! R2 cannot check: swiftlint is not installed. fix: brew install swiftlint [swiftlint]
//! ```

use crate::adapter::{CannotCheck, Stage};
use crate::finding::{Finding, Place};
use serde_json::json;

/// What one tool did in a run.
#[derive(Debug)]
pub struct ToolRun {
    pub name: String,
    pub seconds: f64,
    /// The tool's own exit code; `None` when it was skipped or never ran.
    pub exit: Option<i32>,
    pub skipped: bool,
    pub findings: Vec<Finding>,
    pub cannot: Option<CannotCheck>,
    /// Files the tool ran on but its own parser could not read (`Adapter::unread`):
    /// counted in the header and the record, neither findings nor cannot-check.
    pub unread: Vec<String>,
}

/// A whole run: every tool, the stage, the total time.
#[derive(Debug)]
pub struct Report {
    pub stage: Stage,
    pub seconds: f64,
    pub tools: Vec<ToolRun>,
}

pub fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::Commit => "commit",
        Stage::Push => "push",
        Stage::Merge => "merge",
    }
}

/// The rule a cannot-check line reports: the test step is R7, formatters are R1,
/// every other tool R2. (`CannotCheck` carries no rule, so the core derives it from
/// the tool's name.)
pub fn cannot_rule(tool: &str) -> &'static str {
    if tool == crate::tests_step::NAME {
        "R7"
    } else if tool.contains("fmt") || tool.contains("format") {
        "R1"
    } else {
        "R2"
    }
}

impl Report {
    pub fn findings(&self) -> Vec<&Finding> {
        let mut all: Vec<&Finding> = self.tools.iter().flat_map(|t| &t.findings).collect();
        all.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
        all
    }

    pub fn cannots(&self) -> Vec<(&str, &CannotCheck)> {
        self.tools
            .iter()
            .filter_map(|t| t.cannot.as_ref().map(|c| (t.name.as_str(), c)))
            .collect()
    }

    /// 0 nothing to fix, 1 findings, 2 any cannot-check (2 wins).
    pub fn exit(&self) -> i32 {
        if !self.cannots().is_empty() {
            2
        } else if !self.findings().is_empty() {
            1
        } else {
            0
        }
    }

    /// The plain-text reply, one line per finding, cannot-checks last.
    pub fn text(&self) -> String {
        self.text_as(stage_name(self.stage), 0)
    }

    /// The reply headed `house <label>:`, with "N excused" in the header when N > 0.
    pub fn text_as(&self, label: &str, excused: usize) -> String {
        let findings = self.findings();
        let cannots = self.cannots();
        let mut parts = Vec::new();
        if !findings.is_empty() {
            parts.push(format!("{} to fix", findings.len()));
        }
        if !cannots.is_empty() {
            parts.push(format!("{} cannot check", cannots.len()));
        }
        if parts.is_empty() {
            parts.push("nothing to fix".into());
        }
        if excused > 0 {
            parts.push(format!("{excused} excused"));
        }
        let mut out = format!(
            "house {label}: {} ({:.1} s)",
            parts.join(", "),
            self.seconds
        );
        for t in self.tools.iter().filter(|t| !t.unread.is_empty()) {
            let n = t.unread.len();
            let files = if n == 1 { "file" } else { "files" };
            out.push_str(&format!(" ({n} {files} {} could not read)", t.name));
        }
        out.push('\n');
        for f in findings {
            out.push_str(&finding_line(f));
            out.push('\n');
        }
        for (tool, c) in cannots {
            out.push_str(&format!(
                "{} cannot check: {}. fix: {} [{tool}]\n",
                cannot_rule(tool),
                c.what,
                c.fix
            ));
        }
        out
    }

    /// `--json`: the findings and cannot-checks as one object.
    pub fn json(&self) -> serde_json::Value {
        let cannots: Vec<_> = self
            .cannots()
            .into_iter()
            .map(|(tool, c)| json!({"rule": cannot_rule(tool), "tool": tool, "what": c.what, "fix": c.fix}))
            .collect();
        json!({
            "stage": stage_name(self.stage),
            "seconds": self.seconds,
            "exit": self.exit(),
            "findings": self.findings(),
            "cannot_check": cannots,
        })
    }
}

pub fn place_text(place: &Place) -> &str {
    match place {
        Place::File(path) => path,
        Place::Repo => "repo",
    }
}

pub fn finding_line(f: &Finding) -> String {
    let line = f.line.map(|l| format!(":{l}")).unwrap_or_default();
    format!(
        "{} {}{line}: {}. fix: {}. [{}]",
        f.rule,
        place_text(&f.place),
        f.what,
        f.fix,
        f.tool_rule
    )
}

/// Files first (by path), the repo last; then line (none first), then rule number.
fn sort_key(f: &Finding) -> (bool, &str, Option<u32>, u32, &str) {
    let rule_number = f.rule.trim_start_matches('R').parse().unwrap_or(u32::MAX);
    (
        f.place == Place::Repo,
        place_text(&f.place),
        f.line,
        rule_number,
        &f.tool_rule,
    )
}
