//! tsc, the TypeScript compiler, as a type check (R2): `tsc --noEmit --pretty false
//! --listFiles -p <tsconfig>`, once per project (`projects`). tsc cannot check single
//! files against a project's settings, and a project need not contain a file below it,
//! so each run also lists the files it checked, and a staged TypeScript file that no
//! run lists is a finding of its own (`joined`). Each run's build info goes to a
//! temporary folder outside the repo (`scratch`). Reads tsc's text output (captured
//! with TypeScript 6.0.3).

use super::{Exits, listed, node, path_value, relative, stage_file};
use crate::adapter::{Adapter, CannotCheck, Command, Context, Output};
use crate::finding::{Finding, Place};
use projects::{is_typescript, projects};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

mod projects;

pub struct Tsc;

/// tsc exits 0 clean, 1 with errors and nothing emitted, 2 with errors (and output
/// emitted, none under --noEmit); anything else is a crash. With --listFiles it may
/// exit 0 after errors; the errors are findings all the same.
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[1, 2],
};

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
    fn install(&self, ctx: &Context) -> String {
        node::install(&ctx.root, "typescript", self.brew_formula())
    }
    fn command(&self, ctx: &Context) -> Option<Command> {
        self.commands(ctx).into_iter().next()
    }
    /// One run per project. A stage whose TypeScript files are under no tsconfig at
    /// all runs `tsc --version`: it lists no file, so each is reported (`joined`), and
    /// a missing tsc is still a cannot-check.
    fn commands(&self, ctx: &Context) -> Vec<Command> {
        let program = node::program(&ctx.root, "tsc");
        let run = |args: Vec<String>| Command {
            program: program.clone(),
            args,
        };
        let found = projects(ctx);
        if found.is_empty() && ctx.files.iter().any(|f| is_typescript(f)) {
            return vec![run(vec!["--version".into()])];
        }
        let scratch = scratch(ctx);
        found
            .into_iter()
            .enumerate()
            .map(|(i, project)| {
                let info = scratch.join(format!("{i}.tsbuildinfo"));
                let options = ["--noEmit", "--pretty", "false", "--listFiles"];
                let mut args: Vec<String> = options.iter().map(|a| a.to_string()).collect();
                args.extend([
                    "--tsBuildInfoFile".into(),
                    info.to_string_lossy().into_owned(),
                ]);
                args.extend(["-p".into(), path_value(project)]);
                run(args)
            })
            .collect()
    }
    fn finished(&self, ctx: &Context) {
        let scratch = scratch(ctx);
        if scratch.exists()
            && let Err(e) = std::fs::remove_dir_all(&scratch)
        {
            eprintln!("house: could not remove {}: {e}", scratch.display());
        }
    }
    fn parse(&self, ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        let findings: Vec<Finding> = out
            .stdout
            .lines()
            .filter_map(|l| diagnostic(ctx, l))
            .collect();
        // A project referencing another reads its built declarations. Before that
        // project is built every import from it is TS6305, which says nothing of the
        // code: the check could not run.
        if let Some(unbuilt) = findings.iter().find(|f| f.tool_rule == UNBUILT) {
            return Err(CannotCheck {
                what: format!(
                    "tsc: a referenced project has not been built: {}",
                    unbuilt.what
                ),
                fix: "build the referenced projects first (tsc -b)".into(),
            });
        }
        // Judged before the stage's files are picked: errors only in other files of
        // the project are still a run that found errors.
        let findings = EXITS.judge("tsc", out, findings)?;
        // tsc checks the whole project; as for cargo, a commit of one file does not
        // report every other file's old errors.
        Ok(listed(ctx, findings))
    }
    fn joined(&self, ctx: &Context, outs: &[Output]) -> Vec<Finding> {
        // --listFiles prints each file a run checked, absolute, after its errors.
        // Built once, over every run; each staged file is then one lookup.
        let checked: std::collections::HashSet<String> = outs
            .iter()
            .flat_map(|o| o.stdout.lines())
            .filter(|l| l.starts_with('/') && !l.contains("/node_modules/"))
            .filter_map(|l| stage_file(&ctx.root, &ctx.files, l))
            .collect();
        ctx.files
            .iter()
            .filter(|f| is_typescript(f) && !checked.contains(f.as_str()))
            .map(|f| no_project(f))
            .collect()
    }
}

/// The folder outside the repo where this run's tsc keeps its build info: an
/// `incremental` or `composite` project writes a `.tsbuildinfo` even under --noEmit,
/// beside its tsconfig unless told where, and a check must not write into the working
/// tree. One per house process and repo; removed when the runs are done (`finished`).
fn scratch(ctx: &Context) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    ctx.root.hash(&mut hasher);
    std::env::temp_dir().join(format!(
        "house-tsc-{}-{:016x}",
        std::process::id(),
        hasher.finish()
    ))
}

/// tsc's "Output file ... has not been built from source file ...".
const UNBUILT: &str = "tsc TS6305";

fn no_project(file: &str) -> Finding {
    Finding {
        rule: "R2".into(),
        place: Place::File(file.to_string()),
        line: None,
        what: "tsc: no project includes this file".into(),
        fix: "add it to the include or files of a tsconfig.json, or to a project one references"
            .into(),
        tool_rule: "tsc no-project".into(),
        measure: None,
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
