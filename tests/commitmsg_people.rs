//! `house::commitmsg`, a person's `Co-Authored-By:` line: no model id, a name that is
//! no agent's in any spelling, the key in any case, judged as git will commit it.

mod commitmsg_support;

use commitmsg_support::{BODY, ctx, one, seen};
use house::commitmsg::Commit as Context;

const OPUS: &str = "Co-Authored-By: Claude Opus 5.5 (claude-opus-5-5) <noreply@anthropic.com>";

fn claude() -> Context {
    Context {
        session: Some("claude"),
        ..ctx()
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

// An agent named as a person, in any spelling: the name is compared normalised (NFKC,
// zero-width and format characters gone, case folded, Latin look-alikes from other
// scripts read as Latin, punctuation and spaces gone), and refused when it holds an
// agent's or vendor's name or looks like a model id.
#[test]
fn an_agent_spelled_as_a_person_is_r17_trailer() {
    for name in [
        "ClaudeCode",
        "Claude-Code",
        "Claude, Opus",
        "Claude.ai",
        "Anthropic Claude",
        "OpenAI Codex",
        "Google Gemini",
        "claude-opus-4-1",
        "Opus 5.5",
        "GPT-6",
        "Cl\u{430}ude",
        "\u{FF23}laude",
        "Claude\u{200B} Opus",
        "C\u{200D}laude",
        "\u{1D402}laude",
        "Anthropic",
        "Sam (via OpenAI)",
    ] {
        let line = format!("Co-Authored-By: {name} <noreply@example.com>");
        let text = format!("parser: keep the last line{BODY}\n{line}\n");
        assert_eq!(seen(&text, &ctx()), one("R17/trailer", 7, &line), "{name}");
    }
}

// Beside a valid agent line in a session, a second line posing as a person is refused.
#[test]
fn an_agent_posing_as_a_person_beside_its_own_line_is_r17_trailer() {
    let posing = "Co-Authored-By: claude-opus-4-1 <noreply@anthropic.com>";
    let text = format!("parser: keep the last line{BODY}\n{OPUS}\n{posing}\n");
    assert_eq!(seen(&text, &claude()), one("R17/trailer", 8, posing));
}

#[test]
fn people_names_with_digits_or_marks_still_pass() {
    for name in [
        "sam42",
        "Sam O'Example",
        "Zo\u{eb} Ex\u{e1}mple",
        "Sam Example Jr.",
    ] {
        let line = format!("Co-Authored-By: {name} <sam@example.com>");
        let text = format!("parser: keep the last line{BODY}\n{line}\n");
        assert_eq!(seen(&text, &ctx()), vec![], "{name}");
    }
}

// git reads trailer keys in any case, and GitHub writes `Co-authored-by:`.
#[test]
fn the_co_author_key_is_read_in_any_case() {
    let person = "Co-authored-by: Sam Example <sam@example.com>";
    let text = format!("parser: keep the last line{BODY}\n{person}\n");
    assert_eq!(seen(&text, &ctx()), vec![]);
    for agent in [
        "Co-authored-by: Claude (claude-opus-5-5) <noreply@anthropic.com>",
        "co-authored-by: Claude (claude-opus-5-5) <noreply@anthropic.com>",
        "CO-AUTHORED-BY: Claude (claude-opus-5-5) <noreply@anthropic.com>",
    ] {
        let text = format!("parser: keep the last line{BODY}\n{agent}\n");
        assert_eq!(seen(&text, &claude()), vec![], "{agent}");
    }
    let unlisted = "Co-authored-by: Claude (claude-opus-4-1) <noreply@anthropic.com>";
    let text = format!("parser: keep the last line{BODY}\n{unlisted}\n");
    assert_eq!(
        seen(&text, &ctx()),
        one("R17/trailer", 7, "claude-opus-4-1")
    );
}

// The known limit, as the README says: a person whose name holds an agent's name is
// refused like the agent, and a bracketed nickname reads as an agent line.
#[test]
fn people_named_like_an_agent_are_refused_a_known_limit() {
    for line in [
        "Co-Authored-By: Claude Monet <claude@example.fr>",
        "Co-Authored-By: Gemini Ortiz <gemini@example.com>",
        "Co-Authored-By: Samuel (Sam) Example <sam@example.com>",
    ] {
        let text = format!("parser: keep the last line{BODY}\n{line}\n");
        assert_eq!(seen(&text, &ctx()), one("R17/trailer", 7, line), "{line}");
    }
}

// The hook sees the message before git's clean-up strips trailing whitespace; the line
// is judged as git will commit it.
#[test]
fn trailing_whitespace_on_a_co_author_line_is_not_a_finding() {
    let person = "Co-Authored-By: Sam Example <sam@example.com> ";
    let text = format!("parser: keep the last line{BODY}\n{person}\n");
    assert_eq!(seen(&text, &ctx()), vec![]);
    let agent = "Co-Authored-By: Claude (claude-opus-5-5) <noreply@anthropic.com>\t";
    let text = format!("parser: keep the last line{BODY}\n{agent}\n");
    assert_eq!(seen(&text, &claude()), vec![]);
}
