//! What the JavaScript and TypeScript tools share: which staged files are theirs, and
//! which copy of a tool runs.

use crate::adapter::{Context, JS_EXTENSIONS, Language};
use std::path::Path;

/// The files of `ctx` with a JavaScript or TypeScript extension, or a `node` first line.
pub(crate) fn js_files(ctx: &Context) -> Vec<String> {
    ctx.files
        .iter()
        .filter(|f| {
            JS_EXTENSIONS.iter().any(|ext| f.ends_with(ext)) || ctx.first_line_is(f, Language::Node)
        })
        .cloned()
        .collect()
}

/// The repo's own `node_modules/.bin/<tool>`, absolute, as its package scripts run it;
/// else `tool`, looked up on PATH. Never `npx`, which downloads a missing tool.
pub(crate) fn program(root: &Path, tool: &str) -> String {
    let local = root.join("node_modules/.bin").join(tool);
    if local.is_file() {
        local.to_string_lossy().into_owned()
    } else {
        tool.to_string()
    }
}
