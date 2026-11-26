//! The tsc adapter's projects: which tsconfig each run checks (the nearest, the
//! configs it references beside it, and deeper only through solution-style ones),
//! and where a run's build info goes.

mod tsc_support;

use house::adapter::Adapter;
use house::adapters::tsc::Tsc;
use std::path::PathBuf;
use tsc_support::{ctx_at, projects, tree_with};

/// A folder holding an empty `{}` file at each of `paths`.
fn tree(name: &str, paths: &[&str]) -> PathBuf {
    let files: Vec<(&str, &str)> = paths.iter().map(|p| (*p, "{}\n")).collect();
    tree_with(name, &files)
}

/// Vite's react-ts layout: a solution-style root (`"files": []` and `references`), in
/// JSON with comments and trailing commas, as Vite writes it.
const SOLUTION: &str = r#"{
  // the projects
  "files": [],
  "references": [
    { "path": "./tsconfig.app.json" },
    { "path": "./tsconfig.node.json" }, /* the build script */
  ],
}
"#;

#[test]
fn runs_once_per_nearest_tsconfig_of_the_staged_typescript_files() {
    let root = tree(
        "tsc-projects",
        &["tsconfig.json", "packages/a/tsconfig.json"],
    );
    let c = ctx_at(
        &root,
        &[
            "src/x.ts",
            "packages/a/src/y.tsx",
            "packages/a/z.mts",
            "src/w.cts",
            "README.md",
        ],
    );
    assert_eq!(projects(&c), ["tsconfig.json", "packages/a/tsconfig.json"]);
    assert_eq!(Tsc.commands(&c)[0].program, "tsc");
}

#[test]
fn skips_without_typescript_files() {
    let root = tree("tsc-none", &["tsconfig.json"]);
    assert!(
        Tsc.commands(&ctx_at(&root, &["a.js", "b.jsx", "c.md"]))
            .is_empty()
    );
    assert!(Tsc.command(&ctx_at(&root, &["a.js"])).is_none());
}

#[test]
fn a_staged_tsconfig_runs_its_project() {
    let root = tree("tsc-config", &["web/tsconfig.json"]);
    assert_eq!(
        projects(&ctx_at(&root, &["web/tsconfig.json"])),
        ["web/tsconfig.json"]
    );
}

#[test]
fn a_tsconfig_path_like_an_option_gets_dot_slash() {
    let root = tree("tsc-dash", &["-w/tsconfig.json"]);
    assert_eq!(
        projects(&ctx_at(&root, &["-w/a.ts"])),
        ["./-w/tsconfig.json"]
    );
}

#[test]
fn prefers_the_repos_own_node_modules_copy() {
    let root = tree("tsc-local", &["tsconfig.json", "node_modules/.bin/tsc"]);
    let cmd = Tsc.command(&ctx_at(&root, &["a.ts"])).unwrap();
    assert_eq!(
        PathBuf::from(cmd.program),
        root.join("node_modules/.bin/tsc")
    );
}

#[test]
fn a_solution_style_root_runs_each_referenced_project_not_itself() {
    let root = tree_with(
        "tsc-vite",
        &[
            ("tsconfig.json", SOLUTION),
            ("tsconfig.app.json", "{ \"include\": [\"src\"] }\n"),
            (
                "tsconfig.node.json",
                "{ \"include\": [\"vite.config.ts\"] }\n",
            ),
        ],
    );
    assert_eq!(
        projects(&ctx_at(&root, &["src/main.tsx"])),
        ["tsconfig.app.json", "tsconfig.node.json"]
    );
}

#[test]
fn a_reference_to_a_folder_means_its_tsconfig_and_solution_configs_are_followed_deep() {
    let root = tree_with(
        "tsc-refs",
        &[
            (
                "tsconfig.json",
                "{ \"files\": [], \"references\": [{ \"path\": \"packages\" }] }",
            ),
            (
                "packages/tsconfig.json",
                "{ \"files\": [], \"references\": [{ \"path\": \"a\" }, { \"path\": \"b/tsconfig.json\" }] }",
            ),
            (
                "packages/a/tsconfig.json",
                "{ \"include\": [\"src\"], \"references\": [{ \"path\": \"../c\" }] }",
            ),
            ("packages/b/tsconfig.json", "{}"),
            ("packages/c/tsconfig.json", "{}"),
        ],
    );
    // Through two solution-style configs to a and b; a's own reference to c is not
    // followed: c's files have c as their nearest config.
    assert_eq!(
        projects(&ctx_at(&root, &["main.ts"])),
        ["packages/a/tsconfig.json", "packages/b/tsconfig.json"]
    );
}

#[test]
fn staging_one_app_file_runs_the_app_not_the_libraries_it_references() {
    let app = "{ \"include\": [\"src\"], \"references\": [{ \"path\": \"../libs/a\" }, { \"path\": \"../libs/b\" }, { \"path\": \"../libs/c\" }] }";
    let root = tree_with(
        "tsc-app-libs",
        &[
            ("app/tsconfig.json", app),
            ("libs/a/tsconfig.json", "{}"),
            ("libs/b/tsconfig.json", "{}"),
            ("libs/c/tsconfig.json", "{}"),
        ],
    );
    assert_eq!(
        projects(&ctx_at(&root, &["app/src/main.ts"])),
        ["app/tsconfig.json"]
    );
}

#[test]
fn an_older_vite_root_with_include_runs_with_the_sibling_config_it_references() {
    // Vite's earlier react-ts root: its own include, and a reference to the config
    // beside it that holds vite.config.ts.
    let root = tree_with(
        "tsc-vite-old",
        &[
            (
                "tsconfig.json",
                "{ \"include\": [\"src\"], \"references\": [{ \"path\": \"./tsconfig.node.json\" }] }",
            ),
            (
                "tsconfig.node.json",
                "{ \"include\": [\"vite.config.ts\"] }",
            ),
        ],
    );
    assert_eq!(
        projects(&ctx_at(&root, &["vite.config.ts", "src/main.tsx"])),
        ["tsconfig.json", "tsconfig.node.json"]
    );
}

#[test]
fn a_tsconfig_saved_with_a_byte_order_mark_is_read() {
    // Editors on Windows save JSON with a UTF-8 BOM; tsc reads it.
    let root = tree_with(
        "tsc-bom",
        &[
            ("tsconfig.json", &format!("\u{feff}{SOLUTION}")),
            ("tsconfig.app.json", "{}"),
            ("tsconfig.node.json", "{}"),
        ],
    );
    assert_eq!(
        projects(&ctx_at(&root, &["src/main.ts"])),
        ["tsconfig.app.json", "tsconfig.node.json"]
    );
}

#[test]
fn each_run_keeps_its_build_info_in_a_temporary_folder_outside_the_repo() {
    let root = tree("tsc-buildinfo", &["tsconfig.json", "web/tsconfig.json"]);
    let c = ctx_at(&root, &["a.ts", "web/b.ts"]);
    let commands = Tsc.commands(&c);
    let infos: Vec<PathBuf> = commands
        .iter()
        .map(|cmd| {
            let i = cmd
                .args
                .iter()
                .position(|a| a == "--tsBuildInfoFile")
                .unwrap();
            PathBuf::from(&cmd.args[i + 1])
        })
        .collect();
    assert_eq!(
        commands[0].args,
        [
            "--noEmit",
            "--pretty",
            "false",
            "--listFiles",
            "--tsBuildInfoFile",
            infos[0].to_str().unwrap(),
            "-p",
            "tsconfig.json"
        ]
    );
    assert_ne!(infos[0], infos[1]);
    for info in &infos {
        assert!(!info.starts_with(&root), "{info:?} is in the repo");
        assert_eq!(info.parent(), infos[0].parent());
    }
    // What the runs wrote there is gone once they are done.
    let folder = infos[0].parent().unwrap();
    std::fs::create_dir_all(folder).unwrap();
    std::fs::write(&infos[0], "{}").unwrap();
    Tsc.finished(&c);
    assert!(!folder.exists(), "{folder:?} was left behind");
}
