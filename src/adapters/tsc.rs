//! tsc, the TypeScript compiler, as a type check (R2): `tsc --noEmit --pretty false -p
//! <tsconfig>`. tsc cannot check single files against a project's settings, so it runs
//! once per project: the nearest `tsconfig.json` at or above each staged TypeScript
//! file (`TS_EXTENSIONS`) or staged `tsconfig.json`, inside root. A file with none above
//! it runs the root one, which tsc then reports missing. Reads tsc's text output
//! (captured with TypeScript 6.0.3).

use super::cargo_fmt::listed;
use super::{Exits, node, path_value, relative, stage_file};
use crate::adapter::{Adapter, CannotCheck, Command, Context, Output, TS_EXTENSIONS};
use crate::finding::{Finding, Place};
use std::path::Path;

pub struct Tsc;

/// tsc exits 0 clean, 1 with errors and nothing emitted, 2 with errors (and output
/// emitted, none under --noEmit); anything else is a crash.
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[1, 2],
};

const TSCONFIG: &str = "tsconfig.json";

/// The projects to check, relative to root, first seen first, no repeats.
fn projects(ctx: &Context) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let typescript = ctx.files.iter().filter(|f| {
        TS_EXTENSIONS.iter().any(|ext| f.ends_with(ext))
            || Path::new(f.as_str()).file_name() == Some(TSCONFIG.as_ref())
    });
    for file in typescript {
        let project = nearest(&ctx.root, file).unwrap_or_else(|| TSCONFIG.to_string());
        if !found.contains(&project) {
            found.push(project);
        }
    }
    found
}

/// The nearest `tsconfig.json` at or above `file`'s folder, inside root.
fn nearest(root: &Path, file: &str) -> Option<String> {
    let mut dir = Path::new(file).parent();
    while let Some(d) = dir {
        let config = d.join(TSCONFIG);
        if root.join(&config).is_file() {
            return Some(config.to_string_lossy().replace('\\', "/"));
        }
        dir = d.parent();
    }
    None
}

impl Adapter for Tsc {
    fn name(&self) -> &'static str {
        "tsc"
    }
    fn program(&self) -> &'static str {
        "tsc"
    }
    fn brew_formula(&self) -> &'static str {
        "typescript"
    }
    fn command(&self, ctx: &Context) -> Option<Command> {
        self.commands(ctx).into_iter().next()
    }
    fn commands(&self, ctx: &Context) -> Vec<Command> {
        let program = node::program(&ctx.root, "tsc");
        projects(ctx)
            .into_iter()
            .map(|project| Command {
                program: program.clone(),
                args: ["--noEmit", "--pretty", "false", "-p"]
                    .iter()
                    .map(|a| a.to_string())
                    .chain([path_value(project)])
                    .collect(),
            })
            .collect()
    }
    fn parse(&self, ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        let findings = out
            .stdout
            .lines()
            .filter_map(|l| diagnostic(ctx, l))
            .collect();
        // Judged before the stage's files are picked: errors only in other files of
        // the project are still a run that found errors.
        let findings = EXITS.judge("tsc", out, findings)?;
        // tsc checks the whole project; as for cargo, a commit of one file does not
        // report every other file's old errors.
        Ok(listed(ctx, findings))
    }
}

/// One diagnostic line: `<path>(<line>,<col>): error TS<n>: <message>`, or
/// `error TS<n>: <message>` for the project as a whole. Indented lines continue the
/// one before and are left out.
fn diagnostic(ctx: &Context, line: &str) -> Option<Finding> {
    let (head, tail) = line.split_once("error TS")?;
    let (code, message) = tail.split_once(": ")?;
    if !code.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let (place, line_no) = if head.is_empty() {
        (Place::Repo, None)
    } else {
        let (path, at) = head.strip_suffix("): ")?.rsplit_once('(')?;
        let line_no = at.split_once(',')?.0.parse().ok()?;
        let file =
            stage_file(&ctx.root, &ctx.files, path).unwrap_or_else(|| relative(&ctx.root, path));
        (Place::File(file), Some(line_no))
    };
    let fix = match place {
        Place::Repo => "fix the project's tsconfig.json",
        Place::File(_) => "fix the type error",
    };
    Some(Finding {
        rule: "R2".into(),
        place,
        line: line_no,
        what: message.trim().trim_end_matches('.').to_string(),
        fix: fix.into(),
        tool_rule: format!("tsc TS{code}"),
        measure: None,
    })
}
