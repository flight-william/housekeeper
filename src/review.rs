//! `house review`: the run records read back into one report, so the rules are tuned
//! with evidence. Pure but for `load`: the records' lines and a filter in, a `Review`
//! out. Nothing here reads the clock; the cli gives the period.
//!
//! The record folder also holds lines other programs wrote. A line is house's own
//! record when it is a JSON object with a `start` that begins with a real date, a
//! `stage` and a `tools` list of named tools; any other line, a broken one included,
//! is skipped and counted.

mod suggest;
mod text;

pub use suggest::Suggestion;

use crate::excuses::Date;
use crate::reply::cannot_rule;
use serde::Deserialize;
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
            && self
                .repo
                .as_deref()
                .is_none_or(|want| same_repo(repo, want))
    }
}

fn same_repo(repo: &str, want: &str) -> bool {
    repo == want || Path::new(repo).file_name().is_some_and(|name| name == want)
}

/// One record line, the fields a review reads; every other field is ignored.
#[derive(Deserialize)]
struct Run {
    start: String,
    #[serde(default)]
    repo: String,
    stage: String,
    tools: Vec<Tool>,
    /// The run's excused total.
    #[serde(default)]
    excused: u64,
    /// The findings left to fix, one entry each.
    #[serde(default)]
    findings: Vec<Failed>,
}

#[derive(Deserialize)]
struct Failed {
    rule: String,
    place: String,
    #[serde(default)]
    tool_rule: String,
}

#[derive(Deserialize)]
struct Tool {
    name: String,
    #[serde(default)]
    seconds: f64,
    #[serde(default)]
    skipped: bool,
    #[serde(default)]
    findings: BTreeMap<String, u64>,
    #[serde(default)]
    cannot_check: Option<String>,
    /// Rule to count; `None` in records written before house 0.3.0.
    #[serde(default)]
    excused: Option<BTreeMap<String, u64>>,
}

impl Tool {
    /// The rules this tool's entry names: its findings, its excuses, its cannot-check.
    fn rules(&self) -> impl Iterator<Item = String> + '_ {
        let excused = self.excused.iter().flat_map(|m| m.keys().cloned());
        let cannot = self
            .cannot_check
            .as_ref()
            .map(|_| cannot_rule(&self.name).to_string());
        self.findings.keys().cloned().chain(excused).chain(cannot)
    }
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
    pub seconds: Vec<f64>,
}

impl ToolStat {
    pub fn total_seconds(&self) -> f64 {
        self.seconds.iter().sum()
    }

    pub fn median_seconds(&self) -> f64 {
        let mut s = self.seconds.clone();
        s.sort_by(f64::total_cmp);
        match s.len() {
            0 => 0.0,
            n if n % 2 == 1 => s[n / 2],
            n => (s[n / 2 - 1] + s[n / 2]) / 2.0,
        }
    }
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
    /// Every repo a covered run was in.
    pub repos: BTreeSet<String>,
    /// Every rule a record of the repo names, in the period or not, with its counts
    /// in the period.
    pub rules: BTreeMap<String, Count>,
    pub tools: BTreeMap<String, ToolStat>,
    /// Excused findings in records that name no tool's excused map.
    pub excused_unattributed: u64,
    /// Each place a finding was on, with the number of runs it was found in.
    pub places: BTreeMap<Place, usize>,
    /// The runs a place was found in by a tool: the one tool of the run whose
    /// findings hold the place's rule (none is counted when two tools hold it).
    pub place_tools: BTreeMap<(Place, String), usize>,
}

/// The lines of every `<YYYY-MM>.jsonl` in `dir`, oldest month first; a folder that
/// does not exist holds no records. Bytes that are not UTF-8 make a broken line.
pub fn load(dir: &Path) -> std::io::Result<Vec<String>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(month_file)
        {
            files.push(path);
        }
    }
    files.sort();
    let mut lines = Vec::new();
    for file in files {
        let bytes = std::fs::read(&file)?;
        let text = String::from_utf8_lossy(&bytes);
        lines.extend(
            text.lines()
                .filter(|l| !l.trim().is_empty())
                .map(String::from),
        );
    }
    Ok(lines)
}

/// `YYYY-MM.jsonl`, the name house gives a month's records.
fn month_file(name: &str) -> bool {
    let b = name.as_bytes();
    name.len() == 13
        && name.ends_with(".jsonl")
        && b[4] == b'-'
        && b[..4].iter().chain(&b[5..7]).all(u8::is_ascii_digit)
}

/// A house record line and its start day, or `None` for any other line.
fn parse(line: &str) -> Option<(Run, Date)> {
    let run: Run = serde_json::from_str(line).ok()?;
    let day = Date::parse(run.start.get(..10)?)?;
    Some((run, day))
}

/// The review of `lines` (from `load`) for the runs `filter` covers.
pub fn review(lines: &[String], filter: &Filter) -> Review {
    let mut r = Review {
        filter: filter.clone(),
        ..Review::default()
    };
    let in_repo = Filter {
        repo: filter.repo.clone(),
        ..Filter::default()
    };
    for line in lines {
        let Some((run, day)) = parse(line) else {
            r.skipped_lines += 1;
            continue;
        };
        if in_repo.covers(day, &run.repo) {
            for rule in run.tools.iter().flat_map(Tool::rules) {
                r.rules.entry(rule).or_default();
            }
        }
        if filter.covers(day, &run.repo) {
            r.add(&run, day);
        }
    }
    r
}

impl Review {
    fn add(&mut self, run: &Run, day: Date) {
        self.first = Some(self.first.map_or(day, |d| d.min(day)));
        self.last = Some(self.last.map_or(day, |d| d.max(day)));
        self.runs += 1;
        *self.stages.entry(run.stage.clone()).or_default() += 1;
        self.repos.insert(run.repo.clone());
        if run.tools.iter().all(|t| t.excused.is_none()) {
            self.excused_unattributed += run.excused;
        }
        for tool in &run.tools {
            self.add_tool(tool);
        }
        self.add_places(run);
    }

    fn add_tool(&mut self, tool: &Tool) {
        let stat = self.tools.entry(tool.name.clone()).or_default();
        if !tool.skipped {
            stat.runs += 1;
            stat.checked += usize::from(tool.cannot_check.is_none());
            stat.seconds.push(tool.seconds);
        }
        for (rule, n) in &tool.findings {
            stat.count.findings += n;
            self.rules.entry(rule.clone()).or_default().findings += n;
        }
        for (rule, n) in tool.excused.iter().flatten() {
            stat.count.excused += n;
            self.rules.entry(rule.clone()).or_default().excused += n;
        }
        if tool.cannot_check.is_some() {
            stat.count.cannot_check += 1;
            let rule = cannot_rule(&tool.name).to_string();
            self.rules.entry(rule).or_default().cannot_check += 1;
        }
    }

    /// Each place the run's findings were on, once per run, and the tool that found it.
    fn add_places(&mut self, run: &Run) {
        let places: BTreeSet<Place> = run
            .findings
            .iter()
            .map(|f| Place {
                repo: run.repo.clone(),
                rule: f.rule.clone(),
                place: f.place.clone(),
                tool_rule: f.tool_rule.clone(),
            })
            .collect();
        for place in places {
            let mut holders = run
                .tools
                .iter()
                .filter(|t| t.findings.contains_key(&place.rule));
            if let (Some(tool), None) = (holders.next(), holders.next()) {
                let key = (place.clone(), tool.name.clone());
                *self.place_tools.entry(key).or_default() += 1;
            }
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
