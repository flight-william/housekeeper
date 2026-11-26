//! The one adapter interface, shared by every adapter and the core: a change here
//! changes every adapter at once.
//!
//! An adapter says how to run its tool and how to read the result. It never
//! runs anything itself: the core runs the command (no shell, hang guard) and
//! hands back the captured output, so every adapter is a pure, testable parser.

use crate::finding::Finding;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// When house runs. `Merge` is the runner's check of a pull request: the tools see
/// it as a push of the branch (the same ref lines, files and range), and the core adds
/// the commit-msg hook's checks over the range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Commit,
    Push,
    Merge,
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

/// The extensions that make a file a shell script, whatever its first line says.
pub const SHELL_EXTENSIONS: [&str; 2] = [".sh", ".bash"];

/// The extensions that make a file JavaScript or TypeScript code, for Prettier, ESLint,
/// r3 and Semgrep, and for reading first lines (`files::languages`): code files only,
/// not `.json`, `.css` or `.md`. The TypeScript ones first.
pub const JS_EXTENSIONS: [&str; 8] = [".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs"];

/// The TypeScript ones: the first four of `JS_EXTENSIONS`. A staged one runs tsc.
pub const TS_EXTENSIONS: &[&str] = JS_EXTENSIONS.split_at(4).0;

/// A language a file's first line can name: `#!/usr/bin/env python3`, `#!/bin/sh`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Python,
    /// sh, bash, dash or ksh: what ShellCheck reads. Not zsh.
    Shell,
    /// `node`: JavaScript, for the JS tools (Prettier, ESLint, r3).
    Node,
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

    /// The fix when `program` is not installed: `brew install <brew_formula>`, unless
    /// the tool comes from the repo itself (the JS tools in a repo with package.json).
    fn install(&self, _ctx: &Context) -> String {
        format!("brew install {}", self.brew_formula())
    }

    /// Whether it runs when `.house.toml` lists no tools for the stage, and is named in
    /// `house init`'s starter list. `false` for a tool a repo chooses instead of
    /// another (SwiftFormat beside Apple's swift format): it then runs only by name.
    fn runs_by_default(&self) -> bool {
        true
    }

    /// Findings this tool gives up to another, as (tool rule, adapter name): a finding
    /// with that tool rule is dropped when that adapter runs for the stage too, as
    /// its owner, and kept when it does not, so the rule is never left unchecked.
    fn yields_to(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// Variables set in the command's environment, after house strips the GIT_* ones
    /// it inherited.
    fn env(&self, _ctx: &Context) -> Vec<(String, String)> {
        Vec::new()
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

    /// Findings that need every command's output at once, after each was parsed
    /// (tsc: a staged file that no project lists). Asked only when every command ran;
    /// none by default.
    fn joined(&self, _ctx: &Context, _outs: &[Output]) -> Vec<Finding> {
        Vec::new()
    }

    /// Called once after the adapter's commands, however they ended: drop what its
    /// runs left outside the repo (tsc's build info). Nothing by default.
    fn finished(&self, _ctx: &Context) {}

    /// Files in `out` the tool could not read because of its own limit, not the file's
    /// fault (Semgrep's parser misses valid Swift): counted, never findings. A parse
    /// error from the language's own tool (ruff on Python) is a finding instead.
    fn unread(&self, _ctx: &Context, _out: &Output) -> Vec<String> {
        Vec::new()
    }
}
