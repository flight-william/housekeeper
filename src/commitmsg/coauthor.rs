//! The identity rule of R17: who a `Co-Authored-By:` line names, an agent with its
//! model id or a person, and the line an agent's session owes. Only the final trailer
//! block is read, as git reads it.

use super::{Commit, Hit, Line, blank};
use unicode_normalization::UnicodeNormalization;

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

/// `R17/trailer` and `R17/agent`.
pub(super) fn trailers(lines: &[Line<'_>], commit: &Commit, found: &mut Vec<Hit>) {
    let mut owed_named = false;
    for l in trailer_block(lines).iter().filter(|l| is_trailer(l.text)) {
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

/// The final trailer block, as git and GitHub read one: the lines after the last blank
/// line, never the subject, when the first is `Key: value` and each other one is too or
/// continues the line above (starts with a space or tab); empty otherwise. A co-author
/// line anywhere else is body text, which no tool reads as a co-author.
fn trailer_block<'a, 'b>(lines: &'b [Line<'a>]) -> &'b [Line<'a>] {
    let start = lines
        .iter()
        .rposition(|l| blank(l.text))
        .map_or(1, |i| i + 1)
        .max(1);
    let block = lines.get(start..).unwrap_or(&[]);
    let first = block.first().is_some_and(|l| key_value(l.text));
    let rest = block
        .iter()
        .all(|l| key_value(l.text) || l.text.starts_with([' ', '\t']));
    if first && rest { block } else { &[] }
}

/// `Key: value`, the key letters, digits and `-`.
fn key_value(line: &str) -> bool {
    line.split_once(':').is_some_and(|(key, _)| {
        !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
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
    // As git will commit it: its clean-up strips trailing whitespace.
    let line = line.trim_end();
    // The key in any case, as git reads trailer keys and GitHub writes `Co-authored-by:`.
    let rest = line
        .get(..16)
        .filter(|key| key.eq_ignore_ascii_case("co-authored-by: "))
        .and_then(|_| line.get(16..))?
        .strip_suffix('>')?;
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

/// A `Co-Authored-By:` line, its key in any case as git reads trailer keys.
fn is_trailer(line: &str) -> bool {
    line.get(..15)
        .is_some_and(|key| key.eq_ignore_ascii_case("co-authored-by:"))
}
