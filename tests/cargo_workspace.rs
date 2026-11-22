//! The cargo adapters on a real Rust workspace: one command per workspace root,
//! each clippy finding once (a run per member crate repeated them: a run on a member
//! also lints the members it depends on).

mod common;

use house::adapter::{Adapter, Context, Stage};
use house::adapters::clippy::Clippy;
use std::path::Path;
use std::process::Command;

/// A repo holding a two-crate workspace in `ws/`: crate b depends on crate a, and a
/// has one clippy finding (eight arguments, over clippy's default limit of seven).
fn workspace_repo(name: &str) -> std::path::PathBuf {
    let repo = common::repo(name);
    let files = [
        (
            "ws/Cargo.toml",
            "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\nresolver = \"3\"\n",
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
        (".gitignore", "target/\nCargo.lock\n"),
    ];
    for (path, text) in files {
        let path = repo.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    common::git(&repo, &["add", "-A"]);
    repo
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
fn a_two_crate_workspace_is_one_command_and_each_finding_once() {
    let repo = workspace_repo("cargo-ws");
    let ctx = Context {
        root: repo.clone(),
        stage: Stage::Commit,
        files: staged(&repo),
        push_range: None,
    };
    let cmds = Clippy.commands(&ctx);
    assert_eq!(cmds.len(), 1, "{cmds:?}");
    assert_eq!(cmds[0].args[1..3], ["--manifest-path", "ws/Cargo.toml"]);

    let report = house::cli::check(&repo, Stage::Commit, vec![Box::new(Clippy)]);
    let lines: Vec<_> = report.tools[0]
        .findings
        .iter()
        .map(|f| (f.place.clone(), f.line, f.tool_rule.clone()))
        .collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0].2, "clippy::too_many_arguments");
}
