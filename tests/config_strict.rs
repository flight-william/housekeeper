//! `.house.toml` is read strictly: a misspelled table or key is a cannot-check that
//! names it, never a setting quietly ignored.

mod common;

use house::config;

fn refused(text: &str) -> String {
    match config::parse(text) {
        Ok(_) => panic!("accepted: {text}"),
        Err(c) => c.what,
    }
}

#[test]
fn an_unknown_table_is_cannot_check_naming_it() {
    let what = refused("[tool]\ncommit = [\"ruff\"]\n");
    assert!(
        what.contains("unknown") && what.contains("`tool`"),
        "{what}"
    );
}

#[test]
fn an_unknown_key_in_a_known_table_is_cannot_check_naming_it_and_its_line() {
    let what = refused("[tools]\ncommit = [\"ruff\"]\ncomit = [\"ruff\"]\n");
    assert!(what.contains("`comit`"), "{what}");
    assert!(what.contains("line 3"), "{what}");
}

#[test]
fn an_unknown_key_in_every_table_is_refused() {
    for table in ["commit", "protected", "paths"] {
        let what = refused(&format!("[{table}]\nextra_key = 1\n"));
        assert!(what.contains("`extra_key`"), "[{table}]: {what}");
    }
}

#[test]
fn every_known_table_and_key_is_accepted() {
    let text = "[tools]\ncommit = [\"a\"]\npush = [\"a\"]\n\
        [commit]\nmodels = [\"m\"]\nauthor = \"A\"\nemail = \"a@example.com\"\n\
        [protected]\nextra = [\"x\"]\n[paths]\nexclude = [\"y/\"]\n";
    config::parse(text).unwrap();
}

// ---- [house] minimum: the oldest house the repo's rules run on ----

#[test]
fn an_older_house_than_the_minimum_cannot_check_naming_both_versions() {
    let c = config::minimum_met("0.2.3", "0.2.2").unwrap_err();
    assert!(
        c.what.contains("0.2.3") && c.what.contains("0.2.2"),
        "{}",
        c.what
    );
    assert!(c.fix.contains("upgrade"), "{}", c.fix);
    assert!(
        config::minimum_met("0.10.0", "0.9.9").is_err(),
        "by number, not text"
    );
    assert!(config::minimum_met("1.0.0", "0.99.99").is_err());
}

#[test]
fn the_minimum_itself_or_a_newer_house_runs() {
    for running in ["0.2.3", "0.2.4", "0.3.0", "0.10.0", "1.0.0"] {
        assert_eq!(config::minimum_met("0.2.3", running), Ok(()), "{running}");
    }
}

#[test]
fn a_minimum_that_is_not_a_version_cannot_check() {
    for bad in ["0.2", "v0.2.3", "0.2.3-beta", "0.2.x", "", "0.2.3.4"] {
        let what = refused(&format!("[house]\nminimum = \"{bad}\"\n"));
        assert!(what.contains("minimum"), "{bad:?}: {what}");
    }
}

#[test]
fn the_house_table_is_read_strictly_and_checked_against_this_house() {
    let this = env!("CARGO_PKG_VERSION");
    let config = config::parse(&format!("[house]\nminimum = \"{this}\"\n")).unwrap();
    assert_eq!(config.house.minimum.as_deref(), Some(this));
    let what = refused("[house]\nminimum = \"99.0.0\"\n");
    assert!(what.contains("99.0.0") && what.contains(this), "{what}");
    let what = refused("[house]\nminimal = \"0.2.3\"\n");
    assert!(what.contains("`minimal`"), "{what}");
}

#[test]
fn the_binary_refuses_a_repo_that_asks_for_a_newer_house() {
    let dir = common::repo("minimum");
    std::fs::write(
        dir.join(".house.toml"),
        "[house]\nminimum = \"99.0.0\"\n[tools]\ncommit = []\n",
    )
    .unwrap();
    common::git(&dir, &["add", "-A"]);
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "commit"])
        .current_dir(&dir)
        .env("HOUSE_LOG_DIR", dir.join("log"))
        .env_remove("CLAUDECODE")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(2), "{text}");
    assert!(
        text.contains("99.0.0") && text.contains(env!("CARGO_PKG_VERSION")),
        "{text}"
    );
}
