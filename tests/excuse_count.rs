//! An excuse covers a bounded count of findings, 1 by default. In the last test a
//! file excused for one `# noqa` gains a second, new one, and the unchanged list must
//! not hide it. It runs the real ruff and Semgrep.

mod common;

use common::{git, repo};
use house::excuses::{Date, Excuse, apply, grew, parse};
use house::finding::{Finding, Place};
use std::path::Path;
use std::process::{Command, Stdio};

fn finding(rule: &str, file: &str, line: u32) -> Finding {
    Finding {
        rule: rule.to_string(),
        place: Place::File(file.to_string()),
        line: Some(line),
        what: "function `run` is too long".to_string(),
        fix: "split it".to_string(),
        tool_rule: "clippy::too_many_lines".to_string(),
        measure: None,
    }
}

fn today() -> Date {
    Date::parse("2026-11-22").unwrap()
}

fn list(text: &str) -> Vec<Excuse> {
    parse(text).unwrap()
}

fn malformed(text: &str) -> String {
    parse(text).unwrap_err().what
}

fn grown(base: &str, now: &str) -> Vec<String> {
    grew(&list(base), &list(now))
        .into_iter()
        .map(|f| f.what)
        .collect()
}

const ONE: &str = r#"
[[excuse]]
rule = "R3"
file = "src/old.rs"
symbol = "run"
value = 140
deadline = "2026-12-31"
reason = "old code, split in the refactor slice"
"#;

// A file-wide entry once excused every matching finding, so a new suppression in an
// excused file was hidden by the old entry. An entry now covers `count` findings.
#[test]
fn an_entry_without_a_count_excuses_one_finding() {
    let findings = vec![
        finding("R3", "src/old.rs", 10),
        finding("R3", "src/old.rs", 90),
    ];
    let out = apply(findings, &list(ONE), today());
    assert_eq!(out.excused.len(), 1);
    assert_eq!(out.excused[0].line, Some(10));
    assert_eq!(out.to_fix.len(), 1);
    assert_eq!(out.to_fix[0].line, Some(90));
}

#[test]
fn an_entry_excuses_up_to_its_count_and_no_more() {
    let three = ONE.replace("value = 140\n", "value = 140\ncount = 3\n");
    let findings: Vec<_> = (1..=4).map(|l| finding("R3", "src/old.rs", l)).collect();
    let out = apply(findings.clone(), &list(&three), today());
    assert_eq!((out.excused.len(), out.to_fix.len()), (3, 1));
    assert_eq!(out.to_fix[0].line, Some(4));
    let out = apply(findings[..2].to_vec(), &list(&three), today());
    assert_eq!((out.excused.len(), out.to_fix.len()), (2, 0));
    assert!(
        out.problems.is_empty(),
        "fewer findings than the count is not stale"
    );
    assert_eq!(list(&three)[0].count, 3);
    assert_eq!(list(ONE)[0].count, 1);
}

#[test]
fn a_count_below_one_cannot_be_checked() {
    for bad in ["0", "-1"] {
        let text = ONE.replace("value = 140\n", &format!("value = 140\ncount = {bad}\n"));
        assert!(parse(&text).is_err(), "count = {bad}");
    }
    let what = malformed(&ONE.replace("value = 140\n", "value = 140\ncount = 0\n"));
    assert!(what.contains("count"), "{what}");
}

#[test]
fn a_raised_count_is_the_list_growing_and_a_lowered_one_is_not() {
    let want = vec!["excuse list grew: R3 src/old.rs run"];
    let with = |n: &str| ONE.replace("value = 140\n", &format!("value = 140\ncount = {n}\n"));
    assert_eq!(grown(ONE, &with("2")), want, "from the default 1");
    assert_eq!(grown(&with("2"), &with("3")), want);
    assert!(grown(&with("3"), &with("2")).is_empty());
    assert!(grown(&with("2"), ONE).is_empty(), "dropped: back to 1");
    assert!(grown(ONE, &with("1")).is_empty());
}

const CONFIG: &str = "[tools]\npush = [\"ruff\", \"semgrep\"]\n";
const OLD: &str = "def old():\n    return missing_old()  # noqa: F821\n";
const NEW: &str = "\n\ndef newly_suppressed():\n    missing_name()  # noqa: F821\n";
const LIST: &str = "[[excuse]]\nrule = \"R9\"\nfile = \"hooks/old.py\"\nsymbol = \"noqa\"\ndeadline = \"2999-12-31\"\nreason = \"old suppression\"\n";

fn head(dir: &Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn commit(dir: &Path, files: &[(&str, &str)]) -> String {
    for (path, text) in files {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--no-verify", "-m", "x"]);
    head(dir)
}

fn push(dir: &Path, local: &str, remote: &str, args: &[&str]) -> (i32, String) {
    use std::io::Write;
    let log = common::fresh("excuse-count-log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "push"])
        .args(args)
        .current_dir(dir)
        .env("HOUSE_LOG_DIR", &log)
        .env_remove("CLAUDECODE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let line = format!("refs/heads/f {local} refs/heads/f {remote}\n");
    let mut pipe = child.stdin.take().unwrap();
    pipe.write_all(line.as_bytes()).unwrap();
    drop(pipe);
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    (out.status.code().unwrap_or(-1), text)
}

#[test]
fn an_existing_suppression_is_excused_and_a_new_one_in_the_same_file_is_reported() {
    let dir = repo("excuse-count");
    let base = commit(
        &dir,
        &[
            (".house.toml", CONFIG),
            (".house-excuses.toml", LIST),
            ("hooks/old.py", OLD),
        ],
    );
    // The base itself: its one suppression is excused.
    let touched = commit(&dir, &[("hooks/old.py", &format!("{OLD}\n\nX = 1\n"))]);
    let (code, text) = push(&dir, &touched, &base, &[]);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("1 excused"), "{text}");
    // A new suppressed line appended; the list is byte-for-byte the same.
    let local = commit(&dir, &[("hooks/old.py", &format!("{OLD}{NEW}"))]);
    for args in [&[][..], &["--config-from", &base]] {
        let (code, text) = push(&dir, &local, &base, args);
        assert_eq!(code, 1, "{args:?}: {text}");
        assert!(text.contains("1 to fix, 1 excused"), "{args:?}: {text}");
        assert!(text.contains("R9 hooks/old.py:6"), "{args:?}: {text}");
    }
}
