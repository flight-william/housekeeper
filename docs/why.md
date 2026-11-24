# Why house

> **house lets one developer working with agents scale without losing quality.**

house is groundwork for building software with agents. It does not write code. It makes the
way code is written **consistent, measurable and testable**, and it keeps a record of every
run, so the work gets better over time instead of drifting.

**On this page:** [Key ideas](#key-ideas) · [Who it is for](#who-it-is-for) ·
[Where house fits](#where-house-fits) · [The foundation](#the-foundation) ·
[How it works](#how-it-works) · [Two ways to use it](#two-ways-to-use-it) ·
[Does quality improve over time?](#does-quality-improve-over-time) ·
[The people's role](#the-peoples-role)

---

## Key ideas

| | Idea | What it means |
|---|---|---|
| 1 | **Quality is a property of the system, not of a person** | Good code no longer depends on who wrote it. Every change meets the same written rules, the same tests and the same record. |
| 2 | **Feedback is fixed and immediate** | Every finding comes back in seconds, in one format, with its fix: at commit, at push, and before the main branch. |
| 3 | **The rules themselves improve** | Every run is recorded. The records show which rules catch real problems and which are noise, and the rules are tuned with that evidence. |
| 4 | **People's work moves up a level** | From reading every change to deciding what is needed, what "done" means, which rules apply, and how the design should go. |

---

## Who it is for

**Most of all, one developer working with agents.**

- A team has people who catch each other's mistakes. One person with agents has nobody else.
- One person cannot read everything ten agents write. house checks every change the way a
  careful colleague would, without slowing the work down.
- The developer sees only what needs their judgment: what is needed, the rules, the design.
- Their standards stay written down, in the rules and the records, not only in their head.
- Adding a project or an agent adds output, not risk.

Teams gain too, but they already have code review and process. For one developer, house is the
difference between one person's output and a team's.

---

## Where house fits

house is a thin layer. It holds a list of best-practice rules (R1 to R17) and the wiring
that maps each rule to the existing tools that check it. It never re-implements a checker.
The tools can change per language and per repo; the rule stays.

Running many tools per rule, at chosen stages, blocking or only reporting, is table stakes.
Other tools do it well. MegaLinter runs many linters across many languages, in CI or on your
machine, and can mark a linter as non-blocking. Trunk Code Quality runs third-party linters,
formatters and security scanners from one CLI, checks only new changes by default, and can
run from git hooks. Qlty runs linter plugins from its CLI, with pre-commit and pre-push
hooks, and quality gates on pull requests.

Those tools answer "is this code OK?". house answers that too, and also: **can I trust what
the agents did, and did they follow the owner's rules?**

| What house shares with other tools | What house adds for agent-written code |
|---|---|
| It runs existing linters, formatters, secret scanners and tests. | **One reply for agents,** the same for every tool: one line per finding, each with its fix. |
| One settings file in the repo picks the tools. | **Excuses can only shrink.** An entry added, or a deadline moved later, is a finding; a deadline is at most 56 days out. |
| It checks at commit, at push and on the pull request. | **The owner approves rule changes.** A protected file (the rules, the excuse list, tool settings, hooks, workflows) needs an approval line quoting the owner's words, and at the gate code-owner review sends it to the owner, so an agent cannot approve its own change. |
| It checks what changed; existing problems are set aside. | **A pull request is judged by its base's rules** (`--config-from`), so it cannot weaken the rules it is judged by. |
| Hooks can report without blocking while a repo adopts it. | **Each commit can name its agent and model.** An agent's `Co-Authored-By:` line carries its model id, from a list the repo keeps; a line naming an agent without one is refused. |
| | **Every run is recorded,** and `house review` turns the records into a weekly report: which rules catch real problems, and which are noise. |

**Planned, not shipped:** each rule set on its own: which tools check it, at which stage
(commit, push, merge, weekly), and whether it blocks or only reports. Under that, a floor no
repo can lower: secrets, approvals and commit-message rules always block at merge. Today the
tools are chosen per stage, for the whole repo, in `.house.toml`'s `[tools]`.

---

## The foundation

Each step makes the next one possible.

```mermaid
flowchart LR
    C["Consistent<br/>one way to format, name,<br/>structure and commit"] --> M["Measurable<br/>every rule is a count<br/>a tool produces"]
    M --> T["Testable<br/>every promise<br/>is a test"]
    T --> F["Feedback loop<br/>records show what catches<br/>real problems"]
    F -->|"tune the rules"| C
    F -.->|"frees attention for"| B(["Business needs<br/>what to build, and why"])
```

| Step | What it gives |
|---|---|
| **Consistent** | One way to format, name, structure and commit, across every project. Without consistency, nothing can be compared. |
| **Measurable** | Every rule is something a tool can count: lines, complexity, findings, coverage, seconds. |
| **Testable** | Every promise is a test, from what the code does to how it is designed. "Done" means proven, not "looks right". |
| **Feedback loop** | The records show which rules work. The rules are tuned, and agents get the same reply every time, so they learn what to fix. |

With that in place, attention moves to **what the business needs**: what to build, and why.
Agents handle how, and the gate keeps how from drifting.

---

## How it works

```mermaid
flowchart LR
    A["Agent writes code"] --> K["Commit<br/>house: fast rules"]
    K --> P["Push a branch<br/>house: all rules"]
    P --> R["Pull request<br/>the runner checks it<br/>with main's rules"]
    R -->|"all green"| MN[("main")]
    R -->|"anything red"| RP["One reply,<br/>a fix on every line"]
    K -->|"a finding"| RP
    P -->|"a finding"| RP
    RP --> A
    K & P & R --> L[("Run records")]
    L --> W["house review<br/>a weekly report"]
    W -->|"the owner decides<br/>rule changes"| RU["The rules"]
    RU --> K
```

- **The rules you care about,** written down once, each mapped to an existing tool: formatters,
  linters, secret scanners, test runners, and Semgrep for the patterns no tool covers.
- **One interface** to every tool, so adding a tool or a language is a small adapter.
- **One reply** for agents: one line per finding, each with its fix.
- **One record** per run, so the rules are tuned with evidence.
- **Checks at every stage:** fast rules at commit, all rules at push, and all rules again on a
  separate runner before anything reaches the main branch. Local checks give fast feedback; the
  runner is where trust is enforced.
- **Rule changes need the owner's approval,** and the list of excused old code can only shrink.

---

## Two ways to use it

| Way | How | What you get |
|---|---|---|
| **Loose** | Install from Homebrew. An agent runs `house check --stage commit` on its staged work and fixes what house says before it commits. The hooks `house init` writes help. | Feedback, not a guarantee. Anything local can be skipped: a hook can be bypassed, and an agent can forget to run it. |
| **Sure** | house is the required check on every pull request into main, run on a runner with the base's rules, under branch rules that allow no bypass. [The runner](adopting.md#the-runner) and [the ruleset](adopting.md#the-ruleset) say how. | Nothing reaches main that house refused. |

Start loose to get feedback fast. Make it sure before you rely on it.

---

## Does quality improve over time?

It can, when three things hold:

| Condition | Why it matters |
|---|---|
| **The loop acts, not only records** | The weekly report of `house review` turns repeated findings into fixes, and noisy rules into better ones. |
| **Tests carry the meaning** | The gate proves code passes its tests, not that the tests capture what the business needs. Measuring test strength (coverage, mutation testing) and reviewing tests close part of that gap; clear acceptance criteria close the rest. |
| **Design keeps a person's judgment** | Rules keep code clean, but they cannot choose the right architecture. A short design note before building, boundaries a tool can check, and design promises written as tests give design structure. The choices stay with people. |

---

## The people's role

The role moves from checking code to three things:

1. **Saying what is needed,** and how to know it works.
2. **Approving the rules** and every change to them.
3. **Reading the weekly review** of the run records ([how](review.md)), and deciding what to tighten or loosen.

> The gate stops things getting worse. Clear intent and the feedback loop make them better.

---

**Next:** [Set up a project](setup.md) · [Adopt house in an existing project](adopting.md) · [Read the weekly review](review.md) · [The rules and the tools](../README.md)

house is made by [Mu Media](https://media-mu.com).
