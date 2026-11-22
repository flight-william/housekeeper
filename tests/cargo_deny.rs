//! cargo-deny through house on a real crate, offline: a planted advisory database
//! (one made-up advisory for itoa) and a deny.toml that bans memchr give two R2
//! findings; a staged source file alone does not run cargo-deny.

mod common;

use house::adapter::Stage;
use house::adapters::cargo_deny::CargoDeny;
use std::path::Path;
use std::process::Command;

const ADVISORY: &str = "```toml\n[advisory]\nid = \"RUSTSEC-2099-0001\"\npackage = \"itoa\"\n\
date = \"2026-11-01\"\nurl = \"https://example.invalid/planted\"\n\n[versions]\n\
patched = [\">= 99.0.0\"]\n```\n\n# A planted advisory\n\nMade up for this test.\n";

/// cargo-deny keeps a database fetched from this URL in `<db-path>/planted-db-<hash>`.
const DB_URL: &str = "https://example.invalid/planted-db";
const DB_FOLDER: &str = "planted-db-527dc2ade3c6460c";

/// A git repo holding the one advisory, where cargo-deny looks for `DB_URL`'s copy.
fn advisory_db(dbs: &Path) {
    let db = dbs.join(DB_FOLDER);
    std::fs::create_dir_all(db.join("crates/itoa")).unwrap();
    std::fs::write(db.join("crates/itoa/RUSTSEC-2099-0001.md"), ADVISORY).unwrap();
    common::git(&db, &["init", "-q"]);
    common::git(&db, &["add", "-A"]);
    let who = ["-c", "user.name=test", "-c", "user.email=test@example.com"];
    common::git(&db, &[&who[..], &["commit", "-qm", "db"]].concat());
    common::git(&db, &["fetch", "-q", ".", "HEAD"]);
}

/// A repo with a crate in `app/` depending on itoa and memchr (both in house's own
/// lock file, so cached), its lock file, and a deny.toml banning memchr.
fn repo(name: &str) -> std::path::PathBuf {
    let repo = common::repo(name);
    let dbs = repo.join("dbs");
    advisory_db(&dbs);
    let deny = format!(
        "[bans]\ndeny = [{{ name = \"memchr\" }}]\n\n[licenses]\n\
         allow = [\"MIT\", \"Apache-2.0\", \"Unlicense\"]\n\n[advisories]\n\
         db-path = \"{}\"\ndb-urls = [\"{DB_URL}\"]\n",
        dbs.display()
    );
    let files = [
        (
            "app/Cargo.toml",
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
             license = \"MIT\"\npublish = false\n\n[dependencies]\n\
             itoa = \"=1.0.19\"\nmemchr = \"=2.8.3\"\n",
        ),
        ("app/src/lib.rs", "pub fn f() {}\n"),
        ("app/deny.toml", deny.as_str()),
        (".gitignore", "target/\ndbs/\n"),
    ];
    for (path, text) in files {
        let path = repo.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let lock = Command::new("cargo")
        .args(["generate-lockfile", "--offline", "--manifest-path"])
        .arg("app/Cargo.toml")
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(lock.status.success(), "{lock:?}");
    repo
}

fn installed() -> bool {
    let found = Command::new("cargo-deny").arg("--version").output().is_ok();
    if !found {
        eprintln!("note: cargo-deny is not installed; skipping the real cargo-deny run");
    }
    found
}

#[test]
fn a_staged_lock_file_gives_the_advisory_and_the_ban() {
    if !installed() {
        return;
    }
    let repo = repo("deny-lock");
    common::git(&repo, &["add", "-A"]);
    let report = house::cli::check(&repo, Stage::Commit, vec![Box::new(CargoDeny)]);
    let tool = &report.tools[0];
    assert!(tool.cannot.is_none(), "{:?}", tool.cannot);
    let mut rules: Vec<_> = tool.findings.iter().map(|f| f.tool_rule.as_str()).collect();
    rules.sort_unstable();
    assert_eq!(rules, ["RUSTSEC-2099-0001", "cargo-deny::banned"]);
    assert!(tool.findings.iter().all(|f| f.rule == "R2"));
}

#[test]
fn a_staged_source_file_alone_skips_cargo_deny() {
    let repo = repo("deny-src");
    common::git(&repo, &["add", "-A"]);
    common::git(&repo, &["commit", "-qm", "start"]);
    std::fs::write(repo.join("app/src/lib.rs"), "pub fn g() {}\n").unwrap();
    common::git(&repo, &["add", "-A"]);
    let report = house::cli::check(&repo, Stage::Commit, vec![Box::new(CargoDeny)]);
    assert!(report.tools[0].skipped, "{:?}", report.tools[0]);
}
