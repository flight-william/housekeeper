//! R9 in Cargo.toml: house's own check, not a tool's. An `allow` (or `expect`) level in
//! a `[lints]` or `[workspace.lints]` table silences a lint for a whole crate, as
//! `#[allow(...)]` does for one item; each such entry is one finding at its line.
//! `deny` and `forbid` are stricter, and `warn` is still an error under clippy's
//! `-D warnings`, so none of those is a finding.
//!
//! It runs on every `Cargo.toml` a stage sends, whatever `[tools]` lists, like the
//! excuse list's checks.

use crate::adapter::{CannotCheck, Context};
use crate::finding::{Finding, Place};
use crate::reply::ToolRun;
use std::time::Instant;
use toml::de::{DeTable, DeValue};

/// The name the reply and the record give this check.
pub const NAME: &str = "cargo-lints";

/// The levels that silence a lint.
const SILENT: [&str; 2] = ["allow", "expect"];

/// The findings for every `Cargo.toml` among `ctx.files`; `None` when there is none.
pub fn run(ctx: &Context) -> Option<ToolRun> {
    let manifests: Vec<&String> = ctx
        .files
        .iter()
        .filter(|f| f.rsplit('/').next() == Some("Cargo.toml"))
        .collect();
    if manifests.is_empty() {
        return None;
    }
    let start = Instant::now();
    let mut tool = ToolRun {
        name: NAME.into(),
        seconds: 0.0,
        exit: None,
        skipped: false,
        findings: Vec::new(),
        cannot: None,
        unread: Vec::new(),
    };
    for path in manifests {
        let read = std::fs::read_to_string(ctx.root.join(path)).map_err(|e| CannotCheck {
            what: format!("{path} cannot be read: {e}"),
            fix: format!("make {path} readable"),
        });
        match read.and_then(|text| allowed(path, &text)) {
            Ok(found) => tool.findings.extend(found),
            Err(c) => {
                tool.cannot.get_or_insert(c);
            }
        }
    }
    tool.seconds = start.elapsed().as_secs_f64();
    Some(tool)
}

/// One finding per lint set to a silencing level in `text`, the manifest at `path`.
pub fn allowed(path: &str, text: &str) -> Result<Vec<Finding>, CannotCheck> {
    let doc = DeTable::parse(text).map_err(|e| CannotCheck {
        what: format!("{path} is not valid TOML: {}", e.message().trim()),
        fix: format!("correct {path}"),
    })?;
    let doc = doc.get_ref();
    let mut found = Vec::new();
    let workspace = table(doc, "workspace").and_then(|w| table(w, "lints"));
    for (prefix, lints) in [
        ("lints", table(doc, "lints")),
        ("workspace.lints", workspace),
    ] {
        let Some(lints) = lints else { continue };
        for (tool, levels) in lints {
            let DeValue::Table(levels) = levels.get_ref() else {
                continue; // `workspace = true`: the workspace's table is judged there.
            };
            for (lint, level) in levels {
                if !SILENT.contains(&level_of(level.get_ref()).unwrap_or("")) {
                    continue;
                }
                let tool = tool.get_ref();
                let name = match tool.as_ref() {
                    "rust" => lint.get_ref().to_string(),
                    other => format!("{other}::{}", lint.get_ref()),
                };
                let line = text[..lint.span().start].matches('\n').count() + 1;
                found.push(Finding {
                    rule: "R9".into(),
                    place: Place::File(path.into()),
                    line: u32::try_from(line).ok(),
                    what: format!(
                        "{name} is set to {} in [{prefix}.{tool}]",
                        level_of(level.get_ref()).unwrap_or("")
                    ),
                    fix: "remove the entry and fix what the lint reports".into(),
                    tool_rule: format!("{NAME} allow"),
                    measure: None,
                });
            }
        }
    }
    found.sort_by_key(|f| f.line);
    Ok(found)
}

/// The table under `key` in `t`, if it is one.
fn table<'a, 'i>(t: &'a DeTable<'i>, key: &str) -> Option<&'a DeTable<'i>> {
    match t.get(key)?.get_ref() {
        DeValue::Table(inner) => Some(inner),
        _ => None,
    }
}

/// A lint's level: `"allow"`, or `{ level = "allow", priority = -1 }`.
fn level_of<'a>(value: &'a DeValue<'_>) -> Option<&'a str> {
    match value {
        DeValue::String(s) => Some(s.as_ref()),
        DeValue::Table(t) => match t.get("level")?.get_ref() {
            DeValue::String(s) => Some(s.as_ref()),
            _ => None,
        },
        _ => None,
    }
}
