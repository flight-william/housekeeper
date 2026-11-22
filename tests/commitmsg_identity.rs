//! `house::commitmsg`, the identity rule: trailers and their model ids, the session
//! line a Claude co-author owes, and the author.

mod commitmsg_support;

use commitmsg_support::{BODY, ctx, one, seen};
use house::commitmsg::{Commit as Context, check};
use house::finding::Place;

const OPUS: &str = "Co-Authored-By: Claude Opus 5.5 (claude-opus-5-5) <noreply@anthropic.com>";

#[test]
fn a_trailer_naming_a_listed_model_passes() {
    let other = "Co-Authored-By: GPT Agent (gpt-6.1-sol) <noreply@openai.com>";
    let text = format!("parser: keep the last line{BODY}\n{OPUS}\n{other}\n{SESSION}\n");
    assert_eq!(seen(&text, &ctx()), vec![]);
}

#[test]
fn claude_codes_default_trailer_without_a_model_id_is_r17_trailer() {
    let default = "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>";
    let text = format!("parser: keep the last line{BODY}\n{default}\n");
    assert_eq!(seen(&text, &ctx()), one("R17/trailer", 7, default));
}

#[test]
fn a_trailer_whose_shape_is_wrong_is_r17_trailer_on_its_line() {
    for line in [
        "Co-Authored-By: Claude (claude-opus-5-5) <william@example.com>",
        "Co-Authored-By: Claude (Claude-Opus-5-5) <noreply@anthropic.com>",
        "Co-Authored-By: (claude-opus-5-5) <noreply@anthropic.com>",
        "Co-authored-by: Claude (claude-opus-5-5) <noreply@anthropic.com>",
        "Co-Authored-By: Claude (claude-opus-5-5) <noreply@anthropic.com> extra",
    ] {
        let text = format!("parser: keep the last line{BODY}\n{line}\n");
        assert_eq!(seen(&text, &ctx()), one("R17/trailer", 7, line), "{line}");
    }
}

#[test]
fn a_model_id_missing_from_the_list_is_r17_trailer() {
    let old = "Co-Authored-By: Claude Opus 4.1 (claude-opus-4-1) <noreply@anthropic.com>";
    let text = format!("parser: keep the last line{BODY}\n{old}\n");
    assert_eq!(
        seen(&text, &ctx()),
        one("R17/trailer", 7, "claude-opus-4-1")
    );
}

const SESSION: &str = "Claude-Session: https://claude.ai/code/session_01AbCdEf23";

fn claude() -> Context {
    Context {
        claudecode: Some("1".into()),
        ..ctx()
    }
}

#[test]
fn a_claude_session_commit_with_its_trailer_and_session_link_passes() {
    let text = format!("parser: keep the last line{BODY}\n{OPUS}\n{SESSION}\n");
    assert_eq!(seen(&text, &claude()), vec![]);
}

#[test]
fn a_claude_session_commit_missing_either_line_is_r17_agent() {
    let text = format!("parser: keep the last line{BODY}\n{SESSION}\n");
    assert_eq!(
        seen(&text, &claude()),
        one("R17/agent", 0, "Co-Authored-By:")
    );
    let text = format!("parser: keep the last line{BODY}\n{OPUS}\n");
    assert_eq!(
        seen(&text, &claude()),
        one("R17/agent", 0, "Claude-Session:")
    );
    let text = format!(
        "parser: keep the last line{BODY}\n{OPUS}\nClaude-Session: https://claude.ai/code/session_\n"
    );
    assert_eq!(
        seen(&text, &claude()),
        one("R17/agent", 0, "Claude-Session:")
    );
}

#[test]
fn an_invalid_trailer_does_not_count_for_r17_agent() {
    let default = "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>";
    let text = format!("parser: keep the last line{BODY}\n{default}\n{SESSION}\n");
    let mut want = one("R17/agent", 0, "Co-Authored-By:");
    want.extend(one("R17/trailer", 7, default));
    assert_eq!(seen(&text, &claude()), want);
}

#[test]
fn outside_a_claude_session_neither_line_is_required() {
    let text = format!("parser: keep the last line{BODY}");
    for marker in [None, Some("0"), Some("")] {
        let ctx = Context {
            claudecode: marker.map(String::from),
            ..ctx()
        };
        assert_eq!(seen(&text, &ctx), vec![], "{marker:?}");
    }
}

#[test]
fn an_author_other_than_the_policys_name_is_r17_author() {
    let text = format!("parser: keep the last line{BODY}");
    for (ident, name) in [
        (
            "Sample Agent <agent@example.com> 1795271019 +0800",
            "Sample Agent",
        ),
        (
            "William Example <w@example.com> 1795271019 +0800",
            "William Example",
        ),
        ("william <w@example.com> 1795271019 +0800", "william"),
    ] {
        let ctx = Context {
            author_ident: ident.into(),
            ..ctx()
        };
        assert_eq!(seen(&text, &ctx), one("R17/author", 0, name), "{ident}");
    }
}

// Regression: the owner's name with an agent's email must not pass as the owner.
#[test]
fn with_an_expected_email_an_author_with_another_email_is_r17_author() {
    let text = format!("parser: keep the last line{BODY}");
    let pinned = Context {
        author_email: Some("w@example.com".into()),
        ..ctx()
    };
    assert_eq!(seen(&text, &pinned), vec![]);
    let agent = Context {
        author_ident: "William <agent@example.org> 1795271019 +0800".into(),
        ..pinned
    };
    assert_eq!(
        seen(&text, &agent),
        one("R17/author", 0, "William <agent@example.org>")
    );
    let unpinned = Context {
        author_email: None,
        ..agent
    };
    assert_eq!(seen(&text, &unpinned), vec![], "no email set: name only");
}

#[test]
fn an_author_ident_without_a_name_and_email_cannot_run() {
    let ctx = Context {
        author_ident: "1795271019 +0800".into(),
        ..ctx()
    };
    let err = check(&format!("parser: keep the last line{BODY}"), &ctx).unwrap_err();
    assert!(err.what.contains("GIT_AUTHOR_IDENT"), "{err:?}");
}

#[test]
fn a_finding_is_r9_or_r17_on_the_repo_with_fixes_tomls_text() {
    let text = format!("fix: keep the last line{BODY}");
    let findings = check(&text, &ctx()).unwrap();
    let f = &findings[0];
    assert_eq!(f.rule, "R9");
    assert_eq!(f.place, Place::Repo);
    assert_eq!(f.line, None);
    assert_eq!(f.tool_rule, "house r9.cause");
    assert_eq!(f.what, "fix commit without a Cause: line: Cause:");
    assert_eq!(
        f.fix,
        "add a Cause: line saying why the fault happened, not only what was changed"
    );
    let text = format!("Keep the last line{BODY}");
    let f = &check(&text, &ctx()).unwrap()[0];
    assert_eq!((f.rule.as_str(), f.line), ("R17", Some(1)));
    assert_eq!(f.tool_rule, "house r17.subject");
    assert_eq!(
        f.what,
        "subject is not `area: text` within 72 characters: Keep the last line"
    );
    assert!(!f.fix.ends_with('.'));
}

#[test]
fn a_repeated_bad_trailer_is_a_finding_per_line() {
    let bad = "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>";
    let text = format!("parser: keep the last line{BODY}\n{bad}\n{bad}\n");
    let mut want = one("R17/trailer", 7, bad);
    want.extend(one("R17/trailer", 8, bad));
    assert_eq!(seen(&text, &ctx()), want);
}

#[test]
fn a_claude_co_author_outside_a_session_still_owes_the_session_line() {
    let text = format!("parser: keep the last line{BODY}\n{OPUS}\n");
    assert_eq!(seen(&text, &ctx()), one("R17/agent", 0, "Claude-Session:"));
    let text = format!("parser: keep the last line{BODY}\n{OPUS}\n{SESSION}\n");
    assert_eq!(seen(&text, &ctx()), vec![]);
    // Every agent's trailer is checked for shape and a listed model id; owing a
    // trailer is enforced only in a Claude session (CLAUDECODE=1), and only a Claude
    // co-author owes a session line: other agents' markers are unverified.
    for line in [
        "Co-Authored-By: GPT Agent (gpt-6.1-sol) <noreply@openai.com>",
        "Co-Authored-By: Antigravity (gpt-6.1-sol) <noreply@google.com>",
    ] {
        let text = format!("parser: keep the last line{BODY}\n{line}\n");
        assert_eq!(seen(&text, &ctx()), vec![], "{line}");
    }
    // An invalid Claude trailer is R17/trailer only: it names no agent.
    let default = "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>";
    let text = format!("parser: keep the last line{BODY}\n{default}\n");
    assert_eq!(seen(&text, &ctx()), one("R17/trailer", 7, default));
}

#[test]
fn the_accepted_model_ids_are_the_ones_passed_in() {
    let text = format!("parser: keep the last line{BODY}\n{OPUS}\n{SESSION}\n");
    let ctx = Context {
        models: vec!["claude-sonnet-5-5".into()],
        ..ctx()
    };
    assert_eq!(seen(&text, &ctx), one("R17/trailer", 7, "claude-opus-5-5"));
}
