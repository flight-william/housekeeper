//! runner/job-started.sh, the runner's own job-started hook: it refuses any job
//! from a repository that is not private, and fails closed. It lives on the
//! runner, outside every workflow, because a fork can edit the workflow and skip
//! house.

#[path = "common/temp.rs"]
mod common;

use std::path::Path;
use std::process::{Command, Output};

const HOOK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/runner/job-started.sh");

fn hook(event: &Path) -> Output {
    Command::new("/bin/sh")
        .arg(HOOK)
        .env("GITHUB_EVENT_PATH", event)
        .output()
        .unwrap()
}

fn said(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn the_hook_lets_a_private_repository_run_and_refuses_the_rest() {
    let dir = common::fresh("hook");
    let write = |name: &str, text: &str| {
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        path
    };
    let private = write(
        "private.json",
        r#"{"repository": {"full_name": "owner/kept", "private": true, "visibility": "private"}}"#,
    );
    let public = write(
        "public.json",
        r#"{"repository": {"full_name": "owner/open", "private": false, "visibility": "public"}}"#,
    );
    let no_private = write(
        "no-private.json",
        r#"{"repository": {"full_name": "owner/unsure"}}"#,
    );
    let invalid = write("invalid.json", "{not json");
    let missing = dir.join("missing.json");

    let out = hook(&private);
    assert_eq!(out.status.code(), Some(0), "{}", said(&out));

    let out = hook(&public);
    assert_eq!(out.status.code(), Some(1), "{}", said(&out));
    assert!(
        said(&out).contains("owner/open is not private"),
        "{}",
        said(&out)
    );

    for event in [&no_private, &invalid, &missing] {
        let out = hook(event);
        assert_eq!(
            out.status.code(),
            Some(1),
            "{}: {}",
            event.display(),
            said(&out)
        );
        assert!(
            said(&out).starts_with("runner hook: refusing the job"),
            "{}",
            said(&out)
        );
    }
}

#[test]
fn the_hook_is_executable() {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(HOOK).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755);
}
