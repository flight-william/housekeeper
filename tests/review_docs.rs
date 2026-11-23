//! The sample report in docs/review.md is the real output of `house review` over the
//! made-up records in tests/fixtures/review_docs, whole, not an excerpt.

use std::path::Path;
use std::process::Command;

const DOC: &str = include_str!("../docs/review.md");

/// The fenced block that follows the sample's command line in the doc.
fn sample() -> &'static str {
    let command = "house review --from 2026-11-16 --to 2026-11-22\n```\n\n```\n";
    let start = DOC
        .find(command)
        .unwrap_or_else(|| panic!("docs/review.md shows no sample made by: {command}"))
        + command.len();
    let end = DOC[start..].find("```").expect("the sample's block ends");
    &DOC[start..start + end]
}

#[test]
fn the_docs_sample_is_the_whole_report_over_the_fixture() {
    let log = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/review_docs");
    let out = Command::new(env!("CARGO_BIN_EXE_house"))
        .args(["review", "--from", "2026-11-16", "--to", "2026-11-22"])
        .env("HOUSE_LOG_DIR", &log)
        .env_remove("CLAUDECODE")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(sample(), String::from_utf8_lossy(&out.stdout));
}

#[test]
fn the_docs_say_memory_grows_with_the_folder_and_claim_no_bound() {
    // What a review keeps grows with the distinct names in the folder; only a line
    // has a cap. The page once said the review's state was bounded, which it is not.
    assert!(
        !DOC.contains("holds what it counts"),
        "docs/review.md claims a bound"
    );
    let plain = DOC.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        plain.contains(
            "Memory grows with the number of distinct repos, rules, tools and places in the record folder"
        ),
        "docs/review.md does not say what memory grows with"
    );
    assert!(
        plain.contains("a single line is capped at 1 MiB"),
        "no line cap stated"
    );
}
