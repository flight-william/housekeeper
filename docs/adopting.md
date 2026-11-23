# Adopting house in an existing project

How to move a project that already has its own checks onto house, without a gap in
protection and without a day when nothing is enforced.

**On this page:** [Who it is for](#who-it-is-for) · [The path](#the-path) ·
[The steps](#the-steps) · [The runner](#the-runner) · [The ruleset](#the-ruleset) ·
[The proof cases](#the-proof-cases) · [What can go wrong](#what-can-go-wrong)

---

## Who it is for

A project that already has code, history and some checks (a script, a CI job, a few hooks),
and an owner who wants one set of written rules in their place. The old code will not meet
every rule on day one, so house keeps a dated list of excuses for it: the list may only
shrink, and every entry has a deadline.

For a project with no checks yet, [setup](setup.md) is enough.

---

## The path

```mermaid
flowchart TD
    I["Install<br/>brew tap, brew install"] --> N["house init --owner @you<br/>hooks, starter .house.toml,<br/>CODEOWNERS"]
    N --> E["Excuse list for existing code<br/>deadlines within 56 days, counts"]
    E --> RO["Report-only, beside the old checks<br/>until every difference is explained"]
    RO --> S1["STEP 1: adopt the rules<br/>.house.toml, .house-excuses.toml,<br/>CODEOWNERS in their own commit"]
    S1 --> S2["STEP 2: switch<br/>hooks and CI call house,<br/>old checks removed, by pull request"]
    S2 --> RN["The runner<br/>self-hosted if private,<br/>GitHub-hosted if public"]
    RN --> RS["The ruleset<br/>pull requests only, the required check,<br/>code-owner review, no bypass"]
    RS --> PC["The proof cases"]
```

---

## The steps

### 1. Install

```sh
brew tap flight-william/housekeeper https://github.com/flight-william/housekeeper
brew install house
```

The tools house calls come from Homebrew too; [setup](setup.md) lists them.

### 2. Run `house init`

```sh
cd your-repo
house init --owner @your-github-user
```

It writes three things and prints each: the git hooks (report-only: they show findings but
never block), a starter `.house.toml` naming the tools the repo's files need, and
`.github/CODEOWNERS`, which gives the owner the protected files. An existing hook is left
alone, and house prints the line to add to it. Nothing is committed yet.

### 3. Write the excuse list for existing code

Run `house check --all`: it runs every tool on every tracked file, not only the changed
ones. What it finds in old code goes into `.house-excuses.toml`, one `[[excuse]]` each, with
the rule, the file, a `deadline`, a `reason` and, to bound it, a `count`. Set `added` to
today, and every deadline within 56 days of it. An excuse covers at most `count` findings, so
new code in an excused file is not excused. A later `house check --all` reports an entry that
matches nothing; delete it.

When one finding fits two entries (the same rule and file, and both symbols in its text), it
goes to the most specific one: the longer `symbol`, then any `symbol` over none, then a
`value` over none. A wider entry never takes a finding a narrower one was written for, and
the order of the entries in the file does not matter. So an entry for one kind of finding
(`symbol = "LineLength"`) and an entry for the rest of the same tool's findings
(`symbol = "format"`) can sit side by side.

### 4. Report-only, beside the old checks

For a while, run both: the old checks and house's report-only hooks. Compare what each one
says. For every difference, decide which is right, then fix the rule, the excuse or the old
check. Move on only when every difference is explained.

### 5. STEP 1: adopt the rules

Put `.house.toml`, `.house-excuses.toml` and `.github/CODEOWNERS` in a commit of their own,
with nothing else in it, and merge it. This comes first for a reason: on the runner, house
reads its rules from the **base** of a pull request, never from the pull request, so a change
cannot judge itself. A base with no `.house.toml` is "cannot check: no rules", and
`house check --stage push --config-from <base>` refuses. The first list of excuses is not
growth, but it is a protected file, so the commit needs the owner's approval line.

### 6. STEP 2: switch

In a pull request, make the hooks and CI call house and remove the old checks. The starter
hooks end in `|| true`; take that off where house should block. The pull request is judged
with the rules already on the base from step 1.

---

## The runner

CI runs `house check --stage push --config-from <base>` on a pull request, so the rules come
from the base, and `house check --all` on the main branch.

| Repository | Runner | Why |
|---|---|---|
| Private | Self-hosted, with the job-started hook | Your own machine, your own tool versions. |
| Public | GitHub-hosted only | Anyone can open a pull request from a fork; on a self-hosted runner that code would run on your machine. |

**Never a self-hosted runner on a public repository.** house itself refuses to run there
(exit 2), and it also refuses when it cannot read the event payload.

A fork can edit the workflow and skip house, so a self-hosted runner needs a guard outside
every workflow. `runner/job-started.sh` refuses any job from a repository that is not
private. Install it in the runner folder's `.env`, then restart the runner service:

```sh
ACTIONS_RUNNER_HOOK_JOB_STARTED=<path to runner/job-started.sh>
```

---

## The ruleset

In the repository settings, add a ruleset for the main branch. Each line closes a way round
house:

| Setting | What it stops |
|---|---|
| Pull requests only | A direct push to main. |
| The house check is required | A merge while anything is red. |
| Code-owner review is required | A change to a protected file without the owner. |
| Stale approvals are dismissed on a new push | A change slipped in after the owner approved. |
| No bypass, for anyone | The owner skipping their own rules by habit. |

---

## The proof cases

Do not trust the settings; try to break them, with a throwaway branch each time.

```mermaid
flowchart LR
    A["Push straight to main"] --> A2["Refused by the ruleset"]
    B["Pull request that changes<br/>a protected file"] --> B2["Waits for the owner's approval"]
    C["Push to that pull request<br/>after approval"] --> C2["Approval dismissed,<br/>review needed again"]
```

All three must hold before the old checks are gone for good.

---

## What can go wrong

- **A second `house` on PATH shadows the old one.** An older copy earlier on PATH runs
  instead of the one you installed, and may not know the newer rules or keys. Check
  `which -a house` and `house --version`, and set `[house] minimum` in `.house.toml`.
- **A commit made during a push.** A commit made on the branch while a push of it is running
  can change what the push checks and sends. Wait for the push to finish.
- **The base has no rules.** Step 2 before step 1: the runner finds no `.house.toml` on the
  base and cannot check. Adopt the rules in their own commit first.

---

**Next:** [Read the weekly review](review.md) · [Why house](why.md) · [Set up a project](setup.md)
