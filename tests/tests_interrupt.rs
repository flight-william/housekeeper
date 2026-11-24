//! house stopped while a `[tests]` suite runs (Ctrl-C, a harness's SIGTERM, a closed
//! terminal's SIGHUP) ends with that signal, and the suite's whole process group is
//! stopped with it: a suite runs in a group of its own, so the terminal's signal does
//! not reach it, and a quiet suite would otherwise live on.
//!
//! No sleep and no clock here: FIFOs order the steps. The suite's grandchild opens
//! `out.fifo` for writing and holds it; reading it to its end waits for that process
//! to die. Left alone it would print `natural` after 10 s; stopped, it prints nothing.

mod common;

use std::io::Read;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Command, Stdio};

const SUITE: &str = "(sleep 10; echo natural) > out.fifo & wait";

fn mkfifo(root: &Path, name: &str) {
    let made = Command::new("mkfifo")
        .arg(root.join(name))
        .status()
        .unwrap();
    assert!(made.success());
}

/// Run `house check --stage commit` on a suite that never ends, send `signal` to
/// house's process group as a terminal would, and give how house ended and what the
/// suite's grandchild printed before its output closed.
fn interrupted(name: &str, signal: &str) -> (Option<i32>, String) {
    let root = common::repo(name);
    let toml = format!("[tools]\ncommit = []\n[tests]\ncommit = [[\"sh\", \"-c\", {SUITE:?}]]\n");
    std::fs::write(root.join(".house.toml"), toml).unwrap();
    std::fs::write(root.join("a.txt"), "a\n").unwrap();
    common::git(&root, &["add", "-A"]);
    mkfifo(&root, "out.fifo");
    let mut house = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["check", "--stage", "commit"])
        .current_dir(&root)
        .env("HOUSE_LOG_DIR", common::fresh(&format!("{name}-log")))
        .env_remove("CLAUDECODE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    // Returns once the grandchild has opened it: the suite is running.
    let mut out = std::fs::File::open(root.join("out.fifo")).unwrap();
    let group = format!("-{}", house.id());
    let sent = Command::new("/bin/kill")
        .args(["-s", signal, "--", &group])
        .status()
        .unwrap();
    assert!(sent.success());
    let ended = house.wait().unwrap().signal();
    let mut printed = String::new();
    out.read_to_string(&mut printed).unwrap();
    (ended, printed)
}

#[test]
fn ctrl_c_stops_the_running_suites_group_and_house_ends_with_sigint() {
    let (ended, printed) = interrupted("interrupt-int", "INT");
    assert_eq!(ended, Some(2));
    assert_eq!(printed, "", "the suite's grandchild outlived house");
}

#[test]
fn sigterm_stops_the_running_suites_group_and_house_ends_with_it() {
    let (ended, printed) = interrupted("interrupt-term", "TERM");
    assert_eq!(ended, Some(15));
    assert_eq!(printed, "", "the suite's grandchild outlived house");
}

#[test]
fn sighup_stops_the_running_suites_group_and_house_ends_with_it() {
    let (ended, printed) = interrupted("interrupt-hup", "HUP");
    assert_eq!(ended, Some(1));
    assert_eq!(printed, "", "the suite's grandchild outlived house");
}

#[test]
fn sigkill_of_house_alone_still_stops_the_suites_group() {
    let (ended, printed) = interrupted("interrupt-kill", "KILL");
    assert_eq!(ended, Some(9));
    assert_eq!(printed, "", "the suite's grandchild outlived house");
}
