//! `.house.toml` is read strictly: a misspelled table or key is a cannot-check that
//! names it, never a setting quietly ignored.

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
