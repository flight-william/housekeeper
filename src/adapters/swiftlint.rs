//! SwiftLint: R2 lint, R3 length, R4 complexity.
//!
//! A file's length is r3's (house's 400 lines, the same for every language), so
//! SwiftLint's `file_length` is dropped: one long file is one finding, with a measure,
//! whatever limit the repo's SwiftLint settings give. Function and type length stay.

use super::Exits;
use crate::adapter::{Adapter, CannotCheck, Command, Context, Output};
use crate::finding::{Finding, Place};
use serde::Deserialize;
use std::path::Path;

pub struct SwiftLint;

/// SwiftLint exits 0 (clean, or warnings only), 2 with serious violations;
/// 1 is a fatal error (bad config, crash); a dyld or abort failure is any other code.
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[2],
};

#[derive(Deserialize)]
struct Violation {
    file: String,
    line: Option<u32>,
    reason: String,
    rule_id: String,
}

/// The SwiftLint rule whose finding r3 owns: a file's length.
const FILE_LENGTH: &str = "file_length";

/// The house rule for a SwiftLint rule id; unknown ones are R2.
fn house_rule(rule_id: &str) -> &'static str {
    match rule_id {
        "function_body_length" | "type_body_length" => "R3",
        "cyclomatic_complexity" | "nesting" | "function_parameter_count" => "R4",
        _ => "R2",
    }
}

/// `file` relative to `root`; SwiftLint prints absolute paths, and /tmp may be /private/tmp.
fn relative(root: &Path, file: &str) -> String {
    let unprivate = |p: &str| p.strip_prefix("/private").unwrap_or(p).to_string();
    let root = unprivate(&root.to_string_lossy());
    let file = unprivate(file);
    let prefix = format!("{}/", root.trim_end_matches('/'));
    file.strip_prefix(&prefix).unwrap_or(&file).to_string()
}

impl Adapter for SwiftLint {
    fn name(&self) -> &'static str {
        "swiftlint"
    }

    fn program(&self) -> &'static str {
        "swiftlint"
    }

    fn brew_formula(&self) -> &'static str {
        "swiftlint"
    }

    fn command(&self, ctx: &Context) -> Option<Command> {
        let files: Vec<String> = ctx
            .files
            .iter()
            .filter(|f| f.ends_with(".swift"))
            .cloned()
            .collect();
        if files.is_empty() {
            return None;
        }
        let mut args: Vec<String> = ["lint", "--reporter", "json", "--quiet"]
            .iter()
            .map(|a| a.to_string())
            .collect();
        args.extend(files);
        Some(Command {
            program: "swiftlint".to_string(),
            args,
        })
    }

    fn parse(&self, ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        let violations: Vec<Violation> = serde_json::from_str(&out.stdout).map_err(|_| {
            let why = out.stderr.lines().next().unwrap_or("no output");
            CannotCheck {
                what: format!(
                    "swiftlint gave no readable result (exit {}): {why}",
                    out.exit
                ),
                fix: "run swiftlint lint and read its error".to_string(),
            }
        })?;
        let findings = violations
            .into_iter()
            .map(|v| Finding {
                rule: house_rule(&v.rule_id).to_string(),
                place: Place::File(relative(&ctx.root, &v.file)),
                line: v.line,
                what: v.reason.trim_end_matches('.').to_string(),
                fix: format!("fix the SwiftLint rule {}", v.rule_id),
                tool_rule: format!("swiftlint {}", v.rule_id),
                measure: None,
            })
            .collect();
        let mut findings = EXITS.judge("swiftlint", out, findings)?;
        findings.retain(|f| f.tool_rule != format!("swiftlint {FILE_LENGTH}"));
        Ok(findings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::Stage;
    use std::path::PathBuf;

    const PLANTED: &str = include_str!("../../tests/fixtures/swiftlint/planted.json");
    const CLEAN: &str = include_str!("../../tests/fixtures/swiftlint/clean.json");
    const ROOT: &str = include_str!("../../tests/fixtures/swiftlint/ROOT.txt");

    fn ctx(files: &[&str]) -> Context {
        Context {
            root: PathBuf::from(ROOT.trim()),
            stage: Stage::Commit,
            files: files.iter().map(|f| f.to_string()).collect(),
            push_range: None,
            languages: Default::default(),
        }
    }

    fn out(exit: i32, stdout: &str, stderr: &str) -> Output {
        Output {
            exit,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
        }
    }

    #[test]
    fn planted_problems_give_findings_on_exit_2() {
        let found = SwiftLint
            .parse(&ctx(&["Many.swift"]), &out(2, PLANTED, ""))
            .unwrap();
        let rules: Vec<(&str, &str)> = found
            .iter()
            .map(|f| (f.rule.as_str(), f.tool_rule.as_str()))
            .collect();
        assert_eq!(
            rules,
            [
                ("R4", "swiftlint cyclomatic_complexity"),
                ("R3", "swiftlint function_body_length"),
            ]
        );
        assert_eq!(found[0].place, Place::File("Many.swift".to_string()));
        assert_eq!(found[0].line, Some(5));
        assert_eq!(found[1].fix, "fix the SwiftLint rule function_body_length");
        assert!(!found[1].what.ends_with('.'));
    }

    #[test]
    fn clean_gives_none() {
        let found = SwiftLint
            .parse(&ctx(&["Clean.swift"]), &out(0, CLEAN, ""))
            .unwrap();
        assert!(found.is_empty());
    }

    #[test]
    fn crash_without_json_cannot_check() {
        let err = SwiftLint
            .parse(&ctx(&["A.swift"]), &out(1, "", "dyld: library not loaded"))
            .unwrap_err();
        assert!(err.what.contains("dyld"));
    }

    #[test]
    fn rules_map_to_house_rules() {
        assert_eq!(house_rule("function_parameter_count"), "R4");
        assert_eq!(house_rule("nesting"), "R4");
        assert_eq!(house_rule("force_cast"), "R2");
    }

    #[test]
    fn command_runs_only_for_swift_files() {
        assert!(SwiftLint.command(&ctx(&["a.py", "b.rs"])).is_none());
        let cmd = SwiftLint
            .command(&ctx(&["a.py", "Sources/B.swift"]))
            .unwrap();
        assert_eq!(cmd.program, "swiftlint");
        assert_eq!(
            cmd.args.join(" "),
            "lint --reporter json --quiet Sources/B.swift"
        );
    }
}
