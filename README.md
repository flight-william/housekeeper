# house

**house lets one developer working with agents scale without losing quality.**

## Why house

- **Quality is a property of the system, not of a person:** same rules, tests and record.
- **Feedback is fixed and immediate:** one format, a fix on every line, at every stage.
- **The rules themselves improve:** the run records show which rules catch real problems.
- **People's work moves up a level:** to what is needed, what "done" means, and the design.

More: [why house](docs/why.md) · [adopt it in an existing project](docs/adopting.md) ·
[read the weekly review](docs/review.md)

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
record. At push the tests run whenever the push changes anything, a deletion alone
included, though no file is left for the tools; a push with no change skips them. An
unknown table or key in `.house.toml` is a "cannot check" naming it.

`[house] minimum = "0.2.3"` names the oldest house the rules run on: an older house is a
"cannot check" naming both versions, and the same or a newer one runs. A house older than
0.2.3 does not know `[house]`, so it refuses the file as an unknown table.

Each tool gets the files it reads, chosen by extension. A file with no known extension is
chosen by its first line: `#!/usr/bin/env python3` goes to ruff, ruff format, r3 and Semgrep,
and `#!/bin/sh` or `#!/usr/bin/env bash` to ShellCheck, r3 and Semgrep. A zsh script, which
ShellCheck cannot read, goes to none of them, nor does a binary file.

Every `Cargo.toml` a stage sends is also read by house itself, whatever `[tools]` lists: a
lint set to `allow` (or `expect`) in a `[lints]` or `[workspace.lints]` table silences it for
the whole crate, so each such entry is an R9 finding at its line. `deny`, `forbid` and `warn`
are not: under clippy's `-D warnings` a `warn` lint is still an error.

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

An excuse covers at most `count` findings of its rule in its file (and naming its `symbol`,
when it gives one); `count` is 1 when the entry gives none. A matching finding beyond the
count is to fix, so a new suppression beside an excused one is reported, though the list
has not changed. Raising a count is the list growing.

The list may only shrink: an entry added since the base (HEAD at commit, the push base at
push), or a deadline moved later, is a finding. The first list, added where the base has
none, is adoption, not growth. Its deadlines are still capped at 56 days, and it is a
protected file, so the commit still needs the owner's approval line.

A changed path whose name matches a protected settings file only when case is ignored, such
as `.SEMGREPIGNORE` or `sub/Pyproject.toml`, is an R9 finding at commit and push, whatever
the commit message says: on a case-insensitive filesystem the tool reads it as its settings,
while CODEOWNERS matches case-sensitively and asks no owner to review it. Use the canonical
name.

## Review the records

```sh
house review --from 2026-11-01
```

`house review [--from YYYY-MM-DD] [--to YYYY-MM-DD] [--repo <path or name>] [--json]` reads
the run records (`$HOUSE_LOG_DIR`, else `~/Library/Logs/house/runs`) and prints one report
for the runs that started in the period (UTC days, as house writes each start in UTC; `--to`
is today in UTC when not given): the runs per stage; per rule and per tool, the findings,
the excused findings and the cannot-checks; each tool's total and median seconds (a tool
skipped on every run is left out); the places (rule, file, tool rule) that failed on more
than one run, most often first; and suggestions to weigh, not orders: a rule or tool that
never fires (no findings in at least 5 runs that checked it), one that looks noisy (its
findings mostly excused, or failing again on the same places), and a tool that takes most of
the time.

`--repo` picks one repository's runs, from all its folders: its main checkout and every
worktree. Since 0.3.1 each record names the repository by the root commit of its default
branch (`origin/HEAD`, else `main`, else `master`; the oldest, if it has several), whatever
branch the worktree is on, and by its main checkout, beside the folder the run was in. A
shallow clone records no root commit. `--repo <path>` (it
holds a `/` or starts with `.`) takes the root commit of the repository at that path and
counts every record with that root commit, from whichever folder; a record without one,
written before 0.3.1, counts when its folder is the path's top folder. `--repo <name>`
counts a record when its folder's last part is the name, or its main checkout's last part
is: `--repo shop` counts runs in `/work/shop` and in a worktree `/work/trees/feature-a` of
it. The report's second line then names the repository, its root commit and how many
folders its runs came from.

A line in the folder that is not a house run record, another program's or a broken one, is
skipped and counted. So is an entry named like a month's file that is not a regular file (a
folder, a FIFO, a link). A record written before 0.3.0 keeps only the run's excused total, so
those excuses are counted apart from the rules.

## On a runner

A pull request can change `.house.toml` and `.house-excuses.toml`, so on a runner the rules
come from the base: `house check --stage push --config-from <rev>` reads both files from
that revision's git objects, never the working tree, and judges the pushed content with
them. A file the revision does not hold is absent: the pull request's copy is not used in
its place. A revision with no `.house.toml` at all is "cannot check": `no rules at <rev>`,
fix: adopt the rules in their own commit first, then change code. A change never judges
itself. A revision or file that cannot be read is "cannot check" too. The pushed excuse list is still judged: growth, deadlines and expired
entries.

On a self-hosted GitHub Actions runner, house refuses to run for a public repository, or when
it cannot read the event payload at `GITHUB_EVENT_PATH` (exit 2): there anyone's pull request
code would run on the runner's machine.

A fork can edit the workflow and skip house, so the runner needs a guard outside every
workflow: `runner/job-started.sh` refuses any job from a repository that is not private.
Install it with `ACTIONS_RUNNER_HOOK_JOB_STARTED=<path to runner/job-started.sh>` in the
runner folder's `.env`, then restart the runner service.

## Licence

house is dual-licensed under MIT or Apache-2.0, at your option. See LICENSE-MIT and LICENSE-APACHE.

Contributions are accepted under the same terms.

house is made by [Mu Media](https://media-mu.com).
