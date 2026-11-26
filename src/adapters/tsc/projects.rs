//! The projects tsc runs on: for each staged TypeScript file or staged
//! `tsconfig.json`, the nearest `tsconfig.json` at or above it inside root, with the
//! configs it references beside it in the same folder (Vite's earlier root, whose
//! `include` is `src`, references `tsconfig.node.json` for `vite.config.ts`). A
//! reference to another folder is not followed from there: that folder's files have
//! their own nearest config, so staging one app file does not check every library
//! the app references. A solution-style tsconfig (`"files": []` and no `include`, as
//! Vite's react-ts root now is) holds no file of its own, so it does not run, and
//! every project it references does; the walk goes deeper only through such configs.
//! Not `tsc -b`: that builds the whole graph of referenced projects, in order, and
//! writes their outputs unless told not to. A `tsc -p` run writes only build info, for
//! an `incremental` or `composite` project, and that goes outside the repo (`scratch`
//! in tsc.rs).

use crate::adapter::{Context, TS_EXTENSIONS};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

pub(super) const TSCONFIG: &str = "tsconfig.json";

/// A file tsc checks: a TypeScript extension.
pub(super) fn is_typescript(file: &str) -> bool {
    TS_EXTENSIONS.iter().any(|ext| file.ends_with(ext))
}

/// The projects to run, relative to root, first seen first, no repeats.
pub(super) fn projects(ctx: &Context) -> Vec<String> {
    let mut found = Vec::new();
    let mut seen = Vec::new();
    let concerned = ctx.files.iter().filter(|f| {
        is_typescript(f) || Path::new(f.as_str()).file_name() == Some(TSCONFIG.as_ref())
    });
    for file in concerned {
        if let Some(nearest) = nearest(&ctx.root, file) {
            expand(&ctx.root, nearest, Reach::Nearest, &mut found, &mut seen);
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
            return Some(text(&config));
        }
        dir = d.parent();
    }
    None
}

/// How a config was reached: as a staged file's nearest, or by a reference.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reach {
    Nearest,
    Referenced,
}

/// Add `config` unless it is solution-style. Follow its references when it is
/// solution-style, and, when it is the nearest config, those beside it in its folder.
/// A config house cannot read runs as it is: tsc reports what is wrong with it.
fn expand(
    root: &Path,
    config: String,
    reach: Reach,
    found: &mut Vec<String>,
    seen: &mut Vec<(String, Reach)>,
) {
    let key = (config.clone(), reach);
    if seen.contains(&key) {
        return;
    }
    seen.push(key);
    let value = std::fs::read_to_string(root.join(&config))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&jsonc(&t)).ok());
    let solution = value.as_ref().is_some_and(|v| {
        v.get("files")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
            && v.get("include").is_none()
    });
    if !solution && !found.contains(&config) {
        found.push(config.clone());
    }
    let dir = Path::new(&config)
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();
    for reference in value.iter().flat_map(references) {
        let target = resolve(root, &dir, reference);
        let beside = Path::new(&target).parent() == Some(dir.as_path());
        if solution || (reach == Reach::Nearest && beside) {
            expand(root, target, Reach::Referenced, found, seen);
        }
    }
}

/// The `path` of each entry of `references`.
fn references(config: &Value) -> Vec<&str> {
    config
        .get("references")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| r.get("path")?.as_str())
        .collect()
}

/// A reference's tsconfig, relative to root: `path` from the referencing config's
/// folder; a folder (or any path not naming a `.json` file) means its tsconfig.json.
fn resolve(root: &Path, dir: &Path, path: &str) -> String {
    let joined = normal(&dir.join(path));
    if path.ends_with(".json") && !root.join(&joined).is_dir() {
        text(&joined)
    } else {
        text(&joined.join(TSCONFIG))
    }
}

/// `path` with `.` dropped and each `..` taken against the folder before it.
fn normal(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir
                if matches!(out.components().next_back(), Some(Component::Normal(_))) =>
            {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

fn text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// JSON with comments (`//`, `/* */`) and trailing commas, as tsconfig files are
/// written, made plain JSON, without a leading UTF-8 byte order mark (which tsc
/// accepts and serde_json does not). Strings are kept as they are.
pub(super) fn jsonc(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                out.push(c);
                while let Some(s) = chars.next() {
                    out.push(s);
                    match s {
                        '\\' => out.extend(chars.next()),
                        '"' => break,
                        _ => {}
                    }
                }
            }
            '/' if chars.peek() == Some(&'/') => while chars.next_if(|&n| n != '\n').is_some() {},
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut last = ' ';
                for n in chars.by_ref() {
                    if last == '*' && n == '/' {
                        break;
                    }
                    last = n;
                }
            }
            ']' | '}' => {
                let kept = out.trim_end().len();
                if out[..kept].ends_with(',') {
                    out.truncate(kept - 1);
                }
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}
