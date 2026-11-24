//! R17 over whole histories: made-up commit messages in the format
//! `git log --format='%h%x01%an%x01%B%x00'` prints, newest first, judged as a Claude
//! Code session would (CLAUDECODE=1) under author `William`. sample-history-pass
//! holds messages of every kind that pass; sample-history-old-trailer holds a range
//! that all fails, most of it under Claude Code's old default trailer.

use house::commitmsg::{Commit, hits};

const MODELS: &[&str] = &[
    "claude-opus-5-5",
    "claude-sonnet-5-5",
    "claude-haiku-5-5",
    "claude-fable-5-1",
    "gpt-6.1-sol",
    "gpt-6-astra",
    "gpt-6-luna",
    "gemini-3.8-flash-low",
    "gemini-4-argon-medium",
];

/// `(short hash, failing ids)` per commit, newest first.
fn judge(log: &str) -> Vec<(String, Vec<String>)> {
    log.split('\0')
        .map(|r| r.trim_start_matches('\n'))
        .filter(|r| !r.is_empty())
        .map(|record| {
            let mut parts = record.splitn(3, '\u{1}');
            let hash = parts.next().unwrap_or("").to_string();
            let author = parts.next().unwrap_or("");
            let text = parts.next().unwrap_or("");
            let commit = Commit {
                author_ident: format!("{author} <a@example.com> 0 +0000"),
                author_name: "William".into(),
                author_email: None,
                comment_char: None,
                merge_head: false,
                agent_session: true,
                models: MODELS.iter().map(|m| m.to_string()).collect(),
            };
            let ids = hits(text, &commit)
                .expect("judged")
                .into_iter()
                .map(|h| format!("{} {}", h.id, h.matched))
                .collect();
            (hash, ids)
        })
        .collect()
}

#[test]
fn every_sample_message_passes() {
    let judged = judge(include_str!("fixtures/commitmsg/sample-history-pass.txt"));
    assert_eq!(judged.len(), 8);
    let failed: Vec<_> = judged.iter().filter(|(_, ids)| !ids.is_empty()).collect();
    assert_eq!(failed, Vec::<&(String, Vec<String>)>::new());
}

#[test]
fn the_old_trailer_form_fails() {
    let judged = judge(include_str!(
        "fixtures/commitmsg/sample-history-old-trailer.txt"
    ));
    for (hash, ids) in &judged {
        println!("{hash}: {}", ids.join("; "));
    }
    // All 6 fail. The 4 carrying Claude Code's old default trailer, without a model
    // id, fail R17/trailer.
    let failed = judged.iter().filter(|(_, ids)| !ids.is_empty()).count();
    assert_eq!((judged.len(), failed), (6, 6));
    let old_form = "R17/trailer Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>";
    let trailer: Vec<&str> = judged
        .iter()
        .filter(|(_, ids)| ids.iter().any(|i| i == old_form))
        .map(|(hash, _)| hash.as_str())
        .collect();
    assert_eq!(trailer, ["c20b001", "c20b002", "c20b003", "c20b004"]);
    let ids: Vec<&str> = judged
        .iter()
        .flat_map(|(_, ids)| ids.iter().map(|i| i.split(' ').next().unwrap_or("")))
        .collect();
    for id in ["R17/body", "R17/agent", "R17/subject", "R17/trailer"] {
        assert!(ids.contains(&id), "{id} not exercised: {ids:?}");
    }
}
