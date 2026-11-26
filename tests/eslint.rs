//! The eslint adapter on output captured from ESLint 10.15.0 (with typescript-eslint)
//! run on tests/fixtures/typescript/ planted at /tmp/house-fixture/tsplant
//! (tests/fixtures/eslint/: planted, syntax and ignored exit 1, clean exit 0,
//! no_config exit 2).

#[path = "common/temp.rs"]
mod common;

use house::adapter::{Adapter, Context, Language, Output, Stage};
use house::adapters::eslint::Eslint;
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
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/eslint/");
    let read = |ext: &str| std::fs::read_to_string(format!("{dir}{name}.{ext}")).unwrap();
    Output {
        exit,
        stdout: read("stdout"),
        stderr: read("stderr"),
    }
}

/// ESLint's JSON for one file in `ROOT` with one message of `rule`.
fn one(rule: &str, message: &str) -> Output {
    Output {
        exit: 1,
        stdout: format!(
            r#"[{{"filePath":"{ROOT}/src/a.ts","messages":[{{"ruleId":"{rule}","severity":2,"message":"{message}","line":4,"column":1}}]}}]"#
        ),
        stderr: String::new(),
    }
}

#[test]
fn lints_the_code_files_and_node_scripts_warnings_as_errors() {
    let bare = common::fresh("eslint-path");
    assert!(
        Eslint
            .command(&ctx_at(&bare, &["a.py", "x.json"]))
            .is_none()
    );
    let mut c = ctx_at(&bare, &["a.ts", "b.jsx", "x.json", "bin/tool"]);
    c.languages.insert("bin/tool".into(), Language::Node);
    let cmd = Eslint.command(&c).unwrap();
    assert_eq!(cmd.program, "eslint");
    assert_eq!(
        cmd.args,
        [
            "--format",
            "json",
            "--max-warnings",
            "0",
            "--",
            "a.ts",
            "b.jsx",
            "bin/tool"
        ]
    );
    let bin = bare.join("node_modules/.bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("eslint"), "#!/bin/sh\n").unwrap();
    let local = Eslint.command(&c).unwrap();
    assert_eq!(PathBuf::from(local.program), bin.join("eslint"));
}

#[test]
fn errors_and_warnings_are_r2_findings_at_their_line() {
    let c = ctx(&["src/bad.ts", "src/clean.ts", "src/tool"]);
    let found = Eslint.parse(&c, &fixture("planted", 1)).unwrap();
    let lines: Vec<String> = found
        .iter()
        .map(|f| format!("{} {:?}:{:?} {}", f.rule, f.place, f.line, f.tool_rule))
        .collect();
    assert_eq!(
        lines,
        [
            r#"R2 File("src/bad.ts"):Some(2) eslint @typescript-eslint/no-unused-vars"#,
            r#"R2 File("src/bad.ts"):Some(3) eslint no-console"#,
            r#"R2 File("src/tool"):None eslint no-matching-config"#,
        ]
    );
    assert_eq!(found[0].what, "'unused' is assigned a value but never used");
    assert_eq!(found[0].fix, "correct the code at this line");
    assert_eq!(found[2].what, "ESLint has no configuration for this file");
}

#[test]
fn clean_gives_none() {
    let found = Eslint
        .parse(&ctx(&["src/clean.ts"]), &fixture("clean", 0))
        .unwrap();
    assert!(found.is_empty());
}

#[test]
fn a_file_eslint_cannot_parse_is_one_r2_finding() {
    let found = Eslint
        .parse(&ctx(&["src/broken.ts"]), &fixture("syntax", 1))
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule, "R2");
    assert_eq!(found[0].line, Some(1));
    assert_eq!(
        found[0].what,
        "the tool could not parse this file: Parsing error: Expression expected"
    );
    assert_eq!(found[0].tool_rule, "eslint parse-error");
}

#[test]
fn a_file_the_repos_config_ignores_is_not_a_finding() {
    let found = Eslint
        .parse(&ctx(&["src/bad.ts"]), &fixture("ignored", 1))
        .unwrap();
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_repo_with_no_eslint_config_is_a_finding_not_a_pass() {
    let found = Eslint
        .parse(&ctx(&["src/clean.ts"]), &fixture("no_config", 2))
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rule, "R2");
    assert_eq!(found[0].place, Place::Repo);
    assert_eq!(found[0].what, "ESLint found no eslint.config.* file");
    assert_eq!(
        found[0].fix,
        "add an ESLint flat config (eslint.config.js) at the repo root"
    );
}

#[test]
fn size_and_complexity_rules_map_to_r3_and_r4() {
    let c = ctx(&["src/a.ts"]);
    for (rule, want) in [
        ("max-lines-per-function", "R3"),
        ("max-statements", "R3"),
        ("complexity", "R4"),
        ("max-depth", "R4"),
        ("max-params", "R4"),
        ("max-nested-callbacks", "R4"),
        ("no-undef", "R2"),
    ] {
        let found = Eslint.parse(&c, &one(rule, "x.")).unwrap();
        assert_eq!(found[0].rule, want, "{rule}");
    }
}

#[test]
fn file_length_is_left_to_r3() {
    assert_eq!(Eslint.yields_to(), [("eslint max-lines", "r3")]);
}

#[test]
fn output_it_cannot_read_cannot_check() {
    let out = Output {
        exit: 2,
        stdout: String::new(),
        stderr: "\nOops! Something went wrong! :(\n\nESLint: 10.15.0\n\nTypeError: boom\n".into(),
    };
    let err = Eslint.parse(&ctx(&["src/a.ts"]), &out).unwrap_err();
    assert!(err.what.contains("eslint exited 2"), "{}", err.what);
}
