//! The wiring of `house commit-msg` and `init --owner`.

mod common;
mod wire_run;

use house::cli;
use std::path::{Path, PathBuf};
use std::process::Command as Proc;
use wire_run::{HOUSE, house, records};

const GOOD: &str = "parser: keep the last line\n\nWhy: the parser dropped it.\nProof: cargo test, 12 passed.\nUndo: git revert this commit.\n";

/// A repo whose `.house.toml` names one model and the author `test`.
fn commit_repo(name: &str) -> PathBuf {
    let repo = common::repo(name);
    std::fs::write(
        repo.join(".house.toml"),
        "[commit]\nmodels = [\"claude-opus-5-5\"]\nauthor = \"test\"\n",
    )
    .unwrap();
    repo
}

fn message(repo: &Path, text: &str) {
    std::fs::write(repo.join("MSG"), text).unwrap();
}

// ---- house commit-msg ----

#[test]
fn commit_msg_clean_message_exits_zero_and_records_stage_commit_msg() {
    let repo = commit_repo("cm-clean");
    let log = common::fresh("cm-clean-log");
    message(&repo, GOOD);
    let (code, text) = house(&repo, &log, &["commit-msg", "MSG"], &[]);
    assert_eq!(code, 0, "{text}");
    assert!(
        text.starts_with("house commit-msg: nothing to fix ("),
        "{text}"
    );
    let lines = records(&log);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["stage"], "commit-msg");
    assert_eq!(lines[0]["exit"], 0);
    assert_eq!(lines[0]["excused"], 0);
    assert_eq!(lines[0]["tools"][0]["name"], "commit-msg");
}

// An agent session owes the agent's Co-Authored-By line and nothing else: no session
// line, whose link is noise once the session is gone.
#[test]
fn commit_msg_in_an_agent_session_needs_no_session_line() {
    let repo = commit_repo("cm-no-session");
    let log = common::fresh("cm-no-session-log");
    let agent = "Co-Authored-By: Claude (claude-opus-5-5) <noreply@anthropic.com>";
    message(&repo, &format!("{GOOD}\n{agent}\n"));
    let (code, text) = house(&repo, &log, &["commit-msg", "MSG"], &[("CLAUDECODE", "1")]);
    assert_eq!(code, 0, "{text}");
}

#[test]
fn commit_msg_vague_agent_message_gives_r17_lines_and_exit_one() {
    let repo = commit_repo("cm-bad");
    let log = common::fresh("cm-bad-log");
    message(&repo, "parser: wip\n");
    let (code, text) = house(&repo, &log, &["commit-msg", "MSG"], &[("CLAUDECODE", "1")]);
    assert_eq!(code, 1, "{text}");
    assert!(text.starts_with("house commit-msg: 5 to fix ("), "{text}");
    assert!(
        text.contains("R17 repo:1: subject says only wip, fix or update: wip. fix: say what changed, in words a reader of the log understands without the diff. [house r17.vague]\n"),
        "{text}"
    );
    assert!(
        text.contains("R17 repo: agent commit without a valid Co-Authored-By line naming the agent: Co-Authored-By:. fix: add `Co-Authored-By: <Name> (<model id>) <noreply@…>` with a model id listed in .house.toml. [house r17.agent]\n"),
        "{text}"
    );
    assert!(!text.contains("Claude-Session"), "{text}");
    let line = &records(&log)[0];
    assert_eq!(line["stage"], "commit-msg");
    assert_eq!(line["findings"].as_array().unwrap().len(), 5);
}

#[test]
fn commit_msg_reads_the_author_git_exports_and_the_config_author() {
    let repo = commit_repo("cm-author");
    let log = common::fresh("cm-author-log");
    message(&repo, GOOD);
    let (code, text) = house(
        &repo,
        &log,
        &["commit-msg", "MSG"],
        &[("GIT_AUTHOR_NAME", "Someone Else")],
    );
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("R17 repo: commit author is not the installed author: Someone Else."),
        "{text}"
    );
}

// Regression: the owner's name with an agent's email must not pass as the owner.
#[test]
fn commit_msg_checks_the_email_when_the_config_gives_author_and_email() {
    let repo = commit_repo("cm-email");
    let log = common::fresh("cm-email-log");
    std::fs::write(
        repo.join(".house.toml"),
        "[commit]\nmodels = [\"claude-opus-5-5\"]\nauthor = \"test\"\nemail = \"test@example.com\"\n",
    )
    .unwrap();
    message(&repo, GOOD);
    let (code, text) = house(&repo, &log, &["commit-msg", "MSG"], &[]);
    assert_eq!(code, 0, "{text}");
    let agent = [
        ("GIT_AUTHOR_NAME", "test"),
        ("GIT_AUTHOR_EMAIL", "agent@example.org"),
    ];
    let (code, text) = house(&repo, &log, &["commit-msg", "MSG"], &agent);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("commit author is not the installed author: test <agent@example.org>."),
        "{text}"
    );
}

#[test]
fn commit_msg_through_a_real_hook_sees_git_commit_author() {
    let repo = commit_repo("cm-hook");
    let log = common::fresh("cm-hook-log");
    let hook = repo.join(".git/hooks/commit-msg");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(
        &hook,
        format!(
            "#!/bin/sh\nHOUSE_LOG_DIR='{}' '{HOUSE}' commit-msg \"$1\"\n",
            log.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    common::git(&repo, &["add", ".house.toml"]);
    message(&repo, GOOD);
    let out = Proc::new("git")
        .args([
            "commit",
            "-q",
            "--author",
            "Other <o@example.org>",
            "-F",
            "MSG",
        ])
        .current_dir(&repo)
        .env_remove("CLAUDECODE")
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap();
    let said =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "the hook's exit 1 stops the commit: {said}"
    );
    assert!(
        said.contains("commit author is not the installed author: Other."),
        "{said}"
    );
    // .house.toml is staged and protected: the approval check speaks too.
    assert!(
        said.contains("R9 .house.toml: protected file changed without the owner's approval line."),
        "{said}"
    );
    assert_eq!(records(&log)[0]["stage"], "commit-msg");
}

#[test]
fn commit_msg_runs_the_approval_check_on_staged_files() {
    let repo = commit_repo("cm-approval");
    let log = common::fresh("cm-approval-log");
    common::git(&repo, &["add", ".house.toml"]);
    message(&repo, GOOD);
    let (code, text) = house(&repo, &log, &["commit-msg", "MSG"], &[]);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("R9 .house.toml: protected file changed without the owner's approval line. fix: add Owner-approved: <date> \"<the owner's words>\" to the commit message, quoting the owner. [approval]\n"),
        "{text}"
    );
    message(
        &repo,
        &format!("{GOOD}Owner-approved: 2026-11-22 \"add the commit rules\"\n"),
    );
    let (code, text) = house(&repo, &log, &["commit-msg", "MSG"], &[]);
    assert_eq!(code, 0, "{text}");
}

#[test]
fn commit_msg_without_a_readable_file_cannot_check() {
    let repo = commit_repo("cm-nofile");
    let log = common::fresh("cm-nofile-log");
    let (code, text) = house(&repo, &log, &["commit-msg", "NOPE"], &[]);
    assert_eq!(code, 2, "{text}");
    assert!(
        text.starts_with("house commit-msg: 1 cannot check ("),
        "{text}"
    );
    assert!(
        text.contains("R2 cannot check: the message file "),
        "{text}"
    );
}

#[test]
fn commit_msg_and_init_owner_parse() {
    let a = |s: &[&str]| cli::parse(&s.iter().map(|x| x.to_string()).collect::<Vec<_>>());
    assert_eq!(
        a(&["commit-msg", ".git/COMMIT_EDITMSG"]),
        Ok(cli::Cmd::CommitMsg {
            file: ".git/COMMIT_EDITMSG".into()
        })
    );
    assert!(a(&["commit-msg"]).is_err());
    assert_eq!(
        a(&["init", "--owner", "@someone"]),
        Ok(cli::Cmd::InitOwner {
            owner: "@someone".into()
        })
    );
    assert!(a(&["init", "--owner", "someone"]).is_err());
    assert!(a(&["init", "--owner"]).is_err());
}

#[test]
fn init_owner_writes_codeowners_with_that_owner() {
    let repo = common::repo("init-owner");
    let log = common::fresh("init-owner-log");
    let (code, text) = house(&repo, &log, &["init", "--owner", "@someone"], &[]);
    assert_eq!(code, 0, "{text}");
    let owners = std::fs::read_to_string(repo.join(".github/CODEOWNERS")).unwrap();
    assert!(owners.contains("@someone"), "{owners}");
}
