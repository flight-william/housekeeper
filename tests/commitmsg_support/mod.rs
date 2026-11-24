//! Shared by the commit-message tests: the sample commit context and its helpers.

use house::commitmsg::{Commit as Context, hits};

pub fn ctx() -> Context {
    Context {
        author_ident: "William <w@example.com> 1795271019 +0800".into(),
        author_name: "William".into(),
        author_email: None,
        comment_char: None,
        merge_head: false,
        agent_session: false,
        models: ["claude-opus-5-5", "gpt-6.1-sol"]
            .map(String::from)
            .to_vec(),
    }
}

pub const BODY: &str = "\n\nWhy: the parser dropped the last line.\nProof: cargo test, 12 passed.\nUndo: git revert this commit.\n";

/// `(rule[/sub], line, match)` of each finding, in order.
pub fn seen(text: &str, ctx: &Context) -> Vec<(String, u32, String)> {
    hits(text, ctx)
        .expect("judged")
        .into_iter()
        .map(|h| (h.id.to_string(), h.line, h.matched))
        .collect()
}

pub fn one(id: &str, line: u32, matched: &str) -> Vec<(String, u32, String)> {
    vec![(id.to_string(), line, matched.to_string())]
}
