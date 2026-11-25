//! `house check --stage merge`: the runner's check of a pull request. It runs what
//! push runs, by the same old-shape keys (`push`, falling back to `commit`), on the
//! same ref lines, so a runner can switch from `--stage push` with no other change.

mod merge_support;

use house::adapter::Stage;
use house::cli::{Cmd, parse};
use merge_support::{GOOD, R3_ONLY, ZERO, based, commit, findings, git, line, merge, run, sha};

fn args(s: &[&str]) -> Vec<String> {
    s.iter().map(|x| x.to_string()).collect()
}

#[test]
fn merge_is_a_stage_and_takes_config_from() {
    assert_eq!(
        parse(&args(&["check", "--stage", "merge"])),
        Ok(Cmd::Check {
            stage: Stage::Merge,
            json: false,
            config_from: None,
        })
    );
    assert_eq!(
        parse(&args(&[
            "check",
            "--stage",
            "merge",
            "--config-from",
            "main",
            "--json"
        ])),
        Ok(Cmd::Check {
            stage: Stage::Merge,
            json: true,
            config_from: Some("main".into()),
        })
    );
    assert!(parse(&args(&["check", "--stage", "merge", "--config-from", "-x"])).is_err());
    assert!(parse(&args(&["check", "--stage", "merge", "--all"])).is_err());
}

// The same findings, the same exit, for push and merge on the same ref line; only
// the header's stage differs.
#[test]
fn merge_gives_what_push_gives() {
    let (dir, base) = based("merge-same", &[(".house.toml", R3_ONLY)]);
    let long = "x = 1\n".repeat(401);
    let local = commit(&dir, &[("app.py", &long), ("ok.py", "y = 2\n")], GOOD);
    let push = run(
        &dir,
        "push",
        &["--config-from", &base],
        &line(&local, &base),
        &[],
    );
    let merged = merge(&dir, &base, &local);
    assert_eq!(push.code, 1, "{}", push.out);
    assert_eq!(merged.code, push.code, "{}", merged.out);
    assert_eq!(findings(&merged.out), findings(&push.out));
    assert!(
        merged.out.starts_with("house merge: 1 to fix"),
        "{}",
        merged.out
    );
    assert!(push.out.starts_with("house push: 1 to fix"), "{}", push.out);
    // A clean change is clean at both.
    let local = commit(&dir, &[("app.py", "x = 1\n")], GOOD);
    assert_eq!(merge(&dir, &base, &local).code, 0);
}

// With no `push` key, merge falls back to `commit`, as push does.
#[test]
fn merge_falls_back_to_the_commit_tools() {
    let (dir, base) = based(
        "merge-commit-key",
        &[(".house.toml", "[tools]\ncommit = [\"r3\"]\n")],
    );
    let local = commit(&dir, &[("app.py", &"x = 1\n".repeat(401))], GOOD);
    let out = merge(&dir, &base, &local);
    assert_eq!(out.code, 1, "{}", out.out);
    assert!(out.out.contains("R3 app.py: file has"), "{}", out.out);
}

#[test]
fn merge_names_its_stage_in_json_and_the_record() {
    let (dir, base) = based("merge-json", &[(".house.toml", R3_ONLY)]);
    let local = commit(&dir, &[("a.py", "x = 1\n")], GOOD);
    let out = run(
        &dir,
        "merge",
        &["--config-from", &base, "--json"],
        &line(&local, &base),
        &[],
    );
    assert_eq!(out.code, 0, "{}{}", out.out, out.err);
    let value: serde_json::Value = serde_json::from_str(out.out.trim()).unwrap();
    assert_eq!(value["stage"], "merge");
    assert!(out.record.contains("\"stage\":\"merge\""), "{}", out.record);
}

// A new branch (zero remote sha) is compared with its merge-base with origin/main,
// as at push.
#[test]
fn a_new_branch_at_merge_is_judged_from_its_merge_base() {
    let (dir, base) = based("merge-new-branch", &[(".house.toml", R3_ONLY)]);
    git(&dir, &["update-ref", "refs/remotes/origin/main", &base]);
    let local = commit(&dir, &[("app.py", &"x = 1\n".repeat(401))], GOOD);
    let out = run(
        &dir,
        "merge",
        &["--config-from", &base],
        &line(&local, ZERO),
        &[],
    );
    assert_eq!(out.code, 1, "{}", out.out);
    assert!(out.out.contains("R3 app.py: file has"), "{}", out.out);
    assert_eq!(sha(&dir, "HEAD"), local);
}

// On a laptop merge may run without --config-from, to see what the runner will say;
// in GitHub Actions it is refused without it, so a runner never judges a pull
// request by the pull request's own rules.
#[test]
fn merge_in_actions_needs_config_from() {
    let (dir, base) = based("merge-actions", &[(".house.toml", R3_ONLY)]);
    let local = commit(&dir, &[("a.py", "x = 1\n")], GOOD);
    let laptop = run(&dir, "merge", &[], &line(&local, &base), &[]);
    assert_eq!(laptop.code, 0, "{}", laptop.out);
    let hosted = [
        ("GITHUB_ACTIONS", "true"),
        ("RUNNER_ENVIRONMENT", "github-hosted"),
    ];
    let refused = run(&dir, "merge", &[], &line(&local, &base), &hosted);
    assert_eq!(refused.code, 2, "{}", refused.out);
    assert!(
        refused
            .out
            .contains("--stage merge in GitHub Actions needs --config-from"),
        "{}",
        refused.out
    );
    let ran = run(
        &dir,
        "merge",
        &["--config-from", &base],
        &line(&local, &base),
        &hosted,
    );
    assert_eq!(ran.code, 0, "{}", ran.out);
    // Push keeps its behaviour in Actions.
    let push = run(&dir, "push", &[], &line(&local, &base), &hosted);
    assert_eq!(push.code, 0, "{}", push.out);
}
