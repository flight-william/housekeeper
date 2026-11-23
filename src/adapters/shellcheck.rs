//! shellcheck: shell scripts (R2). Captured with ShellCheck 0.11.0.
//! Files by the `.sh` extension, or a shell first line (`Context::languages`).

use super::Exits;
use crate::adapter::{Adapter, CannotCheck, Command, Context, Language, Output};
use crate::finding::{Finding, Place};
use serde::Deserialize;

pub struct Shellcheck;

/// ShellCheck exits 0 clean, 1 with comments; 2 (a file it could not read),
/// 3 (bad options) and 4 (bad features) are failures.
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[1],
};

#[derive(Deserialize)]
struct Report {
    comments: Vec<Comment>,
}

#[derive(Deserialize)]
struct Comment {
    file: String,
    line: u32,
    code: u32,
    message: String,
}

impl Adapter for Shellcheck {
    fn name(&self) -> &'static str {
        "shellcheck"
    }

    fn program(&self) -> &'static str {
        "shellcheck"
    }

    fn brew_formula(&self) -> &'static str {
        "shellcheck"
    }

    fn command(&self, ctx: &Context) -> Option<Command> {
        let files: Vec<String> = ctx
            .files
            .iter()
            .filter(|f| f.ends_with(".sh") || ctx.first_line_is(f, Language::Shell))
            .cloned()
            .collect();
        if files.is_empty() {
            return None;
        }
        let mut args = vec!["-f".to_string(), "json1".to_string()];
        args.extend(files);
        Some(Command {
            program: "shellcheck".into(),
            args,
        })
    }

    fn parse(&self, _ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        let cannot = |what: String| CannotCheck {
            what,
            fix: "run shellcheck by hand and read its error".into(),
        };
        let report: Report = serde_json::from_str(out.stdout.trim()).map_err(|e| {
            let err = out.stderr.lines().next().unwrap_or("").trim();
            cannot(format!(
                "shellcheck gave no report (exit {}): {e} {err}",
                out.exit
            ))
        })?;
        let findings = report
            .comments
            .into_iter()
            .map(|c| Finding {
                rule: "R2".into(),
                place: Place::File(c.file),
                line: Some(c.line),
                what: c.message.trim_end_matches('.').to_string(),
                fix: format!("see https://www.shellcheck.net/wiki/SC{}", c.code),
                tool_rule: format!("shellcheck SC{}", c.code),
                measure: None,
            })
            .collect();
        EXITS.judge("shellcheck", out, findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::Stage;

    fn ctx(files: &[&str]) -> Context {
        Context {
            root: "/r".into(),
            stage: Stage::Commit,
            files: files.iter().map(|f| f.to_string()).collect(),
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
    fn planted_problems_give_findings() {
        let json = include_str!("../../tests/fixtures/shellcheck/planted.json");
        let f = Shellcheck.parse(&ctx(&["a.sh"]), &out(1, json)).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].rule, "R2");
        assert_eq!(f[0].place, Place::File("a.sh".into()));
        assert_eq!(f[0].line, Some(2));
        assert_eq!(
            f[0].what,
            "Double quote to prevent globbing and word splitting"
        );
        assert_eq!(f[0].fix, "see https://www.shellcheck.net/wiki/SC2086");
        assert_eq!(f[1].tool_rule, "shellcheck SC2046");
    }

    #[test]
    fn clean_gives_none() {
        let json = include_str!("../../tests/fixtures/shellcheck/clean.json");
        assert!(
            Shellcheck
                .parse(&ctx(&["a.sh"]), &out(0, json))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn exit_2_is_cannot_check() {
        assert!(
            Shellcheck
                .parse(&ctx(&["a.sh"]), &out(2, r#"{"comments":[]}"#))
                .is_err()
        );
    }

    #[test]
    fn only_sh_files_are_passed() {
        assert!(Shellcheck.command(&ctx(&["a.py"])).is_none());
        let c = Shellcheck.command(&ctx(&["a.py", "x/b.sh"])).unwrap();
        assert_eq!(c.args, ["-f", "json1", "x/b.sh"]);
    }
}
