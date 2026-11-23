use super::*;
use crate::adapter::Stage;
use std::path::PathBuf;

// Captured with semgrep 1.186.0 on tests/fixtures/semgrep/planted (see that folder).
const PLANTED: &str = include_str!("../../../tests/fixtures/semgrep/planted.json");
const CLEAN: &str = include_str!("../../../tests/fixtures/semgrep/clean.json");
const BAD_CONFIG: &str = include_str!("../../../tests/fixtures/semgrep/bad_config.json");
const R8: &str = include_str!("../../../patterns/r8.yml");

fn ctx(files: &[&str]) -> Context {
    Context {
        root: PathBuf::from("/repo"),
        stage: Stage::Commit,
        files: files.iter().map(|f| (*f).to_string()).collect(),
        push_range: None,
        languages: Default::default(),
    }
}

fn out(exit: i32, stdout: &str) -> Output {
    Output {
        exit,
        stdout: stdout.to_string(),
        stderr: String::new(),
    }
}

fn lines_of(findings: &[Finding], file: &str) -> Vec<(u32, String)> {
    findings
        .iter()
        .filter(|f| f.place == Place::File(file.to_string()))
        .map(|f| (f.line.unwrap_or(0), f.tool_rule.clone()))
        .collect()
}

#[test]
fn skips_when_no_python_swift_rust_or_shell_file() {
    assert_eq!(Semgrep.command(&ctx(&["README.md", "a.toml"])), None);
}

#[test]
fn command_passes_the_pattern_file_and_only_the_concerned_files() {
    let cmd = Semgrep
        .command(&ctx(&[
            "README.md",
            "tests/test_a.py",
            "Tests/ATests.swift",
            "src/lib.rs",
            "scripts/build.sh",
            "scripts/setup.bash",
        ]))
        .expect("command");
    assert_eq!(cmd.program, "semgrep");
    let config = cmd
        .args
        .iter()
        .position(|a| a == "--config")
        .expect("--config");
    assert!(cmd.args[config + 1].ends_with("/r8.yml"));
    assert_eq!(cmd.args[config + 2], "--config");
    assert!(cmd.args[config + 3].ends_with("/r9.yml"));
    assert_eq!(
        cmd.args[..config],
        [
            "scan",
            "--json",
            "--metrics=off",
            "--quiet",
            "--disable-version-check"
        ]
    );
    assert_eq!(
        cmd.args[config + 4..],
        [
            "tests/test_a.py",
            "Tests/ATests.swift",
            "src/lib.rs",
            "scripts/build.sh",
            "scripts/setup.bash"
        ]
    );
}

#[test]
fn fixture_files_are_never_targets() {
    let files = ctx(&[
        "tests/fixtures/semgrep/planted/tests/clock.rs",
        "tests/a.rs",
    ]);
    assert_eq!(targets(&files, ""), ["tests/a.rs"]);
}

#[test]
fn semgrepignore_lines_drop_files_by_prefix() {
    let files = ctx(&["vendor/x.py", "build/gen.rs", "src/app.py", "docs/a.py"]);
    let ignore = "# generated\n\nvendor/\n/build\n./docs/\n";
    assert_eq!(targets(&files, ignore), ["src/app.py"]);
}

#[test]
fn skips_when_only_ignored_files_are_staged() {
    assert_eq!(
        Semgrep.command(&ctx(&["tests/fixtures/semgrep/planted/src/app.py"])),
        None
    );
}

#[test]
fn python_rules_hit_planted_calls_but_not_strings_comments_or_seeded() {
    let found = Semgrep
        .parse(&ctx(&[]), &out(0, PLANTED))
        .expect("findings");
    assert_eq!(
        lines_of(&found, "tests/test_timing.py"),
        [
            (7, "semgrep python-test-sleep".to_string()),
            (13, "semgrep python-test-unseeded-random".to_string()),
            (22, "semgrep python-test-wall-clock".to_string()),
            (23, "semgrep python-test-wall-clock".to_string()),
        ]
    );
}

#[test]
fn swift_rules_hit_planted_calls_but_not_strings_or_comments() {
    let found = Semgrep
        .parse(&ctx(&[]), &out(0, PLANTED))
        .expect("findings");
    assert_eq!(
        lines_of(&found, "Tests/ClockTests.swift"),
        [
            (5, "semgrep swift-test-sleep".to_string()),
            (6, "semgrep swift-test-sleep".to_string()),
            (7, "semgrep swift-test-sleep".to_string()),
            (13, "semgrep swift-test-wall-clock".to_string()),
        ]
    );
}

#[test]
fn rust_rules_hit_planted_calls_but_not_strings_or_comments() {
    let found = Semgrep
        .parse(&ctx(&[]), &out(0, PLANTED))
        .expect("findings");
    assert_eq!(
        lines_of(&found, "tests/clock.rs"),
        [
            (5, "semgrep rust-test-sleep".to_string()),
            (12, "semgrep rust-test-wall-clock".to_string()),
        ]
    );
}

#[test]
fn non_test_files_are_not_matched() {
    let found = Semgrep
        .parse(&ctx(&[]), &out(0, PLANTED))
        .expect("findings");
    assert!(lines_of(&found, "src/app.py").is_empty());
    assert_eq!(found.len(), 10);
}

#[test]
fn finding_carries_r8_message_and_rule_fix() {
    let found = Semgrep
        .parse(&ctx(&[]), &out(0, PLANTED))
        .expect("findings");
    let first = &found[0];
    assert_eq!(first.rule, "R8");
    assert_eq!(
        first.what,
        "test sleeps, so it is slow and timing-dependent"
    );
    assert_eq!(
        first.fix,
        "await the expectation or inject a fake clock instead of sleeping"
    );
    assert!(
        found
            .iter()
            .all(|f| f.rule == "R8" && !f.fix.ends_with('.'))
    );
}

#[test]
fn absolute_paths_are_made_relative_to_root() {
    let json = PLANTED.replace("\"path\":\"", "\"path\":\"/repo/");
    let found = Semgrep.parse(&ctx(&[]), &out(0, &json)).expect("findings");
    assert!(!lines_of(&found, "tests/clock.rs").is_empty());
}

#[test]
fn clean_gives_no_findings() {
    assert_eq!(Semgrep.parse(&ctx(&[]), &out(0, CLEAN)), Ok(vec![]));
}

#[test]
fn errors_without_results_cannot_check() {
    let err = Semgrep
        .parse(&ctx(&[]), &out(7, BAD_CONFIG))
        .expect_err("cannot check");
    assert!(
        err.what
            .starts_with("semgrep failed: Invalid YAML file bad.yml")
    );
}

#[test]
fn crash_without_json_cannot_check() {
    let err = Semgrep
        .parse(&ctx(&[]), &out(2, "Traceback"))
        .expect_err("cannot check");
    assert_eq!(err.what, "semgrep exited 2 without a JSON report");
}

#[test]
fn every_pattern_rule_has_a_fix_and_warning_severity() {
    let rules = R8.matches("  - id: ").count();
    assert_eq!(rules, 35);
    assert_eq!(R8.matches("\n      fix: ").count(), rules);
    assert_eq!(R8.matches("\n    severity: WARNING").count(), rules);
}

/// Errors next to results must not be dropped. A file Semgrep's own parser cannot
/// read is Semgrep's limit (its Swift parser misses valid Swift), not the file's
/// fault: no finding, no cannot-check, counted through `unread`. Any other error,
/// even beside results, is cannot-check, since a rule or file went unchecked.
const PARSE_ERROR: &str = include_str!("../../../tests/fixtures/semgrep/parse_error.json");

#[test]
fn a_file_semgrep_cannot_parse_is_counted_not_a_finding() {
    let found = Semgrep
        .parse(&ctx(&[]), &out(0, PARSE_ERROR))
        .expect("findings");
    let lines: Vec<String> = found.iter().map(crate::reply::finding_line).collect();
    assert_eq!(
        lines,
        [
            "R8 tests/test_slow.py:5: test sleeps with time.sleep, so it is slow and timing-dependent. fix: wait on the event or inject a fake clock instead of sleeping. [semgrep python-test-sleep]"
        ]
    );
    assert_eq!(
        Semgrep.unread(&ctx(&[]), &out(0, PARSE_ERROR)),
        ["app/main.py"]
    );
}

#[test]
fn two_parse_errors_in_one_file_count_it_once() {
    let errors = r#""errors":[{"code":3,"level":"warn","type":"Syntax error","path":"Tests/ATests.swift","spans":[{"file":"Tests/ATests.swift","start":{"line":1,"col":1,"offset":0}}],"message":"Syntax error at line Tests/ATests.swift:1:\n `import Darwin` was unexpected"},{"code":3,"level":"warn","type":["PartialParsing",[]],"path":"/repo/Tests/ATests.swift","message":"Syntax error at line Tests/ATests.swift:9"}]"#;
    let json = CLEAN.replacen(r#""errors":[]"#, errors, 1);
    assert_ne!(json, CLEAN, "the fixture has an empty errors list");
    assert_eq!(Semgrep.parse(&ctx(&[]), &out(0, &json)), Ok(vec![]));
    assert_eq!(
        Semgrep.unread(&ctx(&[]), &out(0, &json)),
        ["Tests/ATests.swift"]
    );
}

#[test]
fn no_report_means_no_unread_files() {
    assert!(Semgrep.unread(&ctx(&[]), &out(2, "Traceback")).is_empty());
    assert!(Semgrep.unread(&ctx(&[]), &out(0, CLEAN)).is_empty());
}

#[test]
fn another_error_next_to_results_still_cannot_check_and_counts_them() {
    let errors = r#""errors":[{"code":2,"level":"error","type":"Timeout","path":"src/x.py","message":"Timeout when running r8 on src/x.py"}]"#;
    let json = PLANTED.replacen(r#""errors":[]"#, errors, 1);
    let err = Semgrep.parse(&ctx(&[]), &out(0, &json)).unwrap_err();
    assert_eq!(
        err.what,
        "semgrep failed: Timeout when running r8 on src/x.py (10 findings not shown)"
    );
}

#[test]
fn a_parse_error_with_no_file_cannot_check() {
    let errors = r#""errors":[{"code":3,"level":"warn","type":"Syntax error","message":"Syntax error at line src/x.py:1"}]"#;
    let json = CLEAN.replacen(r#""errors":[]"#, errors, 1);
    assert!(Semgrep.parse(&ctx(&[]), &out(0, &json)).is_err());
}
