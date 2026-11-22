//! ruff, for Python: `ruff check` (R2 lint, R3/R4 size and complexity) and
//! `ruff format --check` (R1). Both read ruff's JSON output (captured with ruff 0.17.6).

use super::Exits;
use crate::adapter::{Adapter, CannotCheck, Command, Context, Output};
use crate::finding::{Finding, Measure, Place};
use crate::run::first_line;
use serde::Deserialize;
use std::path::Path;

/// `ruff check --output-format json --force-exclude <py files>`.
pub struct Ruff;

/// `ruff format --check --output-format json --force-exclude <py files>`.
pub struct RuffFormat;

/// Both subcommands exit 0 clean, 1 with findings, 2 when ruff "terminates
/// abnormally". One exception, in `RuffFormat::parse`: the formatter exits 2 when a
/// file it cannot parse is in the list, which `ruff check` reports as R2.
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[1],
};

impl Adapter for Ruff {
    fn name(&self) -> &'static str {
        "ruff"
    }
    fn program(&self) -> &'static str {
        "ruff"
    }
    fn brew_formula(&self) -> &'static str {
        "ruff"
    }
    fn command(&self, ctx: &Context) -> Option<Command> {
        command(&["check"], ctx)
    }
    fn parse(&self, ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        // A file ruff cannot parse gives several invalid-syntax diagnostics; it is one
        // R2 finding on that file, at the first (owner, 2026-11-22).
        let mut findings: Vec<Finding> = Vec::new();
        for d in read(out)? {
            if !syntax(&d) {
                findings.push(finding(ctx, d));
                continue;
            }
            let place = Place::File(relative(&ctx.root, &d.filename));
            if findings
                .iter()
                .any(|f| f.place == place && f.tool_rule == SYNTAX_RULE)
            {
                continue;
            }
            findings.push(Finding {
                rule: "R2".into(),
                place,
                line: d.location.map(|l| l.row),
                what: format!(
                    "the tool could not parse this file: {}",
                    sentence(&d.message)
                ),
                fix: "fix the syntax error".into(),
                tool_rule: SYNTAX_RULE.into(),
                measure: None,
            });
        }
        EXITS.judge("ruff", out, findings)
    }
}

impl Adapter for RuffFormat {
    fn name(&self) -> &'static str {
        "ruff-format"
    }
    fn program(&self) -> &'static str {
        "ruff"
    }
    fn brew_formula(&self) -> &'static str {
        "ruff"
    }
    fn command(&self, ctx: &Context) -> Option<Command> {
        command(&["format", "--check"], ctx)
    }
    fn parse(&self, ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        // A file ruff cannot parse is reported by `ruff check` (R2); the formatter skips
        // it, and its exit 2 then means that, so the other files' findings stand.
        let list = read(out)?;
        let met_syntax = list.iter().any(syntax);
        let findings: Vec<Finding> = list
            .into_iter()
            .filter(|d| !syntax(d))
            .map(|d| finding(ctx, d))
            .collect();
        if out.exit == 2 && met_syntax {
            return Ok(findings);
        }
        EXITS.judge("ruff", out, findings)
    }
}

/// The tool rule of the one finding for a file ruff cannot parse.
const SYNTAX_RULE: &str = "ruff invalid-syntax";

fn syntax(d: &Diagnostic) -> bool {
    d.code.as_deref() == Some("invalid-syntax")
}

#[derive(Deserialize)]
struct Diagnostic {
    code: Option<String>,
    filename: String,
    message: String,
    location: Option<Location>,
    fix: Option<Fix>,
    url: Option<String>,
}

#[derive(Deserialize)]
struct Location {
    row: u32,
}

#[derive(Deserialize)]
struct Fix {
    message: Option<String>,
}

fn command(sub: &[&str], ctx: &Context) -> Option<Command> {
    let py: Vec<String> = ctx
        .files
        .iter()
        .filter(|f| f.ends_with(".py") || f.ends_with(".pyi"))
        .cloned()
        .collect();
    if py.is_empty() {
        return None;
    }
    let mut args: Vec<String> = sub.iter().map(|s| s.to_string()).collect();
    args.extend(["--output-format", "json", "--force-exclude", "--"].map(String::from));
    args.extend(py);
    Some(Command {
        program: "ruff".into(),
        args,
    })
}

/// ruff's JSON list on stdout; anything unreadable is cannot-check.
fn read(out: &Output) -> Result<Vec<Diagnostic>, CannotCheck> {
    serde_json::from_str::<Vec<Diagnostic>>(&out.stdout).map_err(|_| CannotCheck {
        what: format!(
            "ruff exited {} without output house can read: {}",
            out.exit,
            first_line(&out.stderr)
        ),
        fix: "run ruff by hand in the repo to see why".into(),
    })
}

fn finding(ctx: &Context, d: Diagnostic) -> Finding {
    let code = d.code.unwrap_or_else(|| "unknown".into());
    let rule = rule_for(&code);
    let place = Place::File(relative(&ctx.root, &d.filename));
    let fix = match (d.fix.and_then(|f| f.message), &d.url) {
        (Some(message), _) => message,
        _ if code == "unformatted" => format!("run ruff format {}", place_text(&place)),
        _ if rule == "R4" => "split it into smaller helpers".into(),
        _ if rule == "R3" => "split it into smaller functions".into(),
        (None, Some(url)) => format!("see {url}"),
        (None, None) => "correct the code at this line".into(),
    };
    let tool_rule = if code == "unformatted" {
        "ruff format".to_string()
    } else {
        format!("ruff {code}")
    };
    Finding {
        rule: rule.into(),
        measure: measure(&code, &d.message),
        place,
        line: d.location.map(|l| l.row),
        what: sentence(&d.message),
        fix: sentence(&fix),
        tool_rule,
    }
}

/// ruff code to house rule: formatting R1, too long R3, too complex R4, the rest R2.
pub fn rule_for(code: &str) -> &'static str {
    match code {
        "unformatted" => "R1",
        "PLR0915" => "R3",
        "C901" | "PLR0911" | "PLR0912" | "PLR0913" | "PLR0917" | "PLR1702" => "R4",
        _ => "R2",
    }
}

/// "`score` is too complex (4 > 2)" gives 4 complexity, limit 2.
fn measure(code: &str, message: &str) -> Option<Measure> {
    let unit = match code {
        "C901" => "complexity",
        "PLR0911" => "returns",
        "PLR0912" => "branches",
        "PLR0913" | "PLR0917" => "parameters",
        "PLR0915" => "statements",
        "PLR1702" => "nesting",
        _ => return None,
    };
    let inner = message.rsplit_once('(')?.1.strip_suffix(')')?;
    let (value, limit) = inner.split_once(" > ")?;
    Some(Measure {
        value: value.trim().parse().ok()?,
        unit: unit.into(),
        limit: limit.trim().parse().ok()?,
    })
}

fn relative(root: &Path, filename: &str) -> String {
    let path = Path::new(filename);
    let canonical = root.canonicalize().ok();
    let stripped = path
        .strip_prefix(root)
        .ok()
        .or_else(|| path.strip_prefix(canonical.as_deref()?).ok());
    stripped
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| filename.to_string())
}

fn place_text(place: &Place) -> &str {
    match place {
        Place::File(path) => path,
        Place::Repo => "repo",
    }
}

fn sentence(text: &str) -> String {
    text.trim().trim_end_matches('.').to_string()
}
