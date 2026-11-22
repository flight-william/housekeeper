//! `house init`: a report-only pre-commit hook (only when none exists) and a
//! starter `.house.toml` naming the tools the repo's files need.

use crate::adapter::{Adapter, CannotCheck, Context, Stage};
use crate::config;
use crate::files::{languages, repo_root, tracked};
use crate::run::git;
use std::path::{Path, PathBuf};

/// The files only the owner may change: the approval check's list itself
/// (one definition), written into CODEOWNERS.
pub use crate::approval::PROTECTED;

pub const CODEOWNERS_PATH: &str = ".github/CODEOWNERS";

/// One hook: its file name, the line that runs house, and the report-only script.
pub struct Hook {
    pub name: &'static str,
    pub line: &'static str,
}

pub const HOOKS: &[Hook] = &[
    Hook {
        name: "pre-commit",
        line: "house check --stage commit || true",
    },
    Hook {
        name: "commit-msg",
        line: "house commit-msg \"$1\" || true",
    },
    Hook {
        name: "pre-push",
        line: "house check --stage push || true",
    },
];

/// The v0 pre-commit script, kept as it was.
pub const HOOK: &str = "#!/bin/sh\n\
# house v0, report-only: never blocks a commit\n\
house check --stage commit || true\n\
exit 0\n";

pub fn hook_script(hook: &Hook) -> String {
    if hook.name == "pre-commit" {
        return HOOK.to_string();
    }
    format!(
        "#!/bin/sh\n# house v1, report-only: never blocks\n{}\nexit 0\n",
        hook.line
    )
}

/// Set up the repo around `cwd`; returns the lines to print.
pub fn init(cwd: &Path, all: Vec<Box<dyn Adapter>>) -> Result<Vec<String>, CannotCheck> {
    init_with_owner(cwd, all, None)
}

/// As `init`, with `owner` (a GitHub user, `@name`) for CODEOWNERS.
pub fn init_with_owner(
    cwd: &Path,
    all: Vec<Box<dyn Adapter>>,
    owner: Option<&str>,
) -> Result<Vec<String>, CannotCheck> {
    let root = repo_root(cwd)?;
    let mut said = hooks(&root)?;
    said.push(starter_config(&root, all)?);
    said.push(codeowners(&root, owner)?);
    Ok(said)
}

fn hooks(root: &Path) -> Result<Vec<String>, CannotCheck> {
    let out = git(root, &["rev-parse", "--git-path", "hooks"])?;
    let mut dir = PathBuf::from(out.stdout.trim());
    if dir.is_relative() {
        dir = root.join(dir);
    }
    HOOKS.iter().map(|h| hook(&dir, h)).collect()
}

fn hook(dir: &Path, hook: &Hook) -> Result<String, CannotCheck> {
    let path = dir.join(hook.name);
    if path.exists() {
        return Ok(format!(
            "house init: {} already exists; add this line to it: {}",
            path.display(),
            hook.line
        ));
    }
    std::fs::create_dir_all(dir).map_err(|e| io_cannot(dir, e))?;
    std::fs::write(&path, hook_script(hook)).map_err(|e| io_cannot(&path, e))?;
    make_executable(&path)?;
    Ok(format!(
        "house init: wrote {} (report-only)",
        path.display()
    ))
}

/// `@name` form; a bare name gets the `@`.
fn owner_handle(owner: &str) -> String {
    format!("@{}", owner.trim_start_matches('@'))
}

fn codeowners_lines(protected: &[String], owner: &str) -> Vec<String> {
    protected
        .iter()
        .map(|p| {
            let pat = if p.starts_with("**/") {
                p.to_string()
            } else {
                format!("/{p}")
            };
            format!("{pat} {owner}")
        })
        .collect()
}

fn codeowners(root: &Path, owner: Option<&str>) -> Result<String, CannotCheck> {
    let path = root.join(CODEOWNERS_PATH);
    let handle = owner.map_or_else(|| "@<github user>".to_string(), owner_handle);
    let lines = codeowners_lines(&crate::approval::protected(root)?, &handle);
    if path.exists() || owner.is_none() {
        let why = if path.exists() {
            format!("{CODEOWNERS_PATH} already exists; left as it is")
        } else {
            format!("no --owner given, {CODEOWNERS_PATH} not written")
        };
        return Ok(format!(
            "house init: {why}; add these lines to it:\n{}",
            lines.join("\n")
        ));
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| io_cannot(dir, e))?;
    }
    let text = format!(
        "# house: the files only the owner may change\n{}\n",
        lines.join("\n")
    );
    std::fs::write(&path, text).map_err(|e| io_cannot(&path, e))?;
    Ok(format!("house init: wrote {CODEOWNERS_PATH} for {handle}"))
}

fn starter_config(root: &Path, all: Vec<Box<dyn Adapter>>) -> Result<String, CannotCheck> {
    let path = root.join(config::FILE);
    if path.exists() {
        return Ok(format!(
            "house init: {} already exists; left as it is",
            config::FILE
        ));
    }
    let files = tracked(root)?;
    let ctx = Context {
        root: root.to_path_buf(),
        stage: Stage::Commit,
        languages: languages(root, &files),
        files,
        push_range: None,
    };
    let names: Vec<String> = all
        .iter()
        .filter(|a| a.command(&ctx).is_some())
        .map(|a| format!("\"{}\"", a.name()))
        .collect();
    let text = format!(
        "# house: the tools each stage runs, by adapter name.\n\
         # Without a list, every tool runs and skips itself when no file concerns it.\n\
         [tools]\n\
         commit = [{}]\n",
        names.join(", ")
    );
    std::fs::write(&path, text).map_err(|e| io_cannot(&path, e))?;
    Ok(format!(
        "house init: wrote {} with tools [{}]",
        config::FILE,
        names.join(", ")
    ))
}

fn make_executable(path: &Path) -> Result<(), CannotCheck> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| io_cannot(path, e))
}

fn io_cannot(path: &Path, e: std::io::Error) -> CannotCheck {
    CannotCheck {
        what: format!("{} cannot be written: {e}", path.display()),
        fix: "check the folder's permissions".into(),
    }
}
