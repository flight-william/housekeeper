//! At `house check --stage merge`, every commit in base..head is judged as the
//! commit-msg hook judges one: R17 and R9's cause line on its message, as git stored
//! it, and the approval check on the protected paths the range changes. A commit made
//! without the hook cannot merge unchecked. The facts come from the commit and the
//! base's rules, never from the runner's environment: no agent session is read, and
//! the expected author is the base's `[commit] author` only.

mod merge_support;

use merge_support::{
    GOOD, R3_ONLY, ZERO, based, commit, findings, git, line, merge, repo, run, sha,
};

const OPUS: &str = "Co-Authored-By: Claude (claude-opus-5-5) <noreply@anthropic.com>";

#[test]
fn a_commit_without_a_record_is_refused_at_merge_not_at_push() {
    let (dir, base) = based("mc-record", &[(".house.toml", R3_ONLY)]);
    let first = commit(&dir, &[("a.py", "x = 1\n")], GOOD);
    let bad = commit(&dir, &[("b.py", "y = 2\n")], "quick change");
    let out = merge(&dir, &base, &bad);
    assert_eq!(out.code, 1, "{}", out.out);
    let short = &bad[..12];
    let lines = findings(&out.out);
    assert!(
        lines.iter().any(|l| l.starts_with("R17 repo:1: commit ")
            && l.contains(short)
            && l.contains("subject is not `area: text`")),
        "{}",
        out.out
    );
    assert_eq!(
        lines.iter().filter(|l| l.contains("body lacks")).count(),
        3,
        "{}",
        out.out
    );
    assert!(
        !out.out.contains(&first[..12]),
        "the good commit has no finding: {}",
        out.out
    );
    assert!(
        out.record.contains("house r17.subject"),
        "{}{}",
        out.record,
        out.err
    );
    // Push is unchanged: the commit-msg hook's checks are not run there.
    let push = run(
        &dir,
        "push",
        &["--config-from", &base],
        &line(&bad, &base),
        &[],
    );
    assert_eq!(push.code, 0, "{}", push.out);
}

#[test]
fn a_fix_without_a_cause_is_refused_at_merge() {
    let (dir, base) = based("mc-cause", &[(".house.toml", R3_ONLY)]);
    let local = commit(
        &dir,
        &[("a.py", "x = 1\n")],
        "app: fix the parser\n\nWhy: w.\nProof: p.\nUndo: u.\n",
    );
    let out = merge(&dir, &base, &local);
    assert_eq!(out.code, 1, "{}", out.out);
    assert!(out.out.contains("R9 repo: commit"), "{}", out.out);
    assert!(
        out.out.contains("fix commit without a Cause: line"),
        "{}",
        out.out
    );
}

// The runner is not the agent's session: the marker in its environment owes nothing,
// and a message naming an agent is judged by its text alone.
#[test]
fn the_session_is_not_read_from_the_runner() {
    let (dir, base) = based("mc-session", &[(".house.toml", R3_ONLY)]);
    let local = commit(&dir, &[("a.py", "x = 1\n")], GOOD);
    let line = line(&local, &base);
    let args = ["--config-from", base.as_str()];
    let out = run(&dir, "merge", &args, &line, &[("CLAUDECODE", "1")]);
    assert_eq!(out.code, 0, "{}", out.out);
    let named = commit(
        &dir,
        &[("b.py", "y = 2\n")],
        &format!("{GOOD}\nCo-Authored-By: Claude <noreply@anthropic.com>\n"),
    );
    let out = merge(&dir, &base, &named);
    assert_eq!(out.code, 1, "{}", out.out);
    assert!(out.out.contains("[house r17.trailer]"), "{}", out.out);
}

// The model ids and the author come from the base's `.house.toml`: the pull
// request's own list does not count, nor the runner's `git config user.name`.
#[test]
fn the_models_and_the_author_come_from_the_base() {
    let rules =
        "[tools]\npush = [\"r3\"]\n[commit]\nmodels = [\"claude-opus-5-5\"]\nauthor = \"test\"\n";
    let (dir, base) = based("mc-base-rules", &[(".house.toml", rules)]);
    let listed = commit(&dir, &[("a.py", "x = 1\n")], &format!("{GOOD}\n{OPUS}\n"));
    assert_eq!(merge(&dir, &base, &listed).code, 0);
    let widened = rules.replace(
        "\"claude-opus-5-5\"",
        "\"claude-opus-5-5\", \"claude-new-9\"",
    );
    let other = "Co-Authored-By: Claude (claude-new-9) <noreply@anthropic.com>";
    let local = commit(
        &dir,
        &[(".house.toml", &widened)],
        &format!("{GOOD}\nOwner-approved: 2026-11-25 \"yes\"\n\n{other}\n"),
    );
    let out = merge(&dir, &base, &local);
    assert_eq!(out.code, 1, "{}", out.out);
    assert!(out.out.contains("claude-new-9"), "{}", out.out);
    // Another author than the base names is refused, whatever git config says.
    git(&dir, &["config", "user.name", "Mallory"]);
    let mallory = commit(&dir, &[("b.py", "y = 2\n")], GOOD);
    git(&dir, &["config", "user.name", "test"]);
    let out = merge(&dir, &base, &mallory);
    assert!(
        out.out
            .contains("commit author is not the installed author: Mallory"),
        "{}",
        out.out
    );
}

#[test]
fn without_a_base_author_no_author_is_expected() {
    let (dir, base) = based("mc-no-author", &[(".house.toml", R3_ONLY)]);
    git(&dir, &["config", "user.name", "Someone Else"]);
    let local = commit(&dir, &[("a.py", "x = 1\n")], GOOD);
    let out = merge(&dir, &base, &local);
    assert_eq!(out.code, 0, "{}", out.out);
}

// A message is judged as git stored it: a `#` line in it is text, not a comment.
#[test]
fn the_stored_message_is_judged_verbatim() {
    let (dir, base) = based("mc-verbatim", &[(".house.toml", R3_ONLY)]);
    let local = commit(
        &dir,
        &[("a.py", "x = 1\n")],
        "app: add the file\n# note\n\nWhy: w.\nProof: p.\nUndo: u.\n",
    );
    let out = merge(&dir, &base, &local);
    assert_eq!(out.code, 1, "{}", out.out);
    assert!(
        out.out.contains("line 2 is not empty: # note"),
        "{}",
        out.out
    );
}

/// A repo with `main` at a base, a feature branch with one good commit, and main
/// moved on by one good commit: returns (dir, main's tip).
fn diverged(name: &str) -> (std::path::PathBuf, String) {
    let (dir, _) = based(name, &[(".house.toml", R3_ONLY)]);
    git(&dir, &["branch", "-q", "-M", "main"]);
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    commit(&dir, &[("a.py", "x = 1\n")], GOOD);
    git(&dir, &["checkout", "-q", "main"]);
    let tip = commit(&dir, &[("m.py", "z = 3\n")], GOOD);
    git(&dir, &["checkout", "-q", "feature"]);
    (dir, tip)
}

// GitHub's "Update branch" writes `Merge branch 'main' into X`, no body, as the
// person who pressed it. A merge that adds nothing to the clean merge of its parents
// is not work of its own and is not refused for being one.
#[test]
fn a_clean_merge_is_not_refused() {
    let (dir, tip) = diverged("mc-clean-merge");
    git(
        &dir,
        &[
            "-c",
            "user.name=github-login",
            "merge",
            "-q",
            "--no-ff",
            "--no-verify",
            "-m",
            "Merge branch 'main' into feature",
            "main",
        ],
    );
    let local = sha(&dir, "HEAD");
    let out = merge(&dir, &tip, &local);
    assert_eq!(out.code, 0, "{}", out.out);
}

// A merge that changes what the clean merge would hold (a conflict resolved, a line
// slipped in) is work, and is judged like any commit; its `Merge ` subject is fine.
#[test]
fn a_merge_with_changes_of_its_own_is_judged() {
    let (dir, tip) = diverged("mc-own-merge");
    git(&dir, &["merge", "-q", "--no-ff", "--no-commit", "main"]);
    std::fs::write(dir.join("m.py"), "z = 4\n").unwrap();
    git(&dir, &["add", "-A"]);
    git(
        &dir,
        &[
            "commit",
            "-q",
            "--no-verify",
            "-m",
            "Merge branch 'main' into feature",
        ],
    );
    let out = merge(&dir, &tip, &sha(&dir, "HEAD"));
    assert_eq!(out.code, 1, "{}", out.out);
    assert_eq!(out.out.matches("body lacks").count(), 3, "{}", out.out);
    assert!(!out.out.contains("subject is not"), "{}", out.out);
    // With a record it passes.
    git(
        &dir,
        &[
            "commit",
            "-q",
            "--amend",
            "--no-verify",
            "-m",
            "Merge branch 'main' into feature\n\nWhy: w.\nProof: p.\nUndo: u.\n",
        ],
    );
    let out = merge(&dir, &tip, &sha(&dir, "HEAD"));
    assert_eq!(out.code, 0, "{}", out.out);
}

// A conflict resolved in the merge is work of its own too.
#[test]
fn a_merge_that_resolved_a_conflict_is_judged() {
    let (dir, tip) = diverged("mc-conflict");
    commit(&dir, &[("m.py", "z = 5\n")], GOOD);
    let merged = std::process::Command::new("git")
        .args(["merge", "-q", "--no-ff", "--no-commit", "main"])
        .current_dir(&dir)
        .env_remove("GIT_DIR")
        .status()
        .unwrap();
    assert!(!merged.success(), "the merge conflicts");
    std::fs::write(dir.join("m.py"), "z = 6\n").unwrap();
    git(&dir, &["add", "-A"]);
    git(
        &dir,
        &[
            "commit",
            "-q",
            "--no-verify",
            "-m",
            "Merge branch 'main' into feature",
        ],
    );
    let out = merge(&dir, &tip, &sha(&dir, "HEAD"));
    assert_eq!(out.code, 1, "{}", out.out);
    assert_eq!(out.out.matches("body lacks").count(), 3, "{}", out.out);
}

// A protected path a commit changes, a deletion too, needs an approval line in that
// commit's own message, as the commit-msg hook asks.
#[test]
fn a_protected_change_in_the_range_needs_approval() {
    let ignore = (".semgrepignore", "tests/\n");
    let (dir, base) = based("mc-approval", &[(".house.toml", R3_ONLY), ignore]);
    std::fs::remove_file(dir.join(".semgrepignore")).unwrap();
    let local = commit(&dir, &[], GOOD);
    let out = merge(&dir, &base, &local);
    assert_eq!(out.code, 1, "{}", out.out);
    let want = format!(
        "R9 .semgrepignore: commit {}: protected file changed without the owner's approval line",
        &local[..12]
    );
    assert!(out.out.contains(&want), "{}", out.out);
    let message = format!("{GOOD}Owner-approved: 2026-11-25 \"drop it\"\n");
    git(
        &dir,
        &["commit", "-q", "--amend", "--no-verify", "-m", &message],
    );
    let out = merge(&dir, &base, &sha(&dir, "HEAD"));
    assert_eq!(out.code, 0, "{}", out.out);
}

// A first branch with no origin/main has no base: every commit it holds is judged.
#[test]
fn an_unbounded_range_judges_every_commit() {
    let dir = repo("mc-unbounded");
    let first = commit(&dir, &[(".house.toml", R3_ONLY)], "start");
    let out = run(&dir, "merge", &[], &line(&first, ZERO), &[]);
    assert_eq!(out.code, 1, "{}", out.out);
    assert!(out.out.contains(&first[..12]), "{}", out.out);
}
