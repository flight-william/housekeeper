//! `house init` writes a tool setting only when it fits the files present: a
//! `.swift-format` with the indentation the repo's Swift files already use, so the
//! formatter's own default (2 spaces) does not turn every line of 4-space code into a
//! finding. No Swift files, an existing `.swift-format`, or no indentation to read:
//! nothing is written.

mod common;

use house::init;
use std::path::{Path, PathBuf};

fn repo_with(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let repo = common::repo(name);
    for (path, text) in files {
        let at = repo.join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
        common::git(&repo, &["add", path]);
    }
    repo
}

fn run(repo: &Path) -> Vec<String> {
    init::init_with_owner(repo, house::adapters::all(), None).unwrap()
}

fn setting(repo: &Path) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(repo.join(".swift-format")).ok()?;
    Some(serde_json::from_str(&text).expect(".swift-format is JSON"))
}

const FOUR: &str =
    "struct A {\n    func b() {\n        if c {\n            d()\n        }\n    }\n}\n";
const TWO: &str = "struct A {\n  func b() {\n    d()\n  }\n}\n";
const TABS: &str = "struct A {\n\tfunc b() {\n\t\td()\n\t}\n}\n";

#[test]
fn swift_code_indented_by_four_gets_a_swift_format_with_four_spaces() {
    let repo = repo_with(
        "init-swift-four",
        &[("Sources/A.swift", FOUR), ("Sources/B.swift", FOUR)],
    );
    let said = run(&repo);
    let json = setting(&repo).expect(".swift-format written");
    assert_eq!(json["indentation"], serde_json::json!({"spaces": 4}));
    assert_eq!(json["version"], 1);
    assert!(
        said.iter().any(|l| l
            == "house init: wrote .swift-format: indentation 4 spaces, as the Swift files use"),
        "{said:?}"
    );
}

#[test]
fn the_indentation_most_swift_files_use_wins() {
    let repo = repo_with(
        "init-swift-most",
        &[
            ("A.swift", TWO),
            ("B.swift", TWO),
            ("C.swift", TWO),
            ("D.swift", FOUR),
        ],
    );
    run(&repo);
    let json = setting(&repo).expect(".swift-format written");
    assert_eq!(json["indentation"], serde_json::json!({"spaces": 2}));
}

#[test]
fn swift_code_indented_by_tabs_gets_tabs() {
    let repo = repo_with("init-swift-tabs", &[("A.swift", TABS)]);
    run(&repo);
    let json = setting(&repo).expect(".swift-format written");
    assert_eq!(json["indentation"], serde_json::json!({"tabs": 1}));
}

#[test]
fn an_existing_swift_format_is_left_as_it_is() {
    let repo = repo_with(
        "init-swift-kept",
        &[("A.swift", FOUR), (".swift-format", "{\"version\": 1}\n")],
    );
    let said = run(&repo);
    assert_eq!(
        std::fs::read_to_string(repo.join(".swift-format")).unwrap(),
        "{\"version\": 1}\n"
    );
    assert!(
        said.iter()
            .any(|l| l == "house init: .swift-format already exists; left as it is"),
        "{said:?}"
    );
}

#[test]
fn a_repo_with_no_swift_files_gets_no_swift_setting() {
    let repo = repo_with("init-swift-none", &[("a.py", "def f():\n    return 1\n")]);
    let said = run(&repo);
    assert!(setting(&repo).is_none());
    let about = |l: &&String| {
        l.starts_with("house init: wrote .swift-format")
            || l.starts_with("house init: no .swift-format")
            || l.starts_with("house init: .swift-format")
    };
    assert!(!said.iter().any(|l| about(&l)), "{said:?}");
}

#[test]
fn swift_files_with_no_indentation_get_none_and_init_says_so() {
    let repo = repo_with("init-swift-flat", &[("A.swift", "let a = 1\nlet b = 2\n")]);
    let said = run(&repo);
    assert!(setting(&repo).is_none());
    assert!(
        said.iter().any(|l| l
            == "house init: no .swift-format written: the Swift files show no indentation to match"),
        "{said:?}"
    );
}
