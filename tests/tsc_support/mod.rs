//! Helpers for the tsc adapter's tests (tests/tsc.rs, tests/tsc_projects.rs): a
//! context at a root, a folder of planted files, and the project each command checks.

#[path = "../common/temp.rs"]
mod common;

use house::adapter::{Adapter, Context, Stage};
use house::adapters::tsc::Tsc;
use std::path::{Path, PathBuf};

pub fn ctx_at(root: &Path, files: &[&str]) -> Context {
    Context {
        root: root.to_path_buf(),
        stage: Stage::Commit,
        files: files.iter().map(|f| f.to_string()).collect(),
        push_range: None,
        languages: Default::default(),
    }
}

/// A folder with each of `files` (path, text) written.
pub fn tree_with(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = common::fresh(name);
    for (p, text) in files {
        let file = root.join(p);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }
    root
}

/// The project each command checks: the argument after `-p` (or the whole command
/// line when it has none, as `--version`).
pub fn projects(c: &Context) -> Vec<String> {
    Tsc.commands(c)
        .iter()
        .map(|cmd| match cmd.args.iter().position(|a| a == "-p") {
            Some(i) => cmd.args[i + 1].clone(),
            None => cmd.args.join(" "),
        })
        .collect()
}
