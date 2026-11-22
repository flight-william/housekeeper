//! The push stage: push lines on stdin, and the files a push sends.

mod common;

use common::{git, repo};
use house::push::{PushLine, files_for_push, parse_lines, pushed};
use std::path::Path;
use std::process::Command;

const ZERO: &str = "0000000000000000000000000000000000000000";
const A: &str = "1111111111111111111111111111111111111111";
const B: &str = "2222222222222222222222222222222222222222";

fn sha(dir: &Path, rev: &str) -> String {
    let out = Command::new("git")
        .args(["rev-parse", rev])
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn commit(dir: &Path, files: &[(&str, &str)], message: &str) -> String {
    for (path, text) in files {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    sha(dir, "HEAD")
}

fn line(local: &str, remote: &str) -> String {
    format!("refs/heads/feature {local} refs/heads/feature {remote}\n")
}

#[test]
fn empty_push_is_no_lines() {
    assert_eq!(parse_lines(""), Ok(vec![]));
    assert_eq!(parse_lines("\n"), Ok(vec![]));
}

#[test]
fn delete_push_is_skipped() {
    assert_eq!(parse_lines(&line(ZERO, A)), Ok(vec![]));
}

#[test]
fn zero_remote_sha_is_a_new_branch() {
    let text = format!("{}refs/tags/v1 {B} refs/tags/v1 {A}\n", line(A, ZERO));
    assert_eq!(
        parse_lines(&text),
        Ok(vec![
            PushLine {
                local_ref: "refs/heads/feature".into(),
                local: A.into(),
                remote_ref: "refs/heads/feature".into(),
                remote: None,
            },
            PushLine {
                local_ref: "refs/tags/v1".into(),
                local: B.into(),
                remote_ref: "refs/tags/v1".into(),
                remote: Some(A.into()),
            },
        ])
    );
}

#[test]
fn malformed_line_cannot_check() {
    for bad in [
        "just words\n",
        "a b c\n",
        &format!("r {A} r nothex\n"),
        "\nr x r y\n",
    ] {
        let err = parse_lines(bad).unwrap_err();
        assert!(err.what.starts_with("not a push line"), "{bad:?}: {err:?}");
    }
}

#[test]
fn existing_branch_gives_files_since_the_remote_sha() {
    let dir = repo("push-existing");
    let remote = commit(&dir, &[("old.py", "x = 1\n"), ("gone.py", "y\n")], "one");
    std::fs::remove_file(dir.join("gone.py")).unwrap();
    let local = commit(&dir, &[("new.py", "z = 2\n"), ("old.py", "x = 3\n")], "two");
    let files = files_for_push(&dir, &line(&local, &remote)).unwrap();
    assert_eq!(
        files,
        vec!["new.py", "old.py"],
        "deleted gone.py is not sent"
    );
}

#[test]
fn nothing_new_gives_no_files() {
    let dir = repo("push-same");
    let head = commit(&dir, &[("a.py", "a\n")], "one");
    assert_eq!(files_for_push(&dir, &line(&head, &head)), Ok(vec![]));
    assert_eq!(files_for_push(&dir, &line(ZERO, &head)), Ok(vec![]));
    assert_eq!(files_for_push(&dir, "\n"), Ok(vec![]));
}

#[test]
fn new_branch_gives_files_since_the_merge_base_with_origin_main() {
    let dir = repo("push-new-branch");
    let main = commit(&dir, &[("base.py", "b\n")], "main");
    git(&dir, &["update-ref", "refs/remotes/origin/main", &main]);
    git(&dir, &["checkout", "-q", "-b", "feature"]);
    let local = commit(&dir, &[("feature.py", "f\n")], "feature");
    let files = files_for_push(&dir, &line(&local, ZERO)).unwrap();
    assert_eq!(files, vec!["feature.py"]);
}

#[test]
fn new_repo_without_origin_main_gives_every_file() {
    let dir = repo("push-first");
    let local = commit(&dir, &[("a.py", "a\n"), ("sub/b.sh", "b\n")], "first");
    let files = files_for_push(&dir, &line(&local, ZERO)).unwrap();
    assert_eq!(files, vec!["a.py", "sub/b.sh"]);
}

/// The remote sha this clone has not fetched is not replaced by another base, such
/// as the merge base: a guessed base could skip files the push sends, so it is
/// cannot-check.
#[test]
fn unfetched_remote_sha_cannot_check() {
    let dir = repo("push-unfetched");
    let main = commit(&dir, &[("base.py", "b\n")], "main");
    git(&dir, &["update-ref", "refs/remotes/origin/main", &main]);
    let local = commit(&dir, &[("mine.py", "m\n")], "mine");
    let err = files_for_push(&dir, &line(&local, A)).unwrap_err();
    assert!(err.what.contains("has not fetched"), "{err:?}");
    assert!(err.fix.contains("git fetch"), "{err:?}");
}

#[test]
fn a_new_branch_sharing_no_history_with_origin_main_cannot_check() {
    let dir = repo("push-unrelated");
    let main = commit(&dir, &[("base.py", "b\n")], "main");
    git(&dir, &["update-ref", "refs/remotes/origin/main", &main]);
    git(&dir, &["checkout", "-q", "--orphan", "other"]);
    let local = commit(&dir, &[("other.py", "o\n")], "other");
    let err = files_for_push(&dir, &line(&local, ZERO)).unwrap_err();
    assert!(err.what.contains("merge-base"), "{err:?}");
}

#[test]
fn several_refs_give_one_sorted_list_without_repeats() {
    let dir = repo("push-several");
    let one = commit(&dir, &[("a.py", "a\n")], "one");
    let two = commit(&dir, &[("b.py", "b\n"), ("a.py", "a2\n")], "two");
    let three = commit(&dir, &[("c.py", "c\n")], "three");
    // Both refs are at the checked-out HEAD, each since its own remote sha.
    let text = format!(
        "refs/heads/x {three} refs/heads/x {one}\nrefs/heads/y {three} refs/heads/y {two}\n"
    );
    assert_eq!(
        files_for_push(&dir, &text),
        Ok(vec!["a.py".into(), "b.py".into(), "c.py".into()])
    );
}

#[test]
fn links_are_dropped() {
    let dir = repo("push-links");
    let one = commit(&dir, &[("a.py", "a\n")], "one");
    std::os::unix::fs::symlink("a.py", dir.join("link.py")).unwrap();
    let two = commit(&dir, &[("other.py", "o\n")], "two");
    assert_eq!(
        files_for_push(&dir, &line(&two, &one)),
        Ok(vec!["other.py".into()])
    );
}

/// A pushed file is never dropped silently. One missing from the working
/// tree, or changed there since the commit, would be judged by content the push
/// does not send, so it is cannot-check, naming the file.
#[test]
fn a_pushed_file_the_working_tree_changed_or_removed_cannot_check() {
    let dir = repo("push-dirty");
    let one = commit(&dir, &[("a.py", "a\n")], "one");
    let two = commit(&dir, &[("other.py", "o\n"), ("later.py", "l\n")], "two");
    std::fs::remove_file(dir.join("later.py")).unwrap();
    let err = files_for_push(&dir, &line(&two, &one)).unwrap_err();
    assert!(err.what.contains("later.py"), "{err:?}");
    assert!(!err.what.contains("other.py"), "{err:?}");
    std::fs::write(dir.join("later.py"), "changed\n").unwrap();
    let err = files_for_push(&dir, &line(&two, &one)).unwrap_err();
    assert!(err.what.contains("later.py"), "{err:?}");
}

/// The tools read the checkout, so a ref whose tip is not HEAD is
/// cannot-check rather than judged by the checkout's unrelated files.
#[test]
fn a_pushed_ref_whose_tip_is_not_head_cannot_check() {
    let dir = repo("push-other-ref");
    let base = commit(&dir, &[("base.py", "b\n")], "base");
    git(&dir, &["checkout", "-q", "-b", "topic"]);
    let topic = commit(&dir, &[("bad.py", "x=1\n")], "topic");
    git(&dir, &["checkout", "-q", "-"]);
    let err = files_for_push(&dir, &line(&topic, &base)).unwrap_err();
    assert!(err.what.contains("refs/heads/feature"), "{err:?}");
    assert!(
        err.fix
            .contains("push from the branch you have checked out"),
        "{err:?}"
    );
}

#[test]
fn the_push_range_is_the_base_to_the_pushed_sha() {
    let dir = repo("push-range");
    let main = commit(&dir, &[("base.py", "b\n")], "main");
    git(&dir, &["update-ref", "refs/remotes/origin/main", &main]);
    let local = commit(&dir, &[("mine.py", "m\n")], "mine");
    assert_eq!(
        pushed(&dir, &line(&local, &main)).unwrap().range,
        Some(format!("{main}..{local}"))
    );
    assert_eq!(
        pushed(&dir, &line(&local, ZERO)).unwrap().range,
        Some(format!("{main}..{local}")),
        "a new branch: since its merge-base with origin/main"
    );
}

/// A first push without origin/main has no base: it sends history no range bounds,
/// so gitleaks scans all of it and the excuse list has no prior list to compare with.
#[test]
fn a_first_push_without_origin_main_is_unbounded_with_no_range() {
    let dir = repo("push-unbounded");
    let local = commit(&dir, &[("a.py", "a\n")], "first");
    let first = pushed(&dir, &line(&local, ZERO)).unwrap();
    assert!(first.unbounded);
    assert_eq!(first.range, None);
    let nothing = pushed(&dir, "").unwrap();
    assert!(!nothing.unbounded, "nothing pushed is not a first push");
    let again = pushed(&dir, &line(&local, &local)).unwrap();
    assert!(!again.unbounded);
}

/// One ref with a base and one without: the push is still unbounded, and the range
/// is dropped rather than narrowed to the ref that has a base.
#[test]
fn one_ref_without_a_base_makes_the_whole_push_unbounded() {
    let dir = repo("push-unbounded-mixed");
    let one = commit(&dir, &[("a.py", "a\n")], "one");
    let two = commit(&dir, &[("b.py", "b\n")], "two");
    let text = format!(
        "{}refs/heads/other {two} refs/heads/other {ZERO}\n",
        line(&two, &one)
    );
    let push = pushed(&dir, &text).unwrap();
    assert!(push.unbounded);
    assert_eq!(push.range, None);
}

// ---- the binary: `house check --stage push` reads the push lines on stdin ----

/// Run the house binary in `dir` with `stdin` as git's pre-push lines.
fn house_push(dir: &Path, stdin: &str) -> (i32, String) {
    use std::io::Write;
    use std::process::Stdio;
    let log = common::fresh("push-bin-log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "push"])
        .current_dir(dir)
        .env("HOUSE_LOG_DIR", &log)
        .env_remove("CLAUDECODE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn the_binary_checks_the_files_the_push_lines_on_stdin_send() {
    let dir = repo("push-bin");
    let config = "[tools]\npush = [\"ruff-format\"]\n";
    let base = commit(&dir, &[(".house.toml", config)], "base");
    git(&dir, &["update-ref", "refs/remotes/origin/main", &base]);
    let local = commit(&dir, &[("bad.py", "x=1\n")], "unformatted");
    // A new branch sends bad.py: ruff-format finds it.
    let (code, text) = house_push(&dir, &line(&local, ZERO));
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("bad.py"), "{text}");
    // The remote already has it: nothing sent, nothing to check.
    let (code, text) = house_push(&dir, &line(&local, &local));
    assert_eq!(code, 0, "{text}");
    // Not a push line: cannot check.
    let (code, text) = house_push(&dir, "garbage\n");
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("not a push line"), "{text}");
}

/// Owner, 2026-11-22: on a first push without origin/main the excuse base is an empty
/// list, so every entry is the list growing (HEAD, the pushed tip, would hide it).
#[test]
fn the_binary_on_a_first_push_counts_every_excuse_as_growth() {
    let dir = repo("push-bin-first-excuse");
    let list = "[[excuse]]\nrule = \"R3\"\nfile = \"src/old.py\"\ndeadline = \"2999-12-31\"\nreason = \"split in v2\"\n";
    let config = "[tools]\npush = []\n";
    let local = commit(
        &dir,
        &[(".house.toml", config), (".house-excuses.toml", list)],
        "list",
    );
    let (code, text) = house_push(&dir, &line(&local, ZERO));
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("excuse list grew: R3 src/old.py."), "{text}");
    // Nothing pushed: no growth to judge.
    let (code, text) = house_push(&dir, "");
    assert_eq!(code, 0, "{text}");
}
