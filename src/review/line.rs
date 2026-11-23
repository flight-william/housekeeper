//! One line of the run record, the fields a review reads, and what makes a line
//! house's own record: valid JSON of this shape, counts a `u32` can hold, seconds
//! from 0 to `MAX_SECONDS`, and a start that is a real time (`start::utc_day`).

use super::start;
use crate::excuses::Date;
use crate::reply::cannot_rule;
use serde::Deserialize;
use std::collections::BTreeMap;

/// One record line, the fields a review reads; every other field is ignored.
#[derive(Deserialize)]
pub(super) struct Run {
    pub(super) start: String,
    #[serde(default)]
    pub(super) repo: String,
    pub(super) stage: String,
    pub(super) tools: Vec<Tool>,
    /// The run's excused total. Every count is a `u32`, the most house writes: a
    /// larger or negative one makes the line not a house record.
    #[serde(default)]
    pub(super) excused: u32,
    /// The findings left to fix, one entry each.
    #[serde(default)]
    pub(super) findings: Vec<Failed>,
}

#[derive(Deserialize)]
pub(super) struct Failed {
    pub(super) rule: String,
    pub(super) place: String,
    #[serde(default)]
    pub(super) tool_rule: String,
}

#[derive(Deserialize)]
pub(super) struct Tool {
    pub(super) name: String,
    #[serde(default, deserialize_with = "seconds")]
    pub(super) seconds: f64,
    #[serde(default)]
    pub(super) skipped: bool,
    #[serde(default)]
    pub(super) findings: BTreeMap<String, u32>,
    #[serde(default)]
    pub(super) cannot_check: Option<String>,
    /// Rule to count; `None` in records written before house 0.3.0.
    #[serde(default)]
    pub(super) excused: Option<BTreeMap<String, u32>>,
}

impl Tool {
    /// The rules this tool's entry names: its findings, its excuses, its cannot-check.
    pub(super) fn rules(&self) -> impl Iterator<Item = &str> + '_ {
        let excused = self
            .excused
            .iter()
            .flat_map(|m| m.keys().map(String::as_str));
        let cannot = self.cannot_check.as_ref().map(|_| cannot_rule(&self.name));
        self.findings
            .keys()
            .map(String::as_str)
            .chain(excused)
            .chain(cannot)
    }
}

/// The most seconds a tool's entry may hold: a day. house stops each command at its
/// hang guard, an hour, so a larger, negative or non-finite duration was not written
/// by house, and the line is not a house record. Totals then stay finite numbers.
pub const MAX_SECONDS: f64 = 86_400.0;

fn seconds<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    let s = f64::deserialize(d)?;
    if (0.0..=MAX_SECONDS).contains(&s) {
        Ok(s)
    } else {
        Err(serde::de::Error::custom("seconds out of range"))
    }
}

/// A house record line and its start day, or `None` for any other line.
pub(super) fn parse(line: &str) -> Option<(Run, Date)> {
    let run: Run = serde_json::from_str(line).ok()?;
    let day = start::utc_day(&run.start)?;
    Some((run, day))
}
