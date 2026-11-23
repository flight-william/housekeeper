//! The approval check: protected paths need the owner's line.

use house::approval::{PROTECTED, check, glob_matches};
use house::finding::Place;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn protected() -> Vec<String> {
    s(PROTECTED)
}

const OK: &str = "tune lint\n\nOwner-approved: 2026-11-22 \"yes, loosen it\"\n";

#[test]
fn protected_change_without_line_is_r9() {
    let f = check(&s(&["pyproject.toml", "src/a.rs"]), &protected(), "msg\n");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].rule, "R9");
    assert_eq!(f[0].place, Place::File("pyproject.toml".into()));
    assert_eq!(
        f[0].what,
        "protected file changed without the owner's approval line"
    );
    assert!(f[0].fix.starts_with("add Owner-approved: <date>"));
}

#[test]
fn approval_line_passes() {
    assert!(check(&s(&[".house.toml"]), &protected(), OK).is_empty());
}

#[test]
fn bad_approval_lines_do_not_pass() {
    for line in [
        "Owner-approved: 2026-13-40 \"x\"",
        "Owner-approved: 2026-11-22",
        "Owner-approved: 2026-11-22 \"\"",
        "Owner-approved: 2026-11-22 x",
        "owner-approved: 2026-11-22 \"x\"",
    ] {
        assert_eq!(
            check(&s(&["CODEOWNERS"]), &protected(), line).len(),
            1,
            "{line}"
        );
    }
}

#[test]
fn unprotected_paths_are_clean() {
    assert!(check(&s(&["src/main.rs", "README.md"]), &protected(), "m").is_empty());
}

#[test]
fn globs() {
    assert!(glob_matches("**/ruff.toml", "ruff.toml"));
    assert!(glob_matches("**/ruff.toml", "a/b/ruff.toml"));
    assert!(!glob_matches("**/ruff.toml", "a/xruff.toml"));
    assert!(glob_matches(
        ".github/workflows/**",
        ".github/workflows/ci.yml"
    ));
    assert!(glob_matches(
        ".github/workflows/**",
        ".github/workflows/x/ci.yml"
    ));
    assert!(!glob_matches(".github/workflows/**", ".github/workflows"));
    assert!(glob_matches("*.toml", "a.toml"));
    assert!(!glob_matches("*.toml", "d/a.toml"));
    assert!(glob_matches("a*c*e", "abcde"));
    assert!(!glob_matches("pyproject.toml", "sub/pyproject.toml"));
}

#[test]
fn default_list_hits_nested_and_workflow_files() {
    let f = check(
        &s(&[
            "app/.swiftlint.yml",
            ".github/workflows/ci.yml",
            ".cargo/config.toml",
        ]),
        &protected(),
        "m",
    );
    assert_eq!(f.len(), 3);
}

/// The tools read their settings from any folder (Ruff opens
/// sub/pyproject.toml, and on case-insensitive filesystems .Ruff.toml as .ruff.toml),
/// and a tracked hook folder runs on every commit.
#[test]
fn protected_names_count_anywhere_and_in_any_case() {
    let paths = [
        "sub/pyproject.toml",
        "sub/ruff.toml",
        ".Ruff.toml",
        "sub/clippy.toml",
        "crates/a/.clippy.toml",
        "sub/rustfmt.toml",
        "sub/.rustfmt.toml",
        "App/.SwiftLint.yml",
        "pkg/.swift-format",
        "sub/.coveragerc",
        "sub/setup.cfg",
        "sub/.semgrepignore",
        "sub/.gitleaks.toml",
        "crates/deny.toml",
        "crates/a/.cargo/config.toml",
        ".cargo/config",
        "docs/CODEOWNERS",
        ".githooks/pre-commit",
        ".House.toml",
    ];
    for path in paths {
        assert_eq!(check(&s(&[path]), &protected(), "m").len(), 1, "{path}");
    }
    assert!(check(&s(&["sub/notes.cfg", "src/config.rs"]), &protected(), "m").is_empty());
}

/// The Rust tools' settings at the repo root, where cargo-deny, clippy and rustfmt
/// read them first: `**/` matches zero folders too.
#[test]
fn rust_tool_settings_at_the_root_are_protected() {
    for path in [
        "deny.toml",
        "clippy.toml",
        ".clippy.toml",
        "rustfmt.toml",
        ".rustfmt.toml",
    ] {
        assert_eq!(check(&s(&[path]), &protected(), "m").len(), 1, "{path}");
    }
}

/// The settings that silence ShellCheck (it reads `.shellcheckrc` and `shellcheckrc`
/// in the script's folder and each parent) and gitleaks (`.gitleaksignore` lists
/// findings to skip) are protected, at the root and in any folder.
#[test]
fn shellcheck_and_gitleaks_suppression_files_are_protected() {
    for path in [
        ".shellcheckrc",
        "scripts/.shellcheckrc",
        "shellcheckrc",
        "a/b/shellcheckrc",
        ".gitleaksignore",
        "sub/.gitleaksignore",
        "Sub/.GitleaksIgnore",
    ] {
        assert_eq!(check(&s(&[path]), &protected(), "m").len(), 1, "{path}");
    }
    assert!(
        check(
            &s(&["x.shellcheckrc", "gitleaksignore.md"]),
            &protected(),
            "m"
        )
        .is_empty()
    );
}

/// This repo's CODEOWNERS names every protected glob, as `house init` writes it.
#[test]
fn this_repos_codeowners_lists_every_protected_glob() {
    let text = include_str!("../.github/CODEOWNERS");
    for glob in PROTECTED {
        let pattern = if glob.starts_with("**/") {
            glob.to_string()
        } else {
            format!("/{glob}")
        };
        let line = format!("{pattern} @flight-william");
        assert!(text.lines().any(|l| l == line), "missing: {line}");
    }
    assert_eq!(text.lines().count(), PROTECTED.len() + 1, "{text}");
}
