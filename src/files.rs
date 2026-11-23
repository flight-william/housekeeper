//! Which files a stage checks: commit, the staged files; push, the files the push sends.

use crate::adapter::{CannotCheck, Language, Stage};
use crate::approval::glob_matches;
use crate::run::git;
use std::collections::BTreeMap;
use std::io::{BufRead, IsTerminal, Read};
use std::path::{Path, PathBuf};

/// The repo's top folder, from any folder inside it.
pub fn repo_root(cwd: &Path) -> Result<PathBuf, CannotCheck> {
    let out = git(cwd, &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(out.stdout.trim()))
}

/// The branches whose root commit names a repository, the first that exists: the
/// remote's default branch, then a local main, then a local master. Each is a
/// `git rev-parse --glob` pattern matching that one ref: a pattern with no glob
/// character would be read as a folder of refs (`refs/heads/main/*`).
const DEFAULT_BRANCHES: [&str; 3] = [
    "--glob=refs/remotes/origin/HEA[D]",
    "--glob=refs/heads/mai[n]",
    "--glob=refs/heads/maste[r]",
];

/// Who a repository is, the same in each of its worktrees whatever their HEAD; each
/// field is known or `None` on its own.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Identity {
    /// The root commit of its default branch (the first of `DEFAULT_BRANCHES`; the
    /// oldest root by commit time, then hash, when it has several). `None` when no
    /// default branch exists, or in a shallow repository, whose oldest commit is
    /// where its history was cut, not its root.
    pub root_commit: Option<String>,
    /// Its main checkout: the folder holding the `.git` folder every worktree shares
    /// (`git rev-parse --git-common-dir`'s parent). `None` when that folder is not
    /// named `.git` (a bare repository).
    pub main_checkout: Option<PathBuf>,
}

/// The identity of the repository holding `dir`, in two git calls: one `rev-parse`
/// for whether it is shallow, its shared `.git` folder and its default branch, then
/// the walk of that branch for its roots. Any git failure leaves a field `None`.
pub fn identity(dir: &Path) -> Identity {
    let mut args = vec![
        "rev-parse",
        "--is-shallow-repository",
        "--git-common-dir",
        "--symbolic-full-name",
    ];
    args.extend(DEFAULT_BRANCHES);
    let Ok(out) = git(dir, &args) else {
        return Identity::default();
    };
    let mut lines = out.stdout.lines().map(str::trim);
    let shallow = lines.next() != Some("false");
    let main_checkout = lines.next().and_then(|common| main_of(dir, common));
    let branch = lines.next().filter(|b| !shallow && !b.is_empty());
    Identity {
        root_commit: branch.and_then(|b| oldest_root(dir, b)),
        main_checkout,
    }
}

/// The parent of the shared `.git` folder `common` (relative to `dir` or absolute).
fn main_of(dir: &Path, common: &str) -> Option<PathBuf> {
    let common = dir.join(common).canonicalize().ok()?;
    if common.file_name()? != ".git" {
        return None;
    }
    common.parent().map(Path::to_path_buf)
}

/// The oldest root commit of `branch`, by commit time, then hash.
fn oldest_root(dir: &Path, branch: &str) -> Option<String> {
    let out = git(dir, &["log", "--max-parents=0", "--format=%ct %H", branch]).ok()?;
    out.stdout
        .lines()
        .filter_map(|l| {
            let (time, hash) = l.trim().split_once(' ')?;
            Some((time.parse::<i64>().ok()?, hash.to_string()))
        })
        .min()
        .map(|(_, hash)| hash)
}

/// What a stage checks: its files, and at push the range and whether it is bounded
/// (`push::Push`).
pub struct Scope {
    pub files: Vec<String>,
    /// Every changed path but deletes, links and excluded paths kept: what the
    /// policy checks (`approval::wrong_case`) judge.
    pub changed: Vec<String>,
    /// The test step runs: at push when the push changes any path (`Push::touched`),
    /// else when there is a file to check.
    pub run_tests: bool,
    pub push_range: Option<String>,
    pub unbounded: bool,
}

/// The files to check at `stage`, relative to `root`, `/`-separated, and at push
/// the push range (`<base>..<local sha>`). Commit: the staged files (every staged
/// change but deletes and links), none with unstaged changes on top. Push: the
/// files the push lines on stdin send (`push::pushed`); stdin a terminal (a run by
/// hand) checks HEAD as a new branch, against its merge-base with origin/main.
pub fn for_stage(root: &Path, stage: Stage) -> Result<Scope, CannotCheck> {
    for_stage_excluding(root, stage, &[])
}

/// `for_stage` without the files `exclude` names (`without_excluded`): they are
/// dropped first, so an unstaged change to one does not stop a commit either.
pub fn for_stage_excluding(
    root: &Path,
    stage: Stage,
    exclude: &[String],
) -> Result<Scope, CannotCheck> {
    match stage {
        Stage::Commit => {
            let all = staged_changes(root)?;
            let changed = not_deleted(&all);
            let files = without_excluded(for_tools(root, &all), exclude);
            let unstaged = split_nul(&git(root, &["diff", "--name-only", "-z"])?.stdout);
            refuse_changed(
                &files,
                &unstaged,
                "unstaged changes: the tools read the working copy, not what is staged",
                "stage or stash the rest: git add the file, or git stash push --keep-index",
            )?;
            Ok(Scope {
                run_tests: !files.is_empty(),
                files,
                changed,
                push_range: None,
                unbounded: false,
            })
        }
        Stage::Push => {
            let stdin = std::io::stdin();
            let text = if stdin.is_terminal() {
                let head = git(root, &["rev-parse", "HEAD"])?.stdout.trim().to_string();
                format!("HEAD {head} HEAD {}\n", "0".repeat(head.len()))
            } else {
                let mut text = String::new();
                stdin
                    .lock()
                    .read_to_string(&mut text)
                    .map_err(|e| CannotCheck {
                        what: format!("push lines on stdin could not be read: {e}"),
                        fix: "run house check --stage push from git's pre-push hook".into(),
                    })?;
                text
            };
            let push = crate::push::pushed(root, &text)?;
            Ok(Scope {
                files: without_excluded(push.files, exclude),
                changed: push.changed,
                run_tests: push.touched,
                push_range: push.range,
                unbounded: push.unbounded,
            })
        }
    }
}

/// `files` without those under an `exclude` entry (`.house.toml` `[paths]`). An
/// entry is a glob (`approval::glob_matches`) matched against a path's leading
/// folders and the whole path: `vendor` drops `vendor` and `vendor/a.py`, not
/// `vendorx/a.py`; a trailing `/` (`tests/fixtures/`) matches folders only.
pub fn without_excluded(files: Vec<String>, exclude: &[String]) -> Vec<String> {
    files
        .into_iter()
        .filter(|f| !exclude.iter().any(|entry| excludes(entry, f)))
        .collect()
}

fn excludes(entry: &str, path: &str) -> bool {
    let folder_only = entry.ends_with('/');
    let glob = entry.trim_end_matches('/');
    let segs: Vec<&str> = path.split('/').collect();
    let longest = if folder_only {
        segs.len() - 1
    } else {
        segs.len()
    };
    (1..=longest).any(|n| glob_matches(glob, &segs[..n].join("/")))
}

/// Cannot-check naming each of `files` that is also in `changed`: "<files> has
/// <why>" (the tools would read content other than what is committed or pushed).
pub(crate) fn refuse_changed(
    files: &[String],
    changed: &[String],
    why: &str,
    fix: &str,
) -> Result<(), CannotCheck> {
    let both: Vec<&str> = files
        .iter()
        .filter(|f| changed.contains(f))
        .map(String::as_str)
        .collect();
    if both.is_empty() {
        return Ok(());
    }
    let verb = if both.len() == 1 { "has" } else { "have" };
    Err(CannotCheck {
        what: format!("{} {verb} {why}", both.join(", ")),
        fix: fix.into(),
    })
}

/// The staged files the tools read: every staged change but deletes, links dropped.
pub fn staged(root: &Path) -> Result<Vec<String>, CannotCheck> {
    Ok(for_tools(root, &staged_changes(root)?))
}

/// Every staged change, for the policy checks (approval): deletes, both ends of a
/// rename (`--no-renames` gives a delete and an add), links, each with git's status.
pub fn staged_changes(root: &Path) -> Result<Vec<Change>, CannotCheck> {
    changes(root, &["diff", "--cached"])
}

/// One changed path and git's status letter for it (`A`, `M`, `D`, `T`...).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub status: char,
    pub path: String,
}

/// `git <diff args> --name-status --no-renames -z`, read into changes.
pub(crate) fn changes(root: &Path, diff: &[&str]) -> Result<Vec<Change>, CannotCheck> {
    let mut args = diff.to_vec();
    args.extend(["--name-status", "--no-renames", "-z"]);
    let out = git(root, &args)?;
    let fields = split_nul(&out.stdout);
    Ok(fields
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[status, path]| Change {
            status: status.chars().next().unwrap_or('?'),
            path: path.clone(),
        })
        .collect())
}

/// The paths of `changes` not deleted, links and all, sorted, no repeats.
pub(crate) fn not_deleted(changes: &[Change]) -> Vec<String> {
    let mut paths: Vec<String> = changes
        .iter()
        .filter(|c| c.status != 'D')
        .map(|c| c.path.clone())
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// The paths of `changes` a tool can read: not deleted, not a link.
pub(crate) fn for_tools(root: &Path, changes: &[Change]) -> Vec<String> {
    let kept = changes.iter().filter(|c| c.status != 'D');
    without_links(root, kept.map(|c| c.path.clone()).collect())
}

/// Drop symbolic links: they are not source a tool can judge, and Semgrep
/// refuses a link given as a scanning root.
pub(crate) fn without_links(root: &Path, files: Vec<String>) -> Vec<String> {
    files
        .into_iter()
        .filter(|f| {
            !std::fs::symlink_metadata(root.join(f)).is_ok_and(|m| m.file_type().is_symlink())
        })
        .collect()
}

/// Every tracked file, for `house init` to see which tools the repo needs.
pub fn tracked(root: &Path) -> Result<Vec<String>, CannotCheck> {
    Ok(split_nul(&git(root, &["ls-files", "-z"])?.stdout))
}

pub(crate) fn split_nul(text: &str) -> Vec<String> {
    text.split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// The extensions the adapters choose files by. A file with any other name has its
/// first line read (`languages`).
const KNOWN: [&str; 6] = [".py", ".pyi", ".sh", ".bash", ".rs", ".swift"];

/// The longest first line read: a shebang longer than this names no language.
const FIRST_LINE_MAX: u64 = 256;

/// The language of each of `files` (relative to `root`) whose name has no known
/// extension, from its first line (`language_of`). Each such file is opened once and
/// at most its first line read; a file that cannot be read, or whose first line is not
/// text (a binary file), has none.
pub fn languages(root: &Path, files: &[String]) -> BTreeMap<String, Language> {
    files
        .iter()
        .filter(|f| !KNOWN.iter().any(|ext| f.ends_with(ext)))
        .filter_map(|f| {
            let file = std::fs::File::open(root.join(f)).ok()?;
            let mut line = Vec::new();
            std::io::BufReader::new(file.take(FIRST_LINE_MAX))
                .read_until(b'\n', &mut line)
                .ok()?;
            let cut = line.len() as u64 == FIRST_LINE_MAX && !line.ends_with(b"\n");
            let text = std::str::from_utf8(&line).ok()?;
            if cut || text.contains('\0') {
                return None;
            }
            Some((f.clone(), language_of(text.trim_end())?))
        })
        .collect()
}

/// The language a first line names: `#!` then the interpreter, by its name, directly
/// or through `env` (`#!/usr/bin/env python3`). python, python3, python3.13: Python;
/// sh, bash, dash, ksh: shell. Anything else, zsh among them, is none.
pub fn language_of(first_line: &str) -> Option<Language> {
    let mut words = first_line.strip_prefix("#!")?.split_whitespace();
    let name = |path: &str| path.rsplit('/').next().unwrap_or(path).to_string();
    let mut program = name(words.next()?);
    if program == "env" {
        program = name(words.find(|w| !w.starts_with('-') && !w.contains('='))?);
    }
    match program.as_str() {
        "sh" | "bash" | "dash" | "ksh" => Some(Language::Shell),
        _ => {
            let version = program.strip_prefix("python")?;
            let digits = version.chars().all(|c| c.is_ascii_digit() || c == '.');
            digits.then_some(Language::Python)
        }
    }
}
