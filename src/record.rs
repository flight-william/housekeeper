//! One JSON line per run, appended to `$HOUSE_LOG_DIR/<YYYY-MM>.jsonl`
//! (default `$HOME/Library/Logs/house/runs/`). No message bodies.

use crate::reply::{Report, place_text, stage_name};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Where records go: `$HOUSE_LOG_DIR`, else `$HOME/Library/Logs/house/runs`.
pub fn log_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("HOUSE_LOG_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Logs/house/runs"))
}

/// The findings the excuse list took out of a run's reply: `total`, and for each tool
/// (by its place in the report's tools) the count by rule, recorded as the tool's
/// `excused` map.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Excused {
    pub total: usize,
    pub by_tool: Vec<BTreeMap<String, u32>>,
}

/// The facts about a run that the report does not hold.
pub struct RunInfo<'a> {
    pub start: SystemTime,
    pub root: &'a Path,
    pub head: Option<String>,
    /// The repository's root commit (`files::root_commit`): one value for all its
    /// worktrees, where `root` is the one folder the run was in.
    pub root_commit: Option<String>,
    /// The repository's main checkout (`files::main_checkout`).
    pub main_checkout: Option<PathBuf>,
    pub user: String,
}

/// The record line for one run.
pub fn line(info: &RunInfo, report: &Report) -> Value {
    let tools: Vec<Value> = report
        .tools
        .iter()
        .map(|t| {
            let mut per_rule = BTreeMap::new();
            for f in &t.findings {
                *per_rule.entry(f.rule.clone()).or_insert(0u32) += 1;
            }
            json!({
                "name": t.name,
                "seconds": round(t.seconds),
                "exit": t.exit,
                "skipped": t.skipped,
                "findings": per_rule,
                "cannot_check": t.cannot.as_ref().map(|c| format!("{}. fix: {}", c.what, c.fix)),
                "unread": t.unread.len(),
                "unread_files": t.unread,
            })
        })
        .collect();
    let findings: Vec<Value> = report
        .findings()
        .into_iter()
        .map(|f| {
            json!({
                "rule": f.rule,
                "place": place_text(&f.place),
                "line": f.line,
                "tool_rule": f.tool_rule,
            })
        })
        .collect();
    let mut m = Map::new();
    m.insert("start".into(), rfc3339(info.start).into());
    m.insert("repo".into(), info.root.display().to_string().into());
    m.insert("head".into(), info.head.clone().into());
    m.insert("root_commit".into(), info.root_commit.clone().into());
    let main = info.main_checkout.as_ref().map(|p| p.display().to_string());
    m.insert("main_checkout".into(), main.into());
    m.insert("stage".into(), stage_name(report.stage).into());
    m.insert("user".into(), info.user.clone().into());
    m.insert("seconds".into(), round(report.seconds).into());
    m.insert("exit".into(), report.exit().into());
    m.insert("tools".into(), tools.into());
    m.insert("findings".into(), findings.into());
    Value::Object(m)
}

/// Put `excused` into `line`: the run's total as `excused`, each tool's by-rule count
/// as that tool's `excused` map (empty when nothing of it was excused).
pub fn add_excused(line: &mut Value, excused: &Excused) {
    let Value::Object(m) = line else {
        return;
    };
    m.insert("excused".into(), excused.total.into());
    if let Some(Value::Array(tools)) = m.get_mut("tools") {
        for (i, tool) in tools.iter_mut().enumerate() {
            if let Value::Object(t) = tool {
                let by_rule = excused.by_tool.get(i).cloned().unwrap_or_default();
                t.insert("excused".into(), json!(by_rule));
            }
        }
    }
}

/// Append `line` to `<dir>/<YYYY-MM>.jsonl` (month of `start`), creating the folder.
pub fn append(dir: &Path, start: SystemTime, line: &Value) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.jsonl", &rfc3339(start)[..7]));
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(file, "{line}")?;
    Ok(path)
}

/// UTC time as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn rfc3339(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        s / 3600,
        s / 60 % 60,
        s % 60
    )
}

/// Days since 1970-01-01 to (year, month, day) in the proleptic Gregorian calendar:
/// shift the epoch to 0000-03-01 so the leap day ends the year, split into 400-year
/// eras of 146 097 days, then read the year, month and day from the day of the era.
pub(crate) fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn round(seconds: f64) -> f64 {
    (seconds * 1000.0).round() / 1000.0
}
