//! The approval check (R9): a protected file changed needs the owner's approval line
//! in the commit message. `check` is pure: paths and a message in, findings out.
//! `check_in` also reads the disk: a changed link named as a folder of a protected
//! glob (`.cargo` linked to another folder) reaches the protected files through it.

use crate::adapter::CannotCheck;
use crate::config;
use crate::excuses::Date;
use crate::finding::{Finding, Place};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Files only the owner may change, compared ignoring case, the Unicode way, and as the
/// filesystem resolves a name (case-insensitive filesystems can resolve `.Ruff.toml`,
/// `.ſwiftformat` and `.ruff.toml` to the same file). The tools' settings count by name in any folder,
/// since each tool reads the nearest one. The one definition: `house init` writes
/// these, with `.house.toml`'s `[protected] extra`, into CODEOWNERS through
/// `init::PROTECTED`, a re-export of this static. The JavaScript tools' settings are
/// listed by the extensions the tools load, so a note named like one is not protected;
/// ESLint's are the flat config only, the one kind house runs.
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
    "**/.swiftformat",
    "**/.coveragerc",
    "**/setup.cfg",
    "**/.semgrepignore",
    "**/.gitleaks.toml",
    "**/.gitleaksignore",
    "**/.shellcheckrc",
    "**/shellcheckrc",
    "**/deny.toml",
    "**/.cargo/config.toml",
    "**/.cargo/config",
    "**/package.json",
    "**/eslint.config.js",
    "**/eslint.config.mjs",
    "**/eslint.config.cjs",
    "**/eslint.config.ts",
    "**/eslint.config.mts",
    "**/eslint.config.cts",
    "**/tsconfig.json",
    "**/tsconfig.*.json",
    "**/.prettierrc",
    "**/.prettierrc.json",
    "**/.prettierrc.yaml",
    "**/.prettierrc.yml",
    "**/.prettierrc.json5",
    "**/.prettierrc.toml",
    "**/.prettierrc.js",
    "**/.prettierrc.mjs",
    "**/.prettierrc.cjs",
    "**/.prettierrc.ts",
    "**/.prettierrc.mts",
    "**/.prettierrc.cts",
    "**/prettier.config.js",
    "**/prettier.config.mjs",
    "**/prettier.config.cjs",
    "**/prettier.config.ts",
    "**/prettier.config.mts",
    "**/prettier.config.cts",
    "**/.prettierignore",
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

/// A name folded as macOS folds it: Unicode case with the full mapping, so the long s
/// (U+017F) and the Kelvin sign (U+212A) fold to `s` and `k`, as `S` and `K` do.
fn fold(text: &str) -> String {
    text.to_uppercase().to_lowercase()
}

/// A file's identity on disk (device, inode), its own entry, links not followed.
fn identity(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    Some((meta.dev(), meta.ino()))
}

/// `path` with each segment the filesystem resolves to a literal name of `protected`
/// spelled as the glob spells it. Only a segment with a non-ASCII character is asked:
/// ASCII case is `fold`'s, and macOS reads `Re\u{301}glages` as `Réglages`, or a
/// spelling folds miss, as the file it opens. A path not on disk stays as it is.
fn resolved(root: Option<&Path>, path: &str, protected: &[String]) -> String {
    let Some(root) = root.filter(|_| !path.is_ascii()) else {
        return path.to_string();
    };
    let names: Vec<&str> = protected
        .iter()
        .flat_map(|g| g.split('/'))
        .filter(|seg| !seg.contains('*'))
        .collect();
    let segs: Vec<&str> = path.split('/').collect();
    let spelled: Vec<&str> = segs
        .iter()
        .enumerate()
        .map(|(i, seg)| {
            let dir = root.join(segs[..i].join("/"));
            let same = |name: &&str| {
                *name != *seg && identity(&dir.join(name)) == identity(&dir.join(seg))
            };
            if seg.is_ascii() || identity(&dir.join(seg)).is_none() {
                return *seg;
            }
            names.iter().copied().find(same).unwrap_or(seg)
        })
        .collect();
    spelled.join("/")
}

/// The first glob of `protected` that `path` is, ignoring case (`fold`) and, with
/// `root`, spelling the filesystem resolves; with whether case alone made it match.
fn protected_as<'a>(
    root: Option<&Path>,
    path: &str,
    protected: &'a [String],
) -> Option<(&'a String, bool)> {
    let folded = fold(path);
    let disk = fold(&resolved(root, path, protected));
    protected.iter().find_map(|g| {
        let glob = fold(g);
        if glob_matches(&glob, &folded) {
            Some((g, true))
        } else {
            glob_matches(&glob, &disk).then_some((g, false))
        }
    })
}

/// Whether `path`, under `root`, is a link that stands for a folder of a glob of
/// `protected` (`reaches`), ignoring case.
fn link_to_protected(root: Option<&Path>, path: &str, protected: &[String]) -> bool {
    if !root.is_some_and(|r| is_link(&r.join(path))) {
        return false;
    }
    let folded = fold(path);
    protected.iter().any(|g| reaches(&fold(g), &folded))
}

fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// Whether a link at `path` stands for a folder of `glob`: `path` matches the glob
/// cut before one of its later segments, the cut ending in a named segment, not `**`.
/// A link `.cargo` to another folder reaches `.cargo/config.toml` through it.
fn reaches(glob: &str, path: &str) -> bool {
    let segs: Vec<&str> = glob.split('/').collect();
    (1..segs.len()).any(|n| segs[n - 1] != "**" && glob_matches(&segs[..n].join("/"), path))
}

/// One R9 finding per changed protected path, unless the message has the approval
/// line. Paths and globs are compared ignoring case, the Unicode way (`fold`).
pub fn check(changed: &[String], protected: &[String], message: &str) -> Vec<Finding> {
    judge(None, changed, protected, message)
}

/// `check`, with each path also judged as the filesystem under `root` resolves it: a
/// spelling that opens a protected file is that file.
pub fn check_in(
    root: &Path,
    changed: &[String],
    protected: &[String],
    message: &str,
) -> Vec<Finding> {
    judge(Some(root), changed, protected, message)
}

/// `check_in` for a commit read from git: `links`, the changed paths the commit's
/// trees hold as links (mode 120000), count as links whatever the disk holds now.
pub fn check_commit(
    root: &Path,
    changed: &[String],
    links: &[String],
    protected: &[String],
    message: &str,
) -> Vec<Finding> {
    let mut found = judge(Some(root), changed, protected, message);
    if approved(message) {
        return found;
    }
    for path in links.iter().filter(|p| changed.contains(p)) {
        let folded = fold(path);
        let reached = protected.iter().any(|g| reaches(&fold(g), &folded));
        if reached && !found.iter().any(|f| f.place == Place::File(path.clone())) {
            found.push(unapproved(path));
        }
    }
    found
}

fn unapproved(path: &str) -> Finding {
    Finding {
        rule: "R9".to_string(),
        place: Place::File(path.to_string()),
        line: None,
        what: "protected file changed without the owner's approval line".to_string(),
        fix: "add Owner-approved: <date> \"<the owner's words>\" to the commit message, quoting the owner".to_string(),
        tool_rule: "approval".to_string(),
        measure: None,
    }
}

fn judge(
    root: Option<&Path>,
    changed: &[String],
    protected: &[String],
    message: &str,
) -> Vec<Finding> {
    if approved(message) {
        return Vec::new();
    }
    changed
        .iter()
        .filter(|path| {
            protected_as(root, path, protected).is_some()
                || link_to_protected(root, path, protected)
        })
        .map(|path| unapproved(path))
        .collect()
}

/// One R9 finding per path of `changed` that is a glob of `protected` only when case
/// is ignored (`fold`), whatever the commit message says. A case-insensitive
/// filesystem lets `.SEMGREPIGNORE` or `.ſemgrepignore` act as `.semgrepignore`, while
/// GitHub CODEOWNERS matches paths case-sensitively, so the owner is never asked to
/// review it.
pub fn wrong_case(changed: &[String], protected: &[String]) -> Vec<Finding> {
    misnamed(None, changed, protected)
}

/// `wrong_case`, and each path the filesystem under `root` resolves to a protected
/// file under another spelling.
pub fn wrong_case_in(root: &Path, changed: &[String], protected: &[String]) -> Vec<Finding> {
    misnamed(Some(root), changed, protected)
}

fn misnamed(root: Option<&Path>, changed: &[String], protected: &[String]) -> Vec<Finding> {
    changed
        .iter()
        .filter_map(|path| {
            let (glob, by_case) = protected_as(root, path, protected)?;
            if protected.iter().any(|g| glob_matches(g, path)) {
                return None;
            }
            let name = canonical(glob, &resolved(root, path, protected));
            let how = if glob == &glob.to_lowercase() {
                "canonical lowercase name"
            } else {
                "canonical name"
            };
            let what = if by_case {
                "settings file name in the wrong case"
            } else {
                "settings file name in another spelling"
            };
            Some(Finding {
                rule: "R9".to_string(),
                place: Place::File(path.clone()),
                line: None,
                what: format!("{what}: {path}"),
                fix: format!("use the {how} {name}"),
                tool_rule: "settings-case".to_string(),
                measure: None,
            })
        })
        .collect()
}

/// `path` spelled as `glob` spells it: each segment equal, ignoring case, to one of
/// the glob's literal segments takes the glob's spelling; failing that, lowercase.
fn canonical(glob: &str, path: &str) -> String {
    let literals: Vec<&str> = glob.split('/').filter(|s| !s.contains('*')).collect();
    let spelled = path
        .split('/')
        .map(|seg| {
            literals
                .iter()
                .find(|l| fold(l) == fold(seg))
                .map_or(seg, |l| *l)
        })
        .collect::<Vec<_>>()
        .join("/");
    if glob_matches(glob, &spelled) {
        spelled
    } else {
        fold(path)
    }
}
