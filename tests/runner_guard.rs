//! house refuses to run on a self-hosted runner for a public repository: there,
//! anyone's pull request code runs on the runner's machine. The check reads the
//! event payload at GITHUB_EVENT_PATH and happens before any tool runs.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const REFUSAL: &str = "house: refusing to run: a public repository on a self-hosted runner can run anyone's pull request code on this machine. fix: use GitHub-hosted runners for public repositories\n";

/// Where the run happens: (GITHUB_ACTIONS, RUNNER_ENVIRONMENT, GITHUB_EVENT_PATH).
struct Runner<'a> {
    actions: Option<&'a str>,
    environment: Option<&'a str>,
    event: Option<&'a Path>,
}

fn house(repo: &Path, args: &[&str], runner: &Runner, path_env: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_house"));
    cmd.args(args)
        .current_dir(repo)
        .env("HOUSE_LOG_DIR", common::fresh("guard-log"))
        .env_remove("GITHUB_ACTIONS")
        .env_remove("RUNNER_ENVIRONMENT")
        .env_remove("GITHUB_EVENT_PATH");
    if let Some(v) = runner.actions {
        cmd.env("GITHUB_ACTIONS", v);
    }
    if let Some(v) = runner.environment {
        cmd.env("RUNNER_ENVIRONMENT", v);
    }
    if let Some(p) = runner.event {
        cmd.env("GITHUB_EVENT_PATH", p);
    }
    if let Some(p) = path_env {
        cmd.env("PATH", p);
    }
    cmd.output().unwrap()
}

/// An event payload whose repository has these JSON fields.
fn payload(name: &str, repository: &str) -> PathBuf {
    let path = common::fresh(name).join("event.json");
    std::fs::write(&path, format!("{{\"repository\": {repository}}}")).unwrap();
    path
}

fn public() -> PathBuf {
    payload(
        "guard-public",
        r#"{"private": false, "visibility": "public"}"#,
    )
}

fn self_hosted(event: &Path) -> Runner<'_> {
    Runner {
        actions: Some("true"),
        environment: Some("self-hosted"),
        event: Some(event),
    }
}

fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn assert_runs(out: &Output) {
    assert_eq!(out.status.code(), Some(0), "{}", text(out));
    assert!(
        text(out).starts_with("house commit: nothing to fix ("),
        "{}",
        text(out)
    );
}

#[test]
fn a_public_repository_on_a_self_hosted_runner_is_refused_at_every_stage() {
    let repo = common::repo("guard-public");
    let event = public();
    for args in [
        &["check", "--stage", "commit"][..],
        &["check", "--stage", "push"],
        &["check", "--all"],
        &["check", "--all", "--json"],
    ] {
        let out = house(&repo, args, &self_hosted(&event), None);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", text(&out));
        assert_eq!(text(&out), REFUSAL, "{args:?}");
    }
}

#[test]
fn visibility_public_alone_is_refused() {
    let repo = common::repo("guard-visibility");
    let event = payload("guard-visibility", r#"{"visibility": "public"}"#);
    let out = house(
        &repo,
        &["check", "--stage", "commit"],
        &self_hosted(&event),
        None,
    );
    assert_eq!(text(&out), REFUSAL);
}

#[test]
fn a_private_repository_on_a_self_hosted_runner_runs() {
    let repo = common::repo("guard-private");
    for repository in [
        r#"{"private": true, "visibility": "private"}"#,
        r#"{"private": true, "visibility": "internal"}"#,
        r#"{"private": true}"#,
    ] {
        let event = payload("guard-private", repository);
        let out = house(
            &repo,
            &["check", "--stage", "commit"],
            &self_hosted(&event),
            None,
        );
        assert_runs(&out);
    }
}

#[test]
fn a_public_repository_on_a_github_hosted_runner_runs() {
    let repo = common::repo("guard-hosted");
    let event = public();
    let runner = Runner {
        actions: Some("true"),
        environment: Some("github-hosted"),
        event: Some(&event),
    };
    assert_runs(&house(
        &repo,
        &["check", "--stage", "commit"],
        &runner,
        None,
    ));
}

#[test]
fn outside_actions_house_runs() {
    let repo = common::repo("guard-outside");
    let event = public();
    // RUNNER_ENVIRONMENT and a payload alone, without GITHUB_ACTIONS=true, are not Actions.
    let runner = Runner {
        actions: None,
        environment: Some("self-hosted"),
        event: Some(&event),
    };
    assert_runs(&house(
        &repo,
        &["check", "--stage", "commit"],
        &runner,
        None,
    ));
    let bare = Runner {
        actions: None,
        environment: None,
        event: None,
    };
    assert_runs(&house(&repo, &["check", "--stage", "commit"], &bare, None));
}

#[test]
fn a_missing_or_unreadable_payload_on_a_self_hosted_runner_is_refused() {
    let repo = common::repo("guard-missing");
    let dir = common::fresh("guard-missing");
    let absent = dir.join("absent.json");
    let garbled = dir.join("garbled.json");
    std::fs::write(&garbled, "{not json").unwrap();
    let silent = payload("guard-silent", r#"{"name": "x"}"#);
    for event in [&absent, &garbled, &silent] {
        let out = house(
            &repo,
            &["check", "--stage", "commit"],
            &self_hosted(event),
            None,
        );
        let said = text(&out);
        assert_eq!(out.status.code(), Some(2), "{said}");
        assert_eq!(said.lines().count(), 1, "{said}");
        assert!(said.starts_with("house: cannot check: "), "{said}");
        assert!(said.contains("GITHUB_EVENT_PATH"), "{said}");
    }
    let unset = Runner {
        actions: Some("true"),
        environment: Some("self-hosted"),
        event: None,
    };
    let out = house(&repo, &["check", "--stage", "commit"], &unset, None);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("GITHUB_EVENT_PATH"), "{}", text(&out));
}

/// A planted `ruff` on PATH that records each call in `calls`, then reports nothing.
fn planted_ruff(calls: &Path) -> String {
    let bin = common::fresh("guard-bin");
    let ruff = bin.join("ruff");
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> '{}'\necho '[]'\n",
        calls.display()
    );
    std::fs::write(&ruff, script).unwrap();
    let mut mode = std::fs::metadata(&ruff).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut mode, 0o755);
    std::fs::set_permissions(&ruff, mode).unwrap();
    format!("{}:/usr/bin:/bin", bin.display())
}

#[test]
fn the_refusal_comes_before_any_tool_runs() {
    let repo = common::repo("guard-first");
    std::fs::write(repo.join("a.py"), "x = 1\n").unwrap();
    common::git(&repo, &["add", "a.py"]);
    let calls = common::fresh("guard-calls").join("calls");
    let path = planted_ruff(&calls);
    // The plant works: on a private repository it is called.
    let private = payload("guard-first", r#"{"private": true}"#);
    house(
        &repo,
        &["check", "--stage", "commit"],
        &self_hosted(&private),
        Some(&path),
    );
    assert!(calls.exists(), "the planted ruff was never called");
    std::fs::remove_file(&calls).unwrap();
    let event = public();
    for args in [&["check", "--stage", "commit"][..], &["check", "--all"]] {
        let out = house(&repo, args, &self_hosted(&event), Some(&path));
        assert_eq!(text(&out), REFUSAL, "{args:?}");
        assert!(!calls.exists(), "{args:?}: a tool ran before the refusal");
    }
}
