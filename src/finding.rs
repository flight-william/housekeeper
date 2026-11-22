//! The one finding shape every adapter returns; the reply, the record and the
//! excuse check all read it, so a change here changes all three.

use serde::Serialize;

/// Where a finding is: a file in the repo, or the repo as a whole.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    /// A path relative to the repo root, with `/` separators.
    File(String),
    /// The whole repo (coverage, a dependency, a missing tool).
    Repo,
}

/// A measured value with its unit and limit, for rules that count (R3, R4, R7).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Measure {
    pub value: f64,
    pub unit: String,
    pub limit: f64,
}

/// One problem an agent must fix.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Finding {
    /// The house rule, "R1".."R17".
    pub rule: String,
    pub place: Place,
    /// 1-based line, when the tool gives one.
    pub line: Option<u32>,
    /// What is wrong, one short sentence, no trailing period.
    pub what: String,
    /// How to fix it, one short imperative sentence, no trailing period.
    pub fix: String,
    /// The tool's own name for it, e.g. "ruff C901", "clippy::too_many_lines".
    pub tool_rule: String,
    pub measure: Option<Measure>,
}
