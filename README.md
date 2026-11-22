# house

house runs the tools that check a repo's rules at commit and at push, replies to agents in
one format, and records every run. It never judges code itself: each rule maps to the tools
that check it (ruff, cargo fmt, clippy and cargo-deny, SwiftLint and swift format, ShellCheck,
gitleaks, Semgrep, its own size counter, commit-message and approval checks).

## The rules (✓ checked by 0.2.0)

R1 ✓ Formatted · R2 ✓ Clean: lint, warnings as errors, no secret · R3 ✓ Small: file ≤ 400
lines, function ≤ 60 · R4 ✓ Simple: complexity ≤ 10 · R5 Not copied · R6 Nothing unused ·
R7 ✓ Tested · R8 ✓ Deterministic tests · R9 ✓ Nothing silenced; fix the root cause ·
R10 Fast, at every stage · R11 State machine first · R12 Good API design · R13 Modular
design · R14 Comments say why · R15 One vocabulary · R16 Standard folder layout ·
R17 ✓ Commit messages are records: `area: what changed`, then Why:, Proof:, Undo:

## Install

```sh
brew tap flight-william/housekeeper https://github.com/flight-william/housekeeper
brew install house
```

The tools themselves come from Homebrew too; [docs/setup.md](docs/setup.md) lists them.

## Start a repo

```sh
cd your-repo
house init
```

`house init` writes `.house.toml` (the tools per stage), CODEOWNERS for the protected files
and the git hooks. Commit them. Files listed under `[paths] exclude` in `.house.toml`
(prefix globs, such as `tests/fixtures/`) are never handed to a tool. Under `[tests]`,
`commit = [["cargo", "test"]]` and `push = [...]` (push falls back to commit) list test
commands, each a program and its arguments with no shell, run from the repo root after the
tools; a failing one is an R7 finding, and the last 20 lines of its output go to the run
record. An unknown table or key in `.house.toml` is a "cannot check" naming it.

## The reply

```
house commit: 1 to fix (2.1 s)
R4 src/score.py:88: complexity 14, limit 10. fix: split the branches into helpers. [ruff C901]
```

One line per finding: rule, place, what, `fix:` the action, then the tool's own rule id.
Exit 0 is nothing to fix, 1 is findings, 2 is "cannot check" (a tool missing or failing).
`house check --json` gives the same as JSON; every run appends one line to the run record.
`house check --all` runs the push stage's tools and tests on every tracked file, not only
the changed ones, and is the only run that reports a stale excuse (an entry in
`.house-excuses.toml` that matches no finding).

An excuse may record `added = "YYYY-MM-DD"`, the day it was written: its deadline may be at
most 56 days after that day (after today for a new entry).

## Licence

UNLICENSED (as in Cargo.toml): no licence is granted yet.
