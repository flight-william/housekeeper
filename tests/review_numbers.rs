//! `house review` on hostile numbers in a record: a count or a duration that house
//! could not have written makes the line not a house record, skipped and counted,
//! and every total stays exact.

#[path = "common/temp.rs"]
mod common;

use std::path::Path;
use std::process::Command;

/// `house review --json` over a fresh folder holding `lines` as November's records.
fn review_json(name: &str, lines: &[String]) -> (i32, serde_json::Value, String) {
    let log = common::fresh(name);
    std::fs::write(log.join("2026-11.jsonl"), lines.join("\n") + "\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_house"))
        .args([
            "review",
            "--from",
            "2026-11-01",
            "--to",
            "2026-11-30",
            "--json",
        ])
        .current_dir(Path::new(&log))
        .env("HOUSE_LOG_DIR", &log)
        .env_remove("CLAUDECODE")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    (
        out.status.code().unwrap_or(-1),
        value,
        format!("{text}{stderr}"),
    )
}

/// A record line with one tool `r3`, its findings and excused maps as given.
fn run(findings: &str, excused: &str) -> String {
    format!(
        r#"{{"start":"2026-11-02T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{{"name":"r3","seconds":0.5,"findings":{findings},"excused":{excused}}}],"findings":[],"excused":0}}"#
    )
}

/// The largest count house writes: a tool's findings per rule are a `u32`.
const MOST: u64 = u32::MAX as u64;

#[test]
fn a_count_house_could_not_write_makes_the_line_skipped_never_a_panic() {
    let huge = u64::MAX;
    for (name, line) in [
        (
            "review-huge-sum",
            run(&format!(r#"{{"R2":{huge},"R3":1}}"#), "{}"),
        ),
        (
            "review-huge-noise",
            run(&format!(r#"{{"R3":{huge}}}"#), r#"{"R3":1}"#),
        ),
        (
            "review-huge-excused",
            run("{}", &format!(r#"{{"R3":{huge}}}"#)),
        ),
        ("review-negative", run(r#"{"R3":-1}"#, "{}")),
    ] {
        let (code, v, text) = review_json(name, &[line, run(r#"{"R3":1}"#, "{}")]);
        assert_eq!(code, 0, "{name}: {text}");
        assert_eq!(v["skipped_lines"], 1, "{name}: {text}");
        assert_eq!(v["runs"], 1, "{name}: {text}");
        assert_eq!(v["rules"][0]["findings"], 1, "{name}: {text}");
    }
    let old = r#"{"start":"2026-11-02T09:00:00Z","repo":"/work/shop","stage":"commit","tools":[{"name":"r3","seconds":0.5,"findings":{}}],"findings":[],"excused":18446744073709551615}"#;
    let (code, v, text) = review_json("review-huge-total", &[old.into(), old.into()]);
    assert_eq!((code, v["skipped_lines"].clone()), (0, 2.into()), "{text}");
}

#[test]
fn the_largest_counts_house_writes_add_up_exactly() {
    let most = run(
        &format!(r#"{{"R2":{MOST}}}"#),
        &format!(r#"{{"R3":{MOST}}}"#),
    );
    let (code, v, text) = review_json("review-most", &[most.clone(), most.clone(), most]);
    assert_eq!(code, 0, "{text}");
    assert_eq!(v["skipped_lines"], 0, "{text}");
    assert_eq!(v["rules"][0]["findings"], 3 * MOST, "{text}");
    assert_eq!(v["rules"][1]["excused"], 3 * MOST, "{text}");
    let noisy = &v["suggestions"][0];
    assert_eq!(noisy["subject"], "R3", "{text}");
    assert_eq!(
        noisy["text"],
        format!(
            "R3 looks noisy: {} of {} findings excused; fix that code, or tune the rule",
            3 * MOST,
            3 * MOST
        ),
        "{text}"
    );
}
