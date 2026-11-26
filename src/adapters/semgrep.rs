//! Semgrep with the house pattern files (R8: deterministic tests; R9: nothing silenced).
//!
//! The pattern files are passed by path: `$HOUSE_PATTERNS_DIR/r8.yml` and `r9.yml` when
//! that variable is set, else the `patterns/` of this crate, baked in at build time.

use super::Exits;
use crate::adapter::{Adapter, CannotCheck, Command, Context, JS_EXTENSIONS, Output};
use crate::finding::{Finding, Place};
use report::{Problem, Report};

mod report;
#[cfg(test)]
mod tests;

pub struct Semgrep;

/// The extensions Semgrep scans, with `JS_EXTENSIONS`.
const EXTENSIONS: [&str; 5] = [".py", ".swift", ".rs", ".sh", ".bash"];

/// Semgrep exits 0 after a scan (with or without results), 1 with results only
/// under --error (not passed); 2 and up are failures (bad config, bad rule, crash).
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[1],
};

/// `--config <dir>/r8.yml --config <dir>/r9.yml`: both house pattern files.
fn config_args() -> Vec<String> {
    let dir = std::env::var("HOUSE_PATTERNS_DIR")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/patterns").to_string());
    let dir = dir.trim_end_matches('/');
    ["r8.yml", "r9.yml"]
        .iter()
        .flat_map(|file| ["--config".to_string(), format!("{dir}/{file}")])
        .collect()
}

/// The files Semgrep should scan: a concerned extension or a first-line language
/// (`Context::languages`), not under tests/fixtures/, and not prefixed by a
/// `.semgrepignore` line (Semgrep ignores that file for explicit targets).
fn targets(ctx: &Context, semgrepignore: &str) -> Vec<String> {
    let ignored: Vec<&str> = semgrepignore
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.trim_start_matches("./").trim_start_matches('/'))
        .chain(["tests/fixtures/"])
        .collect();
    ctx.files
        .iter()
        .filter(|f| {
            EXTENSIONS
                .iter()
                .chain(&JS_EXTENSIONS)
                .any(|ext| f.ends_with(ext))
                || ctx.languages.contains_key(*f)
        })
        .filter(|f| !ignored.iter().any(|prefix| f.starts_with(prefix)))
        .cloned()
        .collect()
}

fn sentence(text: &str) -> String {
    let first = text.lines().next().unwrap_or("").trim();
    first.trim_end_matches('.').to_string()
}

fn relative(ctx: &Context, path: &str) -> String {
    let root = ctx.root.to_string_lossy();
    let rel = path
        .strip_prefix(root.as_ref())
        .map_or(path, |rest| rest.trim_start_matches('/'));
    rel.replace('\\', "/")
}

impl Adapter for Semgrep {
    fn name(&self) -> &'static str {
        "semgrep"
    }

    fn program(&self) -> &'static str {
        "semgrep"
    }

    fn brew_formula(&self) -> &'static str {
        "semgrep"
    }

    fn command(&self, ctx: &Context) -> Option<Command> {
        let ignore = std::fs::read_to_string(ctx.root.join(".semgrepignore")).unwrap_or_default();
        let files = targets(ctx, &ignore);
        if files.is_empty() {
            return None;
        }
        let mut args: Vec<String> = [
            "scan",
            "--json",
            "--metrics=off",
            "--quiet",
            "--disable-version-check",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
        args.extend(config_args());
        args.push("--".to_string());
        args.extend(files);
        Some(Command {
            program: "semgrep".to_string(),
            args,
        })
    }

    fn parse(&self, ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        let report: Report = serde_json::from_str(&out.stdout).map_err(|_| CannotCheck {
            what: format!("semgrep exited {} without a JSON report", out.exit),
            fix: "run the semgrep command by hand and fix what it prints".to_string(),
        })?;
        // A file Semgrep could not parse is Semgrep's own limit: counted by `unread`,
        // not a finding (its Swift parser misses valid Swift). Any other error means a
        // rule or file was not checked, even next to results; those cannot ride
        // along with a cannot-check, so the text counts them.
        if let Some(first) = report.errors.iter().find(|e| e.unparsed_file().is_none()) {
            let hidden = match report.results.len() {
                0 => String::new(),
                1 => " (1 finding not shown)".to_string(),
                n => format!(" ({n} findings not shown)"),
            };
            return Err(CannotCheck {
                what: format!("semgrep failed: {}{hidden}", sentence(&first.message)),
                fix: "fix the semgrep config or the files it could not read".to_string(),
            });
        }
        let mut findings: Vec<Finding> = Vec::new();
        findings.extend(report.results.into_iter().map(|hit| {
            let id = hit.check_id.rsplit('.').next().unwrap_or("").to_string();
            Finding {
                rule: if id.starts_with("r9-") { "R9" } else { "R8" }.to_string(),
                place: Place::File(relative(ctx, &hit.path)),
                line: Some(hit.start.line),
                what: sentence(&hit.extra.message),
                fix: hit
                    .extra
                    .metadata
                    .fix
                    .as_deref()
                    .map_or_else(|| format!("fix the semgrep rule {id}"), sentence),
                tool_rule: format!("semgrep {id}"),
                measure: None,
            }
        }));
        EXITS.judge("semgrep", out, findings)
    }

    fn unread(&self, ctx: &Context, out: &Output) -> Vec<String> {
        let Ok(report) = serde_json::from_str::<Report>(&out.stdout) else {
            return Vec::new();
        };
        let mut files: Vec<String> = Vec::new();
        for path in report.errors.iter().filter_map(Problem::unparsed_file) {
            let path = relative(ctx, path);
            if !files.contains(&path) {
                files.push(path);
            }
        }
        files
    }
}
