//! A file whose name has no known extension is chosen by its first line: a Python or
//! shell script with no `.py` or `.sh` gets the same tools as one with it. A zsh script
//! (ShellCheck cannot read zsh) and a binary file get none. Runs the real tools.

mod common;

use house::adapter::Language;
use house::files::language_of;
use std::path::{Path, PathBuf};

const CONFIG: &str = "[tools]\ncommit = [\"ruff\", \"ruff-format\", \"r3\", \"semgrep\", \"shellcheck\"]\npush = []\n";

/// A repo with `.house.toml` committed and `files` staged as new, each executable.
fn staged(name: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let root = common::repo(name);
    std::fs::write(root.join(".house.toml"), CONFIG).unwrap();
    common::git(&root, &["add", "-A"]);
    common::git(&root, &["commit", "-q", "--no-verify", "-m", "config"]);
    for (path, body) in files {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, body).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    common::git(&root, &["add", "-A"]);
    root
}

/// `house check --stage commit` in `root`: (exit, stdout).
fn commit_check(root: &Path) -> (i32, String) {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "commit"])
        .current_dir(root)
        .env("HOUSE_LOG_DIR", common::fresh("first-line-log"))
        .env_remove("CLAUDECODE")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

/// A Python script with an unused import (ruff), `x=1` (ruff format), a `# noqa`
/// (Semgrep R9) and a 62-line function (r3, which counts Python functions only).
fn python_script(first_line: &str) -> Vec<u8> {
    let body: String = (0..60).map(|i| format!("    print({i})\n")).collect();
    format!("{first_line}\nimport os\n\nx=1\nz = 2  # noqa\n\n\ndef long():\n{body}    return 0\n")
        .into_bytes()
}

/// A shell script with an unquoted `$1` (ShellCheck SC2086) and a
/// `# shellcheck disable` (Semgrep R9).
fn shell_script(first_line: &str) -> Vec<u8> {
    format!("{first_line}\n# shellcheck disable=SC2034\nunused=1\necho $1\n").into_bytes()
}

#[test]
fn an_extensionless_python_script_is_checked_by_ruff_format_r3_and_semgrep() {
    let script = python_script("#!/usr/bin/env python3");
    let root = staged("first-line-py", &[("bin/tool", &script)]);
    let (code, text) = commit_check(&root);
    assert_eq!(code, 1, "{text}");
    for tool_rule in ["[ruff F401]", "[ruff format]", "[r3 function_lines]"] {
        assert!(text.contains(tool_rule), "{tool_rule}: {text}");
    }
    assert!(text.contains("R9 bin/tool:5:"), "semgrep: {text}");
}

#[test]
fn extensionless_shell_scripts_are_checked_by_shellcheck_and_semgrep() {
    let bash = shell_script("#!/usr/bin/env bash");
    let sh = shell_script("#!/bin/sh");
    let bin_bash = shell_script("#!/bin/bash");
    let root = staged(
        "first-line-sh",
        &[("bin/run", &bash), ("bin/plain", &sh), ("bin/b", &bin_bash)],
    );
    let (code, text) = commit_check(&root);
    assert_eq!(code, 1, "{text}");
    for file in ["bin/run", "bin/plain", "bin/b"] {
        assert!(
            text.contains(&format!("R2 {file}:4:")) && text.contains("[shellcheck SC2086]"),
            "shellcheck on {file}: {text}"
        );
        assert!(
            text.contains(&format!("R9 {file}:2:")),
            "semgrep on {file}: {text}"
        );
    }
}

#[test]
fn a_zsh_script_and_a_binary_file_are_given_to_no_tool() {
    let zsh = shell_script("#!/bin/zsh");
    // Starts like a shell script, but its first line is not text.
    let binary: &[u8] = b"#!/bin/sh \0\xff\xfe\x01\x02\necho $1\n# shellcheck disable=SC2034\n";
    let root = staged("first-line-skip", &[("bin/z", &zsh), ("bin/blob", binary)]);
    let (code, text) = commit_check(&root);
    assert_eq!(code, 0, "{text}");
    assert!(text.starts_with("house commit: nothing to fix"), "{text}");
    assert!(!text.contains("bin/"), "{text}");
}

#[test]
fn a_first_line_names_its_language_by_the_interpreter() {
    let python = Some(Language::Python);
    let shell = Some(Language::Shell);
    for (line, want) in [
        ("#!/usr/bin/env python3", python),
        ("#!/usr/bin/python3.13", python),
        ("#!/usr/bin/env -S python3 -u", python),
        ("#!/bin/sh", shell),
        ("#!/bin/bash", shell),
        ("#!/usr/bin/env bash", shell),
        ("#! /bin/sh -e", shell),
        ("#!/bin/zsh", None),
        ("#!/usr/bin/env zsh", None),
        ("#!/usr/bin/env node", Some(Language::Node)),
        ("#!/usr/bin/pythonista", None),
        ("import os", None),
        ("", None),
    ] {
        assert_eq!(language_of(line), want, "{line}");
    }
}
