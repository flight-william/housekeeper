//! R1 for Rust: `cargo fmt --check`, one finding per "Diff in <path>:<line>:" block.

use super::{Exits, listed, path_value};
use crate::adapter::{Adapter, CannotCheck, Command, Context, Output};
use crate::finding::{Finding, Place};
use std::path::Path;

pub struct CargoFmt;

/// `cargo fmt --check` exits 0 formatted, 1 with diffs; 1 with no diff is a
/// file rustfmt could not parse, reported with its own message below.
const EXITS: Exits = Exits {
    ran: &[0],
    found: &[1],
};

/// The manifests to run on, one per workspace: for each staged `.rs` file or
/// `Cargo.toml`, the nearest folder at or above it (inside root) that has a
/// `Cargo.toml`, then that crate's workspace root (`workspace_root`); first seen
/// first, no repeats. Relative to root, e.g. "Cargo.toml" or "tools/inner/Cargo.toml".
/// One run per workspace: a run per member crate also lints the members it
/// depends on, so their findings came back once per run.
pub(crate) fn manifests(ctx: &Context) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut resolved: Vec<(String, String)> = Vec::new();
    let rust = ctx
        .files
        .iter()
        .filter(|f| f.ends_with(".rs") || *f == "Cargo.toml" || f.ends_with("/Cargo.toml"));
    for file in rust {
        let Some(nearest) = nearest_manifest(&ctx.root, file) else {
            continue;
        };
        let m = match resolved.iter().find(|(n, _)| *n == nearest) {
            Some((_, m)) => m.clone(),
            None => {
                let m = workspace_root(&ctx.root, &nearest);
                resolved.push((nearest, m.clone()));
                m
            }
        };
        if !found.contains(&m) {
            found.push(m);
        }
    }
    found
}

/// The nearest `Cargo.toml` at or above `file`'s folder, inside root.
fn nearest_manifest(root: &Path, file: &str) -> Option<String> {
    let mut dir = Path::new(file).parent();
    while let Some(d) = dir {
        let manifest = d.join("Cargo.toml");
        if root.join(&manifest).is_file() {
            return Some(manifest.to_string_lossy().replace('\\', "/"));
        }
        dir = d.parent();
    }
    None
}

/// Cargo's own lookup: the crate's manifest if it has a `[workspace]` table, else
/// the nearest manifest above it (inside root) with one, unless that workspace's
/// `exclude` names the crate's folder; else the crate's own manifest. Cargo refuses
/// a crate under a workspace that neither lists nor excludes it, so a crate below
/// a `[workspace]` that does not exclude it is a member.
fn workspace_root(root: &Path, manifest: &str) -> String {
    let crate_dir = Path::new(manifest).parent().unwrap_or(Path::new(""));
    let mut dir = Some(crate_dir);
    while let Some(d) = dir {
        let candidate = d.join("Cargo.toml");
        if let Some(workspace) = workspace_table(&root.join(&candidate)) {
            let inside = crate_dir.strip_prefix(d).unwrap_or(crate_dir);
            if d == crate_dir || !excludes(&workspace, inside) {
                return candidate.to_string_lossy().replace('\\', "/");
            }
            return manifest.to_string();
        }
        dir = d.parent();
    }
    manifest.to_string()
}

/// The `[workspace]` table of the manifest at `path`, if it is readable and has one.
fn workspace_table(path: &Path) -> Option<toml::Table> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut table: toml::Table = toml::from_str(&text).ok()?;
    match table.remove("workspace")? {
        toml::Value::Table(t) => Some(t),
        _ => None,
    }
}

/// Whether `exclude` names `inside` (the crate's folder relative to the workspace)
/// or a folder above it.
fn excludes(workspace: &toml::Table, inside: &Path) -> bool {
    let Some(toml::Value::Array(list)) = workspace.get("exclude") else {
        return false;
    };
    list.iter()
        .filter_map(toml::Value::as_str)
        .any(|e| inside.starts_with(e.trim_start_matches("./").trim_end_matches('/')))
}

/// A path the tool printed, made relative to the repo root with `/` separators.
pub(crate) fn relative(root: &Path, path: &str) -> String {
    let p = Path::new(path);
    let rel = p.strip_prefix(root).unwrap_or(p);
    rel.to_string_lossy().replace('\\', "/")
}

impl Adapter for CargoFmt {
    fn name(&self) -> &'static str {
        "cargo-fmt"
    }

    fn program(&self) -> &'static str {
        "cargo"
    }

    fn brew_formula(&self) -> &'static str {
        "rust"
    }

    fn command(&self, ctx: &Context) -> Option<Command> {
        self.commands(ctx).into_iter().next()
    }

    fn commands(&self, ctx: &Context) -> Vec<Command> {
        let to_command = |m: String| Command {
            program: "cargo".into(),
            args: vec![
                "fmt".into(),
                "--check".into(),
                "--manifest-path".into(),
                path_value(m),
            ],
        };
        manifests(ctx).into_iter().map(to_command).collect()
    }

    fn parse(&self, ctx: &Context, out: &Output) -> Result<Vec<Finding>, CannotCheck> {
        let findings: Vec<Finding> = out
            .stdout
            .lines()
            .filter_map(|l| diff_place(&ctx.root, l))
            .map(|(file, line)| Finding {
                rule: "R1".into(),
                place: Place::File(file),
                line: Some(line),
                what: "not formatted".into(),
                fix: "run cargo fmt".into(),
                tool_rule: "rustfmt".into(),
                measure: None,
            })
            .collect();
        if findings.is_empty() && out.exit != 0 {
            return Err(CannotCheck {
                what: format!("cargo fmt failed: {}", first_line(&out.stderr)),
                fix: "fix the error cargo fmt reports, then run it again".into(),
            });
        }
        let findings = EXITS.judge("cargo-fmt", out, findings)?;
        Ok(listed(ctx, findings))
    }
}

/// "Diff in /abs/src/lib.rs:89:" gives ("src/lib.rs", 89).
fn diff_place(root: &Path, line: &str) -> Option<(String, u32)> {
    let start = line.find("Diff in ")? + "Diff in ".len();
    let rest = line[start..].trim_end().strip_suffix(':')?;
    let (path, num) = rest.rsplit_once(':')?;
    Some((relative(root, path), num.parse().ok()?))
}

fn first_line(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty());
    line.unwrap_or("no output")
        .trim_end_matches('.')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::Stage;
    use std::path::PathBuf;

    const PLANTED: &str = include_str!("../../tests/fixtures/cargo_fmt/planted.stdout");
    const BROKEN: &str = include_str!("../../tests/fixtures/cargo_fmt/broken.stderr");
    const NESTED: &str = include_str!("../../tests/fixtures/cargo_fmt/nested.stdout");

    fn ctx(root: &str, files: &[&str]) -> Context {
        Context {
            root: PathBuf::from(root),
            stage: Stage::Commit,
            files: files.iter().map(|f| f.to_string()).collect(),
            push_range: None,
            languages: Default::default(),
        }
    }

    fn out(exit: i32, stdout: &str, stderr: &str) -> Output {
        Output {
            exit,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    const ROOT: &str = "/tmp/house-fixture/planted";

    #[test]
    fn planted_diffs_give_one_r1_finding_per_block() {
        let found = CargoFmt
            .parse(&ctx(ROOT, &["src/lib.rs"]), &out(1, PLANTED, ""))
            .unwrap();
        let places: Vec<_> = found.iter().map(|f| (f.place.clone(), f.line)).collect();
        let lib = Place::File("src/lib.rs".into());
        assert_eq!(places, vec![(lib.clone(), Some(1)), (lib, Some(89))]);
        assert_eq!(found[0].rule, "R1");
        assert_eq!(found[0].what, "not formatted");
        assert_eq!(found[0].fix, "run cargo fmt");
        assert_eq!(found[0].tool_rule, "rustfmt");
    }

    #[test]
    fn diffs_in_files_not_staged_are_dropped() {
        let c = ctx(ROOT, &["src/main.rs", "Cargo.toml"]);
        assert_eq!(CargoFmt.parse(&c, &out(1, PLANTED, "")), Ok(vec![]));
    }

    #[test]
    fn clean_gives_no_findings() {
        assert_eq!(CargoFmt.parse(&ctx(ROOT, &[]), &out(0, "", "")), Ok(vec![]));
    }

    #[test]
    fn a_parse_error_cannot_check() {
        let err = CargoFmt
            .parse(&ctx(ROOT, &[]), &out(1, "", BROKEN))
            .unwrap_err();
        assert_eq!(
            err.what,
            "cargo fmt failed: error: this file contains an unclosed delimiter"
        );
    }

    #[test]
    fn runs_only_for_rust_files_in_a_cargo_repo() {
        let here = env!("CARGO_MANIFEST_DIR");
        let cmd = CargoFmt.command(&ctx(here, &["src/main.rs"])).unwrap();
        assert_eq!(cmd.program, "cargo");
        assert_eq!(
            cmd.args,
            vec!["fmt", "--check", "--manifest-path", "Cargo.toml"]
        );
        assert!(CargoFmt.command(&ctx(here, &["Cargo.toml"])).is_some());
        assert!(CargoFmt.command(&ctx(here, &["README.md"])).is_none());
        let elsewhere = format!("{here}/no-such-folder");
        assert!(CargoFmt.command(&ctx(&elsewhere, &["src/a.rs"])).is_none());
    }

    /// A temp repo: an empty root, a workspace in tools/inner, a crate in other/b.
    fn nested_repo(name: &str) -> PathBuf {
        let root = crate::temp::fresh(&format!("nested-{name}"));
        for dir in ["tools/inner/crates/a/src", "other/b/src", "loose"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        for manifest in [
            "tools/inner/Cargo.toml",
            "tools/inner/crates/a/Cargo.toml",
            "other/b/Cargo.toml",
        ] {
            std::fs::write(root.join(manifest), "").unwrap();
        }
        root
    }

    #[test]
    fn one_command_per_nearest_workspace_below_root() {
        let root = nested_repo("fmt");
        let files = [
            "tools/inner/crates/a/src/lib.rs",
            "other/b/src/main.rs",
            "other/b/src/x.rs",
            "tools/inner/build.rs",
            "loose/no_manifest.rs",
            "README.md",
        ];
        let c = ctx(root.to_str().unwrap(), &files);
        assert_eq!(
            manifests(&c),
            [
                "tools/inner/crates/a/Cargo.toml",
                "other/b/Cargo.toml",
                "tools/inner/Cargo.toml"
            ]
        );
        let cmds = CargoFmt.commands(&c);
        assert_eq!(cmds.len(), 3);
        assert_eq!(
            cmds[1].args,
            ["fmt", "--check", "--manifest-path", "other/b/Cargo.toml"]
        );
        assert_eq!(CargoFmt.command(&c), Some(cmds[0].clone()));
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// Members of a `[workspace]` share its root; a crate the workspace excludes,
    /// and a crate under a manifest with no `[workspace]`, keep their own.
    #[test]
    fn members_run_from_their_workspace_root() {
        let root = crate::temp::fresh("ws-root");
        for dir in [
            "ws/crates/a/src",
            "ws/crates/b/src",
            "ws/solo/src",
            "plain/c/src",
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        let package = "[package]\nname = \"x\"\n";
        for (manifest, text) in [
            (
                "ws/Cargo.toml",
                "[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"solo\"]\n",
            ),
            ("ws/crates/a/Cargo.toml", package),
            ("ws/crates/b/Cargo.toml", package),
            ("ws/solo/Cargo.toml", package),
            ("plain/Cargo.toml", package),
            ("plain/c/Cargo.toml", package),
        ] {
            std::fs::write(root.join(manifest), text).unwrap();
        }
        let files = [
            "ws/crates/a/src/lib.rs",
            "ws/crates/b/src/lib.rs",
            "ws/crates/b/Cargo.toml",
            "ws/solo/src/main.rs",
            "plain/c/src/lib.rs",
        ];
        let c = ctx(root.to_str().unwrap(), &files);
        assert_eq!(
            manifests(&c),
            ["ws/Cargo.toml", "ws/solo/Cargo.toml", "plain/c/Cargo.toml"]
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn nested_workspace_diffs_are_relative_to_the_repo() {
        let root = "/tmp/house-fixture/nested";
        let found = CargoFmt
            .parse(
                &ctx(root, &["tools/inner/crates/a/src/lib.rs"]),
                &out(1, NESTED, ""),
            )
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].place,
            Place::File("tools/inner/crates/a/src/lib.rs".into())
        );
        assert_eq!(found[0].line, Some(2));
    }
}
