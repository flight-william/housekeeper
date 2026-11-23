//! SwiftFormat (`swiftformat --lint`, nicklockwood/SwiftFormat): R1 formatting.
//! SwiftFormat reads the repo's `.swiftformat` itself, from each file's folder up,
//! and keeps to its `--exclude` even for files named on its command line.
//! Captured with SwiftFormat 0.64.7.
//!
//! It runs only when `.house.toml` names it: Apple's swift format is the Swift
//! formatter by default, and two formatters on one file disagree.

use super::{Exits, relative};
use crate::adapter::{Adapter, CannotCheck, Command, Context, Output};
use crate::finding::{Finding, Place};
use crate::run::first_line;
use serde::Deserialize;

pub struct Swiftformat;

/// `swiftformat --lint` exits 0 when formatted, 1 when a file needs formatting;
/// 70 is an error (a bad config, a file it cannot parse).
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[1],
};

/// One entry of `--reporter json`.
#[derive(Deserialize)]
struct Violation {
    file: String,
    line: Option<u32>,
    reason: String,
    rule_id: String,
}

impl Adapter for Swiftformat {
    fn name(&self) -> &'static str {
        "swiftformat"
    }

    fn program(&self) -> &'static str {
        "swiftformat"
    }

    fn brew_formula(&self) -> &'static str {
        "swiftformat"
    }

    fn runs_by_default(&self) -> bool {
        false
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
        let mut args: Vec<String> = ["--lint", "--reporter", "json", "--quiet"]
            .iter()
            .map(|a| a.to_string())
            .collect();
        args.extend(files);
        Some(Command {
            program: "swiftformat".into(),
            args,
        })
    }

    fn parse(&self, ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        let violations: Vec<Violation> =
            serde_json::from_str(&out.stdout).map_err(|_| CannotCheck {
                what: format!(
                    "swiftformat gave no readable result (exit {}): {}",
                    out.exit,
                    first_line(&out.stderr)
                ),
                fix: "run swiftformat --lint and read its error".into(),
            })?;
        let findings = violations
            .into_iter()
            .map(|v| {
                let file = relative(&ctx.root, &v.file);
                Finding {
                    rule: "R1".into(),
                    fix: format!("run swiftformat {file}"),
                    place: Place::File(file),
                    line: v.line,
                    what: v.reason.trim_end_matches('.').to_string(),
                    tool_rule: format!("swiftformat {}", v.rule_id),
                    measure: None,
                }
            })
            .collect();
        EXITS.judge("swiftformat", out, findings)
    }
}
