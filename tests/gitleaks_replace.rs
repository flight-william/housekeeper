//! gitleaks reads the commits git really holds: a replace ref (planted by a build script
//! a tool runs earlier, say) that swaps a commit for a twin without the secret does not
//! hide it.

mod common;
#[path = "common/leaks.rs"]
mod leaks;

use common::{fresh, git, repo};
use std::path::Path;
use std::process::Command;

fn out(root: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}");
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

#[test]
fn a_replace_ref_does_not_hide_a_secret_from_gitleaks() {
    if !leaks::installed() {
        return;
    }
    let root = repo("gitleaks-replace");
    std::fs::write(root.join(".house.toml"), "[tools]\npush = [\"gitleaks\"]\n").unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "config"]);
    let clean_tree = out(&root, &["rev-parse", "HEAD^{tree}"]);
    let parent = out(&root, &["rev-parse", "HEAD"]);
    std::fs::write(root.join("creds.txt"), leaks::token_line()).unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "creds"]);
    let tip = out(&root, &["rev-parse", "HEAD"]);
    let twin = out(
        &root,
        &["commit-tree", &clean_tree, "-p", &parent, "-m", "creds"],
    );
    git(&root, &["replace", &tip, &twin]);
    let (code, text) = leaks::pushed(&root, &tip, &fresh("gitleaks-replace-log"));
    assert_eq!(code, Some(1), "{text}");
    assert!(text.contains("R2 creds.txt"), "{text}");
}
