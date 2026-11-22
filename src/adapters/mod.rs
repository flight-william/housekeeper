//! One file per tool, each behind the one Adapter interface (src/adapter.rs).

use crate::adapter::{Adapter, CannotCheck, Output};
use crate::finding::Finding;
use crate::run::first_line;

pub mod cargo_deny;
pub mod cargo_fmt;
pub mod clippy;
pub mod gitleaks;
pub mod r3;
pub mod ruff;
pub mod semgrep;
pub mod shellcheck;
pub mod swift_format;
pub mod swiftlint;

/// Every adapter house knows, in reply order. Each skips itself when no file concerns it.
pub fn all() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(ruff::Ruff),
        Box::new(ruff::RuffFormat),
        Box::new(cargo_fmt::CargoFmt),
        Box::new(clippy::Clippy),
        Box::new(cargo_deny::CargoDeny),
        Box::new(swiftlint::SwiftLint),
        Box::new(swift_format::SwiftFormat),
        Box::new(shellcheck::Shellcheck),
        Box::new(gitleaks::Gitleaks),
        Box::new(semgrep::Semgrep),
        Box::new(r3::R3),
    ]
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
