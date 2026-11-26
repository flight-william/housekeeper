//! The R3 size adapter on output captured from scripts/r3_count.py (Python 3.15.2)
//! run on tests/fixtures/r3/planted/.

use house::adapter::{Adapter, Context, Output, Stage};
use house::adapters::r3::R3;
use house::finding::Place;
use std::path::PathBuf;

fn ctx(files: &[&str]) -> Context {
    Context {
        root: PathBuf::from("/repo"),
        stage: Stage::Commit,
        files: files.iter().map(|f| f.to_string()).collect(),
        push_range: None,
        languages: Default::default(),
    }
}

fn out(exit: i32, stdout: &str) -> Output {
    Output {
        exit,
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

const PLANTED: &str = include_str!("fixtures/r3/planted.jsonl");
const CLEAN: &str = include_str!("fixtures/r3/clean.jsonl");

#[test]
fn skips_without_source_files() {
    assert!(R3.command(&ctx(&["README.md", "a.toml"])).is_none());
}

#[test]
fn command_runs_the_script_on_source_files_only() {
    let cmd = R3
        .command(&ctx(&["a.py", "b.md", "c.rs", "d.swift", "e.sh", "f.bash"]))
        .unwrap();
    assert_eq!(cmd.program, "python3");
    assert_eq!(cmd.args[0], "-I");
    assert!(cmd.args[1].ends_with("/r3_count.py"));
    assert_eq!(cmd.args[2..], ["a.py", "c.rs", "d.swift", "e.sh", "f.bash"]);
}

#[test]
fn planted_gives_function_and_file_findings() {
    let found = R3.parse(&ctx(&[]), &out(0, PLANTED)).unwrap();
    assert_eq!(found.len(), 5);
    let method = &found[1];
    assert_eq!(method.rule, "R3");
    assert_eq!(method.place, Place::File("funcs.py".to_string()));
    assert_eq!(method.what, "function K.method has 62 lines, limit is 60");
    assert_eq!(method.fix, "split the function into smaller functions");
    let m = method.measure.as_ref().unwrap();
    assert_eq!((m.value, m.unit.as_str(), m.limit), (62.0, "lines", 60.0));
    let rs = &found[3];
    assert_eq!(rs.place, Place::File("long.rs".to_string()));
    assert_eq!(rs.what, "file has 401 lines, limit is 400");
    assert_eq!(rs.tool_rule, "r3 file_lines");
    assert!(
        found
            .iter()
            .all(|f| !f.what.ends_with('.') && !f.fix.ends_with('.'))
    );
}

#[test]
fn clean_gives_no_findings() {
    assert_eq!(R3.parse(&ctx(&[]), &out(0, CLEAN)), Ok(vec![]));
}

#[test]
fn script_failure_cannot_check() {
    assert!(R3.parse(&ctx(&[]), &out(1, "Traceback")).is_err());
    assert!(R3.parse(&ctx(&[]), &out(0, "not json")).is_err());
}

#[test]
fn real_script_matches_the_fixture() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/r3/planted");
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/r3_count.py");
    let run = std::process::Command::new("python3")
        .args([
            "-I",
            script,
            "funcs.py",
            "exact_file.py",
            "long_file.py",
            "long.rs",
            "short.rs",
            "long.swift",
        ])
        .current_dir(dir)
        .output()
        .expect("python3 must be installed: this test runs the real scripts/r3_count.py");
    assert!(
        run.status.success(),
        "r3_count.py failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let got = String::from_utf8(run.stdout).unwrap();
    let want = PLANTED;
    let mut g: Vec<_> = got.lines().collect();
    let mut w: Vec<_> = want.lines().collect();
    g.sort();
    w.sort();
    assert_eq!(g, w);
}

#[test]
fn command_takes_the_javascript_and_typescript_files_and_node_scripts() {
    let mut c = ctx(&[
        "a.ts", "b.tsx", "c.mts", "d.cts", "e.js", "f.jsx", "g.mjs", "h.cjs", "x.json", "bin/tool",
    ]);
    c.languages
        .insert("bin/tool".into(), house::adapter::Language::Node);
    let cmd = R3.command(&c).unwrap();
    assert_eq!(
        cmd.args[2..],
        [
            "a.ts", "b.tsx", "c.mts", "d.cts", "e.js", "f.jsx", "g.mjs", "h.cjs", "bin/tool"
        ]
    );
}

/// The real script on a 401-line file of each JavaScript and TypeScript kind, a
/// 400-line one, and a 401-line `node` script with no extension: the same 400-line
/// file limit as every language.
#[test]
fn real_script_counts_javascript_and_typescript_files() {
    let dir = std::env::temp_dir().join(format!("house-r3-js-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let lines = |n: usize| "let a = 1;\n".repeat(n);
    let long = [
        "a.ts", "b.tsx", "c.mts", "d.cts", "e.js", "f.jsx", "g.mjs", "h.cjs",
    ];
    for name in long {
        std::fs::write(dir.join(name), lines(401)).unwrap();
    }
    std::fs::write(dir.join("ok.ts"), lines(400)).unwrap();
    std::fs::write(
        dir.join("tool"),
        format!("#!/usr/bin/env node\n{}", lines(400)),
    )
    .unwrap();
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/r3_count.py");
    let run = std::process::Command::new("python3")
        .args(["-I", script])
        .args(long)
        .args(["ok.ts", "tool"])
        .current_dir(&dir)
        .output()
        .expect("python3 must be installed: this test runs the real scripts/r3_count.py");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let found = R3
        .parse(&ctx(&[]), &out(0, &String::from_utf8(run.stdout).unwrap()))
        .unwrap();
    let files: Vec<String> = found
        .iter()
        .map(|f| format!("{} {}", house::reply::place_text(&f.place), f.what))
        .collect();
    let mut want: Vec<String> = long
        .iter()
        .chain(&["tool"])
        .map(|n| format!("{n} file has 401 lines, limit is 400"))
        .collect();
    want.sort();
    let mut files = files;
    files.sort();
    assert_eq!(files, want);
}
