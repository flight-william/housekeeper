//! The dated excuse list, `.house-excuses.toml` at the repo root. Old code sits on it
//! until a deadline; new code cannot add to it, and an entry that matches nothing must
//! be deleted, so the list only shrinks.
//!
//! An entry excuses a finding with the same rule and file, whose `what` names the
//! entry's symbol as a whole word (when the entry has one), measuring no more than the
//! entry's value (when both have one). It excuses at most `count` such findings (1 when
//! the entry gives none): a new function in an excused file is not excused, nor a new
//! suppression beside an excused one.

use crate::adapter::CannotCheck;
use crate::finding::{Finding, Place};
use crate::run::git;
use serde::Deserialize;
use std::fmt;
use std::path::Path;

/// The excuse list's path, relative to the repo root.
pub const FILE: &str = ".house-excuses.toml";

/// The house rule the excuse list's own findings report (R9, nothing silenced).
const RULE: &str = "R9";

const SHAPE: &str = "fix .house-excuses.toml: each [[excuse]] has rule, file, deadline (YYYY-MM-DD) and reason, optional symbol, value, count (1 or more) and added (YYYY-MM-DD)";

/// A calendar day. `today` is always passed in; nothing here reads the clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl Date {
    /// A real `YYYY-MM-DD` date, or `None`.
    pub fn parse(text: &str) -> Option<Date> {
        let b = text.as_bytes();
        let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !digits(0..4) {
            return None;
        }
        if !digits(5..7) || !digits(8..10) {
            return None;
        }
        let year: u16 = text[0..4].parse().ok()?;
        let month: u8 = text[5..7].parse().ok()?;
        let day: u8 = text[8..10].parse().ok()?;
        let leap =
            year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
        let last = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => return None,
        };
        (1..=last)
            .contains(&day)
            .then_some(Date { year, month, day })
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// The longest an excuse may run, in days, from the day it was added.
pub const MAX_DAYS: i64 = 56;

impl Date {
    /// The day `n` days after this one (before it when `n` is negative).
    pub fn add_days(self, n: i64) -> Option<Date> {
        let (y, m, d) = crate::record::civil(self.days() + n);
        Date::parse(&format!("{y:04}-{m:02}-{d:02}"))
    }

    /// Days since 1970-01-01 (civil calendar), so dates subtract.
    fn days(self) -> i64 {
        let (y, m, d) = (
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        );
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }
}

/// One entry: findings of `rule` in `file` are excused until `deadline`, inclusive.
#[derive(Clone, Debug, PartialEq)]
pub struct Excuse {
    pub rule: String,
    pub file: String,
    pub symbol: Option<String>,
    pub value: Option<f64>,
    /// How many matching findings the entry excuses; 1 when the entry gives none.
    pub count: u32,
    pub deadline: Date,
    pub reason: String,
    /// The day the entry was added, when the entry records it.
    pub added: Option<Date>,
}

impl Excuse {
    /// `R3 src/a.rs run`, as the excuse list's findings name it.
    fn shown(&self) -> String {
        match &self.symbol {
            Some(s) => format!("{} {} {s}", self.rule, self.file),
            None => format!("{} {}", self.rule, self.file),
        }
    }

    fn matches(&self, f: &Finding) -> bool {
        let symbol_ok = self.symbol.as_deref().is_none_or(|s| has_word(&f.what, s));
        let value_ok = match (self.value, &f.measure) {
            (Some(limit), Some(m)) => m.value <= limit,
            _ => true,
        };
        f.rule == self.rule && f.place == Place::File(self.file.clone()) && symbol_ok && value_ok
    }
}

/// Whether `word` occurs in `text` with no letter, digit or `_` right before or after it.
fn has_word(text: &str, word: &str) -> bool {
    let inner = |c: char| c.is_alphanumeric() || c == '_';
    !word.is_empty()
        && text.match_indices(word).any(|(i, _)| {
            let before = text[..i].chars().next_back().is_none_or(|c| !inner(c));
            let after = text[i + word.len()..]
                .chars()
                .next()
                .is_none_or(|c| !inner(c));
            before && after
        })
}

/// What `apply` sorted the findings into.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Applied {
    /// Findings no live entry excuses: the agent fixes these.
    pub to_fix: Vec<Finding>,
    /// Findings a live entry excuses: recorded, not replied.
    pub excused: Vec<Finding>,
    /// The list's own problems (expired or stale entries): the agent fixes these too.
    pub problems: Vec<Finding>,
}

/// Sort `findings` by the list, in their order. An entry excuses while `today <=
/// deadline`, up to its `count` findings; a matching finding beyond that is to fix. An
/// expired entry excuses nothing and gives an "excuse expired" finding; a live entry that
/// matches no finding gives a "stale excuse" finding. Stale is only meaningful when
/// `findings` come from a run over every file the list names.
pub fn apply(findings: Vec<Finding>, excuses: &[Excuse], today: Date) -> Applied {
    let mut out = Applied::default();
    let mut used = vec![0_u32; excuses.len()];
    for f in findings {
        let live = excuses
            .iter()
            .zip(&used)
            .position(|(e, &n)| e.deadline >= today && n < e.count && e.matches(&f));
        match live {
            Some(i) => {
                used[i] += 1;
                out.excused.push(f);
            }
            None => out.to_fix.push(f),
        }
    }
    for (e, used) in excuses.iter().zip(used) {
        if e.deadline < today {
            out.problems.push(expired_problem(e));
        } else if used == 0 {
            out.problems.push(problem(
                "house excuse-stale",
                format!("stale excuse: {} matches no finding", e.shown()),
                format!("delete it from {FILE}"),
            ));
        }
    }
    out
}

/// An "excuse expired" finding for each entry of `excuses` past its deadline.
pub fn expired(excuses: &[Excuse], today: Date) -> Vec<Finding> {
    excuses
        .iter()
        .filter(|e| e.deadline < today)
        .map(expired_problem)
        .collect()
}

fn expired_problem(e: &Excuse) -> Finding {
    problem(
        "house excuse-expired",
        format!("excuse expired: {}, deadline {}", e.shown(), e.deadline),
        format!(
            "fix the {} finding in {}, then delete the entry",
            e.rule, e.file
        ),
    )
}

/// The list may only shrink: each entry of `now` that `base` lacks (same rule, file and
/// symbol), or whose value is above base's (or dropped), or whose count is above base's,
/// or whose deadline is later, is an "excuse list grew" finding. `base` is the list at HEAD (commit) or the push base.
pub fn grew(base: &[Excuse], now: &[Excuse]) -> Vec<Finding> {
    let key = |e: &Excuse| (e.rule.clone(), e.file.clone(), e.symbol.clone());
    now.iter()
        .filter(|e| {
            let Some(b) = base.iter().find(|b| key(b) == key(e)) else {
                return true;
            };
            let raised = match (b.value, e.value) {
                (Some(old), Some(new)) => new > old,
                (Some(_), None) => true,
                (None, _) => false,
            };
            raised || e.count > b.count || e.deadline > b.deadline
        })
        .map(|e| {
            problem(
                "house excuse-grew",
                format!("excuse list grew: {}", e.shown()),
                "remove it, or ask the owner".to_string(),
            )
        })
        .collect()
}

/// An entry whose deadline is more than 56 days after the day it was added: today for an
/// entry `base` lacks (same rule, file and symbol), its `added` date for one `base` has.
/// An older entry that records no `added` date is not judged; `grew` already stops its
/// deadline moving later.
pub fn too_far(base: &[Excuse], now: &[Excuse], today: Date) -> Vec<Finding> {
    let key = |e: &Excuse| (e.rule.clone(), e.file.clone(), e.symbol.clone());
    now.iter()
        .filter(|e| {
            let is_new = !base.iter().any(|b| key(b) == key(e));
            let from = if is_new { Some(today) } else { e.added };
            from.is_some_and(|d| e.deadline.days() - d.days() > MAX_DAYS)
        })
        .map(|e| {
            problem(
                "house excuse-far",
                format!("excuse deadline too far: {}", e.shown()),
                format!("set a deadline within {MAX_DAYS} days"),
            )
        })
        .collect()
}

/// Entries of `excuses` that match none of `findings`. Only a run over every tracked file
/// (`house check --all`) may call this: a commit or push sees changed files only.
pub fn stale(excuses: &[Excuse], findings: &[Finding]) -> Vec<Finding> {
    excuses
        .iter()
        .filter(|e| !findings.iter().any(|f| e.matches(f)))
        .map(|e| {
            problem(
                "house excuse-stale",
                format!("stale excuse: {} matches no finding", e.shown()),
                format!("delete it from {FILE}"),
            )
        })
        .collect()
}

fn problem(tool_rule: &str, what: String, fix: String) -> Finding {
    Finding {
        rule: RULE.to_string(),
        place: Place::File(FILE.to_string()),
        line: None,
        what,
        fix,
        tool_rule: tool_rule.to_string(),
        measure: None,
    }
}

/// The list from the file's text (an empty text is an empty list), or why it cannot be read.
pub fn parse(text: &str) -> Result<Vec<Excuse>, CannotCheck> {
    let bad = |why: String| CannotCheck {
        what: format!("{FILE}: {why}"),
        fix: SHAPE.to_string(),
    };
    let raw: Raw = toml::from_str(text).map_err(|e| bad(one_line(e.message())))?;
    let mut list: Vec<Excuse> = Vec::new();
    for r in raw.excuse {
        let entry = format!("entry {} {}", r.rule, r.file);
        let deadline = Date::parse(&r.deadline).ok_or_else(|| {
            bad(format!(
                "{entry}: deadline `{}` is not a real YYYY-MM-DD date",
                r.deadline
            ))
        })?;
        let rule_ok = (1..=17).any(|n| r.rule == format!("R{n}"));
        if !rule_ok {
            return Err(bad(format!("{entry}: rule `{}` is not R1..R17", r.rule)));
        }
        if r.file.trim().is_empty() || r.reason.trim().is_empty() {
            return Err(bad(format!("{entry}: file and reason must not be empty")));
        }
        let added = match &r.added {
            None => None,
            Some(a) => Some(Date::parse(a).ok_or_else(|| {
                bad(format!(
                    "{entry}: added `{a}` is not a real YYYY-MM-DD date"
                ))
            })?),
        };
        let count = match r.count {
            None => 1,
            Some(n) => u32::try_from(n).ok().filter(|n| *n >= 1).ok_or_else(|| {
                bad(format!(
                    "{entry}: count `{n}` is not a whole number of 1 or more"
                ))
            })?,
        };
        let e = Excuse {
            rule: r.rule,
            file: r.file,
            symbol: r.symbol,
            value: r.value,
            count,
            deadline,
            reason: r.reason,
            added,
        };
        if list
            .iter()
            .any(|o| (&o.rule, &o.file, &o.symbol) == (&e.rule, &e.file, &e.symbol))
        {
            return Err(bad(format!("entry {}: listed twice", e.shown())));
        }
        list.push(e);
    }
    Ok(list)
}

/// The list as committed at `rev` (`HEAD`, or a push base): `None` when `rev` is not
/// a commit yet (a repo before its first commit) or holds no list, so the list is being
/// adopted; a list there that is not a file or cannot be read is cannot-check.
pub fn at_rev(root: &Path, rev: &str) -> Result<Option<Vec<Excuse>>, CannotCheck> {
    let Ok(sha) = git(
        root,
        &["rev-parse", "--verify", "-q", &format!("{rev}^{{commit}}")],
    ) else {
        return Ok(None);
    };
    let Some(text) = crate::rules::file_at(root, sha.stdout.trim(), FILE)? else {
        return Ok(None);
    };
    parse(&text).map(Some).map_err(|c| CannotCheck {
        what: format!("the list at {rev}: {}", c.what),
        fix: c.fix,
    })
}

/// The list in the working tree at `root`: `None` when there is no file.
pub fn in_tree(root: &Path) -> Result<Option<Vec<Excuse>>, CannotCheck> {
    match std::fs::read_to_string(root.join(FILE)) {
        Ok(text) => parse(&text).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(CannotCheck {
            what: format!("{FILE} cannot be read: {e}"),
            fix: format!("make {FILE} readable"),
        }),
    }
}

fn one_line(message: &str) -> String {
    let joined = message.split_whitespace().collect::<Vec<_>>().join(" ");
    joined.trim_end_matches('.').to_string()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    #[serde(default)]
    excuse: Vec<RawExcuse>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExcuse {
    rule: String,
    file: String,
    symbol: Option<String>,
    value: Option<f64>,
    count: Option<i64>,
    deadline: String,
    reason: String,
    added: Option<String>,
}
