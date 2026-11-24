//! `[paths] exclude` reaches a tool that reads more than the files it is handed:
//! gitleaks reads the git history, so a finding on an excluded path is dropped
//! after the tool runs, at every stage and at `--all`.

mod common;
#[path = "common/leaks.rs"]
mod leaks;

use common::{git, repo};
use house::adapter::{Adapter, CannotCheck, Command, Context, Output, Stage};
use house::adapters::gitleaks::Gitleaks;
use house::cli;
use house::finding::{Finding, Place};
use std::path::Path;

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn places(report: &house::reply::Report) -> Vec<String> {
    assert!(report.cannots().is_empty(), "{}", report.text());
    report
        .findings()
        .iter()
        .map(|f| house::reply::place_text(&f.place).to_string())
        .collect()
}

/// Reports one finding on each of `PATHS`, whatever files it is handed, as a tool
/// that reads the history does.
struct History;

const PATHS: [&str; 3] = ["vendor/lib.c", "src/main.c", "vendored/x.c"];

impl Adapter for History {
    fn name(&self) -> &'static str {
        "history"
    }
    fn program(&self) -> &'static str {
        "true"
    }
    fn brew_formula(&self) -> &'static str {
        "coreutils"
    }
    fn command(&self, _: &Context) -> Option<Command> {
        Some(Command {
            program: "true".into(),
            args: Vec::new(),
        })
    }
    fn parse(&self, _: &Context, _: &Output) -> Result<Vec<Finding>, CannotCheck> {
        let finding = |file: &str| Finding {
            rule: "R2".into(),
            place: Place::File(file.into()),
            line: Some(1),
            what: "found".into(),
            fix: "none".into(),
            tool_rule: "history".into(),
            measure: None,
        };
        let mut found: Vec<Finding> = PATHS.iter().map(|p| finding(p)).collect();
        found.push(Finding {
            place: Place::Repo,
            ..finding("")
        });
        Ok(found)
    }
}

#[test]
fn a_finding_on_an_excluded_path_is_dropped() {
    let root = repo("exclude-findings");
    write(&root, ".house.toml", "[paths]\nexclude = [\"vendor/\"]\n");
    write(&root, "src/main.c", "int main;\n");
    git(&root, &["add", "-A"]);
    let report = cli::check(&root, Stage::Commit, vec![Box::new(History)]);
    assert_eq!(places(&report), ["src/main.c", "vendored/x.c", "repo"]);
}

/// A committed vendored file holding a token-shaped string, scanned by `--all`.
fn leak_report(config: &str) -> Vec<String> {
    let root = repo(&format!("exclude-gitleaks-{}", config.len()));
    write(&root, ".house.toml", config);
    // SYNTHETIC, NOT A CREDENTIAL: a ghp_-shaped string generated at run time from a
    // fixed formula (as in tests/gitleaks.rs), never issued by anyone.
    let alphabet: Vec<char> = ('a'..='z').chain('A'..='Z').chain('0'..='9').collect();
    let body: String = (0..36).map(|i| alphabet[(i * 17 + 5) % 62]).collect();
    write(
        &root,
        "vendor/blob.txt",
        &format!("token = \"ghp_{body}\"\n"),
    );
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "base"]);
    places(&cli::check_all(&root, vec![Box::new(Gitleaks)]))
}

#[test]
fn gitleaks_reports_nothing_in_an_excluded_vendored_file() {
    if !leaks::installed() {
        return;
    }
    assert_eq!(
        leak_report("[tools]\npush = [\"gitleaks\"]\n"),
        ["vendor/blob.txt"]
    );
    let excluded = "[tools]\npush = [\"gitleaks\"]\n[paths]\nexclude = [\"vendor/\"]\n";
    assert!(leak_report(excluded).is_empty());
}

const FIXTURES_EXCLUDED: &str = "[tools]\ncommit = [\"gitleaks\"]\npush = [\"gitleaks\"]\n[paths]\nexclude = [\"tests/fixtures/\"]\n";

/// A repo whose first commit (after the config) puts a token under the excluded
/// tests/fixtures/, then `git mv` stages it out to src/. Gives the repo and the
/// commit before the token.
fn moved_out(name: &str) -> (std::path::PathBuf, String) {
    let root = repo(name);
    write(&root, ".house.toml", FIXTURES_EXCLUDED);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "config"]);
    let base = rev(&root, "HEAD");
    write(&root, "tests/fixtures/creds.txt", &leaks::token_line());
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "fixture"]);
    std::fs::create_dir_all(root.join("src")).unwrap();
    git(&root, &["mv", "tests/fixtures/creds.txt", "src/creds.txt"]);
    (root, base)
}

fn rev(root: &Path, name: &str) -> String {
    let out = std::process::Command::new("git")
        .args(["rev-parse", name])
        .current_dir(root)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn a_secret_moved_out_of_an_excluded_folder_is_found_at_commit() {
    if !leaks::installed() {
        return;
    }
    let (root, _) = moved_out("exclude-moved-commit");
    let report = cli::check(&root, Stage::Commit, vec![Box::new(Gitleaks)]);
    assert_eq!(places(&report), ["src/creds.txt"], "{}", report.text());
}

#[test]
fn a_secret_moved_out_of_an_excluded_folder_is_found_by_all() {
    if !leaks::installed() {
        return;
    }
    let (root, _) = moved_out("exclude-moved-all");
    git(&root, &["commit", "-qm", "moved"]);
    let report = cli::check_all(&root, vec![Box::new(Gitleaks)]);
    assert_eq!(places(&report), ["src/creds.txt"], "{}", report.text());
}

#[test]
fn a_secret_moved_out_of_an_excluded_folder_is_found_at_push() {
    if !leaks::installed() {
        return;
    }
    let (root, base) = moved_out("exclude-moved-push");
    git(&root, &["commit", "-qm", "moved"]);
    assert_pushed_at_src(&root, &base, "exclude-moved-push-log");
}

/// `house check --stage push` of HEAD over `base` reports the token at src/creds.txt
/// alone, once.
fn assert_pushed_at_src(root: &Path, base: &str, log: &str) {
    git(root, &["update-ref", "refs/remotes/origin/main", base]);
    let (code, text) = leaks::pushed(root, &rev(root, "HEAD"), &common::fresh(log));
    assert_eq!(code, Some(1), "{text}");
    assert_eq!(text.matches("R2 src/creds.txt:1").count(), 1, "{text}");
    assert!(!text.contains("tests/fixtures/creds.txt"), "{text}");
}

/// `repo` with the config committed, a side branch with one unrelated commit, and
/// `git merge --no-ff --no-commit side` under way. Gives the commit before the merge.
fn merging(root: &Path, before_side: &[(&str, &str)]) -> String {
    write(root, ".house.toml", FIXTURES_EXCLUDED);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "config"]);
    let base = rev(root, "HEAD");
    for (path, text) in before_side {
        write(root, path, text);
        git(root, &["add", "-A"]);
        git(root, &["commit", "-qm", "before side"]);
    }
    git(root, &["checkout", "-q", "-b", "side"]);
    write(root, "side.txt", "side\n");
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "side"]);
    git(root, &["checkout", "-q", "-"]);
    git(root, &["merge", "-q", "--no-ff", "--no-commit", "side"]);
    base
}

/// The token committed under tests/fixtures/, then moved to src/ while a merge is made:
/// `git log -p` shows a merge commit no diff, so the move was never seen.
fn moved_in_a_merge(name: &str) -> (std::path::PathBuf, String) {
    let root = repo(name);
    let token = leaks::token_line();
    let base = merging(&root, &[("tests/fixtures/creds.txt", &token)]);
    std::fs::create_dir_all(root.join("src")).unwrap();
    git(&root, &["mv", "tests/fixtures/creds.txt", "src/creds.txt"]);
    git(&root, &["commit", "-qm", "merge"]);
    (root, base)
}

#[test]
fn a_secret_moved_out_of_an_excluded_folder_in_a_merge_is_found_at_push() {
    if !leaks::installed() {
        return;
    }
    let (root, base) = moved_in_a_merge("exclude-merge-push");
    assert_pushed_at_src(&root, &base, "exclude-merge-push-log");
}

#[test]
fn a_secret_moved_out_of_an_excluded_folder_in_a_merge_is_found_by_all() {
    if !leaks::installed() {
        return;
    }
    let (root, _) = moved_in_a_merge("exclude-merge-all");
    let report = cli::check_all(&root, vec![Box::new(Gitleaks)]);
    assert_eq!(places(&report), ["src/creds.txt"], "{}", report.text());
}

#[test]
fn a_secret_first_added_in_a_merge_is_found_at_push_and_by_all() {
    if !leaks::installed() {
        return;
    }
    let root = repo("exclude-merge-added");
    let base = merging(&root, &[]);
    write(&root, "src/creds.txt", &leaks::token_line());
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "merge"]);
    assert_pushed_at_src(&root, &base, "exclude-merge-added-log");
    let report = cli::check_all(&root, vec![Box::new(Gitleaks)]);
    assert_eq!(places(&report), ["src/creds.txt"], "{}", report.text());
}

/// A secret a side branch adds is in the side commit's diff and in the merge's diff
/// against its first parent: one finding, not two.
#[test]
fn a_secret_merged_in_from_a_side_branch_is_one_finding() {
    if !leaks::installed() {
        return;
    }
    let root = repo("exclude-merge-side");
    write(&root, ".house.toml", FIXTURES_EXCLUDED);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "config"]);
    git(&root, &["checkout", "-q", "-b", "side"]);
    write(&root, "src/creds.txt", &leaks::token_line());
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "side"]);
    git(&root, &["checkout", "-q", "-"]);
    write(&root, "main.txt", "main\n");
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "main"]);
    git(&root, &["merge", "-q", "--no-ff", "-m", "merge", "side"]);
    let report = cli::check_all(&root, vec![Box::new(Gitleaks)]);
    assert_eq!(places(&report), ["src/creds.txt"], "{}", report.text());
}

/// Git finds a copy only from a file the same change modifies: the fixture gains a
/// line and its old text is copied to src/.
#[test]
fn a_secret_copied_out_of_an_excluded_folder_is_found_even_with_copy_detection() {
    if !leaks::installed() {
        return;
    }
    let root = repo("exclude-copied-commit");
    write(&root, ".house.toml", FIXTURES_EXCLUDED);
    let fixture = format!("{}one\ntwo\nthree\nfour\n", leaks::token_line());
    write(&root, "tests/fixtures/creds.txt", &fixture);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "fixture"]);
    git(&root, &["config", "diff.renames", "copies"]);
    write(&root, "src/copy.txt", &fixture);
    write(
        &root,
        "tests/fixtures/creds.txt",
        &format!("{fixture}five\n"),
    );
    git(&root, &["add", "-A"]);
    let report = cli::check(&root, Stage::Commit, vec![Box::new(Gitleaks)]);
    assert_eq!(places(&report), ["src/copy.txt"], "{}", report.text());
}
