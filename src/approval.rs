//! The approval check (R9): a protected file changed needs the owner's approval line
//! in the commit message. `check` is pure: paths and a message in, findings out.

use crate::adapter::CannotCheck;
use crate::config;
use crate::excuses::Date;
use crate::finding::{Finding, Place};
use std::path::Path;

/// Files only the owner may change, compared case-insensitively (case-insensitive
/// filesystems can resolve `.Ruff.toml` and `.ruff.toml` to the same file). The tools' settings count by name in any folder,
/// since each tool reads the nearest one. The one definition: `house init` writes
/// these, with `.house.toml`'s `[protected] extra`, into CODEOWNERS through
/// `init::PROTECTED`, a re-export of this static.
pub static PROTECTED: &[&str] = &[
    ".house.toml",
    ".house-excuses.toml",
    ".github/workflows/**",
    ".githooks/**",
    "**/CODEOWNERS",
    "**/pyproject.toml",
    "**/ruff.toml",
    "**/.ruff.toml",
    "**/clippy.toml",
    "**/.clippy.toml",
    "**/rustfmt.toml",
    "**/.rustfmt.toml",
    "**/.swiftlint.yml",
    "**/.swift-format",
    "**/.coveragerc",
    "**/setup.cfg",
    "**/.semgrepignore",
    "**/.gitleaks.toml",
    "**/deny.toml",
    "**/.cargo/config.toml",
    "**/.cargo/config",
];

/// `PROTECTED` and the globs `.house.toml` adds under `[protected] extra`.
pub fn protected(root: &Path) -> Result<Vec<String>, CannotCheck> {
    let mut list: Vec<String> = PROTECTED.iter().map(|g| g.to_string()).collect();
    list.extend(config::load(root)?.protected.extra);
    Ok(list)
}

/// The message carries `Owner-approved: YYYY-MM-DD "<the owner's words>"` on a real day.
fn approved(message: &str) -> bool {
    message.lines().any(|line| {
        let Some(rest) = line.strip_prefix("Owner-approved: ") else {
            return false;
        };
        let (day, words) = rest.split_at_checked(10).unwrap_or(("", ""));
        let quoted = words
            .strip_prefix(" \"")
            .and_then(|w| w.strip_suffix('"'))
            .is_some_and(|w| !w.is_empty() && !w.contains('"'));
        Date::parse(day).is_some() && quoted
    })
}

/// One segment against a pattern segment, `*` matching any run of characters.
fn segment_matches(pat: &str, seg: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return pat == seg;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if seg.len() < first.len() + last.len() || !seg.starts_with(first) || !seg.ends_with(last) {
        return false;
    }
    let mut rest = &seg[first.len()..seg.len() - last.len()];
    for mid in &parts[1..parts.len() - 1] {
        match rest.find(mid) {
            Some(i) => rest = &rest[i + mid.len()..],
            None => return false,
        }
    }
    true
}

fn segments_match(pat: &[&str], path: &[&str]) -> bool {
    match pat.split_first() {
        None => path.is_empty(),
        // `**` matches zero or more whole folders; a trailing `**` needs at least one.
        Some((&"**", rest)) => {
            if rest.is_empty() {
                return !path.is_empty();
            }
            (0..=path.len()).any(|skip| segments_match(rest, &path[skip..]))
        }
        Some((p, rest)) => match path.split_first() {
            Some((s, tail)) => segment_matches(p, s) && segments_match(rest, tail),
            None => false,
        },
    }
}

/// Simple glob: `**/` any folders, `*` within one name, otherwise exact.
pub fn glob_matches(glob: &str, path: &str) -> bool {
    let pat: Vec<&str> = glob.split('/').collect();
    let segs: Vec<&str> = path.split('/').collect();
    segments_match(&pat, &segs)
}

/// One R9 finding per changed protected path, unless the message has the approval
/// line. Paths and globs are compared case-insensitively.
pub fn check(changed: &[String], protected: &[String], message: &str) -> Vec<Finding> {
    if approved(message) {
        return Vec::new();
    }
    let protected: Vec<String> = protected.iter().map(|g| g.to_lowercase()).collect();
    changed
        .iter()
        .filter(|path| {
            let path = path.to_lowercase();
            protected.iter().any(|g| glob_matches(g, &path))
        })
        .map(|path| Finding {
            rule: "R9".to_string(),
            place: Place::File(path.clone()),
            line: None,
            what: "protected file changed without the owner's approval line".to_string(),
            fix: "add Owner-approved: <date> \"<the owner's words>\" to the commit message, quoting the owner".to_string(),
            tool_rule: "approval".to_string(),
            measure: None,
        })
        .collect()
}
