//! gitleaks at push scans the pushed commits, not the (clean) index: a secret
//! committed and then removed from the index is still in what the push sends.

mod common;

use house::adapter::{Adapter, Context, Stage};
use house::adapters::gitleaks::Gitleaks;
use house::run;
use std::path::Path;

fn head(repo: &Path) -> String {
    let out = run::git(repo, &["rev-parse", "HEAD"]).unwrap();
    out.stdout.trim().to_string()
}

#[test]
fn push_scans_the_pushed_commits_for_a_committed_secret() {
    if std::process::Command::new("gitleaks")
        .arg("version")
        .output()
        .is_err()
    {
        eprintln!("note: gitleaks is not installed; skipping the pushed-commit scan");
        return;
    }
    let repo = common::repo("gitleaks-push");
    std::fs::write(repo.join("README.md"), "hello\n").unwrap();
    common::git(&repo, &["add", "-A"]);
    common::git(&repo, &["commit", "-qm", "base"]);
    let base = head(&repo);
    // SYNTHETIC, NOT A CREDENTIAL: a ghp_-shaped string generated at run time from a
    // fixed formula, so this file holds no secret-shaped text and the value was never
    // issued by anyone. A documented fake key would test nothing, since gitleaks
    // deliberately ignores well-known examples; and the characters are mixed, since
    // gitleaks also skips a low-entropy one such as 36 A's.
    let alphabet: Vec<char> = ('a'..='z').chain('A'..='Z').chain('0'..='9').collect();
    let body: String = (0..36).map(|i| alphabet[(i * 17 + 5) % 62]).collect();
    let token = format!("ghp_{body}");
    std::fs::write(
        repo.join("credential.txt"),
        format!("github_token = \"{token}\"\n"),
    )
    .unwrap();
    common::git(&repo, &["add", "-A"]);
    common::git(&repo, &["commit", "-qm", "secret"]);
    let topic = head(&repo);
    let ctx = Context {
        root: repo.clone(),
        stage: Stage::Push,
        files: vec!["credential.txt".into()],
        push_range: Some(format!("{base}..{topic}")),
    };
    let cmd = Gitleaks.command(&ctx).unwrap();
    let out = run::run(&cmd, &repo, "gitleaks", run::HANG_GUARD)
        .result
        .unwrap();
    let found = Gitleaks.parse(&ctx, &out).unwrap();
    assert_eq!(found.len(), 1, "{out:?}");
    assert_eq!(found[0].rule, "R2");
    assert!(!format!("{found:?}").contains(&token));
}
