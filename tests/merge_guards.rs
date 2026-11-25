//! What `house check --stage merge` refuses rather than passing with nothing judged:
//! no ref line, a delete, an empty range, a shallow clone, grafts; and what the pull
//! request's own code cannot change: the commits are read and judged before any tool
//! runs, with replace refs ignored.

mod merge_support;

use merge_support::{GOOD, R3_ONLY, ZERO, based, commit, findings, git, line, merge, run, sha};
use std::process::Command;

fn hosted() -> [(&'static str, &'static str); 2] {
    [
        ("GITHUB_ACTIONS", "true"),
        ("RUNNER_ENVIRONMENT", "github-hosted"),
    ]
}

#[test]
fn no_ref_line_a_delete_or_an_empty_range_cannot_check() {
    let (dir, base) = based("mg-empty", &[(".house.toml", R3_ONLY)]);
    let local = commit(&dir, &[("a.py", "x = 1\n")], "wip");
    let args = ["--config-from", base.as_str()];
    let delete = format!("refs/heads/feature {ZERO} refs/heads/feature {base}\n");
    for stdin in ["", "\n", delete.as_str()] {
        let out = run(&dir, "merge", &args, stdin, &hosted());
        assert_eq!(out.code, 2, "{stdin:?}: {}", out.out);
        assert!(out.out.contains("cannot check"), "{}", out.out);
    }
    // A delete beside a real line is refused too.
    let both = format!("{delete}{}", line(&local, &base));
    assert_eq!(run(&dir, "merge", &args, &both, &hosted()).code, 2);
    // Base equal to head: nothing sent.
    let out = run(
        &dir,
        "merge",
        &["--config-from", &local],
        &line(&local, &local),
        &hosted(),
    );
    assert_eq!(out.code, 2, "{}", out.out);
    assert!(out.out.contains("no commit to judge"), "{}", out.out);
    // Push keeps an empty push as nothing to fix.
    assert_eq!(run(&dir, "push", &args, "", &[]).code, 0);
}

#[test]
fn a_shallow_clone_cannot_check() {
    let (dir, base) = based("mg-shallow-src", &[(".house.toml", R3_ONLY)]);
    commit(&dir, &[("a.py", "x = 1\n")], "wip");
    let local = commit(&dir, &[("b.py", "y = 2\n")], GOOD);
    let shallow = merge_support::fresh("mg-shallow");
    let url = format!("file://{}", dir.display());
    git(&shallow, &["clone", "-q", "--depth=1", &url, "."]);
    git(&shallow, &["fetch", "-q", "--depth=1", "origin", &base]);
    let out = merge(&shallow, &base, &local);
    assert_eq!(out.code, 2, "{}", out.out);
    assert!(out.out.contains("shallow"), "{}", out.out);
    assert!(out.out.contains("fetch-depth: 0"), "{}", out.out);
}

#[test]
fn grafts_cannot_check() {
    let (dir, base) = based("mg-grafts", &[(".house.toml", R3_ONLY)]);
    let local = commit(&dir, &[("a.py", "x = 1\n")], GOOD);
    std::fs::create_dir_all(dir.join(".git/info")).unwrap();
    std::fs::write(dir.join(".git/info/grafts"), format!("{local}\n")).unwrap();
    let out = merge(&dir, &base, &local);
    assert_eq!(out.code, 2, "{}", out.out);
    assert!(out.out.contains("grafts"), "{}", out.out);
}

/// Make a twin of `sha` with a passing message and the same tree and parents, and
/// replace `sha` with it.
fn replace_with_twin(dir: &std::path::Path, sha: &str) {
    let raw = Command::new("git")
        .args(["cat-file", "commit", sha])
        .current_dir(dir)
        .output()
        .unwrap()
        .stdout;
    let raw = String::from_utf8(raw).unwrap();
    let (head, _) = raw.split_once("\n\n").unwrap();
    let twin_text = format!("{head}\n\n{GOOD}");
    let path = dir.join("twin.txt");
    std::fs::write(&path, twin_text).unwrap();
    let out = Command::new("git")
        .args(["hash-object", "-t", "commit", "-w"])
        .arg(&path)
        .current_dir(dir)
        .output()
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    let twin = String::from_utf8(out.stdout).unwrap().trim().to_string();
    git(dir, &["replace", "-f", sha, &twin]);
}

#[test]
fn a_replace_ref_does_not_change_what_is_judged() {
    let (dir, base) = based("mg-replace", &[(".house.toml", R3_ONLY)]);
    let local = commit(&dir, &[("a.py", "x = 1\n")], "wip");
    replace_with_twin(&dir, &local);
    let out = merge(&dir, &base, &local);
    assert_eq!(out.code, 1, "{}{}", out.out, out.err);
    assert!(out.out.contains(&local[..12]), "{}", out.record);
    assert_eq!(
        findings(&out.out).len(),
        4,
        "wip: subject and three body lines"
    );
}

const LOCK: &str = "version = 4\n\n[[package]]\nname = \"p\"\nversion = \"0.1.0\"\n";

const BUILD_RS: &str = r#"use std::process::Command;
fn git(a: &[&str]) -> String {
    let out = Command::new("git").args(a).output().unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}
fn main() {
    let root = git(&["rev-list", "--max-parents=0", "HEAD"]);
    for sha in git(&["rev-list", "HEAD", "--not", &root]).lines() {
        let tree = git(&["rev-parse", &format!("{sha}^{{tree}}")]);
        let parent = git(&["rev-parse", &format!("{sha}^")]);
        let twin = git(&["commit-tree", &tree, "-p", &parent, "-m", "app: add the file\n\nWhy: w.\nProof: p.\nUndo: u.\n"]);
        git(&["replace", "-f", sha, &twin]);
    }
}
"#;

// The pull request's build script runs under clippy and plants replace refs that give
// its commits passing messages: the commits were judged before clippy ran.
#[test]
fn a_tool_that_plants_replace_refs_cannot_rewrite_the_messages() {
    let cargo = "[package]\nname = \"p\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
    let rules = "[tools]\npush = [\"clippy\"]\n";
    let (dir, base) = based(
        "mg-build-rs",
        &[
            (".house.toml", rules),
            ("Cargo.toml", cargo),
            ("src/lib.rs", "pub fn f() {}\n"),
            (".gitignore", "target/\n"),
            ("Cargo.lock", LOCK),
        ],
    );
    let local = commit(&dir, &[("build.rs", BUILD_RS)], "wip");
    let out = merge(&dir, &base, &local);
    let planted = Command::new("git")
        .args(["for-each-ref", "refs/replace/"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        !planted.stdout.is_empty(),
        "the build script planted its replace ref: {}",
        out.out
    );
    assert_eq!(out.code, 1, "{}", out.out);
    assert!(
        out.out.contains(&format!("commit {}", &local[..12])),
        "{}",
        out.out
    );
    assert_eq!(sha(&dir, "HEAD"), local);
}

// An excuse in the base for a protected file is not a standing approval.
#[test]
fn an_approval_finding_cannot_be_excused() {
    let excuse = "[[excuse]]\nrule = \"R9\"\nfile = \".semgrepignore\"\ndeadline = \"2999-12-31\"\nreason = \"old\"\n";
    let (dir, base) = based(
        "mg-excuse",
        &[
            (".house.toml", R3_ONLY),
            (".house-excuses.toml", excuse),
            (".semgrepignore", "a/\n"),
        ],
    );
    let local = commit(&dir, &[(".semgrepignore", "b/\n")], GOOD);
    let out = merge(&dir, &base, &local);
    assert_eq!(out.code, 1, "{}", out.out);
    assert!(out.out.contains("protected file changed"), "{}", out.out);
}

// GITHUB_ACTIONS set to any value is Actions (design §8 says "set").
#[test]
fn any_github_actions_value_needs_config_from() {
    let (dir, base) = based("mg-actions-any", &[(".house.toml", R3_ONLY)]);
    let local = commit(&dir, &[("a.py", "x = 1\n")], GOOD);
    let env = [("GITHUB_ACTIONS", "1")];
    let out = run(&dir, "merge", &[], &line(&local, &base), &env);
    assert_eq!(out.code, 2, "{}", out.out);
}
