//! The optional `.house.toml` at the repo root:
//!
//! ```toml
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
//! ```
//!
//! Every table and key is known: any other is a cannot-check naming it.
//!
//! Without the file (or without the stage's list) every adapter in `all()` runs,
//! and each skips itself when no file concerns it.

use crate::adapter::{Adapter, CannotCheck, Stage};
use serde::Deserialize;
use std::path::Path;

pub const FILE: &str = ".house.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
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

/// `[tests]`: commands for the test step, each a program and its arguments.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tests {
    pub commit: Option<Vec<Vec<String>>>,
    pub push: Option<Vec<Vec<String>>>,
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
/// the adapters see it. The policy checks (approval) still see every change.
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

/// Read `.house.toml` from `root`; absent is the default (every adapter).
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
        return (all, Vec::new());
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
