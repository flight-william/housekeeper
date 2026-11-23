//! The optional `.house.toml` at the repo root:
//!
//! ```toml
//! [house]
//! minimum = "0.2.3"                 # the oldest house these rules run on
//!
//! [tools]
//! commit = ["ruff", "ruff-format"]
//! push = ["ruff", "ruff-format"]   # optional; falls back to `commit`
//!
//! [commit]                          # for `house commit-msg`
//! models = ["claude-opus-5-5"]      # model ids a Co-Authored-By line may name
//! author = "William"                # the expected author; else git config user.name
//! email = "w@example.com"           # with author: the author's email is checked too
//!
//! [protected]                       # for the approval check and CODEOWNERS
//! extra = ["policy/*.yml"]          # globs only the owner may change, beyond the list
//!
//! [paths]
//! exclude = ["tests/fixtures/"]     # never handed to a tool (`files::without_excluded`)
//!
//! [tests]                           # the test step (`tests_step`), after the tools
//! commit = [["cargo", "test", "--offline"]]   # each a program and its arguments, no shell
//! push = [["cargo", "test"]]        # optional; falls back to `commit`
//! timeout_seconds = 600             # optional; each command's time limit, 600 when unset
//! ```
//!
//! Every table and key is known: any other is a cannot-check naming it. A house older
//! than `[house] minimum` is a cannot-check naming both versions (a house from before
//! `minimum` existed refuses the unknown `[house]` table, so it never runs either).
//!
//! Without the file (or without the stage's list) every adapter in `all()` that runs
//! by default runs (swiftformat only by name), and each skips itself when no file
//! concerns it.

use crate::adapter::{Adapter, CannotCheck, Stage};
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;

pub const FILE: &str = ".house.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub house: HouseTable,
    #[serde(default)]
    pub tools: Tools,
    #[serde(default)]
    pub commit: CommitRules,
    #[serde(default)]
    pub protected: Protected,
    #[serde(default)]
    pub paths: Paths,
    #[serde(default)]
    pub tests: Tests,
}

/// `[house]`: which house the rules are written for.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HouseTable {
    /// The oldest house version, `MAJOR.MINOR.PATCH`, that may judge by these rules.
    pub minimum: Option<String>,
}

/// `running` (this house's version) is at least `minimum`; both `MAJOR.MINOR.PATCH`,
/// compared by number. Otherwise cannot-check naming both.
pub fn minimum_met(minimum: &str, running: &str) -> Result<(), CannotCheck> {
    let Some(wanted) = version(minimum) else {
        return Err(CannotCheck {
            what: format!("{FILE} [house] minimum `{minimum}` is not a MAJOR.MINOR.PATCH version"),
            fix: format!("write it as in minimum = \"{running}\""),
        });
    };
    if version(running).is_some_and(|have| have >= wanted) {
        return Ok(());
    }
    Err(CannotCheck {
        what: format!("{FILE} asks for house {minimum} or newer, and this is house {running}"),
        fix: "upgrade house (brew upgrade house), then run again".into(),
    })
}

/// `MAJOR.MINOR.PATCH`, each part digits only.
fn version(text: &str) -> Option<(u64, u64, u64)> {
    let parts: Vec<&str> = text.split('.').collect();
    let [major, minor, patch] = parts[..] else {
        return None;
    };
    let number = |p: &str| {
        (!p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
            .then(|| p.parse().ok())
            .flatten()
    };
    Some((number(major)?, number(minor)?, number(patch)?))
}

/// `[tests]`: commands for the test step, each a program and its arguments.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tests {
    pub commit: Option<Vec<Vec<String>>>,
    pub push: Option<Vec<Vec<String>>>,
    /// Each command's time limit in seconds (`TEST_LIMIT` when unset), 1 or more.
    pub timeout_seconds: Option<u64>,
}

/// A test command's time limit when `[tests] timeout_seconds` is unset.
pub const TEST_LIMIT: Duration = Duration::from_secs(600);

/// How long each test command may run before it is stopped and reported.
pub fn test_limit(config: &Config) -> Duration {
    config
        .tests
        .timeout_seconds
        .map_or(TEST_LIMIT, Duration::from_secs)
}

/// The test commands to run at `stage`; push falls back to `commit`, as `[tools]` does.
pub fn tests_for(config: &Config, stage: Stage) -> &[Vec<String>] {
    let tests = &config.tests;
    let list = match stage {
        Stage::Commit => tests.commit.as_ref(),
        Stage::Push => tests.push.as_ref().or(tests.commit.as_ref()),
    };
    list.map_or(&[], Vec::as_slice)
}

/// `[paths]`: `exclude` lists prefix globs (a folder, a file, `*` and `**/` as in
/// `[protected]`); a file under one is dropped from every stage's file list before
/// the adapters see it, and a finding a tool still reports on one (gitleaks reads the
/// git history) is dropped. The policy checks (approval) still see every change.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paths {
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// `[protected]`: globs only the owner may change, on top of `approval::PROTECTED`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protected {
    #[serde(default)]
    pub extra: Vec<String>,
}

/// `[commit]`: what `house commit-msg` checks the message against.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitRules {
    #[serde(default)]
    pub models: Vec<String>,
    pub author: Option<String>,
    /// Checked only together with `author`.
    pub email: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tools {
    pub commit: Option<Vec<String>>,
    pub push: Option<Vec<String>>,
}

/// Read `.house.toml` from the working tree at `root`; absent is the default (every
/// adapter). `rules::Rules::at` reads it from a revision instead.
pub fn load(root: &Path) -> Result<Config, CannotCheck> {
    let path = root.join(FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(bad(format!("{FILE} cannot be read: {e}"))),
    };
    parse(&text)
}

/// Parse `.house.toml`. Every table and key is known: a misspelled one (`[tool]`)
/// is a cannot-check naming it and its line, never a setting silently ignored.
pub fn parse(text: &str) -> Result<Config, CannotCheck> {
    let config: Config = toml::from_str(text).map_err(|e| {
        let at = e
            .span()
            .map(|s| format!(" (line {})", text[..s.start].matches('\n').count() + 1))
            .unwrap_or_default();
        bad(format!("{FILE} is not valid: {}{at}", one_line(&e)))
    })?;
    if let Some(minimum) = &config.house.minimum {
        minimum_met(minimum, env!("CARGO_PKG_VERSION"))?;
    }
    if config.tests.timeout_seconds == Some(0) {
        return Err(bad(format!(
            "{FILE} [tests] timeout_seconds is 0: give the seconds each test command may run, 1 or more"
        )));
    }
    let (commit, push) = (&config.tests.commit, &config.tests.push);
    for (stage, list) in [("commit", commit), ("push", push)] {
        if list.iter().flatten().any(Vec::is_empty) {
            return Err(bad(format!(
                "{FILE} [tests] {stage} has an empty command: give a program and its arguments"
            )));
        }
    }
    Ok(config)
}

/// The adapters to run at `stage`, in the config's order; unknown names are cannot-checks.
pub fn select(
    config: &Config,
    stage: Stage,
    all: Vec<Box<dyn Adapter>>,
) -> (Vec<Box<dyn Adapter>>, Vec<CannotCheck>) {
    let wanted = match stage {
        Stage::Commit => config.tools.commit.as_ref(),
        Stage::Push => config.tools.push.as_ref().or(config.tools.commit.as_ref()),
    };
    let Some(wanted) = wanted else {
        let defaults = all.into_iter().filter(|a| a.runs_by_default()).collect();
        return (defaults, Vec::new());
    };
    let known: Vec<&str> = all.iter().map(|a| a.name()).collect();
    let unknown = wanted
        .iter()
        .filter(|w| !known.contains(&w.as_str()))
        .map(|w| CannotCheck {
            what: format!("{FILE} names an unknown tool {w}"),
            fix: format!("use one of: {}", known.join(", ")),
        })
        .collect();
    let mut all = all;
    let mut chosen = Vec::new();
    for name in wanted {
        if let Some(i) = all.iter().position(|a| a.name() == name) {
            chosen.push(all.remove(i));
        }
    }
    (chosen, unknown)
}

fn bad(what: String) -> CannotCheck {
    CannotCheck {
        what,
        fix: format!("correct {FILE} at the repo root"),
    }
}

fn one_line(e: &toml::de::Error) -> String {
    e.message().trim().trim_end_matches('.').to_string()
}
