//! Where a check's rules come from: `.house.toml` and the excuse list, read from the
//! working tree, or with `house check --stage push --config-from <rev>` from that
//! revision's objects (`git ls-tree` and `git cat-file`, GIT_* stripped), never the
//! working tree.
//!
//! At a revision, a file it does not hold is absent: the working tree's copy is never
//! used in its place. A revision with no `.house.toml` at all is cannot-check: the
//! rules are adopted in a commit of their own, so a change never judges itself. A
//! revision or file that cannot be read is cannot-check too.

use crate::adapter::CannotCheck;
use crate::config::{self, Config};
use crate::excuses::{self, Excuse};
use crate::files::split_nul;
use crate::run::git;
use std::path::Path;

/// The rules a check runs by.
#[derive(Debug)]
pub struct Rules {
    pub config: Result<Config, CannotCheck>,
    /// The excuse list that excuses findings; `Ok(None)` when there is none.
    pub excuses: Result<Option<Vec<Excuse>>, CannotCheck>,
    /// The revision both were read from; `None` for the working tree.
    pub rev: Option<String>,
}

impl Rules {
    /// The working tree's `.house.toml` and excuse list.
    pub fn working_tree(root: &Path) -> Rules {
        Rules {
            config: config::load(root),
            excuses: excuses::in_tree(root),
            rev: None,
        }
    }

    /// The rules at `rev`; cannot-check when `rev` holds no `.house.toml`.
    pub fn at(root: &Path, rev: &str) -> Rules {
        let unread = |c: CannotCheck| Rules {
            config: Err(c.clone()),
            excuses: Err(c),
            rev: Some(rev.to_string()),
        };
        let sha = match commit(root, rev) {
            Ok(sha) => sha,
            Err(c) => return unread(c),
        };
        let config = match file_at(root, &sha, config::FILE) {
            Ok(Some(text)) => config::parse(&text).map_err(|c| named(c, rev)),
            Ok(None) => {
                return unread(CannotCheck {
                    what: format!("no rules at {rev}"),
                    fix: "adopt the rules in their own commit first, then change code".into(),
                });
            }
            Err(c) => Err(named(c, rev)),
        };
        let excuses = file_at(root, &sha, excuses::FILE)
            .and_then(|text| text.map(|t| excuses::parse(&t)).transpose())
            .map_err(|c| named(c, rev));
        Rules {
            config,
            excuses,
            rev: Some(rev.to_string()),
        }
    }
}

/// `rev` as a commit id, or cannot-check naming it.
fn commit(root: &Path, rev: &str) -> Result<String, CannotCheck> {
    let spec = format!("{rev}^{{commit}}");
    match git(root, &["rev-parse", "--verify", "-q", &spec]) {
        Ok(out) => Ok(out.stdout.trim().to_string()),
        Err(_) => Err(CannotCheck {
            what: format!("--config-from {rev} is not a commit in this clone"),
            fix: "git fetch, then name the commit whose rules judge the push".into(),
        }),
    }
}

/// The text of `path` (at the top of the tree) in commit `sha`: `None` when the tree
/// has no such entry; cannot-check when it is not a plain file or cannot be read.
pub fn file_at(root: &Path, sha: &str, path: &str) -> Result<Option<String>, CannotCheck> {
    let listed = git(root, &["ls-tree", "-z", sha, "--", path])?;
    let entry = split_nul(&listed.stdout).into_iter().find_map(|e| {
        e.split_once('\t')
            .filter(|(_, p)| *p == path)
            .map(|(m, _)| m.to_string())
    });
    let Some(meta) = entry else {
        return Ok(None);
    };
    let [mode, kind, oid] = meta.split(' ').collect::<Vec<_>>()[..] else {
        return Err(not_a_file(path, sha));
    };
    if kind != "blob" || !matches!(mode, "100644" | "100755") {
        return Err(not_a_file(path, sha));
    }
    Ok(Some(git(root, &["cat-file", "blob", oid])?.stdout))
}

fn not_a_file(path: &str, sha: &str) -> CannotCheck {
    CannotCheck {
        what: format!("{path} at {} is not a file", &sha[..sha.len().min(12)]),
        fix: format!("make {path} a plain file"),
    }
}

/// A problem with a file read at `rev`, saying so.
fn named(c: CannotCheck, rev: &str) -> CannotCheck {
    CannotCheck {
        what: format!("at --config-from {rev}: {}", c.what),
        fix: c.fix,
    }
}
