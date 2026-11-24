//! `house commit-msg <file>`: the message's own rules and the approval check.

use super::{root_or_say, write_record};
use crate::adapter::{CannotCheck, Stage};
use crate::finding::Finding;
use crate::reply::{Report, ToolRun};
use crate::{approval, commitmsg, config, files, run};
use std::path::Path;
use std::time::{Instant, SystemTime};

/// `house commit-msg <file>`: R17 and R9/cause on the message, and the approval
/// check on the staged files. Header `house commit-msg:`; recorded as stage "commit-msg".
pub(super) fn commit_msg(cwd: &Path, file: &str) -> i32 {
    let start = SystemTime::now();
    let clock = Instant::now();
    let Some(root) = root_or_say(cwd, "commit-msg") else {
        return 2;
    };
    let mut tool = ToolRun {
        name: "commit-msg".into(),
        seconds: 0.0,
        exit: None,
        skipped: false,
        findings: Vec::new(),
        cannot: None,
        unread: Vec::new(),
    };
    match judge_message(&root, &cwd.join(file)) {
        Ok(findings) => tool.findings = findings,
        Err(c) => tool.cannot = Some(c),
    }
    tool.seconds = clock.elapsed().as_secs_f64();
    let report = Report {
        stage: Stage::Commit,
        seconds: tool.seconds,
        tools: vec![tool],
    };
    print!("{}", report.text_as("commit-msg", 0));
    write_record(
        &root,
        start,
        &report,
        "commit-msg",
        &Default::default(),
        &[],
    );
    report.exit()
}

/// The message file's findings: `commitmsg::check`, then `approval::check` on every
/// staged change and the message as git keeps it (`commitmsg::kept`).
pub fn judge_message(root: &Path, file: &Path) -> Result<Vec<Finding>, CannotCheck> {
    let text = std::fs::read_to_string(file).map_err(|e| CannotCheck {
        what: format!("the message file {} cannot be read: {e}", file.display()),
        fix: "pass the file git gives the commit-msg hook: house commit-msg \"$1\"".into(),
    })?;
    let facts = commit_facts(root)?;
    let mut findings = commitmsg::check(&text, &facts)?;
    let protected = approval::protected(root)?;
    let changed: Vec<String> = files::staged_changes(root)?
        .into_iter()
        .map(|c| c.path)
        .collect();
    let kept = commitmsg::kept(&text, &facts)?;
    findings.extend(approval::check_in(root, &changed, &protected, &kept));
    Ok(findings)
}

/// What `commitmsg::check` needs besides the text, read from git, the environment
/// and `.house.toml`. The expected author: `[commit] author`, else `git config
/// user.name`, else the author's own name (no author rule); the expected email is
/// `[commit] email`, only when `author` is given too.
pub fn commit_facts(root: &Path) -> Result<commitmsg::Commit, CannotCheck> {
    let cfg = config::load(root)?;
    let author_ident = author_ident(root)?;
    let git_value = |args: &[&str]| {
        run::git(root, args)
            .ok()
            .map(|o| o.stdout.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let own_name = author_ident
        .split_once(" <")
        .map(|(name, _)| name.to_string())
        .unwrap_or_default();
    let author_email = cfg.commit.author.as_ref().and(cfg.commit.email);
    let author_name = cfg
        .commit
        .author
        .or_else(|| git_value(&["config", "--get", "user.name"]))
        .unwrap_or(own_name);
    let merge_head = git_value(&["rev-parse", "--git-path", "MERGE_HEAD"])
        .is_some_and(|p| root.join(p).exists());
    Ok(commitmsg::Commit {
        author_ident,
        author_name,
        author_email,
        comment_char: git_value(&["config", "--get", "core.commentChar"]),
        merge_head,
        claudecode: std::env::var("CLAUDECODE").ok(),
        models: cfg.commit.models,
    })
}

/// `git var GIT_AUTHOR_IDENT`, keeping the GIT_AUTHOR_* that `git commit` exports
/// to its hooks (an `--author` or an agent's override), stripping every other GIT_*.
fn author_ident(root: &Path) -> Result<String, CannotCheck> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(["var", "GIT_AUTHOR_IDENT"]).current_dir(root);
    for (key, _) in std::env::vars_os() {
        let k = key.to_string_lossy();
        if k.starts_with("GIT_") && !k.starts_with("GIT_AUTHOR_") {
            cmd.env_remove(&key);
        }
    }
    let cannot = |why: String| CannotCheck {
        what: format!("git var GIT_AUTHOR_IDENT failed: {why}"),
        fix: "set git config user.name and user.email, then commit again".into(),
    };
    let out = cmd.output().map_err(|e| cannot(e.to_string()))?;
    if !out.status.success() {
        return Err(cannot(run::first_line(&String::from_utf8_lossy(
            &out.stderr,
        ))));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
