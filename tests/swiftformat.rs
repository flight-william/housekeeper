//! SwiftFormat (`swiftformat --lint`): R1, reading the repo's `.swiftformat`.
//! Output captured from SwiftFormat 0.64.7. It runs only when `.house.toml`
//! names it: a repo without a list keeps Apple's swift format alone.

mod common;

use house::adapter::{Adapter, Context, Output, Stage};
use house::adapters::{self, swiftformat::Swiftformat};
use house::finding::Place;
use house::{cli, config, init};
use std::path::{Path, PathBuf};

const PLANTED: &str = include_str!("fixtures/swiftformat/planted.json");
const CLEAN: &str = include_str!("fixtures/swiftformat/clean.json");
const BAD_CONFIG: &str = include_str!("fixtures/swiftformat/bad_config.stderr");

fn ctx(files: &[&str]) -> Context {
    Context {
        root: PathBuf::from("/repo"),
        stage: Stage::Commit,
        files: files.iter().map(|f| f.to_string()).collect(),
        push_range: None,
        languages: Default::default(),
    }
}

fn out(exit: i32, stdout: &str, stderr: &str) -> Output {
    Output {
        exit,
        stdout: stdout.into(),
        stderr: stderr.into(),
    }
}

#[test]
fn command_lints_the_swift_files_as_json() {
    assert!(Swiftformat.command(&ctx(&["a.py", "b.rs"])).is_none());
    let cmd = Swiftformat
        .command(&ctx(&["a.py", "Sources/A.swift", "B.swift"]))
        .unwrap();
    assert_eq!(cmd.program, "swiftformat");
    assert_eq!(
        cmd.args.join(" "),
        "--lint --reporter json --quiet -- Sources/A.swift B.swift"
    );
    assert_eq!(Swiftformat.name(), "swiftformat");
    assert_eq!(Swiftformat.program(), "swiftformat");
    assert_eq!(Swiftformat.brew_formula(), "swiftformat");
}

#[test]
fn planted_problems_give_r1_findings_with_rule_and_line() {
    let found = Swiftformat
        .parse(&ctx(&["Sources/Bad.swift"]), &out(1, PLANTED, ""))
        .unwrap();
    let got: Vec<(Option<u32>, &str)> = found
        .iter()
        .map(|f| (f.line, f.tool_rule.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            (Some(1), "swiftformat consecutiveSpaces"),
            (Some(2), "swiftformat emptyBraces"),
            (Some(2), "swiftformat redundantReturn"),
            (Some(2), "swiftformat spaceInsideParens"),
            (Some(3), "swiftformat indent"),
            (Some(3), "swiftformat redundantReturn"),
        ]
    );
    let first = &found[0];
    assert_eq!(first.rule, "R1");
    assert_eq!(first.place, Place::File("Sources/Bad.swift".into()));
    assert_eq!(first.what, "Replace consecutive spaces with a single space");
    assert_eq!(first.fix, "run swiftformat Sources/Bad.swift");
}

#[test]
fn clean_gives_none() {
    let found = Swiftformat
        .parse(&ctx(&["Sources/Good.swift"]), &out(0, CLEAN, ""))
        .unwrap();
    assert!(found.is_empty());
}

#[test]
fn an_error_is_cannot_check_with_its_message() {
    let err = Swiftformat
        .parse(&ctx(&["A.swift"]), &out(70, "", BAD_CONFIG))
        .unwrap_err();
    assert!(
        err.what.contains("Unknown option --nosuchoption"),
        "{err:?}"
    );
    assert!(err.what.contains("exit 70"), "{err:?}");
    assert!(
        Swiftformat
            .parse(&ctx(&["A.swift"]), &out(1, CLEAN, ""))
            .is_err()
    );
}

#[test]
fn it_runs_when_named_and_not_by_default() {
    let names = |cfg: &str| -> Vec<&'static str> {
        let cfg = config::parse(cfg).unwrap();
        let (chosen, unknown) = config::select(&cfg, Stage::Commit, adapters::all());
        assert!(unknown.is_empty());
        chosen.iter().map(|a| a.name()).collect()
    };
    assert!(!names("").contains(&"swiftformat"));
    assert!(names("").contains(&"swift-format"));
    assert_eq!(
        names("[tools]\ncommit = [\"swiftformat\"]\n"),
        ["swiftformat"]
    );
}

#[test]
fn init_does_not_name_it_for_a_swift_repo() {
    let root = common::repo("swiftformat-init");
    std::fs::write(root.join("A.swift"), "let a = 1\n").unwrap();
    common::git(&root, &["add", "-A"]);
    init::init(&root, adapters::all()).unwrap();
    let cfg = std::fs::read_to_string(root.join(".house.toml")).unwrap();
    assert!(cfg.contains("\"swift-format\""), "{cfg}");
    assert!(!cfg.contains("\"swiftformat\""), "{cfg}");
}

/// A repo that already formats with SwiftFormat: init names swiftformat, not swift
/// format, and writes no `.swift-format`, since two formatters disagree on one file.
#[test]
fn init_names_it_for_a_repo_with_a_swiftformat_file() {
    let root = common::repo("swiftformat-init-own");
    std::fs::write(root.join("A.swift"), "let a = 1\n").unwrap();
    std::fs::write(root.join(".swiftformat"), "--indent 2\n").unwrap();
    common::git(&root, &["add", "-A"]);
    let said = init::init(&root, adapters::all()).unwrap();
    let cfg = std::fs::read_to_string(root.join(".house.toml")).unwrap();
    assert!(cfg.contains("\"swiftformat\""), "{cfg}");
    assert!(!cfg.contains("\"swift-format\""), "{cfg}");
    assert!(!root.join(".swift-format").exists(), "{said:?}");
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn installed() -> bool {
    let found = std::process::Command::new("swiftformat")
        .arg("--version")
        .output()
        .is_ok();
    if !found {
        eprintln!("note: swiftformat is not installed; skipping the run on a repo");
    }
    found
}

/// The real tool, named in `.house.toml`, reads the repo's `.swiftformat`: a rule
/// it disables gives nothing, and a folder it excludes gives nothing though the
/// file is staged.
#[test]
fn the_repos_swiftformat_config_is_read() {
    if !installed() {
        return;
    }
    let root = common::repo("swiftformat-config");
    write(
        &root,
        ".house.toml",
        "[tools]\ncommit = [\"swiftformat\"]\n",
    );
    let config = "--swiftversion 5.9\n--disable consecutiveSpaces\n--exclude Vendor\n";
    write(&root, ".swiftformat", config);
    write(&root, "Sources/A.swift", "let a  = 1\nfunc f( ) {}\n");
    write(&root, "Vendor/V.swift", "func v( ) {}\n");
    common::git(&root, &["add", "-A"]);
    let report = cli::check(&root, Stage::Commit, adapters::all());
    assert!(report.cannots().is_empty(), "{}", report.text());
    let found: Vec<String> = report
        .findings()
        .iter()
        .map(|f| format!("{:?} {:?} {}", f.place, f.line, f.tool_rule))
        .collect();
    assert_eq!(
        found,
        [r#"File("Sources/A.swift") Some(2) swiftformat spaceInsideParens"#],
        "{}",
        report.text()
    );
}
