//! Golden cases: whole messages and the R17 findings they give, judged on the hits.

use house::commitmsg::{Commit as Context, Hit, IDS, hits};

fn ctx() -> Context {
    Context {
        author_ident: "William <w@example.com> 1795271019 +0800".into(),
        author_name: "William".into(),
        author_email: None,
        comment_char: None,
        merge_head: false,
        claudecode: None,
        models: vec!["claude-opus-5-5".into()],
    }
}

fn report(text: &str, ctx: &Context) -> Vec<Hit> {
    hits(text, ctx).expect("judged")
}

fn order(r: &[Hit]) -> Vec<(String, u32)> {
    r.iter().map(|h| (h.id.to_string(), h.line)).collect()
}

#[test]
fn ids_are_section_1cs_subrules_in_table_order() {
    let table = [
        "R17/subject",
        "R17/vague",
        "R17/blank",
        "R17/body",
        "R9/cause",
        "R17/trailer",
        "R17/agent",
        "R17/author",
    ];
    assert_eq!(IDS, table);
}

#[test]
fn findings_come_whole_message_first_then_by_line_in_table_order() {
    let ctx = Context {
        claudecode: Some("1".into()),
        author_ident: "Sample Agent <agent@example.com> 1795271019 +0800".into(),
        ..ctx()
    };
    let text =
        "WIP\nWhy: it was dropped.\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n";
    let r = report(text, &ctx);
    let want = [
        ("R17/body", 0),
        ("R17/body", 0),
        ("R17/agent", 0),
        ("R17/agent", 0),
        ("R17/author", 0),
        ("R17/subject", 1),
        ("R17/blank", 2),
        ("R17/trailer", 3),
    ];
    let want: Vec<(String, u32)> = want.iter().map(|(id, l)| (id.to_string(), *l)).collect();
    assert_eq!(order(&r), want);
    assert!(r.iter().all(|h| IDS.contains(&h.id)));
}

const BODY: &str =
    "\n\nWhy: the gate needs it.\nProof: cargo test, 29 passed.\nUndo: git revert this commit.\n";
const TRAILERS: &str = "\nCo-Authored-By: Claude Opus 5.5 (claude-opus-5-5) <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01AbCdEf23\n";

/// `(case, merging, in a Claude session, message, findings as (id, line))`.
type Case = (&'static str, bool, bool, String, Vec<(&'static str, u32)>);

fn cases() -> Vec<Case> {
    let long_merge = format!("Merge branch '{}' into main", "b".repeat(50));
    vec![
        (
            "house record",
            false,
            false,
            format!("house: add the commit-msg check{BODY}"),
            vec![],
        ),
        (
            "agent record",
            false,
            true,
            format!("house: add the commit-msg check{BODY}{TRAILERS}"),
            vec![],
        ),
        (
            "merge while merging",
            true,
            false,
            format!("Merge branch 'house-wp5' into main{BODY}"),
            vec![],
        ),
        (
            "merge not merging",
            false,
            false,
            format!("Merge branch 'house-wp5' into main{BODY}"),
            vec![("R17/subject", 1)],
        ),
        (
            "merge too long",
            true,
            false,
            format!("{long_merge}{BODY}"),
            vec![("R17/subject", 1)],
        ),
        (
            "revert",
            false,
            false,
            format!(
                "Revert \"house: add the commit-msg check and its {}\"{BODY}",
                "x".repeat(40)
            ),
            vec![],
        ),
        (
            "wip",
            false,
            false,
            format!("house: WIP{BODY}"),
            vec![("R17/vague", 1)],
        ),
        (
            "update.",
            false,
            false,
            format!("house: update.{BODY}"),
            vec![("R17/vague", 1)],
        ),
        (
            "fix, no cause",
            false,
            false,
            format!("house: fix.{BODY}"),
            vec![("R9/cause", 0), ("R17/vague", 1)],
        ),
        (
            "fixed, cause",
            false,
            false,
            format!("house: fixed the parser{BODY}Cause: an off-by-one.\n"),
            vec![],
        ),
        (
            "claude code's default trailer",
            false,
            true,
            format!(
                "house: add the commit-msg check{BODY}\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01AbCdEf23\n"
            ),
            vec![("R17/agent", 0), ("R17/trailer", 7)],
        ),
    ]
}

#[test]
fn section_1cs_examples_judge_as_written() {
    for (case, merging, session, text, want) in cases() {
        let ctx = Context {
            merge_head: merging,
            claudecode: session.then(|| "1".to_string()),
            ..ctx()
        };
        let r = report(&text, &ctx);
        let want: Vec<(String, u32)> = want.iter().map(|(id, l)| (id.to_string(), *l)).collect();
        assert_eq!(order(&r), want, "{case}");
    }
}
