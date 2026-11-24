//! R17 commit messages, and R9's `Cause:` line: `check` judges one commit message.
//!
//! The accepted model ids are an argument, read from `.house.toml` by the core; there
//! is no regex crate (the patterns are written out by hand below); and an identity
//! rule: a `Co-Authored-By:` line is optional, an agent's carries its model id and a
//! person's carries none, and a commit made in an agent's session (`AGENT_MARKERS`)
//! owes that agent's line: its name and one of its own model ids (`AGENTS`). No session line is owed or judged: a session link outlives
//! the session it names.
//!
//! R9/cause is judged on the area (the subject's text before its first colon, trimmed)
//! and the first word after it, whether or not the subject passes R17/subject. The first
//! word is read with trailing ASCII punctuation stripped, so `fix.` and `fixes,` count.

use crate::adapter::CannotCheck;
use crate::finding::{Finding, Place};
use unicode_normalization::UnicodeNormalization;

/// Every id `hits` can emit, in table order (same-line findings sort in this order).
pub const IDS: &[&str] = &[
    "R17/subject",
    "R17/vague",
    "R17/blank",
    "R17/body",
    "R9/cause",
    "R17/trailer",
    "R17/agent",
    "R17/author",
];

/// What `check` knows besides the message text. The core reads each value from git.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    /// `git var GIT_AUTHOR_IDENT`, as printed: `Name <email> <time> <zone>`.
    pub author_ident: String,
    /// The expected author name (the owner's git identity).
    pub author_name: String,
    /// The expected author email; `None` checks the name only.
    pub author_email: Option<String>,
    /// `git config --get core.commentChar`; `None` when unset.
    pub comment_char: Option<String>,
    /// Whether the file `git rev-parse --git-path MERGE_HEAD` names exists.
    pub merge_head: bool,
    /// The agent (`Agent::name`) whose session the commit is made in, from
    /// `session_agent`; `None` outside an agent's session.
    pub session: Option<&'static str>,
    /// The model ids a `Co-Authored-By:` line may name (`.house.toml`).
    pub models: Vec<String>,
}

/// An agent house knows: the name its lines carry, folded (`folded`), and the start
/// of its model ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Agent {
    pub name: &'static str,
    pub models: &'static str,
}

/// The agents, each with its own model-id family.
pub const AGENTS: &[Agent] = &[
    Agent {
        name: "claude",
        models: "claude-",
    },
    Agent {
        name: "codex",
        models: "gpt-",
    },
    Agent {
        name: "antigravity",
        models: "gemini-",
    },
    Agent {
        name: "gemini",
        models: "gemini-",
    },
];

/// The variable and value each agent sets in the shells its session runs, and the
/// agent (`Agent::name`): one row per agent, the same rule for each. Only markers seen
/// set are listed: Claude Code's.
pub const AGENT_MARKERS: &[(&str, &str, &str)] = &[("CLAUDECODE", "1", "claude")];

/// The agent of the first row of `markers` whose variable `var` (an environment
/// lookup) holds at its value; `None` when none does.
pub fn session_agent(
    markers: &[(&str, &str, &'static str)],
    var: impl Fn(&str) -> Option<String>,
) -> Option<&'static str> {
    markers
        .iter()
        .find(|(name, value, _)| var(name).as_deref() == Some(*value))
        .map(|(_, _, agent)| *agent)
}

/// The agent a name names: the first of `AGENTS` whose name is in it, folded.
fn agent_named(name: &str) -> Option<&'static Agent> {
    let name = folded(name);
    AGENTS.iter().find(|a| name.contains(a.name))
}

/// Names a person's line may not hold: the agents' and their makers'.
const NOT_A_PERSON: [&str; 6] = [
    "claude",
    "codex",
    "antigravity",
    "gemini",
    "anthropic",
    "openai",
];

/// A name as compared with agent names, so a respelling reads as what it spells:
/// characters that print nothing gone, NFKC (fullwidth and styled letters to plain
/// ones), case folded, the Cyrillic and Greek letters drawn like Latin ones read as
/// those, then letters and digits only.
fn folded(name: &str) -> String {
    let visible: String = name.chars().filter(|c| !invisible(*c)).collect();
    let plain: String = visible.nfkc().collect();
    plain
        .to_uppercase()
        .to_lowercase()
        .chars()
        .map(latin_look_alike)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Unicode's default-ignorable characters: zero-width, joiners, direction and other
/// format marks, variation selectors, tags. Each prints nothing inside a name.
fn invisible(c: char) -> bool {
    matches!(c,
        '\u{AD}' | '\u{34F}' | '\u{61C}' | '\u{115F}' | '\u{1160}' | '\u{17B4}' | '\u{17B5}'
        | '\u{180B}'..='\u{180F}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{206F}' | '\u{3164}' | '\u{FE00}'..='\u{FE0F}' | '\u{FEFF}'
        | '\u{FFA0}' | '\u{FFF0}'..='\u{FFF8}' | '\u{1BCA0}'..='\u{1BCA3}'
        | '\u{1D173}'..='\u{1D17A}' | '\u{E0000}'..='\u{E0FFF}')
}

/// The Latin letter a lowercase Cyrillic or Greek letter is drawn like, else itself.
fn latin_look_alike(c: char) -> char {
    match c {
        'а' | 'α' => 'a',
        'в' | 'β' => 'b',
        'с' | 'ϲ' => 'c',
        'ԁ' => 'd',
        'е' | 'ε' => 'e',
        'һ' | 'н' => 'h',
        'і' | 'ι' | 'ӏ' => 'i',
        'ј' => 'j',
        'к' | 'κ' => 'k',
        'м' => 'm',
        'η' => 'n',
        'о' | 'ο' | 'σ' => 'o',
        'р' | 'ρ' => 'p',
        'ԛ' => 'q',
        'ѕ' => 's',
        'т' | 'τ' => 't',
        'υ' => 'u',
        'ν' => 'v',
        'ԝ' | 'ω' => 'w',
        'х' | 'χ' => 'x',
        'у' | 'γ' => 'y',
        _ => c,
    }
}

/// A name that is a model id or a model's: a word holding a digit with `-` or `.`
/// (`claude-opus-4-1`, `GPT-6`, `Opus 5.5`).
fn looks_like_a_model(name: &str) -> bool {
    let plain: String = name.nfkc().collect();
    plain
        .split_whitespace()
        .any(|word| word.chars().any(|c| c.is_ascii_digit()) && word.contains(['-', '.']))
}

/// One problem found, before it becomes a `Finding`: which id, where, and what matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub id: &'static str,
    /// The message file's line, 1-based; 0 for a finding about the whole message.
    pub line: u32,
    pub matched: String,
}

/// Every finding of the message: whole-message ones first, then by line, on one line
/// in `IDS` order. Place is `Repo`; `what`/`fix` are the messages returned by this module.
pub fn check(text: &str, commit: &Commit) -> Result<Vec<Finding>, CannotCheck> {
    Ok(hits(text, commit)?.into_iter().map(finding).collect())
}

/// The hits `check` turns into findings, for tests and the record.
pub fn hits(text: &str, commit: &Commit) -> Result<Vec<Hit>, CannotCheck> {
    let Some((author, rest)) = commit.author_ident.split_once(" <") else {
        return Err(cannot("GIT_AUTHOR_IDENT has no `Name <email>`"));
    };
    let email = rest.split_once('>').map_or(rest, |(email, _)| email);
    let lines = message(text, comment_char(commit)?);
    let mut found = Vec::new();
    let parts = subject(&lines, commit, &mut found);
    body(&lines, parts, &mut found);
    trailers(&lines, commit, &mut found);
    let wrong_email = commit.author_email.as_deref().is_some_and(|e| e != email);
    if wrong_email {
        found.push(Hit::whole("R17/author", &format!("{author} <{email}>")));
    } else if author != commit.author_name {
        found.push(Hit::whole("R17/author", author));
    }
    let rank = |id: &str| IDS.iter().position(|i| *i == id).unwrap_or(IDS.len());
    found.sort_by_key(|h| (h.line, rank(h.id)));
    Ok(found)
}

/// `R17/subject` and `R17/vague`; returns `(area, the text after the first colon)` for
/// R9's cause rule, which reads any subject holding a colon, well-formed or not.
fn subject<'a>(
    lines: &[Line<'a>],
    commit: &Commit,
    found: &mut Vec<Hit>,
) -> Option<(&'a str, &'a str)> {
    let Some(s) = lines.first() else {
        found.push(Hit::whole("R17/subject", ""));
        return None;
    };
    let parts = area_text(s.text);
    // Git's own `Merge ` and `Revert "` subjects carry no area.
    let area = s
        .text
        .split_once(':')
        .filter(|_| !s.text.starts_with("Merge ") && !s.text.starts_with("Revert \""))
        .map(|(area, said)| (area.trim(), said.trim()));
    let merge = commit.merge_head && s.text.starts_with("Merge ");
    let shaped = merge || parts.is_some();
    let revert = s.text.starts_with("Revert \"");
    if !revert && (!shaped || s.text.chars().count() > 72) {
        found.push(Hit::on("R17/subject", s, s.text));
    }
    if let Some((_, said)) = parts {
        let lower = said.to_lowercase();
        let word = lower.strip_suffix('.').unwrap_or(&lower);
        if ["wip", "fix", "update"].contains(&word) {
            found.push(Hit::on("R17/vague", s, said));
        }
    }
    area
}

/// `^([a-z0-9][a-z0-9._/-]*): (\S.*)$`: the area and the text, or `None`.
fn area_text(subject: &str) -> Option<(&str, &str)> {
    let (area, rest) = subject.split_once(':')?;
    let said = rest.strip_prefix(' ')?;
    let mut chars = area.chars();
    let first = chars.next()?;
    let ok = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
    let shaped = ok(first) && chars.all(|c| ok(c) || "._/-".contains(c));
    let starts = said.chars().next().is_some_and(|c| !c.is_whitespace());
    (shaped && starts).then_some((area, said))
}

/// `R17/blank`, `R17/body` and `R9/cause`.
fn body(lines: &[Line<'_>], parts: Option<(&str, &str)>, found: &mut Vec<Hit>) {
    if let Some(l) = lines.get(1).filter(|l| !blank(l.text)) {
        found.push(Hit::on("R17/blank", l, l.text));
    }
    for label in ["Why:", "Proof:", "Undo:"] {
        if !lines.iter().any(|l| labelled(l.text, label)) {
            found.push(Hit::whole("R17/body", label));
        }
    }
    let fix = parts.is_some_and(|(area, said)| {
        let first = said.split_whitespace().next().unwrap_or("");
        let first = first.trim_end_matches(|c: char| c.is_ascii_punctuation());
        let first = first.to_lowercase();
        area == "fix" || ["fix", "fixes", "fixed"].contains(&first.as_str())
    });
    if fix && !lines.iter().any(|l| labelled(l.text, "Cause:")) {
        found.push(Hit::whole("R9/cause", "Cause:"));
    }
}

/// `R17/trailer` and `R17/agent`.
fn trailers(lines: &[Line<'_>], commit: &Commit, found: &mut Vec<Hit>) {
    let mut owed_named = false;
    for l in lines.iter().filter(|l| is_trailer(l.text)) {
        match co_author(l.text) {
            None => found.push(Hit::on("R17/trailer", l, l.text)),
            Some(CoAuthor::Agent(name, id)) => {
                let agent = agent_named(name);
                let listed = commit.models.iter().any(|m| m == id);
                let its_own = agent.is_none_or(|a| id.starts_with(a.models));
                if !listed || !its_own {
                    found.push(Hit::on("R17/trailer", l, id));
                } else if agent.is_some_and(|a| Some(a.name) == commit.session) {
                    owed_named = true;
                }
            }
            Some(CoAuthor::Person) => {}
        }
    }
    // The same for every agent: in its session, the commit names it and its model.
    if commit.session.is_some() && !owed_named {
        found.push(Hit::whole("R17/agent", "Co-Authored-By:"));
    }
}

/// Who a valid `Co-Authored-By:` line names: an agent, with its model id, or a person.
enum CoAuthor<'a> {
    /// The name and the model id.
    Agent(&'a str, &'a str),
    Person,
}

/// A valid `Co-Authored-By:` line. With a `(model id)` part it is an agent's, the
/// pattern `^Co-Authored-By: [A-Za-z][A-Za-z0-9 .-]* \(([a-z0-9.-]+)\) <noreply[^>\s]*@[^>\s]+>$`;
/// without, a person's, `Name <local@domain>`, the name not an agent's (`AGENTS`).
fn co_author(line: &str) -> Option<CoAuthor<'_>> {
    let rest = line.strip_prefix("Co-Authored-By: ")?.strip_suffix('>')?;
    let (head, email) = rest.rsplit_once(" <")?;
    if email
        .chars()
        .any(|c| c == '<' || c == '>' || c.is_whitespace())
    {
        return None;
    }
    let Some((name, id)) = head.strip_suffix(')').and_then(|h| h.rsplit_once(" (")) else {
        return person(head, email).then_some(CoAuthor::Person);
    };
    let mut name_chars = name.chars();
    let name_ok = name_chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && name_chars.all(|c| c.is_ascii_alphanumeric() || " .-".contains(c));
    let id_ok = !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || ".-".contains(c));
    let at = email.get(7..email.len().saturating_sub(1)).unwrap_or("");
    let email_ok = email.starts_with("noreply") && at.contains('@');
    (name_ok && id_ok && email_ok).then_some(CoAuthor::Agent(name, id))
}

/// A person's name and address: a name with no brackets, and an address with text on
/// both sides of its `@`. As a second guard, a name that, folded (`folded`), holds an
/// agent's or its maker's name, or looks like a model id, is not a person's: the rule
/// cannot prove who typed a line, but an agent does not pass as a person by spelling.
fn person(name: &str, email: &str) -> bool {
    let fold = folded(name);
    let name_ok = !name.trim().is_empty()
        && name == name.trim()
        && !name.contains(['<', '>', '(', ')'])
        && !NOT_A_PERSON.iter().any(|n| fold.contains(n))
        && !looks_like_a_model(name);
    let email_ok = email
        .split_once('@')
        .is_some_and(|(local, domain)| !local.is_empty() && !domain.is_empty());
    name_ok && email_ok
}

/// A `Co-Authored-By:` line, its key in any case as git reads trailer keys, so a
/// `Co-authored-by:` line is judged (and fails the pattern) rather than ignored.
fn is_trailer(line: &str) -> bool {
    line.get(..15)
        .is_some_and(|key| key.eq_ignore_ascii_case("co-authored-by:"))
}

/// The message text git will commit (`message`'s lines, joined), for the approval
/// check: one clean-up for R17 and approval.
pub fn kept(text: &str, commit: &Commit) -> Result<String, CannotCheck> {
    let lines = message(text, comment_char(commit)?);
    Ok(lines.iter().map(|l| format!("{}\n", l.text)).collect())
}

/// One line of the message that survives clean-up, with its line number in the file.
struct Line<'a> {
    number: u32,
    text: &'a str,
}

/// The message as git will commit it: the file's lines, numbered from 1, without
/// comment lines and without everything from git's scissors line on. Trailing blank
/// lines go too.
fn message(text: &str, comment: char) -> Vec<Line<'_>> {
    let scissors = format!("{comment} ------------------------ >8 ------------------------");
    let mut lines: Vec<Line<'_>> = text
        .lines()
        .zip(1..)
        .take_while(|(text, _)| *text != scissors)
        .filter(|(text, _)| !text.starts_with(comment))
        .map(|(text, number)| Line { number, text })
        .collect();
    while lines.last().is_some_and(|l| blank(l.text)) {
        lines.pop();
    }
    lines
}

impl Hit {
    fn whole(id: &'static str, matched: &str) -> Hit {
        Hit {
            id,
            line: 0,
            matched: matched.to_string(),
        }
    }

    fn on(id: &'static str, line: &Line<'_>, matched: &str) -> Hit {
        Hit {
            id,
            line: line.number,
            matched: matched.to_string(),
        }
    }
}

/// What each R17 and R9/cause finding says and how to fix it; the trailer fix names
/// `.house.toml`, where the accepted model ids are listed.
fn texts(id: &str) -> (&'static str, &'static str) {
    match id {
        "R17/subject" => (
            "subject is not `area: text` within 72 characters",
            "write line 1 as `<area>: <what changed>`, the area lowercase, 72 characters at most",
        ),
        "R17/vague" => (
            "subject says only wip, fix or update",
            "say what changed, in words a reader of the log understands without the diff",
        ),
        "R17/blank" => (
            "line 2 is not empty",
            "leave line 2 empty between the subject and the body",
        ),
        "R17/body" => (
            "body lacks a required line",
            "add the Why:, Proof: and Undo: lines, each with text after the colon",
        ),
        "R9/cause" => (
            "fix commit without a Cause: line",
            "add a Cause: line saying why the fault happened, not only what was changed",
        ),
        "R17/trailer" => (
            "Co-Authored-By line malformed, naming an unknown model, or naming an agent without its model id",
            "write `Co-Authored-By: <Name> (<model id>) <noreply@…>` for an agent, with a model id listed in .house.toml, or `Co-Authored-By: <Name> <address>` for a person",
        ),
        "R17/agent" => (
            "agent commit without a valid Co-Authored-By line naming the agent",
            "add `Co-Authored-By: <Name> (<model id>) <noreply@…>` with a model id listed in .house.toml",
        ),
        _ => (
            "commit author is not the installed author",
            "commit under the author name bound at install (git config user.name)",
        ),
    }
}

fn finding(h: Hit) -> Finding {
    let (rule, sub) = h.id.split_once('/').unwrap_or((h.id, ""));
    let (what, fix) = texts(h.id);
    let what = if h.matched.is_empty() {
        what.to_string()
    } else {
        format!("{what}: {}", h.matched)
    };
    Finding {
        rule: rule.to_string(),
        place: Place::Repo,
        line: (h.line > 0).then_some(h.line),
        what,
        fix: fix.to_string(),
        tool_rule: format!("house {}.{sub}", rule.to_lowercase()),
        measure: None,
    }
}

fn cannot(what: &str) -> CannotCheck {
    CannotCheck {
        what: what.to_string(),
        fix: "set the git setting house reads, then commit again".to_string(),
    }
}

/// `core.commentChar` when set, else `#`. git also takes `auto` and several
/// characters; house reads one character only, so anything else cannot check.
fn comment_char(commit: &Commit) -> Result<char, CannotCheck> {
    let value = commit.comment_char.as_deref().unwrap_or("#");
    let mut chars = value.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Ok(c),
        _ => Err(cannot(&format!(
            "core.commentChar {value:?} is not one character; house reads only one"
        ))),
    }
}

/// Whether `line` is `<label> <text>`: the label, a space, then something not blank.
fn labelled(line: &str, label: &str) -> bool {
    line.strip_prefix(label)
        .and_then(|rest| rest.strip_prefix(' '))
        .is_some_and(|text| !text.trim().is_empty())
}

/// Empty once git's clean-up strips trailing whitespace.
fn blank(line: &str) -> bool {
    line.trim().is_empty()
}
