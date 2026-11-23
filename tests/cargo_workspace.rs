//! The cargo adapters on a real Rust workspace: one command per workspace root,
//! each clippy finding once (a run per member crate repeated them: a run on a member
//! also lints the members it depends on), only for the files asked about, with
//! test targets linted too.

mod common;

use house::adapter::{Adapter, Context, Stage};
use house::adapters::clippy::Clippy;
use house::finding::Place;
use std::path::Path;
use std::process::Command;

/// A repo holding a three-crate workspace in `ws/`: crate b depends on crate a, and a
/// has one clippy finding (eight arguments, over clippy's default limit of seven);
/// crate c builds on its own and has one finding in its integration test.
fn workspace_repo(name: &str) -> std::path::PathBuf {
    let repo = common::repo(name);
    let files = [
        (
            "ws/Cargo.toml",
            "[workspace]\nmembers = [\"crates/a\", \"crates/b\", \"crates/c\"]\nresolver = \"3\"\n",
        ),
        (
            "ws/crates/a/Cargo.toml",
            "[package]\nname = \"a\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        ),
        (
            "ws/crates/a/src/lib.rs",
            "pub fn eight(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8, g: u8, h: u8) -> u8 {\n    \
             a ^ b ^ c ^ d ^ e ^ f ^ g ^ h\n}\n",
        ),
        (
            "ws/crates/b/Cargo.toml",
            "[package]\nname = \"b\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\na = { path = \"../a\" }\n",
        ),
        (
            "ws/crates/b/src/lib.rs",
            "pub fn one() -> u8 {\n    a::eight(1, 2, 3, 4, 5, 6, 7, 8)\n}\n",
        ),
        (
            "ws/crates/c/Cargo.toml",
            "[package]\nname = \"c\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        ),
        ("ws/crates/c/src/lib.rs", "pub fn c() {}\n"),
        (
            "ws/crates/c/tests/t.rs",
            "fn two() -> u8 {\n    return 2;\n}\n\n#[test]\nfn t() {\n    assert_eq!(two(), 2);\n}\n",
        ),
        (".gitignore", "target/\n"),
    ];
    for (path, text) in files {
        let path = repo.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let lock = Command::new("cargo")
        .args([
            "generate-lockfile",
            "--offline",
            "--manifest-path",
            "ws/Cargo.toml",
        ])
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(lock.status.success(), "{lock:?}");
    common::git(&repo, &["add", "-A"]);
    repo
}

/// (place, tool rule) of every clippy finding at commit.
fn clippy_findings(repo: &Path) -> Vec<(String, String)> {
    let report = house::cli::check(repo, Stage::Commit, vec![Box::new(Clippy)]);
    let tool = &report.tools[0];
    assert!(tool.cannot.is_none(), "{:?}", tool.cannot);
    let place = |p: &Place| match p {
        Place::File(f) => f.clone(),
        Place::Repo => "repo".into(),
    };
    tool.findings
        .iter()
        .map(|f| (place(&f.place), f.tool_rule.clone()))
        .collect()
}

fn staged(repo: &Path) -> Vec<String> {
    let out = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(repo)
        .output()
        .unwrap();
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn a_workspace_is_one_command_and_each_finding_once() {
    let repo = workspace_repo("cargo-ws");
    let ctx = Context {
        root: repo.clone(),
        stage: Stage::Commit,
        files: staged(&repo),
        push_range: None,
        languages: Default::default(),
    };
    let cmds = Clippy.commands(&ctx);
    assert_eq!(cmds.len(), 1, "{cmds:?}");
    assert_eq!(cmds[0].args[1..3], ["--manifest-path", "ws/Cargo.toml"]);

    let found = clippy_findings(&repo);
    let lib = (
        "ws/crates/a/src/lib.rs".into(),
        "clippy::too_many_arguments".into(),
    );
    assert!(found.contains(&lib), "{found:?}");
    assert_eq!(found.iter().filter(|f| **f == lib).count(), 1, "{found:?}");
}

#[test]
fn a_lint_in_a_test_target_is_found() {
    let repo = workspace_repo("cargo-ws-tests");
    let found = clippy_findings(&repo);
    let test = (
        "ws/crates/c/tests/t.rs".into(),
        "clippy::needless_return".into(),
    );
    assert!(found.contains(&test), "{found:?}");
}

#[test]
fn files_not_staged_report_nothing() {
    let repo = workspace_repo("cargo-ws-staged");
    common::git(&repo, &["commit", "-qm", "start"]);
    let b = repo.join("ws/crates/b/src/lib.rs");
    std::fs::write(
        &b,
        "pub fn one() -> u8 {\n    a::eight(1, 2, 3, 4, 5, 6, 7, 9)\n}\n",
    )
    .unwrap();
    common::git(&repo, &["add", "-A"]);
    assert_eq!(clippy_findings(&repo), vec![]);
}
