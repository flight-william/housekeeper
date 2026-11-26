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

/// A stand-in tsc for `node_modules/.bin`: a project run that checked no file.
const FAKE_TSC: &str = "#!/bin/sh\nexit 0\n";

#[test]
fn a_staged_typescript_file_no_project_lists_is_reported() {
    let root = staged(
        "node-no-project",
        &["tsc"],
        &[
            ("node_modules/.bin/tsc", FAKE_TSC),
            ("tsconfig.json", "{ \"include\": [\"src\"] }\n"),
            ("scripts/seed.ts", "export const a = 1;\n"),
        ],
    );
    let (code, text) = check(&root, Some("/usr/bin:/bin"));
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("R2 scripts/seed.ts: tsc: no project includes this file."),
        "{text}"
    );
}

/// Vite's react-ts layout: a solution-style root that only references its projects.
const VITE_ROOT: &str = "{\n  \"files\": [],\n  \"references\": [\n    { \"path\": \"./tsconfig.app.json\" },\n    { \"path\": \"./tsconfig.node.json\" }\n  ]\n}\n";
const VITE_APP: &str = "{\n  \"compilerOptions\": {\n    \"strict\": true,\n    /* Bundler mode */\n    \"moduleResolution\": \"bundler\",\n    \"module\": \"ESNext\",\n    \"noEmit\": true,\n  },\n  \"include\": [\"src\"]\n}\n";
const VITE_NODE: &str = "{ \"compilerOptions\": { \"strict\": true, \"noEmit\": true }, \"include\": [\"vite.config.ts\"] }\n";

#[test]
fn real_tsc_follows_references_and_reports_files_no_project_holds() {
    if !installed("tsc") {
        return;
    }
    let root = staged(
        "node-vite",
        &["tsc"],
        &[
            ("tsconfig.json", VITE_ROOT),
            ("tsconfig.app.json", VITE_APP),
            ("tsconfig.node.json", VITE_NODE),
            ("src/main.ts", "export const n: number = \"one\";\n"),
            ("vite.config.ts", "export default {};\n"),
            ("scripts/seed.ts", "export const s = 1;\n"),
        ],
    );
    let (code, text) = check(&root, None);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("R2 src/main.ts:1: "), "{text}");
    assert!(text.contains("[tsc TS2322]"), "{text}");
    assert!(
        text.contains("R2 scripts/seed.ts: tsc: no project includes this file."),
        "{text}"
    );
    assert!(!text.contains("vite.config.ts"), "{text}");
    let tracked = std::process::Command::new("git")
        .args(["status", "--porcelain", "--ignored"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        !String::from_utf8_lossy(&tracked.stdout).contains("tsbuildinfo"),
        "tsc wrote into the working tree"
    );

    let bare = staged(
        "node-orphan",
        &["tsc"],
        &[("a.ts", "export const a = 1;\n")],
    );
    let (code, text) = check(&bare, None);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("R2 a.ts: tsc: no project includes this file."),
        "{text}"
    );
}

#[test]
fn real_prettier_reads_node_scripts_whatever_their_first_line() {
    if !installed("prettier") {
        return;
    }
    let root = staged(
        "node-shebang",
        &["prettier"],
        &[
            (
                "bin/flags",
                "#!/usr/bin/env -S node --no-warnings\nconsole.log( 1 )\n",
            ),
            ("bin/plain", "#!/usr/bin/env node\nconsole.log(1);\n"),
        ],
    );
    let (code, text) = check(&root, None);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("R1 bin/flags: not formatted"), "{text}");
    assert!(!text.contains("bin/plain"), "{text}");
}

#[test]
fn with_a_package_json_a_missing_local_tool_is_npm_installed_not_taken_from_path() {
    let root = staged(
        "node-pkg-missing",
        &["eslint", "prettier", "tsc"],
        &[
            ("package.json", "{ \"private\": true }\n"),
            ("tsconfig.json", "{}\n"),
            ("a.ts", "export const a = 1;\n"),
        ],
    );
    // An eslint on PATH that would report a finding: it must not run.
    let elsewhere = common::fresh("node-path-tools");
    write(&elsewhere, "eslint", "#!/bin/sh\necho '[]'\nexit 1\n");
    let path = format!("{}:/usr/bin:/bin", elsewhere.display());
    let (code, text) = check(&root, Some(&path));
    assert_eq!(code, 2, "{text}");
    for (tool, package) in [
        ("eslint", "eslint"),
        ("prettier", "prettier"),
        ("tsc", "typescript"),
    ] {
        let line = format!(
            "node_modules/.bin/{tool} is not installed. fix: npm install (add {package} to devDependencies) [{tool}]"
        );
        assert!(text.contains(&line), "{line}: {text}");
    }
    assert!(!text.contains("brew install"), "{text}");
}

#[test]
fn the_build_info_a_tsc_run_writes_is_gone_after_the_check() {
    // A stand-in tsc that writes its --tsBuildInfoFile, as an incremental project
    // does, and logs where (outside the repo).
    let log = common::fresh("node-buildinfo-log").join("where");
    let fake = format!(
        "#!/bin/sh\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = --tsBuildInfoFile ]; then\n    mkdir -p \"$(dirname \"$2\")\" && echo x > \"$2\" && echo \"$2\" >> {}\n  fi\n  shift\ndone\nexit 0\n",
        log.display()
    );
    let root = staged(
        "node-buildinfo",
        &["tsc"],
        &[
            ("node_modules/.bin/tsc", &fake),
            ("tsconfig.json", "{}\n"),
            ("a.ts", "export const a = 1;\n"),
        ],
    );
    let (code, text) = check(&root, Some("/usr/bin:/bin"));
    assert_eq!(code, 1, "{text}");
    let written = std::fs::read_to_string(&log).unwrap();
    let info = std::path::Path::new(written.trim());
    assert!(!info.starts_with(&root), "{info:?}");
    assert!(!info.exists(), "{info:?} was left behind");
}

#[test]
fn real_tsc_leaves_the_working_tree_clean_for_incremental_and_composite_projects() {
    if !installed("tsc") {
        return;
    }
    let root = staged(
        "node-composite",
        &["tsc"],
        &[
            (
                "lib/tsconfig.json",
                "{ \"compilerOptions\": { \"composite\": true, \"strict\": true }, \"include\": [\"*.ts\"] }\n",
            ),
            ("lib/a.ts", "export const a: number = \"one\";\n"),
            (
                "app/tsconfig.json",
                "{ \"compilerOptions\": { \"incremental\": true, \"strict\": true }, \"include\": [\"*.ts\"] }\n",
            ),
            ("app/b.ts", "export const b = 1;\n"),
        ],
    );
    let (code, text) = check(&root, None);
    assert_eq!(code, 1, "{text}");
    assert!(text.contains("R2 lib/a.ts:1: "), "{text}");
    let status = Command::new("git")
        .args([
            "status",
            "--porcelain",
            "--ignored",
            "--untracked-files=all",
        ])
        .current_dir(&root)
        .output()
        .unwrap();
    let status = String::from_utf8_lossy(&status.stdout);
    assert!(
        status.lines().all(|l| l.starts_with("A ")),
        "the check wrote into the working tree:\n{status}"
    );
}
