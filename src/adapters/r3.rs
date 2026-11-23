//! R3 size: a file over 400 lines, a Python function over 60.
//!
//! Runs `scripts/r3_count.py`.
//! The script folder is `$HOUSE_SCRIPTS_DIR` when set, else this crate's `scripts`.
//! Functions of .rs and .swift files come from clippy and SwiftLint, not here.

use super::Exits;
use crate::adapter::{Adapter, CannotCheck, Command, Context, Output};
use crate::finding::{Finding, Measure, Place};
use serde::Deserialize;

pub struct R3;

/// The script exits 0 and prints one JSON line per finding; any other exit
/// (a Python error is 1) is a failure.
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[],
};

const EXTENSIONS: [&str; 4] = [".py", ".sh", ".rs", ".swift"];

fn script_path() -> String {
    let dir = std::env::var("HOUSE_SCRIPTS_DIR")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/scripts").to_string());
    format!("{}/r3_count.py", dir.trim_end_matches('/'))
}

#[derive(Deserialize)]
struct Line {
    file: String,
    kind: String,
    #[serde(default)]
    name: String,
    lines: u32,
    limit: u32,
}

impl Adapter for R3 {
    fn name(&self) -> &'static str {
        "r3"
    }

    fn program(&self) -> &'static str {
        "python3"
    }

    fn brew_formula(&self) -> &'static str {
        "python"
    }

    fn command(&self, ctx: &Context) -> Option<Command> {
        let files: Vec<String> = ctx
            .files
            .iter()
            .filter(|f| {
                EXTENSIONS.iter().any(|ext| f.ends_with(ext)) || ctx.languages.contains_key(*f)
            })
            .cloned()
            .collect();
        if files.is_empty() {
            return None;
        }
        let mut args = vec!["-I".to_string(), script_path()];
        args.extend(files);
        Some(Command {
            program: "python3".to_string(),
            args,
        })
    }

    fn parse(&self, _ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        let broken = || CannotCheck {
            what: format!(
                "the R3 size script exited {} with unreadable output",
                out.exit
            ),
            fix: "run the python3 command by hand and fix what it prints".to_string(),
        };
        let findings = out
            .stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                serde_json::from_str::<Line>(l)
                    .map_err(|_| broken())
                    .map(finding)
            })
            .collect::<Result<Vec<_>, _>>()?;
        EXITS.judge("r3", out, findings)
    }
}

fn finding(item: Line) -> Finding {
    let is_function = item.kind == "function";
    let (what, fix, tool_rule) = if is_function {
        (
            format!(
                "function {} has {} lines, limit is {}",
                item.name, item.lines, item.limit
            ),
            "split the function into smaller functions".to_string(),
            "r3 function_lines".to_string(),
        )
    } else {
        (
            format!("file has {} lines, limit is {}", item.lines, item.limit),
            "split the file into smaller modules".to_string(),
            "r3 file_lines".to_string(),
        )
    };
    Finding {
        rule: "R3".to_string(),
        place: Place::File(item.file),
        line: None,
        what,
        fix,
        tool_rule,
        measure: Some(Measure {
            value: f64::from(item.lines),
            unit: "lines".to_string(),
            limit: f64::from(item.limit),
        }),
    }
}
