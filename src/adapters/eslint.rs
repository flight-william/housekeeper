//! ESLint, for JavaScript and TypeScript: `eslint --format json --max-warnings 0` (R2,
//! warnings as errors; R3 and R4 for its size and complexity rules) on the stage's code
//! files and `node` scripts, by the repo's own flat config (`eslint.config.*`). Reads
//! ESLint's JSON (captured with ESLint 10.15.0).

use super::{Exits, node, relative, stage_file, then_files};
use crate::adapter::{Adapter, CannotCheck, Command, Context, Output};
use crate::finding::{Finding, Place};
use crate::run::first_line;
use serde::Deserialize;

pub struct Eslint;

/// ESLint exits 0 clean, 1 with an error or more warnings than `--max-warnings`, 2 when
/// its config or a rule fails (one exception, in `parse`: no config at all).
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[1],
};

/// The tool rule of ESLint's `max-lines`: a file's length is r3's (`yields_to`).
const MAX_LINES: &str = "eslint max-lines";

/// ESLint's text, on stderr, when the repo has no flat config.
const NO_CONFIG: &str = "ESLint couldn't find an eslint.config";

/// ESLint's warning for a file the repo's config ignores: the repo's choice, as
/// ruff's `--force-exclude`. Not a finding.
const IGNORED: &str = "File ignored because of a matching ignore pattern";

/// ESLint's warning for an `eslint-disable` comment that silences nothing.
const UNUSED_DIRECTIVE: &str = "Unused eslint-disable directive";

/// ESLint's warning for a file no `files` pattern of the config covers: the file is
/// not linted at all, so it is a finding.
const UNCONFIGURED: &str = "File ignored because no matching configuration was supplied";

impl Adapter for Eslint {
    fn name(&self) -> &'static str {
        "eslint"
    }
    fn program(&self) -> &'static str {
        "eslint"
    }
    fn brew_formula(&self) -> &'static str {
        "eslint"
    }
    fn install(&self, ctx: &Context) -> String {
        node::install(&ctx.root, "eslint", self.brew_formula())
    }
    fn yields_to(&self) -> &'static [(&'static str, &'static str)] {
        &[(MAX_LINES, "r3")]
    }
    fn command(&self, ctx: &Context) -> Option<Command> {
        let files = node::js_files(ctx);
        if files.is_empty() {
            return None;
        }
        Some(Command {
            program: node::program(&ctx.root, "eslint"),
            args: then_files(&["--format", "json", "--max-warnings", "0"], files),
        })
    }
    fn parse(&self, ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        if out.exit == 2 && out.stderr.contains(NO_CONFIG) {
            return Ok(vec![no_config()]);
        }
        let results = read(out)?;
        let mut ignored = false;
        let mut findings = Vec::new();
        for result in results {
            let place = Place::File(
                stage_file(&ctx.root, &ctx.files, &result.file_path)
                    .unwrap_or_else(|| relative(&ctx.root, &result.file_path)),
            );
            for m in result.messages {
                if m.rule_id.is_none() && m.message.starts_with(IGNORED) {
                    ignored = true;
                    continue;
                }
                findings.push(finding(place.clone(), m));
            }
        }
        // Only ignored files: ESLint counts its own warning against --max-warnings.
        if out.exit == 1 && findings.is_empty() && ignored {
            return Ok(findings);
        }
        EXITS.judge("eslint", out, findings)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileResult {
    file_path: String,
    messages: Vec<Message>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Message {
    rule_id: Option<String>,
    message: String,
    line: Option<u32>,
    #[serde(default)]
    fatal: bool,
}

fn read(out: &Output) -> Result<Vec<FileResult>, CannotCheck> {
    serde_json::from_str(&out.stdout).map_err(|_| CannotCheck {
        what: format!(
            "eslint exited {} without output house can read: {}",
            out.exit,
            first_line(&out.stderr)
        ),
        fix: "run eslint by hand in the repo to see why".into(),
    })
}

fn no_config() -> Finding {
    Finding {
        rule: "R2".into(),
        place: Place::Repo,
        line: None,
        what: "ESLint found no eslint.config.* file".into(),
        fix: "add an ESLint flat config (eslint.config.js) at the repo root".into(),
        tool_rule: "eslint no-config".into(),
        measure: None,
    }
}

fn finding(place: Place, m: Message) -> Finding {
    let (rule, what, fix, tool_rule) = match m.rule_id.as_deref() {
        None if m.fatal => (
            "R2",
            format!(
                "the tool could not parse this file: {}",
                sentence(&m.message)
            ),
            "fix the syntax error".to_string(),
            "eslint parse-error".to_string(),
        ),
        None if m.message.starts_with(UNUSED_DIRECTIVE) => (
            "R2",
            sentence(&m.message),
            "remove the unused eslint-disable comment".to_string(),
            "eslint unused-disable-directive".to_string(),
        ),
        None if m.message.starts_with(UNCONFIGURED) => (
            "R2",
            "ESLint has no configuration for this file".to_string(),
            "add a files glob that matches it to eslint.config (a node script with no extension needs one, such as \"bin/*\"), or ignore it there"
                .to_string(),
            "eslint no-matching-config".to_string(),
        ),
        id => {
            let id = id.unwrap_or("unknown");
            let rule = rule_for(id);
            let fix = match rule {
                "R3" => "split it into smaller functions",
                "R4" => "split it into smaller helpers",
                _ => "correct the code at this line",
            };
            (
                rule,
                sentence(&m.message),
                fix.to_string(),
                format!("eslint {id}"),
            )
        }
    };
    Finding {
        rule: rule.into(),
        place,
        line: m.line,
        what,
        fix,
        tool_rule,
        measure: None,
    }
}

/// ESLint core rule to house rule: too long R3, too complex R4, the rest R2.
fn rule_for(id: &str) -> &'static str {
    match id {
        "max-lines" | "max-lines-per-function" | "max-statements" => "R3",
        "complexity" | "max-depth" | "max-params" | "max-nested-callbacks" => "R4",
        _ => "R2",
    }
}

fn sentence(text: &str) -> String {
    text.trim().trim_end_matches('.').to_string()
}
