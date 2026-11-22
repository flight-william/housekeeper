//! Which files a stage checks: commit, the staged files; push, the files the push sends.

use crate::adapter::{CannotCheck, Stage};
use crate::approval::glob_matches;
use crate::run::git;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};

/// The repo's top folder, from any folder inside it.
pub fn repo_root(cwd: &Path) -> Result<PathBuf, CannotCheck> {
    let out = git(cwd, &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(out.stdout.trim()))
}

/// What a stage checks: its files, and at push the range and whether it is bounded
/// (`push::Push`).
pub struct Scope {
    pub files: Vec<String>,
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
            let files = without_excluded(staged(root)?, exclude);
            let unstaged = split_nul(&git(root, &["diff", "--name-only", "-z"])?.stdout);
            refuse_changed(
                &files,
                &unstaged,
                "unstaged changes: the tools read the working copy, not what is staged",
                "stage or stash the rest: git add the file, or git stash push --keep-index",
            )?;
            Ok(Scope {
                files,
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
