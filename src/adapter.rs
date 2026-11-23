//! The one adapter interface, shared by every adapter and the core: a change here
//! changes every adapter at once.
//!
//! An adapter says how to run its tool and how to read the result. It never
//! runs anything itself: the core runs the command (no shell, hang guard) and
//! hands back the captured output, so every adapter is a pure, testable parser.

use crate::finding::Finding;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// When house runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Commit,
    Push,
}

/// What the adapter is asked to check.
#[derive(Clone, Debug)]
pub struct Context {
    /// The repo's top folder, absolute.
    pub root: PathBuf,
    pub stage: Stage,
    /// Files to check, relative to `root`, `/`-separated. At commit: the staged files.
    pub files: Vec<String>,
    /// At push: `<base>..<local sha>`, the commits the push sends. `None` at commit,
    /// and at a push with no `origin/main`, where the whole history is pushed.
    pub push_range: Option<String>,
    /// The language of each of `files` whose name has no known extension, read by the
    /// core from the file's first line (`files::languages`); a file not here has none.
    /// Adapters choose files by extension or by this map, and never read a file.
    pub languages: BTreeMap<String, Language>,
}

impl Context {
    /// Whether `file` is in `language` by its first line (`languages`).
    pub fn first_line_is(&self, file: &str, language: Language) -> bool {
        self.languages.get(file) == Some(&language)
    }
}

/// A language a file's first line can name: `#!/usr/bin/env python3`, `#!/bin/sh`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Python,
    /// sh, bash, dash or ksh: what ShellCheck reads. Not zsh.
    Shell,
}

/// A command for the core to run, in `root`, with no shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
}

/// What the core captured from the command.
#[derive(Clone, Debug)]
pub struct Output {
    pub exit: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Why a tool could not check (house exit 2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CannotCheck {
    /// One sentence, no trailing period.
    pub what: String,
    /// e.g. "brew install swiftlint".
    pub fix: String,
}

pub trait Adapter {
    /// The tool's name as it appears in the reply and the record, e.g. "ruff".
    fn name(&self) -> &'static str;

    /// The program to look up on PATH; missing means "cannot check: brew install <brew_formula>".
    fn program(&self) -> &'static str;

    /// The Homebrew formula that installs `program`.
    fn brew_formula(&self) -> &'static str;

    /// Whether it runs when `.house.toml` lists no tools for the stage, and is named in
    /// `house init`'s starter list. `false` for a tool a repo chooses instead of
    /// another (SwiftFormat beside Apple's swift format): it then runs only by name.
    fn runs_by_default(&self) -> bool {
        true
    }

    /// The command to run, or `None` when nothing in `ctx.files` concerns this tool
    /// (e.g. no `.py` file staged for ruff): then the tool is skipped, not an error.
    fn command(&self, ctx: &Context) -> Option<Command>;

    /// Every command to run; the core parses each output and joins the findings.
    /// Defaults to `command`; cargo fmt and clippy give one per Rust workspace.
    fn commands(&self, ctx: &Context) -> Vec<Command> {
        self.command(ctx).into_iter().collect()
    }

    /// Read the tool's result. Findings are problems (house exit 1); a tool that exits
    /// non-zero because it found problems gives findings, not `CannotCheck`.
    fn parse(&self, ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck>;

    /// Files in `out` the tool could not read because of its own limit, not the file's
    /// fault (Semgrep's parser misses valid Swift): counted, never findings. A parse
    /// error from the language's own tool (ruff on Python) is a finding instead.
    fn unread(&self, _ctx: &Context, _out: &Output) -> Vec<String> {
        Vec::new()
    }
}
