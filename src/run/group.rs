//! A `[tests]` suite's process group: stopped at its limit by house, and by a watcher
//! when house itself ends first.
//!
//! The watcher is a small `sh` in a group of its own, started beside each suite,
//! whose stdin is a pipe only house holds. It waits on that pipe; when house ends for
//! any reason (Ctrl-C, SIGTERM, SIGHUP, even SIGKILL) the pipe closes and the watcher
//! stops the suite's group as house would have: SIGTERM, up to `GRACE`, SIGKILL. It
//! ignores those signals itself, so it outlives house to do so. When the suite ends
//! or house stops it, house kills the watcher first.

use super::GRACE;
use std::os::unix::process::CommandExt;
use std::process::{Child, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// `kill`, by path: a missing one on PATH must not read as "the group is gone".
const KILL: &str = "/bin/kill";

/// The watcher's script; `$1` is the group. `read` is the shell's own, so the watcher
/// has no child until house ends.
const WATCH: &str = r#"trap '' INT TERM HUP
read -r _
/bin/kill -s TERM -- "-$1" 2>/dev/null || exit 0
i=0
while [ "$i" -lt 20 ] && /bin/kill -0 -- "-$1" 2>/dev/null; do
  sleep 0.1
  i=$((i + 1))
done
/bin/kill -s KILL -- "-$1" 2>/dev/null
exit 0"#;

/// The watcher of one suite's group; dropping it kills the watcher, not the group.
pub(super) struct Watcher(Option<Child>);

impl Watcher {
    /// Start the watcher of the group `leader` heads. If it cannot start, the suite
    /// still runs, stopped at its limit by house alone.
    pub(super) fn start(leader: u32) -> Watcher {
        let child = std::process::Command::new("/bin/sh")
            .args(["-c", WATCH, "house-watch", &leader.to_string()])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .ok();
        Watcher(child)
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Stop the group `child` heads: SIGTERM, up to `GRACE` while any of it lives, then
/// SIGKILL to what is left and up to `GRACE` more until none of it is left.
pub(super) fn stop(child: &mut Child) {
    let id = format!("-{}", child.id());
    signal(&id, "TERM");
    // The leader is reaped as it exits, so the group lives while any other does.
    let gone = |child: &mut Child| child.try_wait().is_err() || !signal(&id, "0");
    if !wait_until_gone(child, &gone) {
        signal(&id, "KILL");
    }
    // The leader alone, should `kill` itself not have run.
    let _ = child.kill();
    let _ = child.wait();
    wait_until_gone(child, &gone);
}

/// Wait up to `GRACE` for `gone`; whether it came.
fn wait_until_gone(child: &mut Child, gone: &dyn Fn(&mut Child) -> bool) -> bool {
    let until = Instant::now() + GRACE;
    loop {
        if gone(child) {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// `kill -<signal> -- <target>`; true when it reached a process (`0` sends none).
fn signal(target: &str, signal: &str) -> bool {
    std::process::Command::new(KILL)
        .args([&format!("-{signal}"), "--", target])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
