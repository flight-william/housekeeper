# house

**house lets one developer working with agents scale without losing quality.**

## Why house

- **A list of rules, not a new checker:** each rule below maps to tools that already exist
  (ruff, cargo fmt, clippy and cargo-deny, SwiftLint, swift format or SwiftFormat,
  ShellCheck, Prettier, ESLint, tsc, gitleaks, Semgrep, its own size counter, commit-message
  and approval checks).
  house never re-implements a checker. The tools can change per language and per repo; the
  rule stays.
- **Running the tools is the easy part.** house also answers the harder question: can I
  trust what the agents did, and did they follow the owner's rules?
- **One reply for agents:** one line per finding, with its fix, whatever the tool.
- **The rules cannot be worn down:** the excuse list can only shrink, and its deadlines never
  move later. A change to a protected file needs an approval line quoting the owner's words.
  A pull request is judged by its base's rules, not its own.
- **Every run is recorded,** and `house review` turns the records into a weekly report.

## Two ways to use it

| Way | How | What you get |
|---|---|---|
| Loose | Install from Homebrew ([below](#install)); an agent runs `house check --stage commit` on its staged work and fixes what it says before it commits. The hooks `house init` writes help. | Feedback, not a guarantee: anything local can be skipped. |
| Sure | house is the required check on every pull request into main, under branch rules that allow no bypass ([how](docs/adopting.md#the-runner)). | Nothing reaches main that house refused. |

More: [why house, and where it fits](docs/why.md) · [adopt it in an existing project](docs/adopting.md) ·
[read the weekly review](docs/review.md)

## The rules (✓ checked by 0.2.0)

R1 ✓ Formatted · R2 ✓ Clean: lint, warnings as errors, no secret · R3 ✓ Small: file ≤ 400
lines, function ≤ 60 · R4 ✓ Simple: complexity ≤ 10 · R5 Not copied · R6 Nothing unused ·
R7 ✓ Tested · R8 ✓ Deterministic tests · R9 ✓ Nothing silenced; fix the root cause ·
R10 Fast, at every stage · R11 State machine first · R12 Good API design · R13 Modular
design · R14 Comments say why · R15 One vocabulary · R16 Standard folder layout ·
R17 ✓ Commit messages are records: `area: what changed`, then Why:, Proof:, Undo:

## Languages

| Rule | Python | Rust | Swift | Shell | TypeScript / JavaScript |
|---|---|---|---|---|---|
| R1 Formatted | ruff format | cargo fmt | swift format or SwiftFormat | – | Prettier |
| R2 Lint, warnings as errors | ruff | clippy, cargo-deny | SwiftLint | ShellCheck | ESLint |
| R2 Types | – | (the compiler, through clippy) | – | – | tsc, TypeScript files only |
| R2 No secret | gitleaks | gitleaks | gitleaks | gitleaks | gitleaks |
| R3 Small | size counter; ruff PLR0915 if the repo's ruff settings select it | clippy too_many_lines, size counter | SwiftLint, size counter | size counter | size counter (files); functions only by ESLint max-lines-per-function and max-statements if the repo's config sets them (`house init` gives the rules) |
| R4 Simple | ruff C901 if the repo's ruff settings select it | clippy too_many_arguments; cognitive_complexity –, not enabled | SwiftLint | – | ESLint complexity, max-depth, max-params and max-nested-callbacks if the repo's config sets them |
| R7 Tested | [tests] commands, any language | ← | ← | ← | ← |
| R8 Deterministic tests | Semgrep, 10 rules | Semgrep, 16 rules (8 for tests/, 8 for test modules) | Semgrep, 9 rules (partial parse of #expect/#require) | – | – |
| R9 Nothing silenced | Semgrep, 6 rules | Semgrep + Cargo.toml lints, 4 rules | Semgrep, 6 rules | Semgrep, 1 rule | – |
| R9 Protected files need the owner's approval | any language | ← | ← | ← | ← |
| R17 Commit messages | any language | ← | ← | ← | ← |

Other languages get only the language-free checks: R2 secrets, R7 tests, R9 approvals, R17.
TypeScript and JavaScript have no Semgrep rules yet, so an `// eslint-disable` or
`// @ts-ignore` comment is not reported (R8 and R9 are –); `house init` names `semgrep` in
such a repo anyway, for its other files and for the rules to come.

A commit message's `Co-Authored-By:` line is optional, its key read in any case as git reads
it (GitHub writes `Co-authored-by:`), and only in the final trailer block, the `Key: value`
lines after the last blank line, where git and GitHub read it. An agent's names its model id, one of
`[commit] models` in `.house.toml`: `Co-Authored-By: Claude (claude-opus-5-5)
<noreply@anthropic.com>`. A person's has none: `Co-Authored-By: Sam Example <sam@example.com>`.
A line naming Claude, Codex, Antigravity or Gemini without a model id is refused: the name is
compared folded (NFKC, invisible characters gone, case folded, look-alike letters read as Latin,
punctuation and spaces gone), so `ClaudeCode`, `Anthropic Claude`, `Ｃlaude` and a model id
such as `claude-opus-4-1` or `Opus 5.5` given as a name are refused too. Outside an agent's
session the rule cannot prove who typed a line: this guard stops a respelling, not a made-up
human name. A known limit: a person whose name holds an agent's (Claude Monet, Gemini
Ortiz, Claudette) is refused like the agent, and a nickname in brackets reads as an agent
line; there is no allow-list. A known
agent's line gives one of its own model ids: `claude-…` for Claude, `gpt-…` for Codex,
`gemini-…` for Antigravity and Gemini. A commit made in an agent's session owes that agent's
line, its name and one of its model ids, the same rule for every agent; another agent's line
does not count. The session is known by a variable the agent sets in its shells, so far
`CLAUDECODE=1` for Claude Code. No session link is asked for or checked. A known limit: the
session is read from the environment the agent itself controls, at its exact value, and only
by `house commit-msg`; push does not judge it again, a runner at merge judges each message's
text but not the session it was made in, and an agent with no marker listed (Codex,
Antigravity, Gemini, until theirs is verified) owes no line.

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
(prefix globs, such as `tests/fixtures/`) are never handed to a tool, and a finding a tool
reports on one anyway is dropped: gitleaks reads the git history, not the files it is handed,
so an excluded vendored file is not scanned for secrets either. The exclusion is judged
at the path a secret has now: gitleaks runs with git's rename detection off, so a secret
moved or copied out of an excluded folder (`git mv`), in a merge commit too, is reported at
its new path. It reads no `.gitattributes`, so a file marked `-diff` or `binary`, or given a
diff driver, is scanned as the text it holds. Under `[tests]`, `commit = [["cargo", "test"]]` and `push = [...]` (push falls back to commit) list test
commands, each a program and its arguments with no shell, run from the repo root after the
tools; a failing one is an R7 finding, and the last 20 lines of its output go to the run
record. Each command may run for `timeout_seconds` (in `[tests]`, 600 when unset, at most
86400). Before 0.4.0 a command could run for an hour, so a slower suite needs the key set on
upgrade. One still running then is stopped, and that is an R7 finding naming the command and
the limit, so a suite that waits on a dialog no one can click fails instead of holding the
push; the last 20 lines of what it had written go to the run record, as for a failure. Each
command runs in a process group of its own, and the whole group is stopped (SIGTERM, then
SIGKILL two seconds later), so no process in that group lives on after it; one the suite
moves to a session of its own (`start_new_session=True`) is out of house's reach. The same
stop happens when house itself ends while a suite runs, by Ctrl-C, SIGTERM, SIGHUP or even
SIGKILL, and house ends with that signal. Being in its own group, a suite cannot read the
terminal: one that asks for a password or a passphrase waits until the limit. At push the
tests run whenever the push changes anything, a deletion alone included, though no file is
left for the tools; a push with no change skips them. An unknown table or key in
`.house.toml` is a "cannot check" naming it.

`[house] minimum = "0.2.3"` names the oldest house the rules run on: an older house is a
"cannot check" naming both versions, and the same or a newer one runs. A house older than
0.2.3 does not know `[house]`, so it refuses the file as an unknown table.

Each tool gets the files it reads, chosen by extension (`.sh` and `.bash` are shell scripts
for ShellCheck, r3 and Semgrep). A file with no known extension is
chosen by its first line: `#!/usr/bin/env python3` goes to ruff, ruff format, r3 and Semgrep,
`#!/bin/sh` or `#!/usr/bin/env bash` to ShellCheck, r3 and Semgrep, and
`#!/usr/bin/env node` to Prettier, ESLint and r3 (not tsc, which reads only its project's
files). A zsh script, which
ShellCheck cannot read, goes to none of them, nor does a binary file. The files follow `--`
on each tool's command line, and a Cargo manifest or `deny.toml` path that starts with `-`
gets `./` before it, so a file named `-x.swift` or `--output=Sources/A.swift` is checked as a
file and never read as one of the tool's options (such as SwiftLint's or Semgrep's
`--output`, which would write over a tracked file).

SwiftLint gets the stage's Swift files by name, with `--force-exclude`, so a file in the
repo's `.swiftlint.yml` `excluded:` is left out, and a stage whose Swift files are all
excluded has nothing to fix. It never walks the whole tree, so files the stage did not touch
are not linted. A known limit: SwiftLint (0.65.1) skips `included:` for a file named to it,
and house reads no YAML, so a staged Swift file outside `included:` is still linted; list
such a folder under `excluded:` to keep it out. ShellCheck follows
the files a script sources, looked up from the script's own folder, so `. ./lib.sh` is not
SC1091 when `lib.sh` sits beside the script.

Swift code is formatted by Apple's swift format (`swift-format`) unless the repo names
SwiftFormat instead: `swiftformat` runs only when `[tools]` lists it, as in
`commit = ["swiftlint", "swiftformat"]`, never by default, since two formatters disagree on
one file. `house init` names it, instead of swift format, only in a repo with a `.swiftformat`
at its root, and then writes no `.swift-format`. It runs `swiftformat --lint` on the stage's Swift
files and reads the repo's own `.swiftformat`, its `--exclude` included.

JavaScript and TypeScript code is `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs` and
`.cjs`. In a repo with a `package.json` at its root, each of the three tools is the repo's
own copy, `node_modules/.bin/<tool>`, as its package scripts run it, and only that one: a
missing one is "cannot check" with the fix `npm install (add <package> to
devDependencies)`, never a copy found on PATH, whose version the repo did not choose. In a
repo without a `package.json`, the copy in `node_modules/.bin` runs when it is there, else
the one on PATH, and a missing one is "cannot check" with its `brew install` line, as for
ruff. house never runs `npx`, which downloads a tool that is missing. A repo needs, as
devDependencies, `prettier`, `eslint` and `typescript` (and `typescript-eslint`, for ESLint
to read TypeScript), and an ESLint flat config, `eslint.config.js` (or `.mjs`, `.cjs`,
`.ts`).

- `prettier`, R1: `prettier --check` on the stage's code files, by the repo's own
  Prettier settings and `.prettierignore`. Only code files: `.json`, `.css` and `.md` are
  not handed to it; list `["prettier", "--check", "."]` under `[tests]` to format those too.
  A `node` script with no extension runs on its own with `--parser babel`: Prettier infers
  no parser from `#!/usr/bin/env -S node --flags`. A file it cannot parse is left to
  ESLint and tsc, which report it.
- `eslint`, R2: `eslint --format json --max-warnings 0`, so a warning is a finding, at
  its file and line, by the repo's own config. A repo with no `eslint.config.*` is an R2
  finding, not a pass, and so is a staged file no `files` glob of the config covers:
  ESLint lints only `.js`, `.mjs` and `.cjs` unless the config says more, so a `.ts` file
  needs typescript-eslint, and a `node` script with no extension needs a glob of its own,
  such as `{ files: ["bin/*"] }`. A file the config ignores is left out. An
  `eslint-disable` comment that silences nothing is a finding
  (`eslint unused-disable-directive`). Its size and complexity rules count as R3 and R4;
  its `max-lines` is dropped when r3 runs, so a file's length has one owner.
- `tsc`, R2 Types: tsc cannot check single files against a project's settings, so it
  runs `tsc --noEmit --pretty false --listFiles -p <tsconfig>` once per project. The
  projects are the nearest `tsconfig.json` at or above each staged `.ts`, `.tsx`, `.mts`
  or `.cts` file (or a staged `tsconfig.json` itself), with the configs it references
  in its own folder (Vite's earlier root, `include` `src` and a reference to
  `tsconfig.node.json` for `vite.config.ts`). A reference to another folder is not
  followed from there, so staging one app file checks the app, not every library it
  references; the libraries' own files have their own nearest config. A solution-style
  `tsconfig.json` (`"files": []` with `references`, as Vite's react-ts template now
  writes it) checks nothing itself, so every project it references runs instead, and the
  walk goes deeper only through such configs. house reads the tsconfig files as tsc
  writes them, comments, trailing commas and a byte order mark included. An
  `incremental` or `composite` project writes a `.tsbuildinfo` file even under
  `--noEmit`, beside its tsconfig unless told where, so each run is given
  `--tsBuildInfoFile` in a temporary folder outside the repo, removed when the runs are
  done: a check leaves the working tree as it was. house does not run `tsc -b`, which
  builds the whole graph of referenced projects, in order, and writes their outputs
  unless told not to. Each error is a finding at its file and
  line. Each run lists the files it checked, and a staged TypeScript file that no run
  lists, because no project's `include` or `files` takes it in or because no
  `tsconfig.json` is above it at all, is an R2 finding: "tsc: no project includes this
  file". Limits: tsc checks the whole project, and only the errors in the stage's files
  are reported (as for clippy), so a change that breaks a caller in a file the stage did
  not touch is found by `house check --all`, not at commit. A project that references a
  `composite` one reads that project's built declarations, so before it is built tsc
  reports TS6305 ("Output file ... has not been built from source file ...") at each
  import from it: that is "cannot check" (exit 2), with the fix "build the referenced
  projects first (tsc -b)", not a finding.
- `r3`: the 400-line file limit, as for every language. house counts no JavaScript or
  TypeScript function: R3's 60-line function limit is ESLint's `max-lines-per-function`,
  and only when the repo's config sets it. `house init` says so in a JavaScript or
  TypeScript repo, and writes the rules to add into the starter `.house.toml` as a
  comment: `rules: { "max-lines-per-function": ["error", 60], "max-lines": ["error",
  400] }` (`max-lines` keeps the file limit in editors; house drops it when r3 runs).

`house init` names the JavaScript tools (prettier, eslint, tsc, with r3 and semgrep) in a
repo with a `tsconfig.json` or a tracked JavaScript or TypeScript file, not in one with
only a `package.json`, which a Python or Rust repo may hold for a tool it runs.

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
as `.SEMGREPIGNORE`, `sub/Pyproject.toml` or `.ſwiftformat` (a long s, which macOS reads as
`s`), or that the filesystem opens as a protected file under another spelling, is an R9
finding at commit and push, whatever the commit message says, and needs the owner's
approval line: on a case-insensitive filesystem the tool reads it as its settings, while
CODEOWNERS matches case-sensitively and asks no owner to review it. Use the canonical
name. House reads the excuse list only when it is spelled exactly `.house-excuses.toml`.

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

A runner checks a pull request with `house check --stage merge --config-from <base>`, the
branch's ref line on stdin as git's pre-push hook gives it
(`refs/heads/<branch> <head sha> refs/heads/<branch> <base sha>`). Merge runs what push runs,
by the same `[tools]` and `[tests]` keys (`push`, else `commit`), and then the commit-msg
hook's checks on every commit in base..head, so a commit made without the hook cannot merge
unchecked. The commits are read and judged before any tool or test runs (they run the pull
request's own code), and house's git calls ignore replace refs (`GIT_NO_REPLACE_OBJECTS=1`),
so a build script cannot rewrite the messages house reads:

- R17 and R9's `Cause:` line on each message as git stored it: a `#` line is text, not a
  comment. A finding names its commit. The author is the commit's, and the expected author
  is the base's `[commit] author` only (with `email` when it gives one), never the runner's
  `git config`, and the committer must match it too; with no `author`, no author is
  expected. The model ids are the base's
  `[commit] models`. No agent session is read: the runner's environment is not the agent's,
  so the message's text is judged and a commit owes no agent line there.
- A merge commit whose tree is the clean merge of its two parents (`git merge-tree
  --write-tree`, git 2.38 or newer) adds no work of its own, and its message is not judged:
  GitHub's "Update branch" writes such a merge, `Merge branch 'main' into <branch>` with no
  body, as whoever pressed it. A merge with changes of its own (a resolved conflict, an
  edit) or with more than two parents is judged like any commit: its `Merge ` subject needs
  no area, and it needs Why:, Proof: and Undo:, as the commit-msg hook asks of a merge.
- The approval check, commit by commit as the hook does it: each commit that changes a
  protected path, a deletion included, carries its own `Owner-approved:` line. One commit's
  line covers no other commit, an empty commit carrying only a line approves nothing, and a
  change put back later in the branch still needs its line. A merge's own paths are those
  that differ from the merge git would make. The approval finding, like the settings-case
  one, is never excused by `.house-excuses.toml`, at any stage. A changed link named as a
  folder of a protected path (`.cargo` linked to another folder) counts as that folder, at
  the commit-msg hook too.
- With a squash merge, the commit that lands on main is the message GitHub writes when the
  button is pressed, after the check ran: it is not judged. Merge or rebase to keep the
  judged commits.

Merge never passes with nothing judged. "Cannot check" (exit 2): no ref line on stdin, a
delete line, a head equal to its base, a shallow clone (its commit walk stops at the
boundary without a word: check out with `fetch-depth: 0`), and grafts (`info/grafts`).

`--stage push --config-from <base>` still works and judges no messages. In GitHub Actions
(`GITHUB_ACTIONS` set to anything), `--stage merge` without `--config-from` is refused
(exit 2); on a laptop it runs by the working tree's rules, to show what the runner will say.

Workflow steps to copy. The merge step comes first, right after checkout and the tools'
install, before any step that runs the pull request's code (a build, its tests): such a
step could move HEAD to a commit it makes. The head is the event's, never asked of the
checkout (`pull_request` checks out GitHub's merge of the pull request into its base, which
is `github.sha`), and HEAD must still be it. The house is a released one from Homebrew,
never a build of the pull request, which would judge its own change. The shas come through
`env`, never inline in the script:

```yaml
- uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
  with:
    persist-credentials: false
    fetch-depth: 0
- name: Install house and the tools it runs
  run: |
    brew tap flight-william/housekeeper https://github.com/flight-william/housekeeper
    brew install house gitleaks semgrep
- name: house check --stage merge
  if: github.event_name == 'pull_request'
  env:
    BASE: ${{ github.event.pull_request.base.sha }}
    HEAD_SHA: ${{ github.sha }}
  run: |
    test "$(git rev-parse HEAD)" = "$HEAD_SHA"
    printf 'refs/heads/pr %s refs/heads/pr %s\n' "$HEAD_SHA" "$BASE" \
      | house check --stage merge --config-from "$BASE"
# then the steps that build or test the pull request
```

The workflow file is the pull request's own copy, so a pull request can drop this step or
change `merge` to `push`. What stops that is the ruleset: code-owner review of
`.github/workflows/**` ([docs/adopting.md](docs/adopting.md#the-ruleset)), not house.

A pull request can change `.house.toml` and `.house-excuses.toml`, so on a runner the rules
come from the base: `--config-from <rev>` reads both files from that revision's git objects,
never the working tree, and judges the pushed content with them. A file the revision does not hold is absent: the pull request's copy is not used in
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

## Known limits

For a later release:

- Every file a tool checks goes on one command line, so a stage whose file names pass the
  system's argument limit (1 MB on macOS; likeliest at push, after a mass rename or a vendor
  import) is a cannot-check, "Argument list too long", for that tool. 0.4.0 ran SwiftLint
  with no paths in a repo with a `.swiftlint.yml`, so there it was not.
- A staged Swift file outside `.swiftlint.yml`'s `included:` is linted, as described under
  Start a repo; list its folder under `excluded:` to keep it out.
- That SwiftLint does not lint a file the stage did not touch is shown by its command line
  only: house drops findings on other files, so a real run could not tell the two apart.
- A test that runs a real tool (SwiftLint, SwiftFormat, ShellCheck, Semgrep, cargo-deny and
  the others) passes with a note when that tool is not installed, so on a machine without
  it the test proves nothing, and the count of passing tests does not show that.

## Licence

house is dual-licensed under MIT or Apache-2.0, at your option. See LICENSE-MIT and LICENSE-APACHE.

Contributions are accepted under the same terms.

house is made by [Mu Media](https://media-mu.com).
