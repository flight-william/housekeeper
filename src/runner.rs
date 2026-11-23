//! The runner guard: house refuses to run on a self-hosted GitHub Actions runner
//! for a public repository. There anyone can open a pull request from a fork, and
//! its workflow runs that pull request's code on the runner's machine. On a
//! self-hosted runner a payload house cannot read is refused too (fail closed).
//! On a GitHub-hosted runner, or outside Actions, nothing changes.

use serde_json::Value;

pub const REFUSAL: &str = "house: refusing to run: a public repository on a self-hosted runner can run anyone's pull request code on this machine. fix: use GitHub-hosted runners for public repositories";

/// The one line to print, exit 2, before any tool runs; `None` to go on.
/// `env` reads an environment variable.
pub fn refusal(env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let actions = env("GITHUB_ACTIONS").as_deref() == Some("true");
    let self_hosted = env("RUNNER_ENVIRONMENT").as_deref() == Some("self-hosted");
    if !(actions && self_hosted) {
        return None;
    }
    let path = env("GITHUB_EVENT_PATH").unwrap_or_default();
    match visibility(&path) {
        Ok(Visibility::Private) => None,
        Ok(Visibility::Public) => Some(REFUSAL.to_string()),
        Err(why) => Some(format!(
            "house: cannot check: on a self-hosted runner the event payload at GITHUB_EVENT_PATH ({path:?}) {why}. fix: run house in the Actions job, which sets GITHUB_EVENT_PATH to the event's JSON"
        )),
    }
}

enum Visibility {
    Public,
    Private,
}

/// The repository's visibility from the event payload at `path`. Public when
/// either field says so; private only when one says private or internal.
fn visibility(path: &str) -> Result<Visibility, String> {
    if path.is_empty() {
        return Err("is not set".into());
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot be read: {e}"))?;
    let event: Value = serde_json::from_str(&text).map_err(|e| format!("is not JSON: {e}"))?;
    let repo = &event["repository"];
    let visibility = repo["visibility"].as_str();
    let private = repo["private"].as_bool();
    if visibility == Some("public") || private == Some(false) {
        Ok(Visibility::Public)
    } else if matches!(visibility, Some("private" | "internal")) || private == Some(true) {
        Ok(Visibility::Private)
    } else {
        Err("names neither repository.private nor repository.visibility".into())
    }
}
