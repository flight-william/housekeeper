//! `house::commitmsg`, the identity rule: Co-Authored-By lines, an agent's with its
//! model id and a person's with none, the agent line an agent session owes, and the
//! author. No session line is owed or judged.

mod commitmsg_support;

use commitmsg_support::{BODY, ctx, one, seen};
use house::commitmsg::{AGENT_MARKERS, AGENTS, Commit as Context, check, session_agent};
use house::finding::Place;

const OPUS: &str = "Co-Authored-By: Claude Opus 5.5 (claude-opus-5-5) <noreply@anthropic.com>";

#[test]
fn a_trailer_naming_a_listed_model_passes() {
    let other = "Co-Authored-By: GPT Agent (gpt-6.1-sol) <noreply@openai.com>";
    let text = format!("parser: keep the last line{BODY}\n{OPUS}\n{other}\n");
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

fn claude() -> Context {
    Context {
        session: Some("claude"),
        ..ctx()
    }
}

#[test]
fn an_agent_session_commit_with_its_co_author_line_passes() {
    let text = format!("parser: keep the last line{BODY}\n{OPUS}\n");
    assert_eq!(seen(&text, &claude()), vec![]);
}

#[test]
fn an_agent_session_commit_without_an_agent_co_author_is_r17_agent() {
    let text = format!("parser: keep the last line{BODY}");
    assert_eq!(
        seen(&text, &claude()),
        one("R17/agent", 0, "Co-Authored-By:")
    );
    // A person as co-author does not stand for the agent that wrote the commit.
    let person = "Co-Authored-By: Sam Example <sam@example.com>";
    let text = format!("parser: keep the last line{BODY}\n{person}\n");
    assert_eq!(
        seen(&text, &claude()),
        one("R17/agent", 0, "Co-Authored-By:")
    );
}

#[test]
fn an_invalid_trailer_does_not_count_for_r17_agent() {
    let default = "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>";
    let text = format!("parser: keep the last line{BODY}\n{default}\n");
    let mut want = one("R17/agent", 0, "Co-Authored-By:");
    want.extend(one("R17/trailer", 7, default));
    assert_eq!(seen(&text, &claude()), want);
}

#[test]
fn outside_an_agent_session_no_co_author_line_is_required() {
    let text = format!("parser: keep the last line{BODY}");
    assert_eq!(seen(&text, &ctx()), vec![]);
}

#[test]
fn an_agent_session_is_a_marker_set_to_its_value() {
    let env = |set: Option<&'static str>| {
        move |name: &str| {
            (name == "CLAUDECODE")
                .then_some(set)
                .flatten()
                .map(String::from)
        }
    };
    assert_eq!(session_agent(AGENT_MARKERS, env(Some("1"))), Some("claude"));
    for value in [None, Some("0"), Some("")] {
        assert_eq!(session_agent(AGENT_MARKERS, env(value)), None, "{value:?}");
    }
    // Each row names an agent house knows, so its session owes that agent's line.
    for (_, _, agent) in AGENT_MARKERS {
        assert!(AGENTS.iter().any(|a| a.name == *agent), "{agent}");
    }
}

// The session line is noise once the session is gone: never owed, never judged.
#[test]
fn no_session_line_is_owed_or_judged_in_or_out_of_a_session() {
    for ctx in [ctx(), claude()] {
        let text = format!("parser: keep the last line{BODY}\n{OPUS}\n");
        assert_eq!(seen(&text, &ctx), vec![]);
        for session in [
            "Claude-Session: https://claude.ai/code/session_01AbCdEf23",
            "Claude-Session: https://claude.ai/code/session_",
            "Claude-Session: anything",
        ] {
            let text = format!("parser: keep the last line{BODY}\n{OPUS}\n{session}\n");
            assert_eq!(seen(&text, &ctx), vec![], "{session}");
        }
    }
}

#[test]
fn a_person_as_co_author_needs_no_model_id() {
    for line in [
        "Co-Authored-By: Sam Example <sam@example.com>",
        "Co-Authored-By: Zo\u{eb} Ex\u{e1}mple <zoe@example.org>",
        "Co-Authored-By: sam <12345+sam@users.noreply.github.com>",
    ] {
        let text = format!("parser: keep the last line{BODY}\n{line}\n");
        assert_eq!(seen(&text, &ctx()), vec![], "{line}");
        let text = format!("parser: keep the last line{BODY}\n{OPUS}\n{line}\n");
        assert_eq!(seen(&text, &claude()), vec![], "{line}");
    }
}

#[test]
fn an_agent_named_without_its_model_id_is_r17_trailer() {
    for line in [
        "Co-Authored-By: Claude <noreply@anthropic.com>",
        "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>",
        "Co-Authored-By: Codex <noreply@openai.com>",
        "Co-Authored-By: Antigravity <noreply@google.com>",
        "Co-Authored-By: Gemini 3 <noreply@google.com>",
        "Co-Authored-By: claude <someone@example.com>",
    ] {
        let text = format!("parser: keep the last line{BODY}\n{line}\n");
        assert_eq!(seen(&text, &ctx()), one("R17/trailer", 7, line), "{line}");
    }
}

#[test]
fn a_person_line_without_a_name_or_an_address_is_r17_trailer() {
    for line in [
        "Co-Authored-By: <sam@example.com>",
        "Co-Authored-By: Sam Example <sam.example.com>",
        "Co-Authored-By: Sam Example sam@example.com",
        "Co-Authored-By: Sam <sam@example.com> extra",
    ] {
        let text = format!("parser: keep the last line{BODY}\n{line}\n");
        assert_eq!(seen(&text, &ctx()), one("R17/trailer", 7, line), "{line}");
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
fn any_agents_trailer_is_checked_for_its_model_id() {
    for line in [
        "Co-Authored-By: GPT Agent (gpt-6.1-sol) <noreply@openai.com>",
        "Co-Authored-By: Codex (gpt-6.1-sol) <noreply@openai.com>",
    ] {
        let text = format!("parser: keep the last line{BODY}\n{line}\n");
        assert_eq!(seen(&text, &ctx()), vec![], "{line}");
    }
    let unlisted = "Co-Authored-By: Codex (gpt-9-nova) <noreply@openai.com>";
    let text = format!("parser: keep the last line{BODY}\n{unlisted}\n");
    assert_eq!(seen(&text, &ctx()), one("R17/trailer", 7, "gpt-9-nova"));
}

#[test]
fn the_accepted_model_ids_are_the_ones_passed_in() {
    let text = format!("parser: keep the last line{BODY}\n{OPUS}\n");
    let ctx = Context {
        models: vec!["claude-sonnet-5-5".into()],
        ..ctx()
    };
    assert_eq!(seen(&text, &ctx), one("R17/trailer", 7, "claude-opus-5-5"));
}

// A known agent's line gives one of that agent's own model ids: Claude claude-*,
// Codex gpt-*, Antigravity and Gemini gemini-*.
#[test]
fn a_known_agents_line_with_another_agents_model_id_is_r17_trailer() {
    for (line, id) in [
        (
            "Co-Authored-By: Antigravity (gpt-6.1-sol) <noreply@google.com>",
            "gpt-6.1-sol",
        ),
        (
            "Co-Authored-By: Codex (claude-opus-5-5) <noreply@openai.com>",
            "claude-opus-5-5",
        ),
        (
            "Co-Authored-By: Claude (gpt-6.1-sol) <noreply@anthropic.com>",
            "gpt-6.1-sol",
        ),
    ] {
        let text = format!("parser: keep the last line{BODY}\n{line}\n");
        assert_eq!(seen(&text, &ctx()), one("R17/trailer", 7, id), "{line}");
    }
}

// In an agent's session the owed line is that agent's: its name and one of its model
// ids. Another agent's line, or a made-up name with a listed id, does not count.
#[test]
fn in_a_claude_session_only_a_claude_line_counts() {
    for line in [
        "Co-Authored-By: Codex (gpt-6.1-sol) <noreply@openai.com>",
        "Co-Authored-By: Sam Example (claude-opus-5-5) <noreply@example.com>",
        "Co-Authored-By: GPT Agent (gpt-6.1-sol) <noreply@openai.com>",
    ] {
        let text = format!("parser: keep the last line{BODY}\n{line}\n");
        assert_eq!(
            seen(&text, &claude()),
            one("R17/agent", 0, "Co-Authored-By:"),
            "{line}"
        );
    }
    for line in [
        OPUS,
        "Co-Authored-By: Claude (claude-opus-5-5) <noreply@anthropic.com>",
        "Co-Authored-By: Claude Code (claude-opus-5-5) <noreply@anthropic.com>",
    ] {
        let text = format!("parser: keep the last line{BODY}\n{line}\n");
        assert_eq!(seen(&text, &claude()), vec![], "{line}");
    }
}
