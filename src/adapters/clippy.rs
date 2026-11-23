//! R2, R3, R4 for Rust: clippy's JSON messages, one finding per lint.
//! The thresholds (60 lines, 6 arguments) live in the project's `clippy.toml`.

use super::Exits;
use super::cargo_fmt::{listed, manifests, relative};
use crate::adapter::{Adapter, CannotCheck, Command, Context, Output};
use crate::finding::{Finding, Measure, Place};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub struct Clippy;

/// cargo exits 0 when clippy found nothing and 101 when it failed; with `-D
/// warnings` every lint is an error, so 101 with lints is findings and 101 with
/// none is a build failure (reported with its own message below).
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[101],
};

const ARGS: &[&str] = &[
    "clippy",
    "--all-targets",
    "--offline",
    "--locked",
    "--keep-going",
    "--message-format=json",
    "--quiet",
    "--",
    "-D",
    "warnings",
    "-D",
    "clippy::too_many_lines",
    "-D",
    "clippy::too_many_arguments",
];

impl Adapter for Clippy {
    fn name(&self) -> &'static str {
        "clippy"
    }

    fn program(&self) -> &'static str {
        "cargo"
    }

    fn brew_formula(&self) -> &'static str {
        "rust"
    }

    fn command(&self, ctx: &Context) -> Option<Command> {
        self.commands(ctx).into_iter().next()
    }

    /// `cargo clippy --manifest-path <m> <ARGS after "clippy">`, one per Rust workspace.
    fn commands(&self, ctx: &Context) -> Vec<Command> {
        let to_command = |m: String| {
            let mut args = vec![ARGS[0].to_string(), "--manifest-path".into(), m];
            args.extend(ARGS[1..].iter().map(|a| a.to_string()));
            Command {
                program: "cargo".into(),
                args,
            }
        };
        manifests(ctx).into_iter().map(to_command).collect()
    }

    fn parse(&self, ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        let mut findings = Vec::new();
        let mut seen = Vec::new();
        for line in out.stdout.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if v["reason"] != "compiler-message" {
                continue;
            }
            let msg = &v["message"];
            let Some(span) = primary_span(msg) else {
                continue; // summaries: "aborting due to", "N warnings emitted", failure notes
            };
            let Some(code) = lint_code(msg) else {
                return Err(cannot_build(msg["message"].as_str().unwrap_or("")));
            };
            let file = source_path(&ctx.root, &v, span["file_name"].as_str().unwrap_or(""));
            let f = finding(msg, span, code, file);
            // With --all-targets a lint in library code comes once per target that
            // builds it (the lib and its unit tests): the same span, said again.
            let column = span["column_start"].as_u64();
            if !seen.contains(&(f.place.clone(), f.line, column, f.what.clone())) {
                seen.push((f.place.clone(), f.line, column, f.what.clone()));
                findings.push(f);
            }
        }
        if findings.is_empty() && out.exit != 0 {
            let first = out.stderr.lines().find(|l| l.starts_with("error"));
            return Err(cannot_build(first.unwrap_or("no output")));
        }
        let findings = EXITS.judge("clippy", out, findings)?;
        Ok(listed(ctx, findings))
    }
}

fn primary_span(msg: &Value) -> Option<&Value> {
    let spans = msg["spans"].as_array()?;
    spans.iter().find(|s| s["is_primary"] == true)
}

/// The lint's name; `None` for a compiler error (no code, or an `E0308`-style code).
fn lint_code(msg: &Value) -> Option<&str> {
    let code = msg["code"]["code"].as_str()?;
    let rustc_error = code.starts_with('E') && code[1..].chars().all(|c| c.is_ascii_digit());
    (!rustc_error).then_some(code)
}

fn cannot_build(first: &str) -> CannotCheck {
    CannotCheck {
        what: format!("the crate does not build: {}", first.trim_end_matches('.')),
        fix: "fix the build error, then run clippy again".into(),
    }
}

/// rustc names files relative to the workspace root, which may be a folder below the
/// repo. The workspace root is the highest folder W at or above the package's folder P
/// (inside root) such that the file name starts with P relative to W.
fn source_path(root: &Path, v: &Value, file_name: &str) -> String {
    let name = Path::new(file_name);
    let package = v["manifest_path"]
        .as_str()
        .map(Path::new)
        .and_then(Path::parent);
    let (Some(package), false) = (package, name.is_absolute()) else {
        return relative(root, file_name);
    };
    let mut above: Vec<&Path> = package
        .ancestors()
        .filter(|a| a.starts_with(root))
        .collect();
    above.reverse();
    let workspace = above.into_iter().find(|w| {
        package
            .strip_prefix(w)
            .is_ok_and(|inner| name.starts_with(inner))
    });
    let full: PathBuf = workspace.unwrap_or(root).join(name);
    relative(root, &full.to_string_lossy())
}

fn finding(msg: &Value, span: &Value, code: &str, file: String) -> Finding {
    let text = msg["message"].as_str().unwrap_or("").trim_end_matches('.');
    let rule = match code {
        "clippy::too_many_lines" => "R3",
        "clippy::too_many_arguments" | "clippy::cognitive_complexity" => "R4",
        _ => "R2",
    };
    Finding {
        rule: rule.into(),
        place: Place::File(file),
        line: span["line_start"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok()),
        what: text.to_string(),
        fix: fix_text(msg),
        tool_rule: code.to_string(),
        measure: measure(code, text),
    }
}

/// The first help that suggests a change (with its replacement when there is one),
/// else the first help that is not boilerplate, else a generic line.
fn fix_text(msg: &Value) -> String {
    let helps: Vec<&Value> = msg["children"]
        .as_array()
        .map(|c| c.iter().filter(|c| c["level"] == "help").collect())
        .unwrap_or_default();
    let replacements = |h: &Value| -> Vec<String> {
        h["spans"]
            .as_array()
            .map(|s| {
                let r = s.iter().filter_map(|s| s["suggested_replacement"].as_str());
                r.map(str::to_string).collect()
            })
            .unwrap_or_default()
    };
    let text = |h: &Value| {
        h["message"]
            .as_str()
            .unwrap_or("")
            .trim_end_matches('.')
            .to_string()
    };
    if let Some(h) = helps.iter().find(|h| !replacements(h).is_empty()) {
        return match replacements(h).as_slice() {
            [one] if !one.is_empty() => format!("{}: `{one}`", text(h)),
            _ => text(h),
        };
    }
    let plain = helps.iter().map(|h| text(h)).find(|t| {
        !t.is_empty() && !t.starts_with("for further information") && !t.starts_with("to override")
    });
    plain.unwrap_or_else(|| "fix the clippy lint".into())
}

/// "this function has too many lines (82/60)" gives 82 lines, limit 60.
fn measure(code: &str, text: &str) -> Option<Measure> {
    let unit = match code {
        "clippy::too_many_lines" => "lines",
        "clippy::too_many_arguments" => "arguments",
        "clippy::cognitive_complexity" => "complexity",
        _ => return None,
    };
    let inner = text.rsplit_once('(')?.1.strip_suffix(')')?;
    let (value, limit) = inner.split_once('/')?;
    Some(Measure {
        value: value.parse().ok()?,
        unit: unit.into(),
        limit: limit.parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::Stage;
    use std::path::PathBuf;

    const PLANTED: &str = include_str!("../../tests/fixtures/clippy/planted.stdout");
    const CLEAN: &str = include_str!("../../tests/fixtures/clippy/clean.stdout");
    const BROKEN_OUT: &str = include_str!("../../tests/fixtures/clippy/broken.stdout");
    const BROKEN_ERR: &str = include_str!("../../tests/fixtures/clippy/broken.stderr");
    const NESTED: &str = include_str!("../../tests/fixtures/clippy/nested.stdout");

    fn ctx(root: &str, files: &[&str]) -> Context {
        Context {
            root: PathBuf::from(root),
            stage: Stage::Commit,
            files: files.iter().map(|f| f.to_string()).collect(),
            push_range: None,
            languages: Default::default(),
        }
    }

    fn run(exit: i32, stdout: &str, stderr: &str) -> Result<Vec<Finding>, CannotCheck> {
        let out = Output {
            exit,
            stdout: stdout.into(),
            stderr: stderr.into(),
        };
        Clippy.parse(&ctx("/tmp/house-fixture/planted", &["src/lib.rs"]), &out)
    }

    #[test]
    fn findings_in_files_not_staged_are_dropped() {
        let out = Output {
            exit: 101,
            stdout: PLANTED.into(),
            stderr: String::new(),
        };
        let c = ctx(
            "/tmp/house-fixture/planted",
            &["src/other.rs", "Cargo.toml"],
        );
        assert_eq!(Clippy.parse(&c, &out), Ok(vec![]));
    }

    #[test]
    fn a_build_error_in_a_file_not_staged_still_cannot_check() {
        let out = Output {
            exit: 101,
            stdout: BROKEN_OUT.into(),
            stderr: BROKEN_ERR.into(),
        };
        let c = ctx("/tmp/house-fixture/planted", &["src/other.rs"]);
        assert!(Clippy.parse(&c, &out).is_err());
    }

    #[test]
    fn planted_lints_map_to_house_rules() {
        let found = run(101, PLANTED, "").unwrap();
        let got: Vec<_> = found
            .iter()
            .map(|f| (f.rule.as_str(), f.tool_rule.as_str(), f.line))
            .collect();
        assert_eq!(
            got,
            vec![
                ("R4", "clippy::too_many_arguments", Some(3)),
                ("R3", "clippy::too_many_lines", Some(7)),
                ("R2", "clippy::needless_return", Some(96)),
                ("R2", "clippy::ptr_arg", Some(94)),
            ]
        );
        assert!(
            found
                .iter()
                .all(|f| f.place == Place::File("src/lib.rs".into()))
        );
    }

    #[test]
    fn too_many_lines_carries_its_measure_and_a_generic_fix() {
        let found = run(101, PLANTED, "").unwrap();
        let long = &found[1];
        assert_eq!(long.what, "this function has too many lines (82/60)");
        assert_eq!(long.fix, "fix the clippy lint");
        let m = long.measure.as_ref().unwrap();
        assert_eq!((m.value, m.unit.as_str(), m.limit), (82.0, "lines", 60.0));
    }

    #[test]
    fn fix_uses_the_tools_suggestion() {
        let found = run(101, PLANTED, "").unwrap();
        assert_eq!(found[2].fix, "remove `return`");
        assert_eq!(found[3].fix, "change this to: `&[i32]`");
        assert_eq!(found[3].measure, None);
    }

    #[test]
    fn clean_gives_no_findings() {
        assert_eq!(run(0, CLEAN, ""), Ok(vec![]));
    }

    #[test]
    fn a_build_error_cannot_check() {
        let err = run(101, BROKEN_OUT, BROKEN_ERR).unwrap_err();
        assert_eq!(err.what, "the crate does not build: mismatched types");
    }

    #[test]
    fn a_failure_without_messages_cannot_check() {
        let err = run(101, "", BROKEN_ERR).unwrap_err();
        assert!(
            err.what.contains("could not compile `broken`"),
            "{}",
            err.what
        );
    }

    #[test]
    fn command_is_clippy_with_house_lints() {
        let cmd = Clippy
            .command(&ctx(env!("CARGO_MANIFEST_DIR"), &["src/lib.rs"]))
            .unwrap();
        assert_eq!(cmd.program, "cargo");
        assert_eq!(cmd.args[..3], ["clippy", "--manifest-path", "Cargo.toml"]);
        assert_eq!(cmd.args[3..], ARGS[1..]);
        for flag in ["--all-targets", "--locked", "--offline", "--keep-going"] {
            let at = cmd.args.iter().position(|a| a == flag);
            let end = cmd.args.iter().position(|a| a == "--");
            assert!(at < end && at.is_some(), "{flag} before --: {:?}", cmd.args);
        }
        assert!(
            Clippy
                .command(&ctx(env!("CARGO_MANIFEST_DIR"), &["a.md"]))
                .is_none()
        );
    }

    #[test]
    fn nested_workspace_paths_are_relative_to_the_repo() {
        let out = Output {
            exit: 101,
            stdout: NESTED.into(),
            stderr: String::new(),
        };
        let root = "/tmp/house-fixture/nested";
        let lib = "tools/inner/crates/a/src/lib.rs";
        let found = Clippy.parse(&ctx(root, &[lib]), &out).unwrap();
        let got: Vec<_> = found
            .iter()
            .map(|f| (f.place.clone(), f.tool_rule.as_str()))
            .collect();
        let lib = Place::File("tools/inner/crates/a/src/lib.rs".into());
        assert_eq!(
            got,
            vec![
                (lib.clone(), "clippy::needless_return"),
                (lib, "clippy::ptr_arg")
            ]
        );
    }

    #[test]
    fn one_clippy_command_per_workspace() {
        let root = std::env::temp_dir().join(format!("house-nested-clippy-{}", std::process::id()));
        std::fs::create_dir_all(root.join("a/src")).unwrap();
        std::fs::create_dir_all(root.join("b/src")).unwrap();
        std::fs::write(root.join("a/Cargo.toml"), "").unwrap();
        std::fs::write(root.join("b/Cargo.toml"), "").unwrap();
        let c = ctx(root.to_str().unwrap(), &["a/src/lib.rs", "b/src/lib.rs"]);
        let manifests: Vec<_> = Clippy
            .commands(&c)
            .into_iter()
            .map(|c| c.args[2].clone())
            .collect();
        assert_eq!(manifests, ["a/Cargo.toml", "b/Cargo.toml"]);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
