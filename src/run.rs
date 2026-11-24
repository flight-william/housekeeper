//! Runs one command for an adapter: in the repo root, no shell, GIT_* stripped
//! from the child's environment, stdout and stderr captured, under the hang guard.
//!
//! A `[tests]` suite runs with `run_group`: in a process group of its own, so that at
//! its limit the whole group is stopped, SIGTERM then SIGKILL after `GRACE`, and no
//! process in its group (a hung code-signing helper) lives on. Being its own group, a
//! suite does not get the terminal's Ctrl-C; a watcher stops the group when house
//! ends first, however it ends (`group`).

use crate::adapter::{CannotCheck, Command, Output};
use std::io::{ErrorKind, Read};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

mod group;

/// The hang guard: one hour, then the child is killed.
pub const HANG_GUARD: Duration = Duration::from_secs(3600);

/// How long a stopped suite's process group has, after SIGTERM, before SIGKILL.
pub const GRACE: Duration = Duration::from_secs(2);

/// One command's result and how long it took.
#[derive(Debug)]
pub struct Ran {
    pub seconds: f64,
    pub result: Result<Output, CannotCheck>,
    /// The command was still running (or holding its output open) at the guard, and
    /// was stopped; `result` is then the "did not finish" cannot-check.
    pub timed_out: bool,
    /// What a stopped command had written by then, its exit -1: what it waited on.
    pub stopped_output: Option<Output>,
}

/// Run `cmd` in `root`. A program not on PATH gives "not installed" with the brew line.
pub fn run(cmd: &Command, root: &Path, brew_formula: &str, guard: Duration) -> Ran {
    timed(cmd, root, brew_formula, guard, false, &[])
}

/// `run`, with `env` set in the command's environment after the GIT_* are stripped.
pub fn run_env(
    cmd: &Command,
    root: &Path,
    brew_formula: &str,
    guard: Duration,
    env: &[(String, String)],
) -> Ran {
    timed(cmd, root, brew_formula, guard, false, env)
}

/// `run`, with the command in a process group of its own; at the guard the whole
/// group is stopped, not the command alone.
pub fn run_group(cmd: &Command, root: &Path, brew_formula: &str, guard: Duration) -> Ran {
    timed(cmd, root, brew_formula, guard, true, &[])
}

fn timed(
    cmd: &Command,
    root: &Path,
    brew_formula: &str,
    guard: Duration,
    group: bool,
    env: &[(String, String)],
) -> Ran {
    let start = Instant::now();
    let (result, stopped_output) = match run_inner(cmd, root, brew_formula, guard, group, env) {
        Ok(out) => (Ok(out), None),
        Err(Stop::Failed(c)) => (Err(c), None),
        Err(Stop::Hung(c, [stdout, stderr])) => {
            let partial = Output {
                exit: -1,
                stdout,
                stderr,
            };
            (Err(c), Some(partial))
        }
    };
    Ran {
        seconds: start.elapsed().as_secs_f64(),
        result,
        timed_out: stopped_output.is_some(),
        stopped_output,
    }
}

/// Run git with GIT_* stripped; used for the file list, HEAD and the hooks folder.
pub fn git(root: &Path, args: &[&str]) -> Result<Output, CannotCheck> {
    let cmd = Command {
        program: "git".into(),
        args: args.iter().map(|a| a.to_string()).collect(),
    };
    let out = run(&cmd, root, "git", Duration::from_secs(60)).result?;
    if out.exit != 0 {
        return Err(CannotCheck {
            what: format!("git {} failed: {}", args.join(" "), first_line(&out.stderr)),
            fix: "run it by hand in the repo to see why".into(),
        });
    }
    Ok(out)
}

/// The first non-empty line of a tool's text, trimmed, for "cannot check" messages.
pub fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no output")
        .to_string()
}

/// Why `run_inner` gave no output: the command could not be run, or it hung (with
/// the stdout and stderr it had written before it was stopped).
enum Stop {
    Failed(CannotCheck),
    Hung(CannotCheck, [String; 2]),
}

/// How long the output of a stopped command may take to arrive: its pipes close as
/// its processes die, unless one outside its group still holds them.
const STOPPED_READ: Duration = Duration::from_secs(1);

fn run_inner(
    cmd: &Command,
    root: &Path,
    brew_formula: &str,
    guard: Duration,
    group: bool,
    env: &[(String, String)],
) -> Result<Output, Stop> {
    let mut command = std::process::Command::new(&cmd.program);
    command
        .args(&cmd.args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command.envs(env.iter().map(|(key, value)| (key, value)));
    if group {
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|e| {
        Stop::Failed(match e.kind() {
            ErrorKind::NotFound => CannotCheck {
                what: format!("{} is not installed", cmd.program),
                fix: format!("brew install {brew_formula}"),
            },
            _ => CannotCheck {
                what: format!("{} could not start: {e}", cmd.program),
                fix: "run it by hand to see why".into(),
            },
        })
    })?;
    // Dropped on every return, after any stop below: the watcher goes last.
    let _watcher = group.then(|| group::Watcher::start(child.id()));
    let (sent, streams) = mpsc::channel();
    reader(child.stdout.take(), 0, sent.clone());
    reader(child.stderr.take(), 1, sent);
    let mut text = [None, None];
    // Stop the command, then keep what it wrote, as far as it arrives in time.
    let hung = |child: &mut Child, text: &mut [Option<String>; 2]| {
        stop(child, group);
        while text.iter().any(Option::is_none) {
            let Ok((which, read)) = streams.recv_timeout(STOPPED_READ) else {
                break;
            };
            text[which] = Some(read);
        }
        let what = format!("{} did not finish in {} s", cmd.program, guard.as_secs());
        let fix = "run it by hand to see why it hangs".into();
        Stop::Hung(
            CannotCheck { what, fix },
            text.clone().map(Option::unwrap_or_default),
        )
    };
    let deadline = Instant::now() + guard;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                return Err(hung(&mut child, &mut text));
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(e) => {
                return Err(Stop::Failed(CannotCheck {
                    what: format!("{} could not be waited on: {e}", cmd.program),
                    fix: "run it by hand to see why".into(),
                }));
            }
        }
    };
    // The guard also covers reading the output: a process the tool started may
    // hold stdout or stderr open after the tool itself has exited.
    while text.iter().any(Option::is_none) {
        let left = deadline.saturating_duration_since(Instant::now());
        let Ok((which, read)) = streams.recv_timeout(left) else {
            return Err(hung(&mut child, &mut text));
        };
        text[which] = Some(read);
    }
    let [stdout, stderr] = text.map(Option::unwrap_or_default);
    Ok(Output {
        // A child killed by a signal has no code; -1 marks it.
        exit: status.code().unwrap_or(-1),
        stdout,
        stderr,
    })
}

/// Stop `child` at the guard: the child alone, or with `group` its whole process
/// group (`group::stop`).
fn stop(child: &mut Child, group: bool) {
    if group {
        group::stop(child);
    } else {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Read `pipe` to its end on a thread, then send (`which`, the text).
fn reader<R: Read + Send + 'static>(
    pipe: Option<R>,
    which: usize,
    sent: mpsc::Sender<(usize, String)>,
) {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        let _ = sent.send((which, String::from_utf8_lossy(&bytes).into_owned()));
    });
}
