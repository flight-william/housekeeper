# Setting up house

house runs the tools that check our rules, replies to agents in one format and records each run.

## Install

```sh
brew tap-new local/house
cp Formula/house.rb "$(brew --repo local/house)/Formula/"
brew install local/house/house
```

The formula builds from source with cargo and installs the Semgrep pattern files to
`$(brew --prefix)/share/house/patterns`. The `house` command is a wrapper that sets
`HOUSE_PATTERNS_DIR` to that folder.

## Tools per language

house calls these; it never judges code itself. A missing tool is reported as "cannot check"
with the install line.

| language | tools |
|---|---|
| all | `brew install gitleaks semgrep` |
| Rust | cargo fmt, clippy (with rustup), `brew install cargo-deny` |
| Swift | `brew install swiftlint` (`swift format` ships with the toolchain); `brew install swiftformat` for a repo that names `swiftformat` in `.house.toml` |
| Python | `brew install ruff` (coverage.py in the project's venv) |
| shell | `brew install shellcheck` |

Each size has one owner, so one long file is one finding. A file's length (R3, 400 lines) is
house's own counter for every language, Swift included: SwiftLint's `file_length` is not
reported, whatever `.swiftlint.yml` sets it to. Function and type length in Swift are
SwiftLint's (`function_body_length`, `type_body_length`), in Rust clippy's, in Python
house's counter.

## Start a repo

```sh
cd your-repo
house init
```

`house init` writes `.house.toml`, the protected-file list and the git hooks, and, for a repo
with Swift files and no `.swift-format`, one with the indentation those files use. Commit them.

## What the reply means

One line per finding:

```
R4 src/score.py:88 score_hand: complexity 14, limit 10. fix: split the branches into helpers. [ruff C901]
```

rule, place, what, `fix:` the action to take, then the tool's own rule id in brackets.
The first line says how many to fix and how many could not be checked.

| exit | meaning | what to do |
|---|---|---|
| 0 | nothing to fix | carry on |
| 1 | findings | fix each line, run again |
| 2 | house could not check (tool missing, crash, hang) | install the tool or report it |

`house check --json` gives the same findings as JSON. Every run adds one line to
`~/Library/Logs/house/runs/<YYYY-MM>.jsonl` (set `HOUSE_LOG_DIR` to move it).
