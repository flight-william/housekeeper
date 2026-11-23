//! Tool settings `house init` writes, each only when it fits the files present and the
//! repo has none: a tool's own default must not turn code that already follows one
//! style into thousands of findings.
//!
//! Today that is `.swift-format`, with the indentation most of the repo's Swift files
//! use (a number of spaces, or tabs), and swift-format's defaults for the rest.

use crate::adapter::CannotCheck;
use crate::files::tracked;
use std::collections::BTreeMap;
use std::path::Path;

pub const SWIFT_FORMAT: &str = ".swift-format";

/// One file's indentation step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Indent {
    Spaces(usize),
    Tabs,
}

/// The line to print about `.swift-format`, or `None` when the repo has no Swift file.
pub fn swift_format(root: &Path) -> Result<Option<String>, CannotCheck> {
    let swift: Vec<String> = tracked(root)?
        .into_iter()
        .filter(|f| f.ends_with(".swift"))
        .collect();
    if swift.is_empty() {
        return Ok(None);
    }
    let path = root.join(SWIFT_FORMAT);
    if path.exists() {
        return Ok(Some(format!(
            "house init: {SWIFT_FORMAT} already exists; left as it is"
        )));
    }
    let texts = swift
        .iter()
        .filter_map(|f| std::fs::read_to_string(root.join(f)).ok());
    let Some(indent) = most_used(texts.filter_map(|t| indent_of(&t))) else {
        return Ok(Some(format!(
            "house init: no {SWIFT_FORMAT} written: the Swift files show no indentation to match"
        )));
    };
    let (json, shown) = match indent {
        Indent::Spaces(n) => (serde_json::json!({ "spaces": n }), format!("{n} spaces")),
        Indent::Tabs => (serde_json::json!({ "tabs": 1 }), "tabs".to_string()),
    };
    let setting = serde_json::json!({ "version": 1, "indentation": json });
    let text = format!("{setting:#}\n");
    std::fs::write(&path, text).map_err(|e| CannotCheck {
        what: format!("{} cannot be written: {e}", path.display()),
        fix: "check the folder's permissions".into(),
    })?;
    Ok(Some(format!(
        "house init: wrote {SWIFT_FORMAT}: indentation {shown}, as the Swift files use"
    )))
}

/// The indentation most files use; on a tie, the smaller step.
fn most_used(found: impl Iterator<Item = Indent>) -> Option<Indent> {
    let mut votes: BTreeMap<Indent, usize> = BTreeMap::new();
    for indent in found {
        *votes.entry(indent).or_default() += 1;
    }
    let most = *votes.values().max()?;
    votes.into_iter().find(|(_, n)| *n == most).map(|(i, _)| i)
}

/// One file's step: tabs when more lines start with a tab than with a space; else the
/// most common rise in leading spaces from one non-blank line to the next (2 or more,
/// so a ` * ` doc-comment line does not count). `None` with nothing indented.
fn indent_of(text: &str) -> Option<Indent> {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let tabs = lines.iter().filter(|l| l.starts_with('\t')).count();
    let spaces = lines.iter().filter(|l| l.starts_with(' ')).count();
    if tabs > spaces {
        return Some(Indent::Tabs);
    }
    let depth = |l: &str| l.len() - l.trim_start_matches(' ').len();
    let rises = lines
        .windows(2)
        .map(|w| depth(w[1]).saturating_sub(depth(w[0])))
        .filter(|rise| *rise >= 2)
        .map(Indent::Spaces);
    most_used(rises)
}
