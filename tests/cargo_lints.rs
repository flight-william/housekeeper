//! R9 in Cargo.toml: an `allow` in a `[lints]` or `[workspace.lints]` table silences
//! a lint for the whole crate, as `#[allow(...)]` does for one item. One finding per
//! entry. `deny` and `forbid` are stricter, and `warn` is still an error under
//! clippy's `-D warnings`, so none of those is reported.

mod common;

use house::finding::Place;
use house::lints::allowed;
use std::process::Command;

fn found(text: &str) -> Vec<(Option<u32>, String)> {
    allowed("Cargo.toml", text)
        .unwrap()
        .into_iter()
        .map(|f| {
            assert_eq!(f.rule, "R9");
            assert_eq!(f.place, Place::File("Cargo.toml".into()));
            assert_eq!(f.tool_rule, "cargo-lints allow");
            assert!(f.fix.contains("remove"), "{}", f.fix);
            (f.line, f.what)
        })
        .collect()
}

const PACKAGE: &str = "[package]\nname = \"a\"\nversion = \"0.1.0\"\n\n";

#[test]
fn an_allow_in_lints_clippy_is_reported_at_its_line() {
    let text = format!("{PACKAGE}[lints.clippy]\ntoo_many_lines = \"allow\"\n");
    assert_eq!(
        found(&text),
        vec![(
            Some(6),
            "clippy::too_many_lines is set to allow in [lints.clippy]".to_string()
        )]
    );
}

#[test]
fn deny_forbid_and_warn_are_not_reported() {
    let text = format!(
        "{PACKAGE}[lints.rust]\nunsafe_code = \"forbid\"\n[lints.clippy]\ntoo_many_lines = \"deny\"\npedantic = {{ level = \"warn\", priority = -1 }}\n"
    );
    assert!(found(&text).is_empty());
}

#[test]
fn every_allow_entry_is_one_finding_in_every_form() {
    let text = format!(
        "{PACKAGE}[lints.clippy]\na = \"allow\"\nb = {{ level = \"allow\", priority = -1 }}\nc = \"deny\"\n\
         [lints.rust]\ndead_code = \"allow\"\n\
         [workspace.lints.clippy]\nd = \"allow\"\n\
         [workspace.lints]\nrustdoc = {{ broken = \"allow\" }}\n"
    );
    let whats: Vec<String> = found(&text).into_iter().map(|(_, w)| w).collect();
    assert_eq!(
        whats,
        vec![
            "clippy::a is set to allow in [lints.clippy]",
            "clippy::b is set to allow in [lints.clippy]",
            "dead_code is set to allow in [lints.rust]",
            "clippy::d is set to allow in [workspace.lints.clippy]",
            "rustdoc::broken is set to allow in [workspace.lints.rustdoc]",
        ]
    );
    let dotted = "lints.clippy.x = \"allow\"\n";
    assert_eq!(found(dotted).len(), 1);
    assert_eq!(found("[lints]\nworkspace = true\n").len(), 0);
}

#[test]
fn an_expect_is_reported_like_an_allow() {
    assert_eq!(found("[lints.clippy]\nx = \"expect\"\n").len(), 1);
}

#[test]
fn a_manifest_without_lints_is_clean_and_one_that_does_not_parse_cannot_check() {
    assert!(found(PACKAGE).is_empty());
    let c = allowed("sub/Cargo.toml", "[lints\n").unwrap_err();
    assert!(c.what.contains("sub/Cargo.toml"), "{}", c.what);
}

// ---- the binary: the check runs on every Cargo.toml a stage sends, whatever [tools] lists

fn house(dir: &std::path::Path) -> (i32, String) {
    let log = common::fresh("cargo-lints-log");
    let out = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "commit"])
        .current_dir(dir)
        .env("HOUSE_LOG_DIR", &log)
        .env_remove("CLAUDECODE")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn the_binary_reports_an_allow_in_a_staged_cargo_toml() {
    let dir = common::repo("cargo-lints-bin");
    std::fs::write(dir.join(".house.toml"), "[tools]\ncommit = []\n").unwrap();
    std::fs::create_dir_all(dir.join("crates/a")).unwrap();
    let manifest = format!("{PACKAGE}[lints.clippy]\ntoo_many_lines = \"allow\"\n");
    std::fs::write(dir.join("crates/a/Cargo.toml"), &manifest).unwrap();
    std::fs::write(dir.join("Cargo.toml.orig"), &manifest).unwrap();
    common::git(&dir, &["add", "-A"]);
    let (code, text) = house(&dir);
    assert_eq!(code, 1, "{text}");
    assert!(
        text.contains(
            "R9 crates/a/Cargo.toml:6: clippy::too_many_lines is set to allow in [lints.clippy]"
        ),
        "{text}"
    );
    assert!(!text.contains("Cargo.toml.orig"), "{text}");
    // An excuse naming the lint covers it, as for any finding.
    let list = "[[excuse]]\nrule = \"R9\"\nfile = \"crates/a/Cargo.toml\"\nsymbol = \"too_many_lines\"\ndeadline = \"2999-12-31\"\nreason = \"old\"\n";
    std::fs::write(dir.join(".house-excuses.toml"), list).unwrap();
    common::git(&dir, &["add", "-A"]);
    common::git(&dir, &["commit", "-q", "--no-verify", "-m", "x"]);
    std::fs::write(dir.join("crates/a/Cargo.toml"), format!("{manifest}\n")).unwrap();
    common::git(&dir, &["add", "-A"]);
    let (code, text) = house(&dir);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("1 excused"), "{text}");
}

#[test]
fn a_stage_without_a_cargo_toml_has_no_cargo_lints_line() {
    let dir = common::repo("cargo-lints-none");
    std::fs::write(dir.join(".house.toml"), "[tools]\ncommit = []\n").unwrap();
    std::fs::write(dir.join("a.txt"), "x\n").unwrap();
    common::git(&dir, &["add", "-A"]);
    let log = common::fresh("cargo-lints-none-log");
    let out = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "commit", "--json"])
        .current_dir(&dir)
        .env("HOUSE_LOG_DIR", &log)
        .env_remove("CLAUDECODE")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(!String::from_utf8_lossy(&out.stdout).contains("cargo-lints"));
}
