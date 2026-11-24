//! A protected name the filesystem resolves from another spelling counts as that name.
//! macOS folds case the Unicode way: `.ſwiftformat` (long s, U+017F) opens
//! `.swiftformat`, `K` (Kelvin sign, U+212A) opens `k`, and a decomposed
//! `Re\u{301}glages` opens `Réglages`. Each such spelling of a protected file needs the
//! owner's approval and is refused at commit and push, as wrong case is; and the
//! excuse list is read only under its own name, so the list on disk and the list in git
//! are always the same file.

mod common;

use common::{git, repo};
use house::approval::{PROTECTED, check, check_in, wrong_case, wrong_case_in};
use house::excuses;
use std::path::Path;
use std::process::Command;

const LONG_S: char = '\u{17f}';
const KELVIN: char = '\u{212a}';

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn protected() -> Vec<String> {
    s(PROTECTED)
}

#[test]
fn a_long_s_spelling_of_a_settings_name_is_refused_with_the_real_name() {
    for (path, real) in [
        (".\u{17f}wiftformat", ".swiftformat"),
        (".hou\u{17f}e-excuses.toml", ".house-excuses.toml"),
        ("App/.\u{17f}wiftlint.yml", "App/.swiftlint.yml"),
        (".githoo\u{212a}s/pre-commit", ".githooks/pre-commit"),
    ] {
        let found = wrong_case(&s(&[path]), &protected());
        assert_eq!(found.len(), 1, "{path}");
        assert_eq!(found[0].rule, "R9");
        assert_eq!(
            found[0].fix,
            format!("use the canonical lowercase name {real}"),
            "{path}"
        );
        assert_eq!(check(&s(&[path]), &protected(), "m").len(), 1, "{path}");
    }
}

// The whole class: every protected name, each `s` a long s and each `k` a Kelvin sign
// (the folder put for `**` has neither).
#[test]
fn every_protected_name_spelled_with_case_variants_needs_approval() {
    for glob in PROTECTED {
        let sample = glob.replace("**/", "dir/").replace("/**", "/x");
        let alias: String = sample
            .chars()
            .map(|c| match c {
                's' => LONG_S,
                'k' => KELVIN,
                c => c,
            })
            .collect();
        if alias == sample {
            continue;
        }
        assert_eq!(wrong_case(&s(&[&alias]), &protected()).len(), 1, "{alias}");
        assert_eq!(check(&s(&[&alias]), &protected(), "m").len(), 1, "{alias}");
    }
}

/// A repo whose `[protected] extra` names `Réglages/**` (composed), with that folder on
/// disk: the decomposed spelling is the same folder.
fn reglages(name: &str) -> (std::path::PathBuf, Vec<String>) {
    let root = repo(name);
    std::fs::create_dir_all(root.join("R\u{e9}glages")).unwrap();
    std::fs::write(root.join("R\u{e9}glages/limits.yml"), "max: 1\n").unwrap();
    let mut list = protected();
    list.push("R\u{e9}glages/**".into());
    (root, list)
}

#[test]
fn a_spelling_the_filesystem_resolves_to_a_protected_file_is_that_file() {
    let (root, list) = reglages("alias-identity");
    let nfd = s(&["Re\u{301}glages/limits.yml"]);
    // As text the two spellings differ; on disk they are one file.
    assert!(check(&nfd, &list, "m").is_empty());
    let approval = check_in(&root, &nfd, &list, "m");
    assert_eq!(approval.len(), 1, "{approval:?}");
    let named = wrong_case_in(&root, &nfd, &list);
    assert_eq!(named.len(), 1, "{named:?}");
    assert_eq!(
        named[0].what,
        "settings file name in another spelling: Re\u{301}glages/limits.yml"
    );
    assert_eq!(
        named[0].fix,
        "use the canonical name R\u{e9}glages/limits.yml"
    );
    // The name as written is no alias, and an unrelated accented file is no finding.
    let nfc = s(&["R\u{e9}glages/limits.yml"]);
    assert_eq!(check_in(&root, &nfc, &list, "m").len(), 1);
    assert!(wrong_case_in(&root, &nfc, &list).is_empty());
    std::fs::write(root.join("Caf\u{e9}.txt"), "x\n").unwrap();
    assert!(check_in(&root, &s(&["Caf\u{e9}.txt"]), &list, "m").is_empty());
}

fn commit_msg(root: &Path) -> (i32, String) {
    let message = root.join("MSG");
    std::fs::write(
        &message,
        "settings: change\n\nWhy: w.\n\nProof: p.\n\nUndo: u.\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_house"))
        .arg("commit-msg")
        .arg(&message)
        .current_dir(root)
        .env("HOUSE_LOG_DIR", common::fresh("settings-alias-log"))
        .env_remove("CLAUDECODE")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    (out.status.code().unwrap_or(-1), text)
}

fn check_commit(root: &Path) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "commit"])
        .current_dir(root)
        .env("HOUSE_LOG_DIR", common::fresh("settings-alias-log"))
        .env_remove("CLAUDECODE")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    (out.status.code().unwrap_or(-1), text)
}

// The reproduction: a long-s `.swiftformat` excluding Sources silenced SwiftFormat,
// and neither the commit check nor the message check said a word.
#[test]
fn a_long_s_swiftformat_is_refused_at_commit_and_needs_approval() {
    let root = repo("alias-swiftformat");
    std::fs::write(root.join(".house.toml"), "[tools]\ncommit = []\n").unwrap();
    std::fs::write(root.join(".\u{17f}wiftformat"), "--exclude Sources\n").unwrap();
    git(&root, &["add", "-A"]);
    let (code, text) = check_commit(&root);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("fix: use the canonical lowercase name .swiftformat. [settings-case]"),
        "{text}"
    );
    let (code, text) = commit_msg(&root);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("protected file changed without the owner's approval line"),
        "{text}"
    );
}

// git keeping the decomposed name (`core.precomposeunicode` off): the staged path is
// spelled apart from the composed glob, and is the same folder on disk.
#[test]
fn a_decomposed_path_into_a_composed_protected_folder_needs_approval() {
    let root = repo("alias-nfd");
    git(&root, &["config", "core.precomposeunicode", "false"]);
    std::fs::write(
        root.join(".house.toml"),
        "[tools]\ncommit = []\n[protected]\nextra = [\"R\u{e9}glages/**\"]\n",
    )
    .unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "--no-verify", "-m", "x"]);
    std::fs::create_dir_all(root.join("Re\u{301}glages")).unwrap();
    std::fs::write(root.join("Re\u{301}glages/limits.yml"), "max: 1\n").unwrap();
    git(&root, &["add", "-A"]);
    let (code, text) = commit_msg(&root);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("protected file changed without the owner's approval line"),
        "{text}"
    );
}

#[test]
fn an_excuse_list_under_another_spelling_is_not_read() {
    let root = repo("alias-excuses");
    let list =
        "[[excuse]]\nrule = \"R2\"\nfile = \"a.py\"\ndeadline = \"2026-12-20\"\nreason = \"w\"\n";
    std::fs::write(root.join(".hou\u{17f}e-excuses.toml"), list).unwrap();
    let err = excuses::in_tree(&root).unwrap_err();
    assert!(
        err.what.contains(".hou\u{17f}e-excuses.toml"),
        "{}",
        err.what
    );
    assert!(err.fix.contains(".house-excuses.toml"), "{}", err.fix);
    // Under its own name it is read.
    std::fs::remove_file(root.join(".hou\u{17f}e-excuses.toml")).unwrap();
    std::fs::write(root.join(".house-excuses.toml"), list).unwrap();
    assert_eq!(excuses::in_tree(&root).unwrap().unwrap().len(), 1);
}
