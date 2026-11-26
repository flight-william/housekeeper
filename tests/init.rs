//! `house init` v1: three hooks and CODEOWNERS, each only when absent.

mod common;

use house::init;
use std::os::unix::fs::PermissionsExt;

/// A repo with no hooks: a configured git template may pre-install some.
fn bare_repo(name: &str) -> std::path::PathBuf {
    let repo = common::repo(name);
    let hooks = repo.join(".git/hooks");
    if hooks.exists() {
        for e in std::fs::read_dir(&hooks).unwrap() {
            std::fs::remove_file(e.unwrap().path()).unwrap();
        }
    }
    repo
}

fn run(repo: &std::path::Path, owner: Option<&str>) -> Vec<String> {
    init::init_with_owner(repo, house::adapters::all(), owner).unwrap()
}

#[test]
fn writes_the_three_hooks_report_only_and_executable() {
    let repo = bare_repo("init2-hooks");
    run(&repo, None);
    for (name, line) in [
        ("pre-commit", "house check --stage commit || true"),
        ("commit-msg", "house commit-msg \"$1\" || true"),
        ("pre-push", "house check --stage push || true"),
    ] {
        let path = repo.join(".git/hooks").join(name);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains(line), "{name}: {text}");
        assert!(text.trim_end().ends_with("exit 0"), "{name}: {text}");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755, "{name}");
    }
}

#[test]
fn an_existing_hook_is_left_and_the_line_is_printed() {
    let repo = bare_repo("init2-existing");
    let hooks = repo.join(".git/hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    std::fs::write(hooks.join("commit-msg"), "#!/bin/sh\necho mine\n").unwrap();
    let said = run(&repo, None);
    assert_eq!(
        std::fs::read_to_string(hooks.join("commit-msg")).unwrap(),
        "#!/bin/sh\necho mine\n"
    );
    assert!(
        said.iter()
            .any(|l| l.ends_with("add this line to it: house commit-msg \"$1\" || true")),
        "{said:?}"
    );
    assert!(hooks.join("pre-push").exists());
}

#[test]
fn codeowners_is_written_with_every_protected_path_for_the_owner() {
    let repo = bare_repo("init2-owner");
    let said = run(&repo, Some("@alice"));
    let text = std::fs::read_to_string(repo.join(".github/CODEOWNERS")).unwrap();
    for p in init::PROTECTED {
        let pat = if p.starts_with("**/") {
            p.to_string()
        } else {
            format!("/{p}")
        };
        assert!(text.lines().any(|l| l == format!("{pat} @alice")), "{p}");
    }
    assert!(said.iter().any(|l| l.contains("wrote .github/CODEOWNERS")));
    assert!(
        text.lines().any(|l| l == "**/.swiftformat @alice"),
        "{text}"
    );
}

#[test]
fn codeowners_also_lists_the_house_tomls_extra_globs() {
    let repo = bare_repo("init2-extra");
    std::fs::write(
        repo.join(".house.toml"),
        "[protected]\nextra = [\"policy/*.yml\"]\n",
    )
    .unwrap();
    run(&repo, Some("@alice"));
    let text = std::fs::read_to_string(repo.join(".github/CODEOWNERS")).unwrap();
    assert!(text.lines().any(|l| l == "/policy/*.yml @alice"), "{text}");
}

#[test]
fn a_bare_owner_name_gets_the_at_sign() {
    let repo = bare_repo("init2-bare");
    run(&repo, Some("bob"));
    let text = std::fs::read_to_string(repo.join(".github/CODEOWNERS")).unwrap();
    assert!(text.contains("/.house.toml @bob"), "{text}");
}

#[test]
fn without_an_owner_codeowners_is_not_written_and_the_lines_are_printed() {
    let repo = bare_repo("init2-noowner");
    let said = run(&repo, None);
    assert!(!repo.join(".github/CODEOWNERS").exists());
    let all = said.join("\n");
    assert!(all.contains("no --owner given"), "{all}");
    assert!(all.contains("/.house.toml @<github user>"), "{all}");
}

#[test]
fn an_existing_codeowners_is_never_overwritten() {
    let repo = bare_repo("init2-keep");
    std::fs::create_dir_all(repo.join(".github")).unwrap();
    std::fs::write(repo.join(".github/CODEOWNERS"), "* @team\n").unwrap();
    let said = run(&repo, Some("@alice"));
    assert_eq!(
        std::fs::read_to_string(repo.join(".github/CODEOWNERS")).unwrap(),
        "* @team\n"
    );
    let all = said.join("\n");
    assert!(all.contains("already exists"), "{all}");
    assert!(all.contains("/.house.toml @alice"), "{all}");
}

/// One definition: CODEOWNERS and the approval check read the same
/// list object, not two lists that happen to be equal. The addresses of the statics
/// themselves are compared: the compiler may merge two equal arrays, never two statics.
#[test]
fn init_protected_is_the_approval_list_itself() {
    assert!(std::ptr::eq(&init::PROTECTED, &house::approval::PROTECTED));
}

/// The starter `[tools] commit` list `house init` writes in a repo holding `files`.
fn starter_tools(name: &str, files: &[(&str, &str)]) -> String {
    let repo = bare_repo(name);
    for (path, text) in files {
        let file = repo.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }
    common::git(&repo, &["add", "-A"]);
    run(&repo, None);
    let cfg = std::fs::read_to_string(repo.join(".house.toml")).unwrap();
    house::config::parse(&cfg).unwrap();
    cfg.lines()
        .find(|l| l.starts_with("commit = "))
        .unwrap()
        .to_string()
}

#[test]
fn a_typescript_repo_starts_with_prettier_eslint_tsc_r3_gitleaks_and_semgrep() {
    let line = starter_tools(
        "init-ts",
        &[
            ("package.json", "{}\n"),
            ("tsconfig.json", "{}\n"),
            ("src/app.ts", "export const a = 1;\n"),
        ],
    );
    assert_eq!(
        line,
        r#"commit = ["prettier", "eslint", "tsc", "gitleaks", "semgrep", "r3"]"#
    );
}

#[test]
fn a_package_json_alone_names_the_javascript_tools_before_any_code_exists() {
    let line = starter_tools("init-pkg", &[("package.json", "{}\n")]);
    assert_eq!(
        line,
        r#"commit = ["prettier", "eslint", "tsc", "gitleaks", "semgrep", "r3"]"#
    );
}
