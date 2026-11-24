//! gitleaks: secrets (R2). At commit it scans the staged changes; at push, the
//! pushed commits (`ctx.push_range`), or all history when no range is known.
//!
//! gitleaks reports a secret at the path whose diff adds its line. With git's rename
//! detection a `git mv` adds no line, so a secret moved out of an excluded folder
//! would be reported, and dropped, at its old path only. gitleaks runs with
//! `diff.renames` off: a rename is a delete and an add, and the secret is reported at
//! the path it now has, where `[paths] exclude` is judged.
//!
//! For a file `.gitattributes` marks `-diff` or `binary`, or gives a diff driver with
//! a textconv, git prints "Binary files differ" or the driver's text, not the file,
//! so a secret in it would never be read. gitleaks runs with `GIT_ATTR_SOURCE` set to
//! the empty tree: git reads no attributes, and every file's own text is scanned.
//! Captured with gitleaks 8.30.1.

use super::Exits;
use crate::adapter::{Adapter, CannotCheck, Command, Context, Output, Stage};
use crate::finding::{Finding, Place};
use crate::run;
use serde::Deserialize;
use std::path::Path;

pub struct Gitleaks;

/// gitleaks exits 0 with no leaks and 1 with leaks (its default --exit-code).
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[1],
};

/// The first `ERR` log line on stderr, without its colour codes, time and level:
/// gitleaks logs a failed `git log` (a bad range) and still exits 0 with `[]`.
fn logged_error(stderr: &str) -> Option<String> {
    stderr.lines().find_map(|line| {
        let mut plain = String::new();
        let mut chars = line.chars();
        while let Some(c) = chars.next() {
            if c == '\u{1b}' {
                chars.by_ref().find(|c| c.is_ascii_alphabetic());
            } else {
                plain.push(c);
            }
        }
        let mut words = plain.split_whitespace();
        let _time = words.next();
        (words.next() == Some("ERR")).then(|| words.collect::<Vec<_>>().join(" "))
    })
}

#[derive(Deserialize)]
struct Leak {
    #[serde(rename = "RuleID")]
    rule_id: String,
    #[serde(rename = "File")]
    file: String,
    #[serde(rename = "StartLine")]
    start_line: u32,
}

/// git's rename detection off for the `git diff` and `git log` gitleaks runs.
const NO_RENAMES: &[(&str, &str)] = &[
    ("GIT_CONFIG_COUNT", "1"),
    ("GIT_CONFIG_KEY_0", "diff.renames"),
    ("GIT_CONFIG_VALUE_0", "false"),
];

/// git's empty tree in a SHA-1 repo; a SHA-256 repo has its own, which git gives.
const EMPTY_TREE_SHA1: &str = "4b825dc642cb6eb9a060e64bf8d69288fbee4904";

/// The empty tree's id in the repo at `root`: as `GIT_ATTR_SOURCE`, the tree git reads
/// attributes from instead of the working tree's `.gitattributes`.
fn empty_tree(root: &Path) -> String {
    run::git(root, &["hash-object", "-t", "tree", "/dev/null"])
        .map(|out| out.stdout.trim().to_string())
        .ok()
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| EMPTY_TREE_SHA1.to_string())
}

impl Adapter for Gitleaks {
    fn name(&self) -> &'static str {
        "gitleaks"
    }

    fn env(&self, ctx: &Context) -> Vec<(String, String)> {
        let mut env: Vec<(String, String)> = NO_RENAMES
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        env.push(("GIT_ATTR_SOURCE".into(), empty_tree(&ctx.root)));
        env
    }

    fn program(&self) -> &'static str {
        "gitleaks"
    }

    fn brew_formula(&self) -> &'static str {
        "gitleaks"
    }

    fn command(&self, ctx: &Context) -> Option<Command> {
        let scope = match (ctx.stage, &ctx.push_range) {
            (Stage::Commit, _) => vec!["--staged".to_string()],
            (Stage::Push, Some(range)) => vec![format!("--log-opts={range}")],
            (Stage::Push, None) => vec![],
        };
        let rest = [
            "--report-format",
            "json",
            "--report-path",
            "-",
            "--no-banner",
            "--redact",
        ];
        let mut args = vec!["git".to_string()];
        args.extend(scope);
        args.extend(rest.iter().map(|a| a.to_string()));
        Some(Command {
            program: "gitleaks".into(),
            args,
        })
    }

    fn parse(&self, _ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        let cannot = |why: String| CannotCheck {
            what: why,
            fix: "run the gitleaks git command by hand and read its error".into(),
        };
        let leaks: Vec<Leak> = serde_json::from_str(out.stdout.trim()).map_err(|e| {
            let err = out.stderr.lines().last().unwrap_or("").trim();
            cannot(format!(
                "gitleaks gave no report (exit {}): {e} {err}",
                out.exit
            ))
        })?;
        if let Some(error) = logged_error(&out.stderr) {
            return Err(cannot(format!("gitleaks logged an error: {error}")));
        }
        let findings = leaks
            .into_iter()
            .map(|l| Finding {
                rule: "R2".into(),
                place: Place::File(l.file),
                line: Some(l.start_line),
                what: format!("possible secret: {}", l.rule_id),
                fix: "remove the secret and rotate it; keep secrets in the Keychain".into(),
                tool_rule: format!("gitleaks {}", l.rule_id),
                measure: None,
            })
            .collect();
        EXITS.judge("gitleaks", out, findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Context {
        Context {
            root: "/r".into(),
            stage: Stage::Commit,
            files: vec!["conf.txt".into()],
            push_range: None,
            languages: Default::default(),
        }
    }

    fn out(exit: i32, stdout: &str) -> Output {
        Output {
            exit,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    #[test]
    fn planted_secret_gives_one_finding_without_the_secret() {
        let json = include_str!("../../tests/fixtures/gitleaks/leak.json");
        let f = Gitleaks.parse(&ctx(), &out(1, json)).unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "R2");
        assert_eq!(f[0].place, Place::File("conf.txt".into()));
        assert_eq!(f[0].line, Some(1));
        assert_eq!(f[0].what, "possible secret: aws-access-token");
        assert_eq!(f[0].tool_rule, "gitleaks aws-access-token");
        assert!(!format!("{:?}", f[0]).contains("AKIA"));
    }

    #[test]
    fn clean_gives_none() {
        let json = include_str!("../../tests/fixtures/gitleaks/clean.json");
        assert!(Gitleaks.parse(&ctx(), &out(0, json)).unwrap().is_empty());
    }

    #[test]
    fn garbage_is_cannot_check() {
        assert!(Gitleaks.parse(&ctx(), &out(1, "")).is_err());
    }

    #[test]
    fn command_is_staged_json_to_stdout() {
        let c = Gitleaks.command(&ctx()).unwrap();
        assert_eq!(c.program, "gitleaks");
        assert_eq!(c.args[..2], ["git", "--staged"]);
        assert!(c.args.contains(&"--redact".to_string()));
    }

    #[test]
    fn push_scans_the_pushed_range_not_the_index() {
        let push = Context {
            stage: Stage::Push,
            push_range: Some("abc..def".into()),
            languages: Default::default(),
            ..ctx()
        };
        let c = Gitleaks.command(&push).unwrap();
        assert_eq!(c.args[..2], ["git", "--log-opts=abc..def"]);
        assert!(!c.args.contains(&"--staged".to_string()));
    }

    #[test]
    fn push_without_a_range_scans_all_history_not_the_index() {
        let push = Context {
            stage: Stage::Push,
            ..ctx()
        };
        let c = Gitleaks.command(&push).unwrap();
        assert_eq!(c.args[0], "git");
        assert!(
            !c.args
                .iter()
                .any(|a| a == "--staged" || a.starts_with("--log-opts"))
        );
    }

    /// gitleaks logs git's error and still exits 0 with `[]` (a bad --log-opts range).
    #[test]
    fn an_err_log_line_with_exit_0_is_cannot_check() {
        let err_line = "\u{1b}[90m10:33PM\u{1b}[0m \u{1b}[31mERR\u{1b}[0m \u{1b}[1m[git] fatal: ambiguous argument 'a..b'\u{1b}[0m\n";
        let info =
            "\u{1b}[90m10:33PM\u{1b}[0m \u{1b}[32mINF\u{1b}[0m \u{1b}[1mno leaks found\u{1b}[0m\n";
        let bad = Output {
            exit: 0,
            stdout: "[]\n".into(),
            stderr: format!("{err_line}{info}"),
        };
        let err = Gitleaks.parse(&ctx(), &bad).unwrap_err();
        assert!(err.what.contains("fatal: ambiguous argument"), "{err:?}");
        let fine = Output {
            stderr: info.into(),
            ..bad
        };
        assert_eq!(Gitleaks.parse(&ctx(), &fine), Ok(vec![]));
    }
}
