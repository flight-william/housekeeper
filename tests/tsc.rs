//! The tsc adapter on output captured from TypeScript 6.0.3 run on
//! tests/fixtures/typescript/ planted at /tmp/house-fixture/tsplant
//! (tests/fixtures/tsc/: planted and syntax exit 2, clean exit 0, no_tsconfig exit 1).

#[path = "common/temp.rs"]
mod common;

use house::adapter::{Adapter, Context, Output, Stage};
use house::adapters::tsc::Tsc;
use house::finding::Place;
use std::path::{Path, PathBuf};

const ROOT: &str = "/private/tmp/house-fixture/tsplant";

fn ctx_at(root: &Path, files: &[&str]) -> Context {
    Context {
        root: root.to_path_buf(),
        stage: Stage::Commit,
        files: files.iter().map(|f| f.to_string()).collect(),
        push_range: None,
        languages: Default::default(),
    }
}

fn ctx(files: &[&str]) -> Context {
    ctx_at(Path::new(ROOT), files)
}

fn fixture(name: &str, exit: i32) -> Output {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tsc/");
    let read = |ext: &str| std::fs::read_to_string(format!("{dir}{name}.{ext}")).unwrap();
    Output {
        exit,
        stdout: read("stdout"),
        stderr: read("stderr"),
    }
}

/// A folder holding an empty file at each of `paths`.
fn tree(name: &str, paths: &[&str]) -> PathBuf {
    let root = common::fresh(name);
    for p in paths {
        let file = root.join(p);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "{}\n").unwrap();
    }
    root
}

fn projects(c: &Context) -> Vec<String> {
    Tsc.commands(c)
        .iter()
        .map(|cmd| cmd.args.join(" "))
        .collect()
}

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
    assert_eq!(
        projects(&c),
        [
            "--noEmit --pretty false -p tsconfig.json",
            "--noEmit --pretty false -p packages/a/tsconfig.json"
        ]
    );
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
        ["--noEmit --pretty false -p web/tsconfig.json"]
    );
}

#[test]
fn a_typescript_file_with_no_tsconfig_runs_the_root_one_which_tsc_reports_missing() {
    let root = tree("tsc-missing", &[]);
    assert_eq!(
        projects(&ctx_at(&root, &["a.ts"])),
        ["--noEmit --pretty false -p tsconfig.json"]
    );
    let found = Tsc
        .parse(&ctx(&["a.ts"]), &fixture("no_tsconfig", 1))
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].place, Place::Repo);
    assert_eq!(found[0].tool_rule, "tsc TS5058");
    assert_eq!(
        found[0].what,
        "The specified path does not exist: 'missing/tsconfig.json'"
    );
}

#[test]
fn a_tsconfig_path_like_an_option_gets_dot_slash() {
    let root = tree("tsc-dash", &["-w/tsconfig.json"]);
    assert_eq!(
        projects(&ctx_at(&root, &["-w/a.ts"])),
        ["--noEmit --pretty false -p ./-w/tsconfig.json"]
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
fn each_type_error_is_an_r2_finding_at_its_line() {
    let found = Tsc
        .parse(&ctx(&["src/bad.ts"]), &fixture("planted", 2))
        .unwrap();
    assert_eq!(found.len(), 1);
    let f = &found[0];
    assert_eq!(f.rule, "R2");
    assert_eq!(f.place, Place::File("src/bad.ts".into()));
    assert_eq!(f.line, Some(1));
    assert_eq!(f.what, "Type 'string' is not assignable to type 'number'");
    assert_eq!(f.fix, "fix the type error");
    assert_eq!(f.tool_rule, "tsc TS2322");
}

#[test]
fn a_syntax_error_is_a_finding_too() {
    let found = Tsc
        .parse(&ctx(&["src/broken.ts"]), &fixture("syntax", 2))
        .unwrap();
    assert_eq!(found[0].tool_rule, "tsc TS1109");
    assert_eq!(found[0].line, Some(1));
}

#[test]
fn only_errors_in_the_stages_files_are_reported() {
    let found = Tsc
        .parse(&ctx(&["src/clean.ts"]), &fixture("planted", 2))
        .unwrap();
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn clean_gives_none() {
    let found = Tsc
        .parse(&ctx(&["src/clean.ts"]), &fixture("clean", 0))
        .unwrap();
    assert!(found.is_empty());
}

#[test]
fn a_failure_with_nothing_readable_cannot_check() {
    let out = Output {
        exit: 3,
        stdout: String::new(),
        stderr: "RangeError: Maximum call stack size exceeded\n".into(),
    };
    let err = Tsc.parse(&ctx(&["a.ts"]), &out).unwrap_err();
    assert!(err.what.contains("tsc exited 3"), "{}", err.what);
}
