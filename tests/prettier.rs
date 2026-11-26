//! The prettier adapter on output captured from Prettier 3.10.1 run on
//! tests/fixtures/typescript/ planted at /tmp/house-fixture/tsplant
//! (tests/fixtures/prettier/: planted exit 1, clean exit 0, syntax exit 2).

#[path = "common/temp.rs"]
mod common;

use house::adapter::{Adapter, Context, Language, Output, Stage};
use house::adapters::prettier::Prettier;
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
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/prettier/");
    let read = |ext: &str| std::fs::read_to_string(format!("{dir}{name}.{ext}")).unwrap();
    Output {
        exit,
        stdout: read("stdout"),
        stderr: read("stderr"),
    }
}

#[test]
fn checks_the_code_files_and_node_scripts_only() {
    let bare = common::fresh("prettier-path");
    assert!(
        Prettier
            .command(&ctx_at(&bare, &["README.md", "a.py", "package.json"]))
            .is_none()
    );
    let mut c = ctx_at(
        &bare,
        &[
            "a.ts",
            "b.tsx",
            "c.mts",
            "d.cts",
            "e.js",
            "f.jsx",
            "g.mjs",
            "h.cjs",
            "x.json",
            "y.css",
            "z.md",
            "bin/tool",
            "bin/other",
        ],
    );
    c.languages.insert("bin/tool".into(), Language::Node);
    c.languages.insert("bin/other".into(), Language::Shell);
    let cmd = Prettier.command(&c).unwrap();
    assert_eq!(cmd.program, "prettier");
    assert_eq!(
        cmd.args,
        [
            "--check", "--", "a.ts", "b.tsx", "c.mts", "d.cts", "e.js", "f.jsx", "g.mjs", "h.cjs",
            "bin/tool"
        ]
    );
}

#[test]
fn prefers_the_repos_own_node_modules_copy() {
    let root = common::fresh("prettier-local");
    let bin = root.join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("prettier"), "#!/bin/sh\n").unwrap();
    let cmd = Prettier.command(&ctx_at(&root, &["a.ts"])).unwrap();
    assert_eq!(PathBuf::from(cmd.program), bin.join("prettier"));
}

#[test]
fn each_unformatted_file_is_one_r1_finding() {
    let c = ctx(&["src/bad.ts", "src/clean.ts", "src/tool"]);
    let found = Prettier.parse(&c, &fixture("planted", 1)).unwrap();
    let places: Vec<&Place> = found.iter().map(|f| &f.place).collect();
    assert_eq!(
        places,
        [
            &Place::File("src/bad.ts".into()),
            &Place::File("src/tool".into())
        ]
    );
    let first = &found[0];
    assert_eq!(first.rule, "R1");
    assert_eq!(first.line, None);
    assert_eq!(first.what, "not formatted as Prettier formats it");
    assert_eq!(first.fix, "run prettier --write src/bad.ts");
    assert_eq!(first.tool_rule, "prettier --check");
}

#[test]
fn clean_gives_none() {
    let found = Prettier
        .parse(&ctx(&["src/clean.ts"]), &fixture("clean", 0))
        .unwrap();
    assert!(found.is_empty());
}

#[test]
fn a_file_prettier_cannot_parse_is_left_to_eslint_and_tsc() {
    let c = ctx(&["src/clean.ts", "src/broken.ts", "src/bad.ts"]);
    let found = Prettier.parse(&c, &fixture("syntax", 2)).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].place, Place::File("src/bad.ts".into()));
}

#[test]
fn any_other_failure_cannot_check() {
    let out = Output {
        exit: 2,
        stdout: String::new(),
        stderr: "[error] Invalid configuration for file \"src/a.ts\":\n".into(),
    };
    let err = Prettier.parse(&ctx(&["src/a.ts"]), &out).unwrap_err();
    assert!(err.what.contains("prettier exited 2"), "{}", err.what);
    assert!(err.what.contains("Invalid configuration"), "{}", err.what);
}
