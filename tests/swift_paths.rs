//! SwiftLint and SwiftFormat print paths in decomposed Unicode (`e` and U+0301);
//! git and the disk hold the composed form (`é`). A finding is matched to the stage's
//! file by the file itself, not by the bytes of its path, and is reported in git's
//! spelling, so excuses and `[paths] exclude` written that way match it too.

mod common;

use house::adapter::{Adapter, Stage};
use house::adapters::{swiftformat::Swiftformat, swiftlint::SwiftLint};
use house::cli;
use house::reply::place_text;
use std::path::Path;

/// `Café.swift`, composed, as git spells it.
const CAFE: &str = "Caf\u{e9}.swift";

fn places(root: &Path, adapter: Box<dyn Adapter>) -> Vec<String> {
    let report = cli::check(root, Stage::Commit, vec![adapter]);
    assert!(report.cannots().is_empty(), "{}", report.text());
    report
        .findings()
        .iter()
        .map(|f| place_text(&f.place).to_string())
        .collect()
}

fn staged(root: &Path, file: &str, text: &str) {
    std::fs::write(root.join(file), text).unwrap();
    common::git(root, &["add", "-A"]);
}

#[test]
fn a_swiftlint_finding_on_an_accented_file_name_is_kept_in_gits_spelling() {
    let root = common::repo("swift-paths-file");
    staged(&root, CAFE, "let value = 1;\n");
    assert_eq!(places(&root, Box::new(SwiftLint)), [CAFE]);
}

#[test]
fn a_swiftlint_finding_in_a_repo_under_an_accented_folder_is_kept() {
    let root = common::repo("swift-paths-D\u{e9}velop");
    staged(&root, "Plain.swift", "let value = 1;\n");
    assert_eq!(places(&root, Box::new(SwiftLint)), ["Plain.swift"]);
}

#[test]
fn a_swiftlint_finding_with_the_repos_config_and_an_accented_name_is_kept() {
    let root = common::repo("swift-paths-config");
    std::fs::write(root.join(".swiftlint.yml"), "disabled_rules: []\n").unwrap();
    staged(&root, CAFE, "let value = 1;\n");
    assert_eq!(places(&root, Box::new(SwiftLint)), [CAFE]);
}

#[test]
fn a_swiftformat_finding_on_an_accented_file_name_is_in_gits_spelling() {
    let root = common::repo("swift-paths-swiftformat");
    let toml = "[tools]\ncommit = [\"swiftformat\"]\n";
    std::fs::write(root.join(".house.toml"), toml).unwrap();
    staged(&root, CAFE, "let value = 1 ;\n");
    let got = places(&root, Box::new(Swiftformat));
    assert!(!got.is_empty());
    assert!(got.iter().all(|p| p == CAFE), "{got:?}");
}
