//! Semgrep's `--json` report, the parts house reads.

use serde::Deserialize;

/// Both lists are required: a report without them is not one Semgrep writes.
#[derive(Deserialize)]
pub(super) struct Report {
    pub(super) results: Vec<Hit>,
    pub(super) errors: Vec<Problem>,
}

#[derive(Deserialize)]
pub(super) struct Hit {
    pub(super) check_id: String,
    pub(super) path: String,
    pub(super) start: Position,
    pub(super) extra: Extra,
}

#[derive(Deserialize)]
pub(super) struct Position {
    pub(super) line: u32,
}

#[derive(Deserialize)]
pub(super) struct Extra {
    pub(super) message: String,
    #[serde(default)]
    pub(super) metadata: Metadata,
}

#[derive(Deserialize, Default)]
pub(super) struct Metadata {
    pub(super) fix: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct Problem {
    #[serde(default)]
    pub(super) message: String,
    /// `"Syntax error"`, or `["PartialParsing", [...]]`: a name, or a name first.
    #[serde(default, rename = "type")]
    pub(super) kind: serde_json::Value,
    pub(super) path: Option<String>,
}

/// Semgrep's error types for a file it could not parse (seen in 1.186.0 output:
/// "Syntax error" and "PartialParsing"; the others are its other parser failures).
const PARSE_ERRORS: [&str; 5] = [
    "Syntax error",
    "Lexical error",
    "Other syntax error",
    "PartialParsing",
    "AST builder error",
];

impl Problem {
    /// The file this error is a parse failure of; `None` for any other error.
    pub(super) fn unparsed_file(&self) -> Option<&str> {
        let name = match &self.kind {
            serde_json::Value::String(name) => Some(name.as_str()),
            serde_json::Value::Array(parts) => parts.first().and_then(|p| p.as_str()),
            _ => None,
        }?;
        PARSE_ERRORS
            .contains(&name)
            .then_some(self.path.as_deref()?)
    }
}
