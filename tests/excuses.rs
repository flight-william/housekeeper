//! The dated excuse list: match, expired, stale, malformed.

use house::excuses::{Date, Excuse, apply, grew, parse};
use house::finding::{Finding, Measure, Place};

fn finding(rule: &str, file: &str, line: u32) -> Finding {
    Finding {
        rule: rule.to_string(),
        place: Place::File(file.to_string()),
        line: Some(line),
        what: "function `run` is too long".to_string(),
        fix: "split it".to_string(),
        tool_rule: "clippy::too_many_lines".to_string(),
        measure: None,
    }
}

fn today() -> Date {
    Date::parse("2026-11-22").unwrap()
}

fn list(text: &str) -> Vec<Excuse> {
    parse(text).unwrap()
}

const ONE: &str = r#"
[[excuse]]
rule = "R3"
file = "src/old.rs"
symbol = "run"
value = 140
deadline = "2026-12-31"
reason = "old code, split in the refactor slice"
"#;

// This test replaced `a_matching_entry_excuses_every_finding_of_that_rule_in_that_file`,
// which pinned a v0 shortcut (rule + file only). The rule: an entry excuses its rule,
// file and symbol, up to its value.
#[test]
fn an_entry_excuses_findings_of_its_rule_file_and_symbol() {
    let findings = vec![
        finding("R3", "src/old.rs", 10),
        finding("R3", "src/old.rs", 90),
        finding("R4", "src/old.rs", 10),
        finding("R3", "src/new.rs", 10),
    ];
    let out = apply(findings, &list(ONE), today());
    assert_eq!(out.excused.len(), 2);
    assert!(out.excused.iter().all(|f| f.rule == "R3"));
    assert_eq!(out.to_fix.len(), 2);
    assert!(out.problems.is_empty());
}

fn measured(what: &str, value: f64) -> Finding {
    Finding {
        rule: "R3".to_string(),
        place: Place::File("old.py".to_string()),
        line: None,
        what: what.to_string(),
        fix: "split the function into smaller functions".to_string(),
        tool_rule: "r3 function_lines".to_string(),
        measure: Some(Measure {
            value,
            unit: "lines".to_string(),
            limit: 60.0,
        }),
    }
}

const OLD: &str = r#"
[[excuse]]
rule = "R3"
file = "old.py"
symbol = "old"
value = 80
deadline = "2026-12-31"
reason = "old code"
"#;

// Regression: an entry for `old` at 80 lines must not excuse a new 300-line function.
#[test]
fn a_new_function_in_an_excused_file_is_not_excused() {
    let f = measured("function new has 300 lines, limit is 60", 300.0);
    let out = apply(vec![f], &list(OLD), today());
    assert_eq!((out.excused.len(), out.to_fix.len()), (0, 1));
}

#[test]
fn the_symbol_matches_as_a_whole_word_only() {
    let f = measured("function old_helper has 70 lines, limit is 60", 70.0);
    let out = apply(vec![f], &list(OLD), today());
    assert_eq!((out.excused.len(), out.to_fix.len()), (0, 1));
}

#[test]
fn a_value_above_the_entrys_is_not_excused_and_one_at_it_is() {
    let worse = measured("function old has 81 lines, limit is 60", 81.0);
    let same = measured("function old has 80 lines, limit is 60", 80.0);
    let out = apply(vec![worse, same], &list(OLD), today());
    assert_eq!(out.excused.len(), 1);
    assert_eq!(out.excused[0].measure.as_ref().unwrap().value, 80.0);
    assert_eq!(out.to_fix.len(), 1);
}

#[test]
fn an_entry_without_a_value_excuses_its_symbol_at_any_value() {
    let f = measured("function old has 300 lines, limit is 60", 300.0);
    let text = OLD.replace("value = 80\n", "");
    let out = apply(vec![f], &list(&text), today());
    assert_eq!(out.excused.len(), 1);
}

#[test]
fn the_deadline_day_itself_still_excuses() {
    let out = apply(
        vec![finding("R3", "src/old.rs", 1)],
        &list(ONE),
        Date::parse("2026-12-31").unwrap(),
    );
    assert_eq!(out.excused.len(), 1);
    assert!(out.problems.is_empty());
}

#[test]
fn an_expired_entry_excuses_nothing_and_says_so() {
    let out = apply(
        vec![finding("R3", "src/old.rs", 1)],
        &list(ONE),
        Date::parse("2027-01-01").unwrap(),
    );
    assert!(out.excused.is_empty());
    assert_eq!(out.to_fix.len(), 1);
    assert_eq!(out.problems.len(), 1);
    let p = &out.problems[0];
    assert_eq!(p.rule, "R9");
    assert_eq!(p.place, Place::File(".house-excuses.toml".to_string()));
    assert_eq!(p.tool_rule, "house excuse-expired");
    assert_eq!(
        p.what,
        "excuse expired: R3 src/old.rs run, deadline 2026-12-31"
    );
    assert_eq!(
        p.fix,
        "fix the R3 finding in src/old.rs, then delete the entry"
    );
}

#[test]
fn an_entry_matching_nothing_is_stale() {
    let out = apply(vec![finding("R4", "src/old.rs", 1)], &list(ONE), today());
    assert_eq!(out.to_fix.len(), 1);
    assert_eq!(out.problems.len(), 1);
    let p = &out.problems[0];
    assert_eq!(p.rule, "R9");
    assert_eq!(p.tool_rule, "house excuse-stale");
    assert_eq!(p.what, "stale excuse: R3 src/old.rs run matches no finding");
    assert_eq!(p.fix, "delete it from .house-excuses.toml");
}

#[test]
fn no_file_text_means_no_excuses() {
    assert!(list("").is_empty());
    let out = apply(vec![finding("R3", "a.rs", 1)], &[], today());
    assert_eq!(out.to_fix.len(), 1);
    assert!(out.excused.is_empty() && out.problems.is_empty());
}

#[test]
fn a_repo_wide_finding_is_never_excused() {
    let mut f = finding("R3", "src/old.rs", 1);
    f.place = Place::Repo;
    let out = apply(vec![f], &list(ONE), today());
    assert_eq!(out.to_fix.len(), 1);
    assert_eq!(out.problems.len(), 1, "the entry is stale");
}

#[test]
fn symbol_and_value_are_optional() {
    let got = list(
        r#"
[[excuse]]
rule = "R2"
file = "a.py"
deadline = "2026-12-01"
reason = "vendored"
"#,
    );
    assert_eq!(got[0].symbol, None);
    assert_eq!(got[0].value, None);
    assert_eq!(list(ONE)[0].value, Some(140.0));
}

fn malformed(text: &str) -> String {
    let err = parse(text).unwrap_err();
    assert!(!err.what.ends_with('.'), "{}", err.what);
    assert!(!err.what.contains('\n'), "{}", err.what);
    assert!(err.fix.contains("rule, file, deadline"), "{}", err.fix);
    err.what
}

#[test]
fn malformed_files_cannot_be_checked() {
    assert!(malformed("[[excuse]\n").starts_with(".house-excuses.toml: "));
    let missing = malformed("[[excuse]]\nrule = \"R3\"\nfile = \"a.rs\"\nreason = \"x\"\n");
    assert!(missing.contains("deadline"), "{missing}");
    let unknown = malformed(&ONE.replace("reason", "becaus"));
    assert!(unknown.contains("becaus"), "{unknown}");
}

#[test]
fn bad_entries_cannot_be_checked() {
    let bad_date = malformed(&ONE.replace("2026-12-31", "2026-02-30"));
    assert!(bad_date.contains("2026-02-30"), "{bad_date}");
    let loose_date = malformed(&ONE.replace("2026-12-31", "2026-1-31"));
    assert!(loose_date.contains("YYYY-MM-DD"), "{loose_date}");
    let bad_rule = malformed(&ONE.replace("\"R3\"", "\"too-long\""));
    assert!(bad_rule.contains("too-long"), "{bad_rule}");
    let no_reason = malformed(&ONE.replace("old code, split in the refactor slice", " "));
    assert!(no_reason.contains("reason"), "{no_reason}");
    let twice = malformed(&format!("{ONE}{ONE}"));
    assert!(twice.contains("listed twice"), "{twice}");
}

// Regression: R0 and R999 were accepted, then sat inert or stale; only R1 to R17 exist.
#[test]
fn a_rule_outside_r1_to_r17_cannot_be_checked() {
    for bad in ["R0", "R18", "R999", "R03", "R"] {
        let what = malformed(&ONE.replace("\"R3\"", &format!("\"{bad}\"")));
        assert!(what.contains("is not R1..R17"), "{bad}: {what}");
    }
    for good in ["R1", "R17"] {
        assert_eq!(
            list(&ONE.replace("\"R3\"", &format!("\"{good}\"")))[0].rule,
            good
        );
    }
}

#[test]
fn dates_compare_by_calendar() {
    let d = |s| Date::parse(s).unwrap();
    assert!(d("2026-11-22") < d("2026-12-01"));
    assert!(d("2026-12-31") < d("2027-01-01"));
    assert!(Date::parse("2028-02-29").is_some());
    assert!(Date::parse("2027-02-29").is_none());
    assert!(Date::parse("2026-13-01").is_none());
    assert_eq!(d("2026-01-05").to_string(), "2026-01-05");
}

fn grown(base: &str, now: &str) -> Vec<String> {
    let found = grew(&list(base), &list(now));
    for f in &found {
        assert_eq!(f.rule, "R9");
        assert_eq!(f.place, Place::File(".house-excuses.toml".to_string()));
        assert_eq!(f.tool_rule, "house excuse-grew");
        assert_eq!(f.fix, "remove it, or ask the owner");
    }
    found.into_iter().map(|f| f.what).collect()
}

// Regression: a newly added entry, even with a far-future deadline, is the list growing.
#[test]
fn an_added_entry_is_the_list_growing() {
    let added = "[[excuse]]\nrule = \"R1\"\nfile = \"bad.py\"\ndeadline = \"9999-12-31\"\nreason = \"new violation\"\n";
    assert_eq!(grown("", added), vec!["excuse list grew: R1 bad.py"]);
    assert_eq!(
        grown(ONE, &format!("{ONE}{added}")),
        vec!["excuse list grew: R1 bad.py"]
    );
}

#[test]
fn a_raised_value_a_dropped_value_or_a_later_deadline_is_the_list_growing() {
    let want = vec!["excuse list grew: R3 src/old.rs run"];
    assert_eq!(grown(ONE, &ONE.replace("140", "141")), want);
    assert_eq!(grown(ONE, &ONE.replace("value = 140\n", "")), want);
    assert_eq!(grown(ONE, &ONE.replace("2026-12-31", "2027-01-01")), want);
}

#[test]
fn an_unchanged_or_shrinking_list_has_not_grown() {
    assert!(grown(ONE, ONE).is_empty());
    assert!(grown(ONE, "").is_empty());
    assert!(grown(ONE, &ONE.replace("140", "100")).is_empty());
    assert!(grown(ONE, &ONE.replace("2026-12-31", "2026-12-01")).is_empty());
    assert!(
        grown(
            ONE,
            &ONE.replace("old code, split", "renamed reason, split")
        )
        .is_empty()
    );
}
