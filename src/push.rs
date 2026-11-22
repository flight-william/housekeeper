//! The push stage: which files a push sends.
//!
//! git's pre-push hook gets one line per ref on stdin:
//! `<local ref> <local sha> <remote ref> <remote sha>`. Stdin of only
//! newlines is an empty push; a zero local sha is a delete (skipped); a zero
//! remote sha is a new branch on the remote.

use crate::adapter::CannotCheck;
use crate::files::{self, Change, split_nul};
use crate::run::git;
use std::path::Path;

/// The branch a new branch is compared with: `git merge-base <this> <local sha>`.
pub const NEW_BRANCH_BASE: &str = "origin/main";

/// One ref being pushed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushLine {
    pub local_ref: String,
    pub local: String,
    pub remote_ref: String,
    /// `None` when the remote sha is zero: the remote has no such ref yet.
    pub remote: Option<String>,
}

/// The refs to check on git's pre-push stdin. Deletes are skipped; stdin of
/// only newlines (a push with nothing to send) is no lines.
pub fn parse_lines(text: &str) -> Result<Vec<PushLine>, CannotCheck> {
    let mut lines = Vec::new();
    if text.bytes().all(|b| b == b'\n') {
        return Ok(lines);
    }
    for line in text.lines() {
        let bad = || {
            CannotCheck {
            what: format!("not a push line: {line:?}"),
            fix: "run house check --stage push from git's pre-push hook, which gives the push lines on stdin".into(),
        }
        };
        let [local_ref, local, remote_ref, remote] = line.split(' ').collect::<Vec<_>>()[..] else {
            return Err(bad());
        };
        if !is_sha(local) || !is_sha(remote) {
            return Err(bad());
        }
        if is_zero(local) {
            continue;
        }
        lines.push(PushLine {
            local_ref: local_ref.into(),
            local: local.into(),
            remote_ref: remote_ref.into(),
            remote: (!is_zero(remote)).then(|| remote.to_string()),
        });
    }
    Ok(lines)
}

/// What a push sends: the files to check and the commits (`<base>..<local sha>`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Push {
    /// Relative to `root`, `/`-separated, sorted, no repeats; deletes and links dropped.
    pub files: Vec<String>,
    /// Every pushed path but deletes, links kept (`files::not_deleted`).
    pub changed: Vec<String>,
    /// The widest ref's range; `None` when nothing is pushed or the push is unbounded.
    pub range: Option<String>,
    /// A pushed ref has no base (a first push without origin/main): the push sends
    /// history no range bounds, the whole tree and all of HEAD's commits.
    pub unbounded: bool,
}

/// The files a push sends (`pushed(..).files`).
pub fn files_for_push(root: &Path, stdin_text: &str) -> Result<Vec<String>, CannotCheck> {
    Ok(pushed(root, stdin_text)?.files)
}

/// What the push lines send: per ref, `git diff --name-status --no-renames
/// <base>..<local sha>`, where base is the remote sha, or for a new branch the
/// merge-base with `origin/main` (no origin/main: the whole tree). The tools read
/// the checkout, so every pushed ref must be at HEAD and no pushed file may differ
/// from HEAD in the working tree: otherwise cannot-check, never a silent drop. An
/// unfetched remote sha or a failed merge-base is cannot-check, not another base.
pub fn pushed(root: &Path, stdin_text: &str) -> Result<Push, CannotCheck> {
    let lines = parse_lines(stdin_text)?;
    if lines.is_empty() {
        return Ok(Push {
            files: Vec::new(),
            changed: Vec::new(),
            range: None,
            unbounded: false,
        });
    }
    let head = git(root, &["rev-parse", "HEAD"])?.stdout.trim().to_string();
    let mut changes = Vec::new();
    let mut range: Option<(usize, String)> = None;
    let mut unbounded = false;
    for line in &lines {
        at_head(root, line, &head)?;
        let Some(base) = base(root, line)? else {
            unbounded = true;
            let out = git(root, &["ls-tree", "-r", "--name-only", "-z", &line.local])?;
            changes.extend(
                split_nul(&out.stdout)
                    .into_iter()
                    .map(|path| Change { status: 'A', path }),
            );
            continue;
        };
        let this = format!("{base}..{}", line.local);
        changes.extend(files::changes(root, &["diff", &this])?);
        let count = git(root, &["rev-list", "--count", &this])?;
        let count = count.stdout.trim().parse().unwrap_or(0);
        if range.as_ref().is_none_or(|(widest, _)| count > *widest) {
            range = Some((count, this));
        }
    }
    let mut files = files::for_tools(root, &changes);
    files.sort();
    files.dedup();
    let dirty = split_nul(&git(root, &["diff", "--name-only", "-z", "HEAD"])?.stdout);
    files::refuse_changed(
        &files,
        &dirty,
        "changed in the working tree since the pushed commit: the tools read the working tree",
        "commit or stash the change, then push again",
    )?;
    Ok(Push {
        changed: files::not_deleted(&changes),
        files,
        range: range.filter(|_| !unbounded).map(|(_, r)| r),
        unbounded,
    })
}

/// The pushed ref's commit is HEAD, or cannot-check.
fn at_head(root: &Path, line: &PushLine, head: &str) -> Result<(), CannotCheck> {
    let commit = git(root, &["rev-parse", &format!("{}^{{commit}}", line.local)])?;
    if commit.stdout.trim() == head {
        return Ok(());
    }
    Err(CannotCheck {
        what: format!(
            "{} is pushed at {}, not at the checked-out HEAD, and the tools read the checkout",
            line.local_ref,
            short(&line.local)
        ),
        fix: "push from the branch you have checked out".into(),
    })
}

/// The commit the ref is compared with; `None` for a new branch when there is no
/// origin/main (a first push).
fn base(root: &Path, line: &PushLine) -> Result<Option<String>, CannotCheck> {
    if let Some(sha) = &line.remote {
        if !is_commit(root, sha) {
            return Err(CannotCheck {
                what: format!(
                    "the remote {} is at {}, which this clone has not fetched",
                    line.remote_ref,
                    short(sha)
                ),
                fix: "git fetch, then push again".into(),
            });
        }
        return Ok(Some(sha.clone()));
    }
    if !is_commit(root, NEW_BRANCH_BASE) {
        return Ok(None);
    }
    let out =
        git(root, &["merge-base", NEW_BRANCH_BASE, &line.local]).map_err(|_| CannotCheck {
            what: format!(
                "git merge-base {NEW_BRANCH_BASE} {} found no common commit",
                short(&line.local)
            ),
            fix: format!(
                "git fetch, then push a branch that shares history with {NEW_BRANCH_BASE}"
            ),
        })?;
    Ok(Some(out.stdout.trim().to_string()))
}

fn is_commit(root: &Path, rev: &str) -> bool {
    git(root, &["cat-file", "-e", &format!("{rev}^{{commit}}")]).is_ok()
}

fn short(sha: &str) -> &str {
    sha.get(..12).unwrap_or(sha)
}

fn is_sha(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// git's zero id: what a missing ref is written as on a push line.
fn is_zero(s: &str) -> bool {
    s.bytes().all(|b| b == b'0')
}
