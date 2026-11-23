//! `house review`: the run records read back into one report, so the rules are tuned
//! with evidence: the record folder (`read`) and a filter in, a `Review` out. Nothing
//! here reads the clock; the cli gives the period.
//!
//! The record folder also holds lines other programs wrote. A line is house's own
//! record when it is valid UTF-8 and a JSON object with a `start` time (`start`
//! reads it as its UTC day), a `stage` and a `tools` list of named tools; any other
//! line, a blank or broken one included, is skipped and counted.

mod line;
mod read;
mod start;
mod suggest;
mod text;

pub use line::MAX_SECONDS;
pub use read::MAX_LINE;
pub use suggest::{NEVER_FIRES_MIN_RUNS, Suggestion};

use crate::excuses::Date;
use crate::reply::cannot_rule;
use line::{Run, Tool, parse};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Which runs a review covers: those whose start day is within `from..=to` (either
/// end open when `None`), in `repo` when given: the record's repo path itself, or its
/// last folder name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub from: Option<Date>,
    pub to: Option<Date>,
    pub repo: Option<String>,
}

impl Filter {
    fn covers(&self, day: Date, repo: &str) -> bool {
        self.from.is_none_or(|from| day >= from)
            && self.to.is_none_or(|to| day <= to)
            && self.in_repo(repo)
    }

    fn in_repo(&self, repo: &str) -> bool {
        self.repo
            .as_deref()
            .is_none_or(|want| same_repo(repo, want))
    }
}

fn same_repo(repo: &str, want: &str) -> bool {
    repo == want || Path::new(repo).file_name().is_some_and(|name| name == want)
}

/// A place a rule failed: the repo, the rule, the file (or "repo") and the tool's
/// own rule, which stands in for the symbol the record does not keep.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Place {
    pub repo: String,
    pub rule: String,
    pub place: String,
    pub tool_rule: String,
}

/// Findings, excused findings and cannot-checks, for one rule or one tool.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Count {
    pub findings: u64,
    pub excused: u64,
    pub cannot_check: u64,
}

/// A tool across the runs: its counts, the runs it was not skipped in, and the
/// seconds of each of those runs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolStat {
    pub count: Count,
    pub runs: usize,
    /// The runs it was not skipped in and could check.
    pub checked: usize,
    /// It ran in a record written before 0.3.0 that excused findings: some may have
    /// been its own, so it cannot be said to have found nothing.
    pub excused_unknown: bool,
    pub seconds: Vec<f64>,
}

impl ToolStat {
    pub fn total_seconds(&self) -> f64 {
        not_negative(self.seconds.iter().sum())
    }

    pub fn median_seconds(&self) -> f64 {
        let mut s = self.seconds.clone();
        s.sort_by(f64::total_cmp);
        not_negative(match s.len() {
            0 => 0.0,
            n if n % 2 == 1 => s[n / 2],
            n => (s[n / 2 - 1] + s[n / 2]) / 2.0,
        })
    }
}

/// Seconds are never below zero: a sum of no runs is -0.0 in Rust, and a record may
/// hold -0.0, either of which would print as "-0.0 s".
fn not_negative(seconds: f64) -> f64 {
    if seconds > 0.0 { seconds } else { 0.0 }
}

/// What the runs in a period add up to.
#[derive(Debug, Default)]
pub struct Review {
    pub filter: Filter,
    /// The start days of the first and the last run covered.
    pub first: Option<Date>,
    pub last: Option<Date>,
    pub runs: usize,
    /// Runs by stage: commit, commit-msg, push, all.
    pub stages: BTreeMap<String, usize>,
    /// Lines in the record files that are not house's run records.
    pub skipped_lines: usize,
    /// Entries named like a month's record file that are not regular files.
    pub skipped_files: usize,
    /// Every repo a covered run was in.
    pub repos: BTreeSet<String>,
    /// Every rule a record of the repo names, in the period or not, with its counts
    /// in the period.
    pub rules: BTreeMap<String, Count>,
    /// Every tool that ran (was not skipped) on a covered run.
    pub tools: BTreeMap<String, ToolStat>,
    /// For each rule, the tools a record of the repo names it under, in the period
    /// or not: the tools that check it.
    pub rule_tools: BTreeMap<String, BTreeSet<String>>,
    /// The covered runs by the set of tools that checked in them (not skipped, no
    /// cannot-check): how often each rule was checked.
    pub checked_sets: BTreeMap<BTreeSet<String>, usize>,
    /// Excused findings in records that name no tool's excused map.
    pub excused_unattributed: u64,
    /// A sum passed `u64::MAX`; the review is then refused, never wrapped.
    pub overflowed: bool,
    /// Each place a finding was on, with the number of runs it was found in.
    pub places: BTreeMap<Place, usize>,
    /// The findings on each place, every one counted, across the covered runs.
    pub place_findings: BTreeMap<Place, u64>,
    /// The findings on a place by a tool: the one tool of the run whose findings hold
    /// the place's rule. When two tools hold it, the finding is no tool's: the record
    /// does not say which, and none is invented.
    pub place_tools: BTreeMap<(Place, String), u64>,
}

/// The most distinct places, rules, tools, stages and repos a review keeps: what it
/// holds grows with these, never with the number or size of the lines.
pub const MAX_KEYS: usize = 100_000;

/// Why a review was not made.
#[derive(Debug)]
pub enum ReviewError {
    /// The record folder or a record file could not be read.
    Read(std::io::Error),
    /// The records are past what a review holds: a sum past `u64::MAX`, or more
    /// than `MAX_KEYS` distinct keys. One sentence, no trailing period.
    Limit(String),
}

/// The review of the records in `dir` for the runs `filter` covers.
pub fn review_folder(dir: &Path, filter: &Filter) -> Result<Review, ReviewError> {
    let mut r = Review {
        filter: filter.clone(),
        ..Review::default()
    };
    let folder = read::each_line(dir, |line| {
        if !r.full() {
            r.add_line(line);
        }
    })
    .map_err(ReviewError::Read)?;
    r.skipped_files = folder.skipped_files;
    if r.overflowed {
        return Err(ReviewError::Limit(
            "a count in the records adds up to more than 18446744073709551615".into(),
        ));
    }
    if r.full() {
        return Err(ReviewError::Limit(format!(
            "the records name more than {MAX_KEYS} distinct places, rules, tools, stages and repos"
        )));
    }
    Ok(r)
}

/// `total + n`, or `overflowed` set and `total` kept when the sum does not fit.
fn add_count(total: &mut u64, n: u32, overflowed: &mut bool) {
    add_findings(total, u64::from(n), overflowed);
}

fn add_findings(total: &mut u64, n: u64, overflowed: &mut bool) {
    match total.checked_add(n) {
        Some(sum) => *total = sum,
        None => *overflowed = true,
    }
}

impl Review {
    /// Past `MAX_KEYS`: nothing more is added, and the review is refused.
    fn full(&self) -> bool {
        let keys = self.places.len()
            + self.rules.len()
            + self.checked_sets.len()
            + self.tools.len()
            + self.stages.len()
            + self.repos.len();
        keys > MAX_KEYS
    }

    /// One line of a record file: a covered run is added, and any line that is not a
    /// house record is counted: a blank one, one that is not UTF-8, one too long.
    fn add_line(&mut self, line: read::Line) {
        let read::Line::Bytes(bytes) = line else {
            self.skipped_lines += 1;
            return;
        };
        let Ok(line) = std::str::from_utf8(bytes) else {
            self.skipped_lines += 1;
            return;
        };
        let Some((run, day)) = parse(line) else {
            self.skipped_lines += 1;
            return;
        };
        if self.filter.in_repo(&run.repo) {
            for tool in &run.tools {
                for rule in tool.rules() {
                    let tools = self.rule_tools.entry(rule.clone()).or_default();
                    tools.insert(tool.name.clone());
                    self.rules.entry(rule).or_default();
                }
            }
        }
        if self.filter.covers(day, &run.repo) {
            self.add(&run, day);
        }
    }
}

impl Review {
    fn add(&mut self, run: &Run, day: Date) {
        self.first = Some(self.first.map_or(day, |d| d.min(day)));
        self.last = Some(self.last.map_or(day, |d| d.max(day)));
        self.runs += 1;
        *self.stages.entry(run.stage.clone()).or_default() += 1;
        self.repos.insert(run.repo.clone());
        let unattributed = run.excused > 0 && run.tools.iter().all(|t| t.excused.is_none());
        if unattributed {
            add_count(
                &mut self.excused_unattributed,
                run.excused,
                &mut self.overflowed,
            );
        }
        for tool in &run.tools {
            self.add_tool(tool, unattributed);
        }
        let checked: BTreeSet<String> = run
            .tools
            .iter()
            .filter(|t| !t.skipped && t.cannot_check.is_none())
            .map(|t| t.name.clone())
            .collect();
        *self.checked_sets.entry(checked).or_default() += 1;
        self.add_places(run);
    }

    /// A tool skipped on a run did not run: a tool skipped on every run of the period
    /// is not one of its tools.
    fn add_tool(&mut self, tool: &Tool, excused_unknown: bool) {
        if tool.skipped {
            return;
        }
        let stat = self.tools.entry(tool.name.clone()).or_default();
        stat.runs += 1;
        stat.excused_unknown |= excused_unknown;
        stat.checked += usize::from(tool.cannot_check.is_none());
        stat.seconds.push(tool.seconds);
        for (rule, n) in &tool.findings {
            add_count(&mut stat.count.findings, *n, &mut self.overflowed);
            let by_rule = &mut self.rules.entry(rule.clone()).or_default().findings;
            add_count(by_rule, *n, &mut self.overflowed);
        }
        for (rule, n) in tool.excused.iter().flatten() {
            add_count(&mut stat.count.excused, *n, &mut self.overflowed);
            let by_rule = &mut self.rules.entry(rule.clone()).or_default().excused;
            add_count(by_rule, *n, &mut self.overflowed);
        }
        if tool.cannot_check.is_some() {
            stat.count.cannot_check += 1;
            let rule = cannot_rule(&tool.name).to_string();
            self.rules.entry(rule).or_default().cannot_check += 1;
        }
    }

    /// The places the run's findings were on: each place's runs count once per run,
    /// its findings once per finding, by the tool that found them.
    fn add_places(&mut self, run: &Run) {
        let mut places: BTreeMap<Place, u64> = BTreeMap::new();
        for f in &run.findings {
            let place = Place {
                repo: run.repo.clone(),
                rule: f.rule.clone(),
                place: f.place.clone(),
                tool_rule: f.tool_rule.clone(),
            };
            *places.entry(place).or_default() += 1;
        }
        for (place, n) in places {
            let mut holders = run
                .tools
                .iter()
                .filter(|t| t.findings.contains_key(&place.rule));
            if let (Some(tool), None) = (holders.next(), holders.next()) {
                let key = (place.clone(), tool.name.clone());
                add_findings(
                    self.place_tools.entry(key).or_default(),
                    n,
                    &mut self.overflowed,
                );
            }
            let findings = self.place_findings.entry(place.clone()).or_default();
            add_findings(findings, n, &mut self.overflowed);
            *self.places.entry(place).or_default() += 1;
        }
    }

    /// The places found on more than one run, most runs first.
    pub fn repeated(&self) -> Vec<(&Place, usize)> {
        let mut repeated: Vec<(&Place, usize)> = self
            .places
            .iter()
            .filter(|(_, runs)| **runs > 1)
            .map(|(p, runs)| (p, *runs))
            .collect();
        repeated.sort_by_key(|(p, runs)| (std::cmp::Reverse(*runs), *p));
        repeated
    }

    /// The rules in rule-number order: R2 before R10, R17 before R17/subject.
    pub fn rules_in_order(&self) -> Vec<(&str, &Count)> {
        let mut rules: Vec<(&str, &Count)> =
            self.rules.iter().map(|(k, v)| (k.as_str(), v)).collect();
        rules.sort_by_key(|(rule, _)| (rule_number(rule), *rule));
        rules
    }
}

/// The number in `R12` or `R17/subject`; a name without one sorts last.
fn rule_number(rule: &str) -> u32 {
    let digits: String = rule
        .trim_start_matches('R')
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().unwrap_or(u32::MAX)
}
