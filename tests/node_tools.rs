//! The JavaScript and TypeScript lane through `house check --stage commit`: the
//! repo's own `node_modules/.bin` copy of a tool runs before one on PATH, a `node`
//! script with no extension goes to the JS tools, a missing tool is a cannot-check
//! with its install line, and the real Prettier, ESLint and tsc give their findings.
//! A test that runs a real tool passes with a note when that tool is not installed.

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A repo with `.house.toml` naming `tools` committed, then `files` staged.
fn staged(name: &str, tools: &[&str], files: &[(&str, &str)]) -> PathBuf {
    let root = common::repo(name);
    let list: Vec<String> = tools.iter().map(|t| format!("\"{t}\"")).collect();
    let config = format!("[tools]\ncommit = [{}]\npush = []\n", list.join(", "));
    std::fs::write(root.join(".house.toml"), config).unwrap();
    common::git(&root, &["add", "-A"]);
    common::git(&root, &["commit", "-q", "--no-verify", "-m", "config"]);
    for (path, text) in files {
        write(&root, path, text);
    }
    common::git(&root, &["add", "-A"]);
    root
}

fn write(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, text).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// `house check --stage commit` in `root`, with `path` as PATH when given: (exit, stdout).
fn check(root: &Path, path: Option<&str>) -> (i32, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_house"));
    cmd.args(["check", "--stage", "commit"])
        .current_dir(root)
        .env("HOUSE_LOG_DIR", common::fresh("node-tools-log"))
        .env_remove("CLAUDECODE");
    if let Some(path) = path {
        cmd.env("PATH", path);
    }
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

fn installed(program: &str) -> bool {
    let found = Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !found {
        eprintln!("note: {program} is not installed; skipping the run on a repo");
    }
    found
}

/// A stand-in Prettier for `node_modules/.bin`: it names every file it is given as
/// unformatted, as `prettier --check` does, and exits 1.
const FAKE_PRETTIER: &str = "#!/bin/sh\n\
    for f in \"$@\"; do [ \"$f\" = --check ] || [ \"$f\" = -- ] || echo \"[warn] $f\" >&2; done\n\
    exit 1\n";

#[test]
fn the_repos_own_prettier_runs_and_a_node_script_goes_to_it() {
    let root = staged(
        "node-local",
        &["prettier"],
        &[
            ("node_modules/.bin/prettier", FAKE_PRETTIER),
            ("bin/tool", "#!/usr/bin/env node\nconsole.log(1);\n"),
            ("bin/sh-tool", "#!/bin/sh\necho 1\n"),
            ("src/a.ts", "export const a = 1;\n"),
        ],
    );
    // A PATH with no Prettier on it: only the repo's own copy can have run.
    let (code, text) = check(&root, Some("/usr/bin:/bin"));
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("R1 bin/tool: not formatted as Prettier formats it"),
        "{text}"
    );
    assert!(text.contains("R1 src/a.ts: "), "{text}");
    assert!(!text.contains("sh-tool"), "{text}");
}

#[test]
fn a_missing_tool_cannot_check_with_its_install_line() {
    let root = staged("node-missing", &["eslint"], &[("a.js", "let a = 1;\n")]);
    let (code, text) = check(&root, Some("/usr/bin:/bin"));
    assert_eq!(code, 2, "{text}");
    assert!(
        text.contains("cannot check: eslint is not installed. fix: brew install eslint [eslint]"),
        "{text}"
    );
}

#[test]
fn real_prettier_reports_an_unformatted_file() {
    if !installed("prettier") {
        return;
    }
    let root = staged(
        "node-prettier",
        &["prettier"],
        &[("a.ts", "const a = 1\n"), ("b.ts", "export const b = 1;\n")],
    );
    let (code, text) = check(&root, None);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("R1 a.ts: not formatted as Prettier formats it"),
        "{text}"
    );
    assert!(!text.contains("b.ts"), "{text}");
}

#[test]
fn real_eslint_reports_a_lint_error_and_a_missing_config() {
    if !installed("eslint") {
        return;
    }
    let config = "export default [{ rules: { \"no-unused-vars\": \"error\" } }];\n";
    let root = staged(
        "node-eslint",
        &["eslint"],
        &[
            ("eslint.config.mjs", config),
            ("a.js", "const unused = 1;\n"),
        ],
    );
    let (code, text) = check(&root, None);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("R2 a.js:1: "), "{text}");
    assert!(text.contains("[eslint no-unused-vars]"), "{text}");

    let bare = staged("node-eslint-bare", &["eslint"], &[("a.js", "let a = 1;\n")]);
    let (code, text) = check(&bare, None);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("ESLint found no eslint.config.* file"),
        "{text}"
    );
}

#[test]
fn real_tsc_reports_type_errors_in_each_project() {
    if !installed("tsc") {
        return;
    }
    let tsconfig = "{ \"compilerOptions\": { \"strict\": true, \"noEmit\": true } }\n";
    let wrong = "export const n: number = \"one\";\n";
    let root = staged(
        "node-tsc",
        &["tsc"],
        &[
            ("tsconfig.json", tsconfig),
            ("src/a.ts", wrong),
            ("packages/b/tsconfig.json", tsconfig),
            ("packages/b/b.ts", wrong),
        ],
    );
    let (code, text) = check(&root, None);
    assert_eq!(code, 1, "{text}");
    for place in ["R2 src/a.ts:1: ", "R2 packages/b/b.ts:1: "] {
        assert!(text.contains(place), "{place}: {text}");
    }
    assert!(text.contains("[tsc TS2322]"), "{text}");
}
