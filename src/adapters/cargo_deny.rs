//! R2 for Rust dependencies: `cargo deny --offline check`, one finding per error it
//! reports (an advisory, a ban, a license, a source), with the advisory id or the
//! check's code as the tool rule.
//!
//! Runs only when a `Cargo.lock` or `Cargo.toml` is in `ctx.files` and the workspace
//! that file belongs to has a `deny.toml`. Always offline: a commit never waits on the
//! network. With no advisory database cached the run cannot check, and says to run
//! `cargo deny fetch` once.

use super::Exits;
use super::cargo_fmt::manifests;
use crate::adapter::{Adapter, CannotCheck, Command, Context, Output};
use crate::finding::{Finding, Place};
use serde_json::Value;
use std::path::Path;

pub struct CargoDeny;

/// cargo-deny exits 0 when every check passed; otherwise the failed checks as bits
/// (advisories 1, bans 2, licenses 4, sources 8). A bad deny.toml or a missing
/// database also exits 1; those runs print no summary and are told apart below.
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
};

impl Adapter for CargoDeny {
    fn name(&self) -> &'static str {
        "cargo-deny"
    }

    fn program(&self) -> &'static str {
        "cargo-deny"
    }

    fn brew_formula(&self) -> &'static str {
        "cargo-deny"
    }

    fn command(&self, ctx: &Context) -> Option<Command> {
        self.commands(ctx).into_iter().next()
    }

    /// `cargo-deny --offline --locked --format json --manifest-path <ws>/Cargo.toml
    /// --config <ws>/deny.toml check`, one per workspace that has a deny.toml.
    fn commands(&self, ctx: &Context) -> Vec<Command> {
        workspaces(ctx)
            .into_iter()
            .map(|(manifest, config)| {
                let mut args: Vec<String> = ["--offline", "--locked", "--format", "json"]
                    .map(String::from)
                    .to_vec();
                args.extend(["--manifest-path".into(), manifest]);
                args.extend(["--config".into(), config, "check".into()]);
                Command {
                    program: "cargo-deny".into(),
                    args,
                }
            })
            .collect()
    }

    fn parse(&self, ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        // cargo-deny writes its JSON to stderr; stdout is read too, in case it moves.
        let lines: Vec<Value> = (out.stderr.lines().chain(out.stdout.lines()))
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        if !lines.iter().any(|v| v["type"] == "summary") {
            return Err(failed(&lines, &out.stderr));
        }
        let place = lock_place(ctx);
        let findings = lines
            .iter()
            .filter(|v| v["type"] == "diagnostic" && v["fields"]["severity"] == "error")
            .filter_map(|v| finding(&v["fields"], &place))
            .collect();
        EXITS.judge("cargo-deny", out, findings)
    }
}

/// (manifest, deny.toml) per workspace, relative to root: each `Cargo.lock` or
/// `Cargo.toml` in `ctx.files` resolved to its workspace (as cargo fmt and clippy
/// do), kept when that workspace's folder has a `deny.toml`.
fn workspaces(ctx: &Context) -> Vec<(String, String)> {
    let named = |f: &str, name: &str| f == name || f.ends_with(&format!("/{name}"));
    let files = ctx
        .files
        .iter()
        .filter(|f| named(f, "Cargo.lock") || named(f, "Cargo.toml"))
        .map(|f| {
            f.strip_suffix("Cargo.lock")
                .map_or(f.clone(), |d| format!("{d}Cargo.toml"))
        })
        .collect();
    let asked = Context {
        files,
        ..ctx.clone()
    };
    manifests(&asked)
        .into_iter()
        .filter_map(|m| {
            let config = beside(&m, "deny.toml");
            ctx.root.join(&config).is_file().then_some((m, config))
        })
        .collect()
}

/// `name` in the folder of the manifest `manifest`, e.g. "ws/Cargo.toml" gives "ws/<name>".
fn beside(manifest: &str, name: &str) -> String {
    let dir = Path::new(manifest).parent().unwrap_or(Path::new(""));
    dir.join(name).to_string_lossy().replace('\\', "/")
}

/// The workspace's `Cargo.lock` when one workspace was checked. cargo-deny's output
/// does not name its workspace, so with several the finding is on the repo.
fn lock_place(ctx: &Context) -> Place {
    match workspaces(ctx).as_slice() {
        [(manifest, _)] => Place::File(beside(manifest, "Cargo.lock")),
        _ => Place::Repo,
    }
}

/// An error cargo-deny reports: the advisory id as tool rule for an advisory, else
/// the check's code (banned, rejected, ...). `None` for an error with no code.
fn finding(fields: &Value, place: &Place) -> Option<Finding> {
    let code = fields["code"].as_str()?;
    let message = fields["message"]
        .as_str()
        .unwrap_or("")
        .trim_end_matches('.');
    let krate = &fields["graphs"][0]["Krate"];
    let name = krate["name"].as_str().unwrap_or("");
    let (tool_rule, what, fix) = match fields["advisory"]["id"].as_str() {
        Some(id) => (
            id.to_string(),
            format!(
                "{name} {}: {message}",
                krate["version"].as_str().unwrap_or("")
            ),
            format!("update {name} to a fixed version (https://rustsec.org/advisories/{id})"),
        ),
        None => (
            format!("cargo-deny::{code}"),
            message.to_string(),
            "fix the dependency or deny.toml so `cargo deny check` passes".to_string(),
        ),
    };
    Some(Finding {
        rule: "R2".into(),
        place: place.clone(),
        line: None,
        what,
        fix,
        tool_rule,
        measure: None,
    })
}

/// A run that printed no summary did not check: a bad deny.toml, no advisory
/// database cached, a lock file that is not up to date. Says the first error.
fn failed(lines: &[Value], stderr: &str) -> CannotCheck {
    let error = |v: &&Value| {
        let f = &v["fields"];
        f["severity"] == "error" || f["level"] == "ERROR"
    };
    let first = lines
        .iter()
        .find(error)
        .and_then(|v| v["fields"]["message"].as_str());
    let first = first.or_else(|| stderr.lines().find(|l| !l.trim().is_empty()));
    let first = first.unwrap_or("no output").trim().trim_end_matches('.');
    let no_database = first.contains("FETCH_HEAD") || first.contains("advisory database");
    let fix = if no_database {
        "run `cargo deny fetch` once (it needs the network), then run house again"
    } else {
        "fix what cargo deny reports, then run it again"
    };
    CannotCheck {
        what: format!("cargo deny failed: {}", first.lines().next().unwrap_or("")),
        fix: fix.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::Stage;
    use std::path::PathBuf;

    const PLANTED: &str = include_str!("../../tests/fixtures/cargo_deny/planted.stderr");
    const CLEAN: &str = include_str!("../../tests/fixtures/cargo_deny/clean.stderr");
    const BADCONF: &str = include_str!("../../tests/fixtures/cargo_deny/badconf.stderr");
    const NODB: &str = include_str!("../../tests/fixtures/cargo_deny/nodb.stderr");

    fn ctx(root: &str, files: &[&str]) -> Context {
        Context {
            root: PathBuf::from(root),
            stage: Stage::Commit,
            files: files.iter().map(|f| f.to_string()).collect(),
            push_range: None,
            languages: Default::default(),
        }
    }

    fn output(exit: i32, stderr: &str) -> Output {
        Output {
            exit,
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }

    /// Parsed in the workspace `ws/` of a temp repo, with its lock file asked about.
    fn run(exit: i32, stderr: &str) -> Result<Vec<Finding>, CannotCheck> {
        let root = repo(&format!("parse-{exit}-{}", stderr.len()));
        let c = ctx(root.to_str().unwrap(), &["ws/Cargo.lock"]);
        let parsed = CargoDeny.parse(&c, &output(exit, stderr));
        std::fs::remove_dir_all(&root).unwrap();
        parsed
    }

    #[test]
    fn an_advisory_and_a_ban_are_two_r2_findings_on_the_lock_file() {
        let found = run(3, PLANTED).unwrap();
        let got: Vec<_> = found
            .iter()
            .map(|f| (f.rule.as_str(), f.tool_rule.as_str(), f.what.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (
                    "R2",
                    "cargo-deny::banned",
                    "crate 'memchr = 2.8.3' is explicitly banned"
                ),
                (
                    "R2",
                    "RUSTSEC-2099-0001",
                    "itoa 1.0.19: A planted advisory for a test fixture"
                ),
            ]
        );
        let lock = Place::File("ws/Cargo.lock".into());
        assert!(found.iter().all(|f| f.place == lock && f.line.is_none()));
        assert_eq!(
            found[1].fix,
            "update itoa to a fixed version (https://rustsec.org/advisories/RUSTSEC-2099-0001)"
        );
        assert_eq!(
            found[0].fix,
            "fix the dependency or deny.toml so `cargo deny check` passes"
        );
    }

    #[test]
    fn with_two_workspaces_checked_the_finding_is_on_the_repo() {
        let root = repo("two");
        std::fs::write(root.join("other/deny.toml"), "").unwrap();
        let c = ctx(
            root.to_str().unwrap(),
            &["ws/Cargo.lock", "other/Cargo.lock"],
        );
        assert_eq!(CargoDeny.commands(&c).len(), 2);
        let found = CargoDeny.parse(&c, &output(3, PLANTED)).unwrap();
        assert!(found.iter().all(|f| f.place == Place::Repo), "{found:?}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn clean_gives_no_findings() {
        assert_eq!(run(0, CLEAN), Ok(vec![]));
    }

    #[test]
    fn a_bad_deny_toml_cannot_check_with_its_message() {
        let err = run(1, BADCONF).unwrap_err();
        assert!(
            err.what
                .starts_with("cargo deny failed: found 1 unexpected keys"),
            "{}",
            err.what
        );
    }

    #[test]
    fn no_advisory_database_cannot_check_and_says_to_fetch() {
        let err = run(1, NODB).unwrap_err();
        assert!(
            err.what
                .starts_with("cargo deny failed: failed to get 'FETCH_HEAD'"),
            "{}",
            err.what
        );
        assert_eq!(
            err.fix,
            "run `cargo deny fetch` once (it needs the network), then run house again"
        );
    }

    #[test]
    fn a_failed_run_with_no_output_cannot_check() {
        assert!(run(1, "").is_err());
    }

    /// A temp repo: a workspace with deny.toml in ws/, a crate without one in other/.
    fn repo(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("house-deny-{name}-{}", std::process::id()));
        for dir in ["ws/crates/a/src", "other/src"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        let package = "[package]\nname = \"x\"\n";
        for (path, text) in [
            ("ws/Cargo.toml", "[workspace]\nmembers = [\"crates/a\"]\n"),
            ("ws/Cargo.lock", ""),
            ("ws/deny.toml", ""),
            ("ws/crates/a/Cargo.toml", package),
            ("other/Cargo.toml", package),
            ("other/Cargo.lock", ""),
        ] {
            std::fs::write(root.join(path), text).unwrap();
        }
        root
    }

    #[test]
    fn runs_per_workspace_with_a_deny_toml_when_its_manifest_or_lock_is_asked_about() {
        let root = repo("cmd");
        let at = |files: &[&str]| CargoDeny.commands(&ctx(root.to_str().unwrap(), files));
        let want = Command {
            program: "cargo-deny".into(),
            args: [
                "--offline",
                "--locked",
                "--format",
                "json",
                "--manifest-path",
                "ws/Cargo.toml",
                "--config",
                "ws/deny.toml",
                "check",
            ]
            .map(String::from)
            .to_vec(),
        };
        assert_eq!(at(&["ws/Cargo.lock"]), vec![want.clone()]);
        assert_eq!(at(&["ws/crates/a/Cargo.toml", "ws/Cargo.lock"]), vec![want]);
        assert_eq!(at(&["ws/crates/a/src/lib.rs"]), vec![]);
        assert_eq!(at(&["other/Cargo.lock", "other/Cargo.toml"]), vec![]);
        assert_eq!(at(&["README.md"]), vec![]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_deny_toml_at_the_repo_root_is_found() {
        let root = repo("top");
        std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"top\"\n").unwrap();
        std::fs::write(root.join("deny.toml"), "").unwrap();
        let cmds = CargoDeny.commands(&ctx(root.to_str().unwrap(), &["Cargo.lock"]));
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].args[5..8], ["Cargo.toml", "--config", "deny.toml"]);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
