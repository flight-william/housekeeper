//! A changed path that matches a protected settings name only when case is ignored
//! (`.SEMGREPIGNORE`, `Pyproject.toml`) is refused at commit and push. On a
//! case-insensitive filesystem the tool reads such a file as its settings, while
//! GitHub CODEOWNERS matches paths case-sensitively and asks no owner to review it.

mod common;

use common::{git, repo};
use house::approval::{PROTECTED, wrong_case};
use house::finding::Place;
use std::path::Path;
use std::process::{Command, Stdio};

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn protected() -> Vec<String> {
    s(PROTECTED)
}

/// (path, fix) of each finding for `paths`.
fn refused(paths: &[&str]) -> Vec<(String, String)> {
    wrong_case(&s(paths), &protected())
        .into_iter()
        .map(|f| {
            assert_eq!(f.rule, "R9");
            assert_eq!(f.tool_rule, "settings-case");
            let Place::File(path) = &f.place else {
                panic!("{f:?}")
            };
            assert_eq!(
                f.what,
                format!("settings file name in the wrong case: {path}")
            );
            (path.clone(), f.fix)
        })
        .collect()
}

#[test]
fn a_mixed_case_settings_name_is_refused_with_the_canonical_name() {
    for (path, canonical) in [
        (".SEMGREPIGNORE", ".semgrepignore"),
        ("Pyproject.toml", "pyproject.toml"),
        ("sub/Pyproject.toml", "sub/pyproject.toml"),
        (".ShellCheckRC", ".shellcheckrc"),
        ("scripts/ShellCheckRC", "scripts/shellcheckrc"),
        (".SHELLCHECKRC", ".shellcheckrc"),
        (".GitleaksIgnore", ".gitleaksignore"),
        (".Gitleaks.toml", ".gitleaks.toml"),
        ("App/.SwiftLint.yml", "App/.swiftlint.yml"),
        (".House.toml", ".house.toml"),
        (".HOUSE-EXCUSES.toml", ".house-excuses.toml"),
        (".GitHub/workflows/ci.yml", ".github/workflows/ci.yml"),
        (".GITHOOKS/pre-commit", ".githooks/pre-commit"),
        ("crates/a/.Cargo/Config.toml", "crates/a/.cargo/config.toml"),
        ("Deny.toml", "deny.toml"),
        ("RUFF.TOML", "ruff.toml"),
    ] {
        assert_eq!(
            refused(&[path]),
            vec![(
                path.to_string(),
                format!("use the canonical lowercase name {canonical}")
            )],
            "{path}"
        );
    }
}

#[test]
fn canonical_and_unrelated_names_are_not_refused() {
    let paths = [
        ".semgrepignore",
        "sub/pyproject.toml",
        ".shellcheckrc",
        "shellcheckrc",
        ".github/workflows/ci.yml",
        "docs/CODEOWNERS",
        "Src/Main.rs",
        "README.md",
        "Pyproject.toml.bak",
        "notes/Ruff.toml.md",
    ];
    assert!(refused(&paths).is_empty());
}

#[test]
fn codeowners_keeps_its_canonical_capitals() {
    assert_eq!(
        refused(&["docs/codeowners"]),
        vec![(
            "docs/codeowners".to_string(),
            "use the canonical name docs/CODEOWNERS".to_string()
        )]
    );
}

// The whole class, not one spelling: every protected name in another case is refused.
#[test]
fn every_protected_name_in_another_case_is_refused() {
    for glob in PROTECTED {
        let sample = glob.replace("**/", "sub/").replace("/**", "/x");
        assert!(refused(&[&sample]).is_empty(), "{sample}");
        // Every letter's case swapped: `sub/CODEOWNERS` becomes `SUB/codeowners`.
        let alias: String = sample
            .chars()
            .map(|c| {
                if c.is_uppercase() {
                    c.to_ascii_lowercase()
                } else {
                    c.to_ascii_uppercase()
                }
            })
            .collect();
        assert_eq!(refused(&[&alias]).len(), 1, "{glob}: {alias}");
    }
}

#[test]
fn an_extra_protected_glob_counts_too() {
    let mut list = protected();
    list.push("policy/*.yml".into());
    let found = wrong_case(&s(&["Policy/a.yml"]), &list);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].fix,
        "use the canonical lowercase name policy/a.yml"
    );
}

// ---- the binary, at push with the base's rules and at commit ----

fn sha(dir: &Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn commit(dir: &Path, files: &[(&str, &str)]) -> String {
    for (path, text) in files {
        std::fs::write(dir.join(path), text).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--no-verify", "-m", "x"]);
    sha(dir)
}

fn push_from_base(dir: &Path, base: &str, local: &str) -> (i32, String) {
    use std::io::Write;
    let log = common::fresh("settings-case-log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "push", "--config-from", base])
        .current_dir(dir)
        .env("HOUSE_LOG_DIR", &log)
        .env_remove("CLAUDECODE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let line = format!("refs/heads/f {local} refs/heads/f {base}\n");
    let mut pipe = child.stdin.take().unwrap();
    pipe.write_all(line.as_bytes()).unwrap();
    drop(pipe);
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    (out.status.code().unwrap_or(-1), text)
}

// The reproduction: the base runs ruff and Semgrep on a clean app.py; the change adds
// .SEMGREPIGNORE naming app.py and a silenced undefined name in app.py. On a
// case-insensitive filesystem the ignore file hides app.py from Semgrep.
#[test]
fn an_upper_case_semgrepignore_is_refused_at_push_by_the_base_rules() {
    let dir = repo("case-semgrepignore");
    let config = "[tools]\npush = [\"ruff\", \"semgrep\"]\n";
    let base = commit(&dir, &[(".house.toml", config), ("app.py", "X = 1\n")]);
    let local = commit(
        &dir,
        &[
            (".SEMGREPIGNORE", "app.py\n"),
            ("app.py", "def good():\n    missing_name()  # noqa: F821\n"),
        ],
    );
    let (code, text) = push_from_base(&dir, &base, &local);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains(
            "R9 .SEMGREPIGNORE: settings file name in the wrong case: .SEMGREPIGNORE. \
             fix: use the canonical lowercase name .semgrepignore. [settings-case]"
        ),
        "{text}"
    );
}

#[test]
fn an_upper_case_shellcheckrc_is_refused_at_push_by_the_base_rules() {
    let dir = repo("case-shellcheckrc");
    let config = "[tools]\npush = [\"shellcheck\"]\n";
    let base = commit(
        &dir,
        &[
            (".house.toml", config),
            ("run.sh", "#!/bin/sh\necho \"$1\"\n"),
        ],
    );
    let local = commit(
        &dir,
        &[
            (".SHELLCHECKRC", "disable=all\n"),
            ("run.sh", "#!/bin/sh\necho $1\n"),
        ],
    );
    let (code, text) = push_from_base(&dir, &base, &local);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains("R9 .SHELLCHECKRC: settings file name in the wrong case"),
        "{text}"
    );
}

#[test]
fn a_staged_mixed_case_settings_file_is_refused_at_commit() {
    let dir = repo("case-commit");
    std::fs::write(dir.join(".house.toml"), "[tools]\ncommit = []\n").unwrap();
    std::fs::write(dir.join("Pyproject.toml"), "[tool.ruff]\n").unwrap();
    git(&dir, &["add", "-A"]);
    let log = common::fresh("settings-case-commit-log");
    let out = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "commit"])
        .current_dir(&dir)
        .env("HOUSE_LOG_DIR", &log)
        .env_remove("CLAUDECODE")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(
        text.contains("R9 Pyproject.toml: settings file name in the wrong case: Pyproject.toml. fix: use the canonical lowercase name pyproject.toml."),
        "{text}"
    );
}
