//! The tsc adapter on output captured from TypeScript 6.0.3 run on
//! tests/fixtures/typescript/ planted at /tmp/house-fixture/tsplant
//! (tests/fixtures/tsc/: planted and syntax exit 2, clean exit 0, no_tsconfig exit 1;
//! unbuilt, exit 2, at /tmp/house-fixture/tsmono). Its projects: tests/tsc_projects.rs.

mod tsc_support;

use house::adapter::{Adapter, Context, Output};
use house::adapters::tsc::Tsc;
use house::finding::{Finding, Place};
use std::path::Path;
use tsc_support::{ctx_at, projects, tree_with};

const ROOT: &str = "/private/tmp/house-fixture/tsplant";

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

/// Each finding as `rule place: what [tool rule]`.
fn lines(found: &[Finding]) -> Vec<String> {
    found
        .iter()
        .map(|f| {
            format!(
                "{} {}: {} [{}]",
                f.rule,
                house::reply::place_text(&f.place),
                f.what,
                f.tool_rule
            )
        })
        .collect()
}

#[test]
fn a_typescript_file_under_no_tsconfig_is_in_no_project() {
    let root = tree_with("tsc-missing", &[]);
    let c = ctx_at(&root, &["a.ts"]);
    // No project to run: tsc --version still proves tsc is there, and the file is
    // reported, not passed.
    assert_eq!(projects(&c), ["--version"]);
    let found = Tsc.joined(&c, &[fixture("version", 0)]);
    assert_eq!(
        lines(&found),
        ["R2 a.ts: tsc: no project includes this file [tsc no-project]"]
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

#[test]
fn a_staged_file_no_project_lists_is_an_r2_finding() {
    // tests/fixtures/typescript/tsconfig.json includes only src/.
    let c = ctx(&["src/bad.ts", "scripts/seed.ts", "src/clean.ts"]);
    let found = Tsc.joined(&c, &[fixture("planted", 2)]);
    assert_eq!(
        lines(&found),
        ["R2 scripts/seed.ts: tsc: no project includes this file [tsc no-project]"]
    );
    assert_eq!(
        found[0].fix,
        "add it to the include or files of a tsconfig.json, or to a project one references"
    );
}

#[test]
fn a_file_any_one_project_lists_is_included() {
    // The clean run lists src/clean.ts (src/bad.ts was gone when it was captured).
    let c = ctx(&["src/clean.ts"]);
    let other = Output {
        exit: 0,
        stdout: String::new(),
        stderr: String::new(),
    };
    assert!(Tsc.joined(&c, &[other, fixture("clean", 0)]).is_empty());
}

#[test]
fn the_listed_files_are_not_findings() {
    let found = Tsc
        .parse(&ctx(&["src/clean.ts"]), &fixture("clean", 0))
        .unwrap();
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn an_unbuilt_referenced_project_cannot_check_rather_than_a_finding() {
    // tests/fixtures/tsc/unbuilt: app references a composite lib whose output was never
    // built, so tsc reports TS6305 at app's import; captured at /tmp/house-fixture/tsmono.
    let c = ctx_at(
        Path::new("/private/tmp/house-fixture/tsmono"),
        &["app/src/main.ts"],
    );
    let err = Tsc.parse(&c, &fixture("unbuilt", 2)).unwrap_err();
    assert!(
        err.what
            .starts_with("tsc: a referenced project has not been built: Output file"),
        "{}",
        err.what
    );
    assert_eq!(err.fix, "build the referenced projects first (tsc -b)");
}
