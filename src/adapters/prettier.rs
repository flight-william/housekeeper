//! Prettier, for JavaScript and TypeScript: `prettier --check` (R1) on the stage's code
//! files (`JS_EXTENSIONS`) and, with `--parser babel`, its `node` scripts, read from its text output (Prettier
//! 3.10.1). It reads the repo's own Prettier settings and `.prettierignore`.

use super::{Exits, node, stage_file, then_files};
use crate::adapter::{Adapter, CannotCheck, Command, Context, Language, Output};
use crate::finding::{Finding, Place};

pub struct Prettier;

/// `prettier --check` exits 0 formatted, 1 with a file to format, 2 when something
/// went wrong: one exception, in `parse`, is a file it cannot parse.
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[1],
};

impl Adapter for Prettier {
    fn name(&self) -> &'static str {
        "prettier"
    }
    fn program(&self) -> &'static str {
        "prettier"
    }
    fn brew_formula(&self) -> &'static str {
        "prettier"
    }
    fn install(&self, ctx: &Context) -> String {
        node::install(&ctx.root, "prettier", self.brew_formula())
    }
    fn command(&self, ctx: &Context) -> Option<Command> {
        self.commands(ctx).into_iter().next()
    }
    /// The code files, then the `node` scripts with `--parser babel`: Prettier infers a
    /// parser from a file's extension or a plain `#!/usr/bin/env node`, not from
    /// `#!/usr/bin/env -S node --flags`, and exits 2 for such a script.
    fn commands(&self, ctx: &Context) -> Vec<Command> {
        let (scripts, code): (Vec<String>, Vec<String>) = node::js_files(ctx)
            .into_iter()
            .partition(|f| ctx.first_line_is(f, Language::Node));
        let program = node::program(&ctx.root, "prettier");
        [
            (code, &["--check"][..]),
            (scripts, &["--check", "--parser", "babel"][..]),
        ]
        .into_iter()
        .filter(|(files, _)| !files.is_empty())
        .map(|(files, options)| Command {
            program: program.clone(),
            args: then_files(options, files),
        })
        .collect()
    }
    fn parse(&self, ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        // `[warn] <file>` on stderr names each file to format; its last `[warn]` line
        // is a summary, which names no staged file.
        let findings: Vec<Finding> = out
            .stderr
            .lines()
            .filter_map(|l| l.strip_prefix("[warn] "))
            .filter_map(|path| stage_file(&ctx.root, &ctx.files, path))
            .map(finding)
            .collect();
        // A file Prettier cannot parse is ESLint's and tsc's to report (R2); it exits 2
        // then, and the other files' findings stand.
        if out.exit == 2 && unparsed(ctx, &out.stderr) {
            return Ok(findings);
        }
        EXITS.judge("prettier", out, findings)
    }
}

/// Whether `stderr` has `[error] <staged file>: SyntaxError: ...`.
fn unparsed(ctx: &Context, stderr: &str) -> bool {
    stderr.lines().any(|l| {
        l.strip_prefix("[error] ")
            .and_then(|rest| rest.split_once(": SyntaxError: "))
            .is_some_and(|(path, _)| stage_file(&ctx.root, &ctx.files, path).is_some())
    })
}

fn finding(file: String) -> Finding {
    Finding {
        rule: "R1".into(),
        fix: format!("run prettier --write {file}"),
        place: Place::File(file),
        line: None,
        what: "not formatted as Prettier formats it".into(),
        tool_rule: "prettier --check".into(),
        measure: None,
    }
}
