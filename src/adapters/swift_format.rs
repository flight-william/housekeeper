//! Apple's `swift format lint --strict`: R1 formatting.
//! It ships with Xcode, not Homebrew.

use super::Exits;
use crate::adapter::{Adapter, CannotCheck, Command, Context, Output};
use crate::finding::{Finding, Place};
use std::path::Path;

pub struct SwiftFormat;

/// `swift format lint --strict` exits 0 clean, 1 with findings (warnings are
/// errors under --strict); anything else is a failure.
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[1],
};

/// One line: `<path>:<line>:<col>: error|warning: [<Rule>] <message>`.
fn parse_line(root: &Path, line: &str) -> Option<Finding> {
    let (head, tail) = [": error: ", ": warning: "]
        .iter()
        .find_map(|sep| line.split_once(sep))?;
    let mut parts = head.rsplitn(3, ':');
    let _col: u32 = parts.next()?.parse().ok()?;
    let line_no: u32 = parts.next()?.parse().ok()?;
    let path = parts.next()?;
    let (rule, message) = tail.strip_prefix('[')?.split_once("] ")?;
    let root = root.to_string_lossy();
    let prefix = format!("{}/", root.trim_end_matches('/'));
    let file = path.strip_prefix(&prefix).unwrap_or(path);
    Some(Finding {
        rule: "R1".to_string(),
        place: Place::File(file.to_string()),
        line: Some(line_no),
        what: format!("format: {}", message.trim_end_matches('.')),
        fix: format!("run swift format --in-place {file}"),
        tool_rule: format!("swift-format {rule}"),
        measure: None,
    })
}

impl Adapter for SwiftFormat {
    fn name(&self) -> &'static str {
        "swift-format"
    }

    fn program(&self) -> &'static str {
        "swift"
    }

    /// Not a Homebrew formula: Apple's swift format comes with Xcode.
    fn brew_formula(&self) -> &'static str {
        "xcode (xcode-select --install)"
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
        let mut args: Vec<String> = ["format", "lint", "--strict"]
            .iter()
            .map(|a| a.to_string())
            .collect();
        args.extend(files);
        Some(Command {
            program: "swift".to_string(),
            args,
        })
    }

    fn parse(&self, ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        let found: Vec<Finding> = out
            .stderr
            .lines()
            .chain(out.stdout.lines())
            .filter_map(|l| parse_line(&ctx.root, l))
            .collect();
        // Exit 1 with findings is findings; failing with nothing readable is "cannot check".
        if found.is_empty() && out.exit != 0 {
            let why = out.stderr.lines().next().unwrap_or("no output");
            return Err(CannotCheck {
                what: format!(
                    "swift format gave no readable result (exit {}): {why}",
                    out.exit
                ),
                fix: "run swift format lint --strict and read its error".to_string(),
            });
        }
        EXITS.judge("swift-format", out, found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::Stage;
    use std::path::PathBuf;

    const PLANTED: &str = include_str!("../../tests/fixtures/swift_format/planted.txt");
    const CLEAN: &str = include_str!("../../tests/fixtures/swift_format/clean.txt");

    fn ctx(files: &[&str]) -> Context {
        Context {
            root: PathBuf::from("/repo"),
            stage: Stage::Commit,
            files: files.iter().map(|f| f.to_string()).collect(),
            push_range: None,
        }
    }

    fn out(exit: i32, stderr: &str) -> Output {
        Output {
            exit,
            stdout: String::new(),
            stderr: stderr.to_string(),
        }
    }

    #[test]
    fn planted_problems_give_r1_findings_on_exit_1() {
        let found = SwiftFormat
            .parse(&ctx(&["Bad.swift"]), &out(1, PLANTED))
            .unwrap();
        assert_eq!(found.len(), PLANTED.lines().count());
        let first = &found[0];
        assert_eq!(first.rule, "R1");
        assert_eq!(first.place, Place::File("Bad.swift".to_string()));
        assert_eq!(first.line, Some(1));
        assert_eq!(first.what, "format: add 1 space");
        assert_eq!(first.fix, "run swift format --in-place Bad.swift");
        assert_eq!(first.tool_rule, "swift-format Spacing");
    }

    #[test]
    fn clean_gives_none() {
        let found = SwiftFormat
            .parse(&ctx(&["Clean.swift"]), &out(0, CLEAN))
            .unwrap();
        assert!(found.is_empty());
    }

    #[test]
    fn absolute_paths_become_relative() {
        let line = "/repo/Sources/A.swift:3:1: warning: [Indentation] unindent by 2 spaces";
        let f = parse_line(Path::new("/repo"), line).unwrap();
        assert_eq!(f.place, Place::File("Sources/A.swift".to_string()));
    }

    #[test]
    fn failure_without_findings_cannot_check() {
        let err = SwiftFormat
            .parse(&ctx(&["A.swift"]), &out(1, "error: unable to find swift"))
            .unwrap_err();
        assert!(err.what.contains("unable to find swift"));
    }

    #[test]
    fn command_runs_only_for_swift_files() {
        assert!(SwiftFormat.command(&ctx(&["a.py"])).is_none());
        let cmd = SwiftFormat.command(&ctx(&["a.py", "B.swift"])).unwrap();
        assert_eq!(cmd.program, "swift");
        assert_eq!(cmd.args.join(" "), "format lint --strict B.swift");
        assert_eq!(SwiftFormat.brew_formula(), "xcode (xcode-select --install)");
    }
}
