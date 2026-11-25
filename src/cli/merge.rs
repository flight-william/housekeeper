//! At `house check --stage merge`: the commit-msg hook's checks on each commit the
//! branch sends, so a commit made without the hook cannot merge unchecked. They run
//! before any tool or test, which run the pull request's own code, and every git call
//! ignores replace refs (`run::GIT_ENV`).
//!
//! Each commit's message is judged as git stored it (`commitmsg::check_stored`), with
//! facts read from the commit and the rules the check runs by (the base's, with
//! `--config-from`), never from the runner: its author from the commit, the expected
//! author only from `[commit] author` (the runner's `git config` is not the owner's),
//! which the committer must match too, and no agent session, since the runner's
//! environment is not the agent's. The approval check judges each commit's own
//! changed paths, deletes included, against that commit's own message, as the hook
//! does: one commit's approval line covers no other commit.
//!
//! A merge commit whose tree is the clean merge of its two parents (`git merge-tree
//! --write-tree`, git 2.38 or newer) holds no work of its own: its parents' commits
//! are the record, so it is not judged. GitHub's "Update branch" writes such a merge,
//! with no body. A merge with changes of its own (a resolved conflict, an edit) is
//! judged like any commit, its own paths being those that differ from the merge git
//! would make; one of more than two parents is judged on what differs from its first
//! parent. Its `Merge ` subject is git's own and needs no area.

use crate::adapter::{CannotCheck, Command};
use crate::config::Config;
use crate::finding::{Finding, Place};
use crate::reply::ToolRun;
use crate::run::{self, git};
use crate::{approval, commitmsg, files};
use std::path::Path;
use std::time::{Duration, Instant};

/// The commit-msg checks on each of `commits`, as one tool run named `commit-msg`;
/// the first commit that cannot be read is its cannot-check, and the others are
/// still judged.
pub(super) fn judge_range(root: &Path, commits: &[String], config: &Config) -> ToolRun {
    let clock = Instant::now();
    let mut tool = ToolRun {
        name: "commit-msg".into(),
        seconds: 0.0,
        exit: None,
        skipped: false,
        findings: Vec::new(),
        cannot: None,
        unread: Vec::new(),
    };
    let mut protected: Vec<String> = approval::PROTECTED.iter().map(|g| g.to_string()).collect();
    protected.extend(config.protected.extra.iter().cloned());
    for sha in commits {
        match judge_one(root, sha, config, &protected) {
            Ok(findings) => tool.findings.extend(findings),
            Err(c) => {
                tool.cannot.get_or_insert(c);
            }
        }
    }
    tool.seconds = clock.elapsed().as_secs_f64();
    tool
}

/// One commit as git stored it.
struct Stored {
    sha: String,
    tree: String,
    parents: Vec<String>,
    /// `Name <email> <time> <zone>`, as `git var GIT_AUTHOR_IDENT` prints it.
    author_ident: String,
    /// `Name <email>` of the committer.
    committer: String,
    message: String,
}

/// The commit's R17, R9/cause and approval findings, each naming the commit; none
/// for a clean merge.
fn judge_one(
    root: &Path,
    sha: &str,
    config: &Config,
    protected: &[String],
) -> Result<Vec<Finding>, CannotCheck> {
    let stored = read(root, sha)?;
    let Some(own) = own_paths(root, &stored)? else {
        return Ok(Vec::new());
    };
    let rules = &config.commit;
    let facts = commitmsg::Commit {
        author_ident: stored.author_ident.clone(),
        author_name: rules
            .author
            .clone()
            .unwrap_or(name_of(&stored.author_ident)),
        author_email: rules.author.as_ref().and(rules.email.clone()),
        comment_char: None,
        merge_head: stored.parents.len() > 1,
        session: None,
        models: rules.models.clone(),
    };
    let mut findings = commitmsg::check_stored(&stored.message, &facts)?;
    findings.extend(committer(&stored, config));
    findings.extend(approval::check_commit(
        root,
        &own.paths,
        &own.links,
        protected,
        &stored.message,
    ));
    let short = &sha[..sha.len().min(12)];
    for f in &mut findings {
        f.what = format!("commit {short}: {}", f.what);
    }
    Ok(findings)
}

fn name_of(ident: &str) -> String {
    ident
        .split_once(" <")
        .map(|(name, _)| name.to_string())
        .unwrap_or_default()
}

/// With `[commit] author` set, a committer other than it (or than `email`, when
/// given) is an R17 finding: the author line alone is any name typed.
fn committer(stored: &Stored, config: &Config) -> Option<Finding> {
    let author = config.commit.author.as_ref()?;
    let email = stored
        .committer
        .split_once(" <")
        .map(|(_, rest)| rest.trim_end_matches('>'))
        .unwrap_or_default();
    let wrong_email = config.commit.email.as_deref().is_some_and(|e| e != email);
    (name_of(&stored.committer) != *author || wrong_email).then(|| Finding {
        rule: "R17".into(),
        place: Place::Repo,
        line: None,
        what: format!(
            "committer is not the installed author: {}",
            stored.committer
        ),
        fix: "commit under the author name bound at install (git config user.name)".into(),
        tool_rule: "house r17.committer".into(),
        measure: None,
    })
}

/// The commit's tree, parents, author, committer and message, from one `git log` of
/// it. The message comes re-encoded to UTF-8 when the commit names another encoding.
fn read(root: &Path, sha: &str) -> Result<Stored, CannotCheck> {
    let format = "--format=%T%x00%P%x00%an <%ae> %ad%x00%cn <%ce>%x00%B";
    let out = git(root, &["log", "-1", "--date=raw", format, sha, "--"])?;
    let mut parts = out.stdout.splitn(5, '\0');
    let mut next = || parts.next().unwrap_or_default().to_string();
    let (tree, parents, author_ident, committer, message) =
        (next(), next(), next(), next(), next());
    if tree.is_empty() || author_ident.is_empty() {
        return Err(CannotCheck {
            what: format!("commit {sha} could not be read"),
            fix: "git fetch the pull request's commits, then run house again".into(),
        });
    }
    Ok(Stored {
        sha: sha.to_string(),
        tree,
        parents: parents.split_whitespace().map(str::to_string).collect(),
        author_ident,
        committer,
        message,
    })
}

/// The paths the commit changes itself, deletes included, with its links; `None` for
/// a clean merge, which changes nothing of its own.
fn own_paths(root: &Path, commit: &Stored) -> Result<Option<Own>, CannotCheck> {
    let from = match &commit.parents[..] {
        [] => git(root, &["hash-object", "-t", "tree", "/dev/null"])?
            .stdout
            .trim()
            .to_string(),
        [parent] => parent.clone(),
        [first, second] => match merged_tree(root, first, second)? {
            (tree, true) if tree == commit.tree => return Ok(None),
            (tree, _) => tree,
        },
        [first, ..] => first.clone(),
    };
    let raw = git(
        root,
        &[
            "diff-tree",
            "-r",
            "--raw",
            "--no-renames",
            "-z",
            &from,
            &commit.sha,
        ],
    )?;
    Ok(Some(raw_paths(&raw.stdout)))
}

/// What one commit changes: every path, and those its trees hold as a link (mode
/// 120000, before or after), read from git and not from the disk.
struct Own {
    paths: Vec<String>,
    links: Vec<String>,
}

/// `git diff-tree --raw -z`: `:<old mode> <new mode> <oid> <oid> <status>` then the path.
fn raw_paths(text: &str) -> Own {
    let fields = files::split_nul(text);
    let mut own = Own {
        paths: Vec::new(),
        links: Vec::new(),
    };
    for [meta, path] in fields.as_chunks::<2>().0 {
        let mut modes = meta.trim_start_matches(':').split(' ').take(2);
        if modes.any(|m| m == "120000") {
            own.links.push(path.clone());
        }
        own.paths.push(path.clone());
    }
    own
}

/// The tree `git merge-tree --write-tree` makes of the two parents, conflicts and all,
/// and whether the merge was clean.
fn merged_tree(root: &Path, first: &str, second: &str) -> Result<(String, bool), CannotCheck> {
    let cmd = Command {
        program: "git".into(),
        args: ["merge-tree", "--write-tree", first, second]
            .map(String::from)
            .to_vec(),
    };
    let guard = Duration::from_secs(60);
    let out = run::run_env(&cmd, root, "git", guard, &run::git_env()).result?;
    let tree = out.stdout.lines().next().unwrap_or_default().to_string();
    if matches!(out.exit, 0 | 1) && !tree.is_empty() {
        return Ok((tree, out.exit == 0));
    }
    Err(CannotCheck {
        what: format!(
            "git merge-tree --write-tree failed: {}",
            run::first_line(&out.stderr)
        ),
        fix: "install git 2.38 or newer, which merge-tree --write-tree needs".into(),
    })
}
