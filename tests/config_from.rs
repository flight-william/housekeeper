//! `house check --stage push --config-from <rev>`: `.house.toml` and the excuse list
//! come from that revision's objects, never the working tree, and judge the pushed
//! content. A file absent at the revision is absent; a revision with no `.house.toml`
//! at all is the first adoption, judged by the pushed rules with a note.

mod common;

use common::{git, repo};
use std::path::Path;
use std::process::{Command, Stdio};

const ZERO: &str = "0000000000000000000000000000000000000000";
const R3_ONLY: &str = "[tools]\npush = [\"r3\"]\n";
const APP_EXCUSE: &str = "[[excuse]]\nrule = \"R3\"\nfile = \"app.py\"\ndeadline = \"2999-12-31\"\nreason = \"split later\"\n";

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
    git(dir, &["commit", "-q", "--no-verify", "-m", message]);
    sha(dir, "HEAD")
}

/// A Python file of 401 lines: one R3 finding (file over 400 lines).
fn long_py() -> String {
    "x = 1\n".repeat(401)
}

/// The binary at push with `args` after `check --stage push`, the push lines on
/// stdin: (exit, stdout, stderr).
fn push(dir: &Path, args: &[&str], stdin: &str) -> (i32, String, String) {
    use std::io::Write;
    let log = common::fresh("config-from-log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "push"])
        .args(args)
        .current_dir(dir)
        .env("HOUSE_LOG_DIR", &log)
        .env("GIT_DIR", "/nonexistent")
        .env_remove("CLAUDECODE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut pipe = child.stdin.take().unwrap();
    pipe.write_all(stdin.as_bytes()).unwrap();
    drop(pipe);
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn line(local: &str, remote: &str) -> String {
    format!("refs/heads/feature {local} refs/heads/feature {remote}\n")
}

/// A repo whose base commit holds `base_files`; `rev` names that commit.
fn based(name: &str, base_files: &[(&str, &str)]) -> (std::path::PathBuf, String) {
    let dir = repo(name);
    let mut files = vec![("README.md", "base\n")];
    files.extend_from_slice(base_files);
    let base = commit(&dir, &files, "base");
    (dir, base)
}

// The base has rules but no excuse list; the change adds a
// 401-line file and its own excuse. The pushed list must not excuse it.
#[test]
fn an_excuse_list_absent_at_the_revision_is_absent() {
    let (dir, base) = based("cf-no-base-list", &[(".house.toml", R3_ONLY)]);
    let local = commit(
        &dir,
        &[("app.py", &long_py()), (".house-excuses.toml", APP_EXCUSE)],
        "long file, excused by itself",
    );
    let (code, out, _) = push(&dir, &["--config-from", &base], &line(&local, &base));
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("R3 app.py: file has"), "{out}");
    // Without --config-from the working tree's list excuses it, as before (its new
    // entry's far deadline is a finding of its own).
    let (_, out, _) = push(&dir, &[], &line(&local, &base));
    assert!(
        out.contains("1 excused") && !out.contains("R3 app.py: file has"),
        "{out}"
    );
}

// The change switches R3 off in its own .house.toml: the base's tools still run, and
// the changed rules file is not refused as a path changed in the working tree.
#[test]
fn the_tools_come_from_the_revision_not_the_pushed_house_toml() {
    let (dir, base) = based("cf-tools", &[(".house.toml", R3_ONLY)]);
    let local = commit(
        &dir,
        &[
            ("app.py", &long_py()),
            (".house.toml", "[tools]\npush = []\n"),
        ],
        "long file, R3 switched off",
    );
    let (code, out, _) = push(&dir, &["--config-from", &base], &line(&local, &base));
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("app.py"), "{out}");
    assert!(!out.contains("changed in the working tree"), "{out}");
}

// The base's excuse list applies even when the change rewrites it.
#[test]
fn the_excuse_list_comes_from_the_revision() {
    let (dir, base) = based(
        "cf-list",
        &[
            (".house.toml", R3_ONLY),
            (".house-excuses.toml", APP_EXCUSE),
            ("app.py", &long_py()),
        ],
    );
    let local = commit(
        &dir,
        &[("app.py", &format!("{}y = 2\n", long_py()))],
        "longer still",
    );
    let (code, out, _) = push(&dir, &["--config-from", &base], &line(&local, &base));
    assert_eq!(code, 0, "the base's entry excuses it: {out}");
    // A change that deletes the list does not escape the base's entry, nor lose it.
    std::fs::remove_file(dir.join(".house-excuses.toml")).unwrap();
    let local = commit(&dir, &[], "list deleted");
    let (code, out, _) = push(&dir, &["--config-from", &base], &line(&local, &base));
    assert_eq!(code, 0, "{out}");
}

// A change that fixes an excused finding and deletes its expired entry is done: the
// base's expired entry is not reported against it.
#[test]
fn an_expired_entry_the_change_deletes_is_not_reported() {
    let expired = APP_EXCUSE.replace("2999-12-31", "2000-01-01");
    let (dir, base) = based(
        "cf-expired",
        &[(".house.toml", R3_ONLY), (".house-excuses.toml", &expired)],
    );
    std::fs::remove_file(dir.join(".house-excuses.toml")).unwrap();
    let local = commit(&dir, &[("app.py", "x = 1\n")], "fixed, entry deleted");
    let (code, out, _) = push(&dir, &["--config-from", &base], &line(&local, &base));
    assert_eq!(code, 0, "{out}");
    // Still listed in the change: reported as expired.
    let local = commit(&dir, &[(".house-excuses.toml", &expired)], "kept");
    let (code, out, _) = push(&dir, &["--config-from", &base], &line(&local, &base));
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("excuse expired"), "{out}");
}

// The first adoption: the revision has no .house.toml at all. The pushed rules judge
// the change, and one note line says the owner must approve it.
#[test]
fn a_revision_without_rules_uses_the_pushed_rules_with_a_note() {
    // The base holds a list but no .house.toml: the pushed .house.toml runs R3, and
    // the list (the working tree's) excuses its finding.
    let (dir, base) = based("cf-adopt", &[(".house-excuses.toml", APP_EXCUSE)]);
    let local = commit(
        &dir,
        &[(".house.toml", R3_ONLY), ("app.py", &long_py())],
        "adopt house",
    );
    let (code, out, err) = push(&dir, &["--config-from", &base], &line(&local, &base));
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("1 excused"), "{out}");
    let note = "no base rules: the owner must approve this change";
    assert_eq!(err.matches(note).count(), 1, "{err}");
    // With base rules there is no note.
    let (dir, base) = based("cf-adopt-not", &[(".house.toml", R3_ONLY)]);
    let local = commit(&dir, &[("a.py", "x = 1\n")], "small");
    let (code, out, err) = push(&dir, &["--config-from", &base], &line(&local, &base));
    assert_eq!(code, 0, "{out}");
    assert!(!err.contains("no base rules"), "{err}");
}

#[test]
fn a_revision_that_cannot_be_read_cannot_check() {
    let (dir, base) = based("cf-unread", &[(".house.toml", R3_ONLY)]);
    let local = commit(&dir, &[("a.py", "x = 1\n")], "small");
    let (code, out, _) = push(
        &dir,
        &["--config-from", "no-such-rev"],
        &line(&local, &base),
    );
    assert_eq!(code, 2, "{out}");
    assert_eq!(
        out.matches("no-such-rev is not a commit").count(),
        1,
        "{out}"
    );
    // A .house.toml at the revision that is not a file is cannot-check, not absence.
    let (dir, base) = based("cf-dir", &[(".house.toml/x", "y\n")]);
    let local = commit(&dir, &[("a.py", "x = 1\n")], "small");
    let (code, out, _) = push(&dir, &["--config-from", &base], &line(&local, &base));
    assert_eq!(code, 2, "{out}");
    assert!(out.contains(".house.toml"), "{out}");
    // A list at the revision that does not parse is cannot-check.
    let (dir, base) = based(
        "cf-bad-list",
        &[
            (".house.toml", R3_ONLY),
            (".house-excuses.toml", "nonsense"),
        ],
    );
    let local = commit(&dir, &[("a.py", "x = 1\n")], "small");
    let (code, out, _) = push(&dir, &["--config-from", &base], &line(&local, &base));
    assert_eq!(code, 2, "{out}");
    assert!(out.contains(".house-excuses.toml"), "{out}");
}

#[test]
fn config_from_is_for_the_push_stage_only() {
    let dir = repo("cf-usage");
    for args in [
        &["check", "--stage", "commit", "--config-from", "HEAD"][..],
        &["check", "--all", "--config-from", "HEAD"],
        &["check", "--stage", "push", "--config-from"],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_house"))
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("usage"));
    }
    assert_eq!(
        house::cli::parse(&["check", "--stage", "push", "--config-from", "main"].map(String::from)),
        Ok(house::cli::Cmd::Check {
            stage: house::adapter::Stage::Push,
            json: false,
            config_from: Some("main".into()),
        })
    );
}

#[test]
fn a_new_branch_is_judged_by_the_revision_too() {
    let (dir, base) = based("cf-new-branch", &[(".house.toml", R3_ONLY)]);
    git(&dir, &["update-ref", "refs/remotes/origin/main", &base]);
    let local = commit(
        &dir,
        &[("app.py", &long_py()), (".house-excuses.toml", APP_EXCUSE)],
        "long",
    );
    let (code, out, _) = push(&dir, &["--config-from", &base], &line(&local, ZERO));
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("R3 app.py: file has"), "{out}");
    let (_, out, _) = push(&dir, &[], &line(&local, ZERO));
    assert!(
        out.contains("1 excused") && !out.contains("R3 app.py: file has"),
        "without --config-from the pushed list excuses it: {out}"
    );
}
