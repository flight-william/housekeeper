//! One file per tool, each behind the one Adapter interface (src/adapter.rs).

use crate::adapter::{Adapter, CannotCheck, Output};
use crate::finding::Finding;
use crate::run::first_line;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

pub mod cargo_deny;
pub mod cargo_fmt;
pub mod clippy;
pub mod gitleaks;
pub mod r3;
pub mod ruff;
pub mod semgrep;
pub mod shellcheck;
pub mod swift_format;
pub mod swiftformat;
pub mod swiftlint;

/// Every adapter house knows, in reply order. Each skips itself when no file concerns it;
/// one that does not run by default (`Adapter::runs_by_default`) runs only when named.
pub fn all() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(ruff::Ruff),
        Box::new(ruff::RuffFormat),
        Box::new(cargo_fmt::CargoFmt),
        Box::new(clippy::Clippy),
        Box::new(cargo_deny::CargoDeny),
        Box::new(swiftlint::SwiftLint),
        Box::new(swift_format::SwiftFormat),
        Box::new(swiftformat::Swiftformat),
        Box::new(shellcheck::Shellcheck),
        Box::new(gitleaks::Gitleaks),
        Box::new(semgrep::Semgrep),
        Box::new(r3::R3),
    ]
}

/// `options`, then `--`, then `files`: a staged file named like an option (`-x.swift`,
/// `--output=Sources/A.swift`) reaches the tool as a file. Without `--`, SwiftLint
/// read `--output=Sources/A.swift` as its report file, walked the whole tree and wrote
/// over the tracked `Sources/A.swift`. Every tool given files this way takes `--`.
pub(crate) fn then_files(options: &[&str], files: Vec<String>) -> Vec<String> {
    let mut args: Vec<String> = options.iter().map(|o| o.to_string()).collect();
    args.push("--".to_string());
    args.extend(files);
    args
}

/// A path given as an option's value, with `./` before it when it starts with `-`:
/// cargo reads `--manifest-path -ws/Cargo.toml` as the option `-w`, and so do the
/// cargo commands cargo fmt and cargo-deny run with the same path, so neither `--`
/// nor `--manifest-path=` helps.
pub(crate) fn path_value(path: String) -> String {
    if path.starts_with('-') {
        format!("./{path}")
    } else {
        path
    }
}

/// `file` relative to `root`. SwiftLint and SwiftFormat print absolute paths, and
/// may print /tmp for a root git names /private/tmp.
pub(crate) fn relative(root: &Path, file: &str) -> String {
    let unprivate = |p: &str| p.strip_prefix("/private").unwrap_or(p).to_string();
    let root = unprivate(&root.to_string_lossy());
    let file = unprivate(file);
    let prefix = format!("{}/", root.trim_end_matches('/'));
    file.strip_prefix(&prefix).unwrap_or(&file).to_string()
}

/// The stage file a tool's `path` names, in git's spelling, or `None` when it names
/// none of `files`. Matched by text first, then by the file itself (device and inode):
/// SwiftLint and SwiftFormat print decomposed Unicode (`e` and U+0301) where git and
/// the disk hold `é`, so the text of an accented path never matches.
pub(crate) fn stage_file(root: &Path, files: &[String], path: &str) -> Option<String> {
    let text = relative(root, path);
    if files.contains(&text) {
        return Some(text);
    }
    let id = |p: &Path| {
        let m = std::fs::metadata(p).ok()?;
        Some((m.dev(), m.ino()))
    };
    let wanted = id(&root.join(path))?;
    files
        .iter()
        .find(|f| id(&root.join(f)) == Some(wanted))
        .cloned()
}

/// A tool's documented exit codes; each adapter keeps its table in its own file.
/// After a `ran` exit the parsed result stands (findings or none); a `found` exit
/// must come with findings. Any other exit, a signal (-1) among them, is
/// cannot-check even when the output parses: a run that did not finish proves nothing.
pub struct Exits {
    pub ran: &'static [i32],
    pub found: &'static [i32],
}

impl Exits {
    /// `findings` if `out.exit` agrees with this table, else cannot-check.
    pub fn judge(
        &self,
        tool: &str,
        out: &Output,
        findings: Vec<Finding>,
    ) -> Result<Vec<Finding>, CannotCheck> {
        let why = first_line(&out.stderr);
        let what = if out.exit == -1 {
            format!("{tool} was stopped by a signal: {why}")
        } else if !self.ran.contains(&out.exit) && !self.found.contains(&out.exit) {
            format!(
                "{tool} exited {}, not one of its result codes: {why}",
                out.exit
            )
        } else if self.found.contains(&out.exit) && findings.is_empty() {
            format!("{tool} exited {} but reported nothing: {why}", out.exit)
        } else {
            return Ok(findings);
        };
        Err(CannotCheck {
            what,
            fix: format!("run {tool} by hand in the repo to see why"),
        })
    }
}
