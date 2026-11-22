//! `house::commitmsg`, the message's shape: R17 on the subject, comments, the body
//! labels and the blank line, and R9 on a fix commit's Cause line.

mod commitmsg_support;

use commitmsg_support::{BODY, ctx, one, seen};
use house::commitmsg::{Commit as Context, check};

#[test]
fn a_message_meeting_every_rule_has_no_findings() {
    let text = format!("parser: keep the last line{BODY}");
    assert_eq!(seen(&text, &ctx()), vec![]);
}

#[test]
fn a_subject_without_an_area_is_r17_subject_on_line_1() {
    let text = format!("Keep the last line{BODY}");
    assert_eq!(
        seen(&text, &ctx()),
        one("R17/subject", 1, "Keep the last line")
    );
}

#[test]
fn a_subject_over_72_characters_is_r17_subject_and_72_is_not() {
    // "parser: " is 8 characters; 64 more make 72.
    let at_limit = format!("parser: {}", "k".repeat(64));
    assert_eq!(seen(&format!("{at_limit}{BODY}"), &ctx()), vec![]);
    let wide = format!("parser: {}é", "k".repeat(63)); // 72 characters, 73 bytes
    assert_eq!(seen(&format!("{wide}{BODY}"), &ctx()), vec![]);
    let over = format!("{at_limit}é");
    assert_eq!(
        seen(&format!("{over}{BODY}"), &ctx()),
        one("R17/subject", 1, &over)
    );
}

#[test]
fn comment_lines_are_dropped_and_findings_keep_the_files_line_numbers() {
    let text =
        format!("# Please enter the commit message\n#\nKeep the last line{BODY}# On branch main\n");
    assert_eq!(
        seen(&text, &ctx()),
        one("R17/subject", 3, "Keep the last line")
    );
}

#[test]
fn the_configured_comment_character_replaces_the_hash() {
    let ctx = Context {
        comment_char: Some(";".into()),
        ..ctx()
    };
    let text = format!("; Please enter the commit message\nparser: keep the last line{BODY}");
    assert_eq!(seen(&text, &ctx), vec![]);
    let text = format!("# not a comment now\nparser: keep the last line{BODY}");
    let mut want = one("R17/subject", 1, "# not a comment now");
    want.extend(one("R17/blank", 2, "parser: keep the last line"));
    assert_eq!(seen(&text, &ctx), want);
}

#[test]
fn a_comment_setting_that_is_not_one_character_cannot_run() {
    for value in ["auto", "//", ""] {
        let ctx = Context {
            comment_char: Some(value.into()),
            ..ctx()
        };
        let err = check(&format!("parser: keep the last line{BODY}"), &ctx).unwrap_err();
        assert!(err.what.contains("core.commentChar"), "{err:?}");
    }
}

#[test]
fn each_missing_why_proof_or_undo_line_is_r17_body_for_the_whole_message() {
    let text = "parser: keep the last line\n\nWhy: the parser dropped it.\n";
    let mut want = one("R17/body", 0, "Proof:");
    want.extend(one("R17/body", 0, "Undo:"));
    assert_eq!(seen(text, &ctx()), want);
}

#[test]
fn a_body_label_without_text_after_it_counts_as_missing() {
    let text = "parser: keep the last line\n\nWhy:   \nProof: cargo test.\nUndo:\nWhy:no space\n";
    let mut want = one("R17/body", 0, "Why:");
    want.extend(one("R17/body", 0, "Undo:"));
    assert_eq!(seen(text, &ctx()), want);
}

#[test]
fn everything_from_the_scissors_line_on_is_dropped() {
    let scissors = "# ------------------------ >8 ------------------------";
    let text = format!(
        "parser: keep the last line\n\nWhy: it was dropped.\nProof: cargo test.\n{scissors}\n# Do not modify or remove the line above.\nUndo: git revert this commit.\n"
    );
    assert_eq!(seen(&text, &ctx()), one("R17/body", 0, "Undo:"));
    // The scissors line uses the configured comment character.
    let ctx = Context {
        comment_char: Some(";".into()),
        ..ctx()
    };
    let text = text.replace(scissors, &scissors.replacen('#', ";", 1));
    assert_eq!(seen(&text, &ctx), one("R17/body", 0, "Undo:"));
}

#[test]
fn a_merge_subject_is_exempt_from_the_pattern_only_while_merge_head_exists() {
    let merging = Context {
        merge_head: true,
        ..ctx()
    };
    let subject = "Merge branch 'house-wp5' into main";
    let text = format!("{subject}{BODY}");
    assert_eq!(seen(&text, &merging), vec![]);
    assert_eq!(seen(&text, &ctx()), one("R17/subject", 1, subject));
}

#[test]
fn a_merge_subject_is_not_exempt_from_the_length() {
    let merging = Context {
        merge_head: true,
        ..ctx()
    };
    let subject = format!("Merge branch '{}' into main", "b".repeat(50));
    let text = format!("{subject}{BODY}");
    assert_eq!(seen(&text, &merging), one("R17/subject", 1, &subject));
}

#[test]
fn a_revert_subject_is_exempt_from_the_pattern_and_the_length() {
    let subject = format!("Revert \"parser: {}\"", "k".repeat(70));
    assert_eq!(seen(&format!("{subject}{BODY}"), &ctx()), vec![]);
    // Only git's own form: `Revert` then a quote.
    let subject = "Revert parser change";
    assert_eq!(
        seen(&format!("{subject}{BODY}"), &ctx()),
        one("R17/subject", 1, subject)
    );
}

#[test]
fn a_subject_saying_only_wip_fix_or_update_is_r17_vague() {
    // A Cause: line, so the fix subjects judge R17/vague alone.
    let body = format!("{BODY}Cause: an off-by-one in the line loop.\n");
    for said in ["wip", "WIP", "Fix.", "update", "Update."] {
        let text = format!("parser: {said}{body}");
        assert_eq!(seen(&text, &ctx()), one("R17/vague", 1, said), "{said}");
    }
    for said in ["fix the last line", "fix..", "updated", "wip: more"] {
        let text = format!("parser: {said}{body}");
        assert_eq!(seen(&text, &ctx()), vec![], "{said}");
    }
}

#[test]
fn a_body_without_an_empty_line_2_is_r17_blank_on_that_line() {
    let text = format!(
        "parser: keep the last line\n{}",
        BODY.trim_start_matches('\n')
    );
    let line2 = "Why: the parser dropped the last line.";
    assert_eq!(seen(&text, &ctx()), one("R17/blank", 2, line2));
    // Line 2 is counted after comment lines go: here it is the file's line 3.
    let text = format!("parser: keep the last line\n# a comment\n{line2}\nProof: x.\nUndo: y.\n");
    assert_eq!(seen(&text, &ctx()), one("R17/blank", 3, line2));
    // A line of spaces is empty: git's clean-up makes it so before it commits.
    let text = format!("parser: keep the last line\n  {BODY}");
    assert_eq!(seen(&text, &ctx()), vec![]);
}

#[test]
fn trailing_blank_lines_are_dropped_so_an_empty_message_has_no_subject_line() {
    let text = "\n  \n# Please enter the commit message\n\n";
    let mut want = one("R17/subject", 0, "");
    for label in ["Why:", "Proof:", "Undo:"] {
        want.extend(one("R17/body", 0, label));
    }
    assert_eq!(seen(text, &ctx()), want);
}

#[test]
fn a_fix_commit_without_a_cause_line_is_r9_cause() {
    for subject in [
        "fix: keep the last line",
        "parser: fix the dropped last line",
        "parser: Fixes the dropped last line",
        "parser: FIXED the dropped last line",
        "parser: fixes, at last, the dropped last line",
    ] {
        let text = format!("{subject}{BODY}");
        assert_eq!(
            seen(&text, &ctx()),
            one("R9/cause", 0, "Cause:"),
            "{subject}"
        );
        let text = format!("{subject}{BODY}Cause:  \n");
        assert_eq!(
            seen(&text, &ctx()),
            one("R9/cause", 0, "Cause:"),
            "{subject}"
        );
        let text = format!("{subject}{BODY}Cause: the loop stopped one line early.\n");
        assert_eq!(seen(&text, &ctx()), vec![], "{subject}");
    }
}

#[test]
fn a_malformed_fix_subject_is_still_judged_for_its_cause() {
    for subject in [
        "fix:  keep the last line",
        "fix:keep the last line",
        "fix:\tkeep the last line",
        "fix:",
        "fix: ",
    ] {
        let text = format!("{subject}{BODY}");
        // Whole-message findings sit on line 0, ahead of the subject's line 1.
        let mut want = one("R9/cause", 0, "Cause:");
        want.extend(one("R17/subject", 1, subject));
        assert_eq!(seen(&text, &ctx()), want, "{subject}");
    }
}

#[test]
fn a_commit_that_only_mentions_fixing_needs_no_cause_line() {
    for subject in [
        "prefix: keep the last line",
        "parser: fixture for the last line",
        "parser: fix-up the last line",
        "parser: keep the last line, a fix",
        "fixes: keep the last line",
    ] {
        assert_eq!(
            seen(&format!("{subject}{BODY}"), &ctx()),
            vec![],
            "{subject}"
        );
    }
}
