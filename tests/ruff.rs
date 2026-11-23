//! The ruff adapters on output captured from ruff 0.17.6 (tests/fixtures/ruff/).

use house::adapter::{Adapter, Context, Output, Stage};
use house::adapters::ruff::{Ruff, RuffFormat, rule_for};
use house::finding::Place;
use house::reply::finding_line;
use std::path::PathBuf;

const ROOT: &str = "/tmp/house-fixture/ruffplant";

fn ctx(files: &[&str]) -> Context {
    Context {
        root: PathBuf::from(ROOT),
        stage: Stage::Commit,
        files: files.iter().map(|f| f.to_string()).collect(),
        push_range: None,
        languages: Default::default(),
    }
}

fn fixture(name: &str, exit: i32) -> Output {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/ruff/");
    let read = |ext: &str| std::fs::read_to_string(format!("{dir}{name}.{ext}")).unwrap();
    Output {
        exit,
        stdout: read("stdout"),
        stderr: read("stderr"),
    }
}

#[test]
fn skips_without_python_files() {
    assert!(Ruff.command(&ctx(&["src/main.rs", "README.md"])).is_none());
    assert!(RuffFormat.command(&ctx(&[])).is_none());
}

#[test]
fn command_names_only_python_files() {
    let cmd = Ruff.command(&ctx(&["a.py", "b.rs", "c/d.pyi"])).unwrap();
    assert_eq!(cmd.program, "ruff");
    assert_eq!(
        cmd.args,
        [
            "check",
            "--output-format",
            "json",
            "--force-exclude",
            "--",
            "a.py",
            "c/d.pyi"
        ]
    );
    let fmt = RuffFormat.command(&ctx(&["a.py"])).unwrap();
    assert_eq!(fmt.args[..2], ["format", "--check"]);
}

#[test]
fn planted_check_gives_findings_with_house_rules() {
    let c = ctx(&["src/bad.py", "src/clean.py"]);
    let found = Ruff.parse(&c, &fixture("check_planted", 1)).unwrap();
    let lines: Vec<String> = found.iter().map(finding_line).collect();
    assert_eq!(
        lines,
        [
            "R2 src/bad.py:1: `os` imported but unused. fix: Remove unused import: `os`. [ruff F401]",
            "R4 src/bad.py:4: `score` is too complex (4 > 2). fix: split it into smaller helpers. [ruff C901]",
            "R4 src/bad.py:4: Too many arguments in function definition (7 > 5). fix: split it into smaller helpers. [ruff PLR0913]",
            "R2 src/bad.py:14: Statement ends with an unnecessary semicolon. fix: Remove unnecessary semicolon. [ruff E703]",
        ]
    );
    let c901 = found[1].measure.as_ref().unwrap();
    assert_eq!(
        (c901.value, c901.unit.as_str(), c901.limit),
        (4.0, "complexity", 2.0)
    );
    assert_eq!(found[0].place, Place::File("src/bad.py".into()));
}

#[test]
fn clean_check_gives_none() {
    let found = Ruff
        .parse(&ctx(&["src/clean.py"]), &fixture("check_clean", 0))
        .unwrap();
    assert!(found.is_empty());
}

// Was `syntax_error_is_an_r2_finding_from_check_only`, which counted one finding per
// diagnostic (3 for one file). Owner, 2026-11-22: a file a tool cannot parse is ONE
// R2 finding on that file. `ruff check` owns it; the formatter skips the file.
#[test]
fn a_file_ruff_cannot_parse_is_one_r2_finding_from_check_only() {
    let c = ctx(&["src/broken.py"]);
    let found = Ruff.parse(&c, &fixture("check_syntax", 1)).unwrap();
    let lines: Vec<String> = found.iter().map(finding_line).collect();
    assert_eq!(
        lines,
        [
            "R2 src/broken.py:1: the tool could not parse this file: Expected an identifier. fix: fix the syntax error. [ruff invalid-syntax]"
        ]
    );
    let fmt = RuffFormat.parse(&c, &fixture("format_syntax", 2)).unwrap();
    assert!(fmt.is_empty());
}

#[test]
fn planted_format_gives_r1_per_file() {
    let c = ctx(&["src/bad.py", "src/clean.py", "src/ugly.py"]);
    let found = RuffFormat.parse(&c, &fixture("format_planted", 1)).unwrap();
    let lines: Vec<String> = found.iter().map(finding_line).collect();
    assert_eq!(
        lines,
        [
            "R1 src/bad.py:14: File would be reformatted. fix: run ruff format src/bad.py. [ruff format]",
            "R1 src/ugly.py:1: File would be reformatted. fix: run ruff format src/ugly.py. [ruff format]",
        ]
    );
}

#[test]
fn clean_format_gives_none() {
    let found = RuffFormat
        .parse(&ctx(&["src/clean.py"]), &fixture("format_clean", 0))
        .unwrap();
    assert!(found.is_empty());
}

#[test]
fn unreadable_output_is_cannot_check() {
    let out = Output {
        exit: 2,
        stdout: String::new(),
        stderr: "ruff failed\n  Cause: bad config\n".into(),
    };
    let c = Ruff.parse(&ctx(&["a.py"]), &out).unwrap_err();
    assert_eq!(
        c.what,
        "ruff exited 2 without output house can read: ruff failed"
    );
}

#[test]
fn rule_mapping() {
    assert_eq!(rule_for("C901"), "R4");
    assert_eq!(rule_for("PLR0912"), "R4");
    assert_eq!(rule_for("PLR0913"), "R4");
    assert_eq!(rule_for("PLR0915"), "R3");
    assert_eq!(rule_for("E501"), "R2");
    assert_eq!(rule_for("S101"), "R2");
    assert_eq!(rule_for("unformatted"), "R1");
}

/// Exit 2 is ruff's "terminated abnormally"; only the formatter's exit 2
/// on files it cannot parse (ruff check reports those) is not a failure, and then
/// the other files' findings stand (the formatter met a syntax error and an
/// unformatted file: format_mixed, captured from ruff 0.17.6).
#[test]
fn exit_2_is_cannot_check_unless_the_formatter_met_syntax_errors() {
    let c = ctx(&["src/bad.py"]);
    assert!(Ruff.parse(&c, &fixture("check_planted", 2)).is_err());
    assert!(RuffFormat.parse(&c, &fixture("format_planted", 2)).is_err());
    assert!(RuffFormat.parse(&c, &fixture("format_clean", 2)).is_err());
    assert_eq!(
        RuffFormat.parse(&c, &fixture("format_syntax", 2)),
        Ok(vec![])
    );
    let mixed = RuffFormat
        .parse(
            &ctx(&["ugly.py", "tests/broken.py"]),
            &fixture("format_mixed", 2),
        )
        .unwrap();
    let lines: Vec<String> = mixed.iter().map(finding_line).collect();
    assert_eq!(
        lines,
        ["R1 ugly.py:1: File would be reformatted. fix: run ruff format ugly.py. [ruff format]"]
    );
}
