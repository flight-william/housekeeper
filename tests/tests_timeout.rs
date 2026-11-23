//! A time limit for each `[tests]` command: `timeout_seconds` in `[tests]`, 600 when
//! unset. A command still running at the limit is stopped, and that is an R7 finding
//! naming the command and the limit, never a hang and never a "cannot check". Each
//! suite here sleeps 30 s under a 1 s limit: only a stopped one gives the finding.

mod common;

use house::adapter::Stage;
use house::finding::Place;
use house::reply::Report;
use house::{cli, config};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn repo_with(name: &str, toml: &str) -> PathBuf {
    let dir = common::repo(name);
    std::fs::write(dir.join(".house.toml"), toml).unwrap();
    common::git(&dir, &["add", ".house.toml"]);
    common::git(&dir, &["commit", "-q", "--no-verify", "-m", "config"]);
    std::fs::write(dir.join("a.txt"), "a\n").unwrap();
    common::git(&dir, &["add", "a.txt"]);
    dir
}

fn check(root: &Path) -> Report {
    cli::check(root, Stage::Commit, Vec::new())
}

/// A suite that waits on something that never comes (a dialog no one clicks).
const WAITS: &str = r#"["sleep", "30"]"#;

#[test]
fn a_suite_past_its_limit_is_one_r7_finding_naming_it_and_the_limit() {
    let toml = format!("[tools]\ncommit = []\n[tests]\ntimeout_seconds = 1\ncommit = [{WAITS}]\n");
    let root = repo_with("timeout-finding", &toml);
    let r = check(&root);
    assert!(r.cannots().is_empty(), "{}", r.text());
    let found = r.findings();
    assert_eq!(found.len(), 1, "{}", r.text());
    let f = found[0];
    assert_eq!(
        (f.rule.as_str(), &f.place, f.what.as_str()),
        (
            "R7",
            &Place::Repo,
            "test command did not finish in 1 s: sleep 30"
        )
    );
    assert!(f.fix.contains("timeout_seconds"), "{}", f.fix);
    assert_eq!(f.tool_rule, "house tests timeout");
    assert_eq!(r.exit(), 1);
}

#[test]
fn a_suite_whose_child_holds_the_output_open_still_stops_at_the_limit() {
    let toml = format!(
        "[tools]\ncommit = []\n[tests]\ntimeout_seconds = 1\ncommit = [{}]\n",
        r#"["sh", "-c", "sleep 30; true"]"#
    );
    let root = repo_with("timeout-child", &toml);
    let r = check(&root);
    let whats: Vec<&str> = r.findings().iter().map(|f| f.what.as_str()).collect();
    assert_eq!(
        whats,
        ["test command did not finish in 1 s: sh -c sleep 30; true"]
    );
}

#[test]
fn the_other_suites_still_run_after_one_times_out() {
    let toml = format!(
        "[tools]\ncommit = []\n[tests]\ntimeout_seconds = 1\ncommit = [{WAITS}, [\"false\"]]\n"
    );
    let root = repo_with("timeout-others", &toml);
    let mut whats: Vec<String> = check(&root)
        .findings()
        .iter()
        .map(|f| f.what.clone())
        .collect();
    whats.sort();
    assert_eq!(
        whats,
        [
            "test command did not finish in 1 s: sleep 30",
            "test command failed: false"
        ]
    );
}

#[test]
fn the_limit_is_600_seconds_unless_set() {
    let unset = config::parse("[tests]\ncommit = [[\"true\"]]\n").unwrap();
    assert_eq!(config::test_limit(&unset), Duration::from_secs(600));
    let set = config::parse("[tests]\ntimeout_seconds = 90\n").unwrap();
    assert_eq!(config::test_limit(&set), Duration::from_secs(90));
}

#[test]
fn a_limit_below_one_second_cannot_be_checked() {
    let err = config::parse("[tests]\ntimeout_seconds = 0\n").unwrap_err();
    assert!(err.what.contains("timeout_seconds"), "{}", err.what);
    assert!(config::parse("[tests]\ntimeout_seconds = -5\n").is_err());
}

/// Whether process `pid` is alive. house waits for a stopped suite's whole group to
/// be gone before it reports, so one look is enough.
fn alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success()
}

/// Run `script` as a suite under a 2 s limit; the grandchild it starts writes its pid
/// to `grandchild.pid`. Gives the timeout findings and whether the grandchild lives.
fn grandchild_after_limit(name: &str, script: &str) -> (Vec<String>, bool) {
    let toml = format!(
        "[tools]\ncommit = []\n[tests]\ntimeout_seconds = 2\ncommit = [[\"sh\", \"-c\", {script:?}]]\n"
    );
    let root = repo_with(name, &toml);
    let whats = check(&root)
        .findings()
        .iter()
        .map(|f| f.what.clone())
        .collect();
    let pid = std::fs::read_to_string(root.join("grandchild.pid")).unwrap();
    let pid = pid.trim().to_string();
    let lives = alive(&pid);
    if lives {
        let killed = std::process::Command::new("kill")
            .args(["-9", &pid])
            .status();
        assert!(
            killed.unwrap().success(),
            "the grandchild {pid} could not be killed"
        );
    }
    (whats, lives)
}

#[test]
fn a_suite_stopped_at_its_limit_leaves_no_grandchild_alive() {
    // A helper the suite started and waits on, as a hung code-signing helper.
    let script = "sleep 30 & echo $! > grandchild.pid; wait";
    let (whats, lives) = grandchild_after_limit("timeout-group", script);
    assert_eq!(
        whats,
        [format!(
            "test command did not finish in 2 s: sh -c {script}"
        )]
    );
    assert!(!lives, "the suite's grandchild outlived the time limit");
}

#[test]
fn a_grandchild_that_ignores_sigterm_is_killed_after_the_grace() {
    let script = "trap '' TERM; sleep 30 & echo $! > grandchild.pid; wait";
    let (whats, lives) = grandchild_after_limit("timeout-group-term", script);
    assert_eq!(whats.len(), 1, "{whats:?}");
    assert!(
        !lives,
        "a grandchild that ignores SIGTERM outlived the limit"
    );
}

#[test]
fn a_suite_that_exits_but_leaves_a_grandchild_holding_its_output_stops_it() {
    let script = "sleep 30 & echo $! > grandchild.pid";
    let (whats, lives) = grandchild_after_limit("timeout-group-left", script);
    assert_eq!(
        whats,
        [format!(
            "test command did not finish in 2 s: sh -c {script}"
        )]
    );
    assert!(
        !lives,
        "the grandchild holding the output outlived the limit"
    );
}
