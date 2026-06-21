# expect: Human Expectations as Agent-Run Behavior Tests

## Problem Statement

We are generating code faster than we can trust it. An agent writes a feature,
the tests it writes itself pass, and we have no independent signal that the thing
actually does what a human wanted — or that it still does next week after the
model, the prompt, or three unrelated edits have shifted underneath it. Two
failures compound:

1. **No human-owned statement of intent.** The acceptance criteria live in the
   agent's head for the length of one session and then evaporate. There is no
   durable, human-readable, version-controlled artifact that says "this is what
   correct looks like" — written by a person, reviewable in a PR, and checkable
   forever after.
2. **Drift is the default, not the exception.** The same hosted model endpoint
   measurably changes behavior over weeks with no version bump visible to
   callers. Prompts change. Even at temperature 0, LLM output is not bitwise
   stable across runs because kernel results depend on server batch size. A
   feature that worked is not a feature that keeps working, and nothing today
   tells us when it quietly stopped.

The goal: **a human writes an expectation in plain language; an agent runs it as
a tool and renders a verdict; the system controls drift by treating every change
in that verdict as something a human must approve.** Cucumber had the right
instinct twenty years ago — Given/When/Then as a shared, human-readable contract
— but paid for it with a brittle regex-glue layer that everyone eventually
abandoned. An LLM dissolves the glue. What remains is exactly the part that was
always valuable: the contract.

## What Everyone Is Trying In This Space (Research)

This design is grounded in a survey of the current (2024–2026) landscape. The
short version: the market has split into three camps, and we should steal the
best idea from each rather than pick a side.

### The Cucumber lesson: the cost was the glue, the value was the contract

Dan North created BDD in 2006 to get business, testers, and devs onto one shared
Given/When/Then vocabulary *before* code. Gherkin binds each natural-language
step to a method via a regex/Cucumber-expression; the `Then` step holds the
assertion. The honest practitioner verdict on what happened next:

- **Vendors abandoned it.** SmartBear handed Cucumber to the Open Source
  Collective; Tricentis shut down SpecFlow entirely.
- **The glue was the pain.** Cucumber's own docs flag *Feature-Coupled Step
  Definitions* ("an explosion of step definitions, code duplication, and high
  maintenance costs"). Steps must be "written perfectly and identically every
  time."
- **The collaboration rarely materialized.** Gojko Adzic's 10-year *Specification
  by Example* retrospective: only ~12% of teams kept specs as version-controlled
  text; ~25% cut business reps out of the conversation entirely. The living
  documentation "didn't really work out as expected."
- **But the conversation/contract was the real win.** Liz Keogh's hierarchy,
  endorsed by Adzic: "having conversations is more important than capturing
  conversations is more important than automating conversations."

The synthesis every credible source converges on: **teams kept the executable
syntax and dropped the collaboration it was a vehicle for, and that is why it
hurt.** An LLM changes the economics by dissolving the step-definition glue — the
exact part practitioners hated — which makes the human-auditable contract
*cheaper to keep* and *more valuable* as a governance surface over fast AI
codegen.

Sources:
[Adzic SbE 10 years](https://gojko.net/2020/03/17/sbe-10-years.html) ·
[Automation Panda — Is BDD Dying?](https://automationpanda.com/2025/03/06/is-bdd-dying/) ·
[Cucumber anti-patterns](https://cucumber.io/docs/guides/anti-patterns/) ·
[dannorth.net — Introducing BDD](https://dannorth.net/introducing-bdd/)

### LLMs already execute Gherkin with no step definitions

The step-def layer is genuinely gone in shipping tools and peer-reviewed work:

- **TestZeus Hercules** (open source): takes `.feature` files, "Gherkin in,
  results out," no step definitions. A Planner agent interprets the
  Given/When/Then; a Browser agent executes by calling pre-built tools (it does
  not generate code). [github](https://github.com/test-zeus-ai/testzeus-hercules)
- **Momentic's thesis**: "Natural language understanding eliminates the glue
  code… LLMs can translate plain English directly into test actions without
  requiring explicit programming for each scenario." [blog](https://momentic.ai/blog/behavior-driven-development)
- **ACM A-TEST 2024**: multi-agent system executes Gherkin directly — strong on
  happy paths, **notably weaker at detecting genuine failures.** [acm](https://dl.acm.org/doi/10.1145/3678719.3685692)
- **Selector-free variants** that don't even need Gherkin keywords: Shortest
  (`shortest("user can sign up and create a $5 product")`), ZeroStep, Auto
  Playwright, Magnitude, Skyvern, Stagehand, browser-use.

Three distinct runtime mechanisms to keep separate: **tool-calling binding** (LLM
maps a step to a predefined tool — Hercules, ZeroStep), **code-gen-on-the-fly**
(LLM writes test code per scenario), and **vision/pixel grounding** (acts on
screenshots — Skyvern, Magnitude).

But Gherkin is *not* dead ceremony — it is a high-value intermediate
representation for the LLM itself. Measured: raw NL prompts produced 71%
executable / 15% pass, while Gherkin-structured prompts produced **97.8%
executable / 96.7% pass**. The Given/When/Then markers "guide generation
reliably." [arxiv 2506.06509](https://arxiv.org/html/2506.06509)

### The runtime-AI split (the central design tension)

The vendor landscape divides on **whether the AI runs at test time**:

- **Pro-runtime-agent** — QA.tech, Spur, CamelQA (real mobile devices),
  Momentic's intent-locators, Mabl's GenAI Assertions. Resilient to UI change,
  handles non-deterministic apps, judges by intent — but introduces
  non-determinism and false-positive risk *in the judge itself*.
- **Anti-runtime-agent** — Octomind's explicit "AI doesn't belong in test
  runtime," Meticulous's deterministic record/replay, Ranger's freeze-to-
  Playwright. AI authors and maintains; execution stays deterministic and
  reproducible — at the cost of literal diffing instead of semantic judgment.
- **The emerging hybrid consensus** (from Magnitude's own cost critique — $1.05
  for one product search, and "if each step has a .95 chance… after not very many
  steps you have a pretty small overall probability of success"): **author/record
  once with the LLM, replay deterministically, fall back to the LLM only on
  cache miss or failure.** Only **Stagehand** documents a cross-run resolved-
  action cache that replays without an LLM call. This is the highest-leverage
  idea in the whole survey.

### SmartBear specifically

SmartBear is not one new tool but three layers: **HaloAI** (a GenAI brand
embedded across PactFlow/TestComplete/Zephyr/ReadyAPI, May 2024); **Reflect**
(acquired 2024 — natural-language → executable web/mobile tests, the core
relevant engine); and a **2026 agentic layer** — agentic Reflect that generates
context-aware tests *inside the dev environment via a SmartBear MCP server*,
framed under **"Application Integrity": continuous assurance that software
performs as intended even as AI accelerates development cycles.** Their motivating
stat: "70% of testing/quality leaders say software quality is already declining
as AI accelerates code creation." The framing is the same problem this doc opens
with; the public materials are thin on the concrete validation mechanism.
[BusinessWire, Mar 2026](https://www.businesswire.com/news/home/20260331994897/en/SmartBear-Delivers-AI-Enhancements-Across-Entire-Software-Application-Testing-Lifecycle)

### How outcomes actually get validated

Validation methods cluster into three families, and the mature tools deliberately
**mix** them rather than trust the LLM judge alone:

1. **Deterministic assertions** — exact/regex/schema/JSON, and for agents the
   single highest-signal check: tool-call/function-call validation. Sub-ms, zero
   cost, never flaky, catches 30–60% of failures. (Promptfoo `contains`/`regex`,
   OpenAI Evals Basic templates, SWE-bench `FAIL_TO_PASS`+`PASS_TO_PASS`,
   WebArena state validators, τ-bench DB-state comparison.)
2. **Embedding tolerance bands** — cosine similarity vs a golden value, threshold
   ~0.8 (Promptfoo `similar`). Catches semantically-equivalent rewordings exact
   match would fail. The embedding model checkpoint **must be pinned**.
3. **LLM-as-judge** — only on the residual the cheap layers can't decide.
   G-Eval's chain-of-thought + form-filling; Promptfoo `llm-rubric` returning
   `{pass, score, reason}`; Mabl GenAI Assertions ("validate the outcome you
   expect" in plain English → pass/fail); DeepEval's `TaskCompletionMetric`
   reading the full execution trace.

The hard, unsolved risk that recurs everywhere: **LLMs complete patterns well but
infer intent poorly.** Documented failure — an agent accepted a 401 where 200 was
expected, "satisfied simply because the test failed." The judge "checks code
against itself, not against intent." The fix the literature keeps re-deriving:
**state intent explicitly** (don't leave it implicit in the example), **bound it
to ~3–5 key dimensions** (Adzic's "key examples"; rubric focus dilutes past 5),
and **calibrate the judge against human labels.**

Sources:
[Promptfoo model-graded](https://www.promptfoo.dev/docs/configuration/expected-outputs/model-graded/) ·
[G-Eval](https://arxiv.org/abs/2303.16634) ·
[MT-Bench / LLM-as-judge](https://arxiv.org/abs/2306.05685) ·
[Mabl GenAI Assertions](https://help.mabl.com/hc/en-us/articles/31576174565268-GenAI-Assertions) ·
[SWE-bench Verified](https://openai.com/index/introducing-swe-bench-verified/) ·
[τ-bench](https://arxiv.org/abs/2406.12045)

### The drift literature

- **Drift is real and silent.** The same "GPT-4" endpoint shifted measurably over
  three months with no visible version change. Version-pinning snapshots only
  *defers* drift (snapshots expire). [arxiv 2307.09009](https://arxiv.org/abs/2307.09009)
- **Eval-Driven Development** (Hamel Husain): missing eval systems are the common
  root cause of failed AI products. Tier by cost/cadence — L1 cheap assertions
  every change, L2 human/model eval on a cadence, L3 A/B after big changes. And:
  LLM assertions "don't necessarily need a 100% pass rate." [hamel.dev](https://hamel.dev/blog/posts/evals/)
- **Approval testing is the drift-control workflow.** ApprovalTests / Jest
  snapshots / Verify: a run writes `.received`, diffs against `.approved`, and a
  human approves by promoting received → approved. The universal lever is
  **scrubbers** that normalize volatile content (dates, GUIDs, paths) before
  comparison. Jest's CI guard: snapshots are **not** auto-written in CI — an
  un-committed snapshot must fail, not silently pass.
- **The judge drifts too, and is gameable.** On SWE-bench-Verified, 88% of
  trajectories self-verify yet 35.7% of those still fail, and ~13.8% show
  reward-hacking ("modified tests that hide the bug"). The acceptance check must
  be **tamper-resistant**: the agent can run it but not weaken it.
- **Reliability ≠ average.** τ-bench's `pass^k` (all k trials succeed) is the
  metric that matters for agents: a 90%-pass@1 agent falls to ~57% at pass^8.
  Agents *complete* tasks but not *dependably*. Repeat runs ≥2–3; use thresholds
  and tolerance bands, not exact match.

## Design: `expect`

`expect` is an operation-based MCP tool (same shape as `code_context`, `review`,
`rules`) plus a file format for human-authored expectations and a CLI surface.
The three pieces:

1. **Expectation files** — human-written Markdown, optional Given/When/Then, that
   state intent and acceptance criteria.
2. **The `expect` tool** — an agent reads an expectation, drives the system under
   test, and renders a structured verdict through a tiered ladder.
3. **The drift ledger** — golden verdicts with tolerance, committed to the repo,
   that a human must approve every change to.

### Relationship to existing tools

| Tool | Axis | Subject |
|------|------|---------|
| `rules check` | static | does the *code* follow our rules? |
| `review` | static | is this *diff* correct/clean? |
| **`expect`** | **dynamic** | does the running *system* do what a human intended, and is that still true? |

`expect` is the missing runtime/behavioral axis. It reuses the same agent
infrastructure `rules check` already uses (`ToolContext.agent_config`, the
`create_agent_from_config` path) and slots a new `AgentUseCase::Expectations`
into the use-case-based agent assignment proposed in [rule_agent.md](./rule_agent.md).

### Expectation File Format

Markdown, because it is the project's lingua franca and humans read it in PRs.
The frontmatter is deliberately thin — the **whole document is the intent**, so
there is no `intent:` key and no `id:`; the prose says what correct means and the
file's own path identifies it. Like a skill, the only required frontmatter is
`description` (plus `surface`). Given/When/Then is *available* but not *required*
— the one hard content requirement is at least one **acceptance criterion**.
Specs are named `*.expect.md` and may live **anywhere** in the tree, colocated
with the feature they describe (`src/checkout/coupon.expect.md`); repo-global
expectations that aren't tied to one source dir live under
`.expect/expectations/`. Either way, all run state and config lives in `.expect/`
(see the ledger).

```markdown
---
description: A valid coupon reduces the order total by its discount, exactly once
surface: cli            # cli | http | browser | gui | file | db
reliability: pass^3     # all of 3 runs must pass (default: pass^1)
model: qwen-coder-flash # named sah model for grading; omit to use the sah model default
tags: [checkout, pricing]
---

# A valid coupon reduces the total, exactly once

When a shopper applies a valid coupon to an order, the displayed total drops by
the coupon's discount amount, and applying the same coupon a second time does not
stack. The discount must come off the subtotal, not be a coincidence of some
other arithmetic.

## Given
- A cart with one $50 item
- A coupon `SAVE10` worth $10 off, currently valid

## When
- The shopper applies `SAVE10`
- The shopper applies `SAVE10` again

## Then
- [ ] After the first apply, the total is $40
- [ ] The UI confirms the coupon was applied
- [ ] After the second apply, the total is still $40 (no stacking)
- [ ] An error or notice explains the coupon is already applied

## Notes
The discount must come off subtotal before tax. Don't accept a $40 total that
was reached by the wrong arithmetic (e.g. a 20% rounding coincidence) — the
reason must be the coupon.
```

Design choices in the format, each tied to a research finding:

- **The document is the intent; no `intent:` field.** The prose body states what
  correct means — including the `Notes` block, which exists precisely to pin the
  intent the example alone can't (the 401-vs-200 / right-reason problem). A spec
  whose body doesn't make the intended behavior explicit is the dominant failure
  mode, and `doctor` flags a body that is all mechanics and no stated intent.
- **No `id:`; identity is the file path.** Requiring a unique id duplicates what
  the filesystem already guarantees. The expectation's path relative to the repo
  root is its identity and derives its golden file name (see the ledger), so
  renaming/moving the file is a deliberate, reviewable act.
- **`description` like a skill.** One line, used in `list` and reports and as a
  retrieval hook for `create`. It is the only required-by-convention prose field.
- **`Then` is a checklist, not prose.** Each item is one bounded criterion — the
  grading model evaluates them one at a time, binary, with evidence (the τ-bench /
  acceptance-criteria shape). Keep it to ~3–5; rubric focus dilutes past that.
- **Given/When/Then optional.** A pure intent + criteria list is valid. The
  keywords are kept because they measurably improve LLM execution (97.8% vs 71%),
  not because the ceremony is sacred.
- **`surface` declares how the agent perceives and acts.** Six concrete adapters
  (defined in the reference below) — no `custom` escape hatch. This is where
  determinism is won or lost.
- **`reliability: pass^k`** makes flakiness a first-class, declared property, not
  a surprise.

### Operations

Op-dispatched, matching the `code_context` / `review` pattern:

```
expect init               # scaffold the .expect/ tree + config; idempotent
expect create <intent>    # a coding agent authors expectation file(s) from instructions
expect doctor [scope]     # STATIC: are the expectation files well-formed? no execution
expect check [scope]      # DYNAMIC: doctor, then run against the system, compare to golden
expect list [--tag ...]   # enumerate expectations + their current golden verdict
expect status             # drift report: what changed vs golden, what's unapproved
expect approve <scope>    # promote .received → golden (the human approval gate)
expect explain <scope>    # show the last run's trajectory + evidence + reasoning
```

`expect init` is run once per repo; `expect create` is how expectations get
written; `expect check` is the one op that runs an expectation — inner loop *and*
CI gate, no separate "run" — and `expect approve` is the human-in-the-loop drift
gate.

There is no separate `run` op. "Execute the expectation" and "gate on the
expectation" were nearly the same thing, so they are one op: **`expect check`
always doctors the spec first, then runs it against the system and compares to the
golden.** It exits non-zero on a malformed spec *or* an unmet expectation *or* an
unapproved drift. (Outside CI it still prints the verdict and writes `.received`
for inspection; the only thing `CI=true` changes is that an unapproved drift
becomes a hard failure instead of a prompt to `approve`.)

**Two different things are being checked — `doctor` is the static half of
`check`.** Checking the *expectation file* and checking the *code against the
expectation* are distinct in cost, inputs, and what a failure means — but they are
not separate workflows: `check` is `doctor` plus execution. `doctor` exists on its
own only because the static half is cheap enough to run constantly (on save, in
`create`, inside `sah doctor`) without paying to drive the system.

| | `expect doctor` (the static half) | `expect check` (doctor + run) |
|---|---|---|
| Question | Is this *spec* well-formed? | Does the *code* meet this spec? |
| Reads | the `.expect.md` file only | the file **and** the running system |
| Cost | instant, no agent, no model | drives the system, may call a model |
| Runs | every save / pre-commit / in `create` / `sah doctor` | inner loop + CI gate |
| A failure means | the author wrote a bad spec | the program is wrong (or drifted) |

`expect doctor` is a pure static health check on the spec files themselves —
sah's existing diagnostic verb, applied to expectations. It parses the
frontmatter against the closed enumeration (below), requires `description` and
`surface`, requires a body that states intent and at least one criterion, rejects
unknown keys, and confirms any referenced golden resolves and the named `model:`
exists in the registry. No system is touched and no model is consulted. It
reports findings as diagnostics (`ok` / `warning` / `error` with a fix hint), the
same shape `sah doctor` already speaks.

**The tool is doctorable.** Rather than living only behind `expect doctor`, the
expectation diagnostics register into the sah doctor framework, so a plain `sah
doctor` includes "are the expectation specs valid?" alongside every other system
check. `expect doctor` is just the scoped entry point into the same diagnostic
provider. `expect check` always runs the doctor pass first and refuses to run a
malformed spec, so a CI failure is never ambiguous between "bad spec" and "bad
code."

**Scope resolution.** Every op that takes a `<scope>` (`doctor`, `check`,
`approve`) accepts the same three forms, resolved in this order:

1. **a specific expectation** — by path to one `*.expect.md` file, or by its
   repo-relative path with the extension dropped (`src/checkout/coupon`);
2. **a folder** — every `*.expect.md` under it, recursively
   (`expect check src/checkout/`);
3. **a glob** — shell-style (`expect check 'src/**/*pricing*.expect.md'`,
   or by tag via `--tag pricing`).

With no scope at all, `doctor`/`check` discover every `**/*.expect.md` in the repo
— the default CI invocation is a bare `expect check`.

#### `expect init`

Everything `expect` owns lives in a single `.expect/` dot folder at the repo root
— consistent with the rest of sah and with `.claude/` / `.github/`. The
expectation `*.expect.md` files themselves may live **anywhere** in the tree
(colocated with the feature they describe), but all machinery — config, goldens,
received runs, and repo-global expectations — is under `.expect/`. `init`
scaffolds it and is safe to re-run (it never overwrites an existing file):

```
.expect/
  config.toml            # pinned grading model, embedder, thresholds, approval policy
  README.md              # what expectations are + how to write one (links the spec)
  example.expect.md      # one worked expectation, ready to copy
  expectations/          # repo-global expectations not tied to a single source dir
  goldens/               # approved verdicts, mirroring each spec's repo-relative path
  received/              # last run per spec (gitignored)
  .gitignore             # ignores received/, keeps goldens/
```

A feature-local spec at `src/checkout/coupon.expect.md` keeps its golden at
`.expect/goldens/src/checkout/coupon.golden.json` — the golden tree mirrors the
repo-relative path of each spec, so identity needs no `id`. `init` also detects
the project's surfaces (CLI binary, HTTP server, desktop app, etc. — reusing the
`detected-projects` machinery) and writes sensible `surface` defaults into
`config.toml` so the first `expect create` has context to work from.

#### `expect create`

The authoring op, and the one a coding agent drives on your behalf. You give it
intent in plain language; it researches the system, writes one or more
`*.expect.md` files with explicit intent + bounded criteria, runs `doctor` on
what it wrote, and proposes (but does not approve) an initial golden by doing a
first `check`.

```
expect create "a valid coupon reduces the order total by its discount, once"
expect create --from-session     # turn what just happened in this session into an expectation
expect create --surface http "the /health endpoint returns 200 with {status:ok}"
```

The agent is handed the **frontmatter enumeration** (below) as its schema and the
**intent-is-mandatory / keep-criteria-bounded / state-the-right-reason** rules as
its authoring instructions, so what it produces is valid and reviewable by
construction. A human still owns the result: `create` leaves the file and a
proposed-but-unapproved golden for a person to edit for *intent* and then
`approve`. The `--from-session` flavor captures a working run as the draft
(answering "make an expectation out of what I just verified by hand").

#### Errors that teach (`create` ↔ `doctor`/`check` repair loop)

Both `doctor` and `check` exist to be *corrected against*, by a human or by an
agent running `create`. Their output is designed as a repair instruction, not a
stack trace. Every finding carries four things:

1. **What** is wrong, in one line.
2. **Where** — the file, the frontmatter key or the specific criterion, with a
   line number.
3. **Why** it's wrong — the rule it violates, named (e.g. "`surface` must be one
   of cli/http/browser/gui/file/db").
4. **A concrete fix** — the corrected value or a minimal patch, phrased so it can
   be applied verbatim.

```
✗ checkout/coupon.expect.md
  frontmatter: unknown key `surfce` (line 2)
    → did you mean `surface`? allowed: cli | http | browser | gui | file | db
  body: states intended behavior ✓
  criteria: "After the first apply, the total is $40" (line 17)
    → ok, deterministic (Tier 1)
  criteria: "the checkout feels fast" (line 20)
    → not checkable: no observable signal. state a threshold, e.g.
      "the cart page responds in under 500ms", or drop this criterion.
```

This is the load-bearing reason `doctor` is cheap and separate: an agent can write
a spec with `create`, get a precise list of what's malformed or uncheckable, and
fix it in a tight loop **without ever driving the system**. And when `check`
*runs* and a criterion fails, the same structured feedback distinguishes "the
program is wrong" from "your criterion was ambiguous/uncheckable, here's how to
sharpen it" — so a failing `check` can route back into `create` to repair the
expectation, not just the code. Vague, untestable, or right-for-the-wrong-reason
criteria are the dominant failure mode (the 401-vs-200 problem); the error
messages are the primary defense against them.

### Frontmatter Reference

The complete, closed set of frontmatter keys. Anything not listed is rejected by
the parser (so a typo fails loudly rather than being silently ignored). This table
is also the schema handed to `expect create`.

| Key | Required | Type / allowed values | Default | Meaning |
|-----|----------|-----------------------|---------|---------|
| `description` | **yes** | string (one line) | — | what this expectation is, like a skill's `description`; shown in `list`/reports, retrieval hook for `create` |
| `surface` | **yes** | `cli` \| `http` \| `browser` \| `gui` \| `file` \| `db` | — | how the agent perceives and acts on the system under test |
| `model` | no | named sah model (e.g. `qwen-coder-flash`) | `[model].default`, else sah model default | the model that **grades** criteria (Tier 3) |
| `reliability` | no | `pass^N` where N ≥ 1 (e.g. `pass^1`, `pass^3`) | `pass^1` | all N repeated runs must pass |
| `repeat` | no | integer ≥ 1 | derived from `reliability` and surface | how many times to run before judging reliability |
| `tiers` | no | subset of `[deterministic, tolerance, judgment]` | all three | which verdict-ladder tiers may decide a criterion |
| `similarity_threshold` | no | float 0.0–1.0 | `[embedder].similarity_threshold` (0.80) | Tier-2 cosine cutoff, per-expectation override |
| `timeout` | no | duration (`30s`, `5m`) | `60s` | wall-clock budget for one run |
| `tags` | no | list of kebab-case strings | `[]` | grouping for `list --tag` / glob-by-tag scope |
| `setup` | no | string or list | — | surface-specific bootstrap (e.g. the command to launch, base URL, fixture) |

Identity is the file path, not a frontmatter field: an expectation at
`src/checkout/coupon.expect.md` is addressed as `src/checkout/coupon` and its
golden lives at the mirrored path under `.expect/goldens/` (see the ledger). There
is no `id`, no `intent` (the body is the intent), and no `title` (`description` is
the label).

Closed enumerations, spelled out so authors and `create` have no ambiguity:

- **`surface`** — exactly six, each a concrete adapter with a defined perception
  and action vocabulary (no `custom` escape hatch):
  - `cli` — a command-line program. Run an argv, capture stdout, stderr, exit
    code, and files it writes; assert on those. Deterministic by construction.
  - `http` — an HTTP service or API. Issue requests; assert on status code,
    headers, and (JSON/text) body. Deterministic.
  - `browser` — a web UI running in a browser. Perceive and drive via the DOM
    **accessibility tree** (role + accessible name); assert on visible a11y state.
  - `gui` — a platform-native desktop application (macOS, Windows, Linux).
    Perceive and drive via the OS accessibility API — AX (macOS), UI Automation
    (Windows), AT-SPI (Linux) — assert on native widget state. The desktop analog
    of `browser`, not a screenshot-diff.
  - `file` — filesystem state. Assert on files/directories and their content
    after a run (the approval-testing surface). Deterministic.
  - `db` — database state. Assert on rows/tables at end of run (the τ-bench
    state-comparison surface). Deterministic.
- **`tiers`** — `deterministic` (Tier 1) · `tolerance` (Tier 2) · `judgment`
  (Tier 3). Listing a subset forbids the others; e.g. `tiers: [deterministic]`
  makes an expectation fully deterministic with no model in the loop.
- **`reliability`** — the literal form `pass^N`; `pass^1` is a single run,
  `pass^k` requires all k runs to pass (the τ-bench reliability metric).

Runtime enums (verdict/ledger, not frontmatter):

- **verdict per criterion** — `pass` \| `fail` \| `error` (could not be evaluated)
  \| `escalated` (low confidence, routed to a human).
- **ledger state per expectation** — `approved` (matches golden within tolerance)
  \| `drifted` (verdict changed, awaiting human approval) \| `new` (no golden yet)
  \| `stale` (expectation edited since its golden was approved).

### The Verdict Ladder (how outcomes are validated)

Every acceptance criterion is resolved through a cost/precision ladder. **Gate the
judge; don't lead with it.** Cheap deterministic checks run first and short-
circuit; the LLM judge only sees what the cheap layers couldn't decide.

```
criterion
   │
   ├─ Tier 1  Deterministic        exact / regex / schema / exit-code /
   │          (free, never flaky)   tool-call args / DB-state / file-state
   │              │ decided? ──────────────────────────────► verdict
   │              ▼ no
   ├─ Tier 2  Tolerance band        embedding cosine vs golden (~0.8, pinned
   │          (cheap, stable)        model) / numeric tolerance / Levenshtein
   │              │ decided? ──────────────────────────────► verdict
   │              ▼ no
   └─ Tier 3  Model judgment        rubric grade against the stated intent,
              (costly, fuzzy)        chain-of-thought + binary form-filling,
                                     by the expectation's `model:`, with evidence
```

**What's under test is the program, not a model.** `expect` validates the
behavior of the program the LLM generated — which may or may not itself call an
LLM at runtime. So the classic LLM-as-judge worries about a model grading *its
own* outputs (self-preference bias, "never let it grade its own family") mostly
don't apply: the grading model and the system under test are unrelated. Tier 3 is
still an LLM grading a rubric, but the subject is observed program output, so the
only judge concern that survives is that **the grading model can itself drift over
time** — which the ledger handles by pinning (below). Which model does the
grading is just a named sah `model`, set per-expectation via `model:` and
defaulting to the sah model default — the same model resolution `review` and
`rules` already use.

The verdict is structured, never a bare boolean — sparse pass/fail is too weak to
drive the next agent edit:

```rust
pub struct CriterionVerdict {
    pub criterion: String,
    pub tier: VerdictTier,          // which layer decided it
    pub pass: bool,
    pub score: Option<f32>,         // continuous, for tolerance bands / judge
    pub evidence: Vec<Evidence>,    // the observed output that justifies the call
    pub reason: String,             // why — especially the judge's reasoning
    pub confidence: Option<f32>,    // for the human-escalation queue
}

pub struct ExpectationVerdict {
    pub path: String,               // repo-relative path of the spec — its identity
    pub criteria: Vec<CriterionVerdict>,
    pub reliability: Reliability,   // pass^k result across repeated runs
    pub trajectory: Trajectory,     // what the agent perceived/did, for `explain`
}
```

### The Drift Ledger (controlling drift)

This is the heart of the design and the thing none of the runtime-agent vendors
do well. We borrow the **approval-testing** workflow wholesale and adapt it for
non-determinism.

- **The golden is a verdict-with-tolerance, not a golden string.** We store, per
  criterion, the approved tier, the approved pass/score, and the tolerance band —
  not a frozen blob of model output. Storing a string would guarantee false
  failures every run.
- **`expect check` compares this run's verdict to the golden.** Same pass/fail at
  each criterion within tolerance → green, silent. Any criterion that flips, or a
  score that leaves its band, → **drift**, and the run fails (in CI) and lands in
  the unapproved queue.
- **Humans approve every change.** `expect approve` promotes the received run
  (`.expect/received/…`) → golden (`.expect/goldens/…`). This is the deliberate
  analog of `jest -u` — and, like Jest, **`expect
  check` never auto-approves in CI** (`CI=true` ⇒ an unapproved drift is a hard
  failure, never a silent write). A drift is either a real regression (fix the
  code) or an intended behavior change (approve the new golden, in a reviewable
  diff).
- **Pin the grading model and embedder.** The named `model:`, the embedding
  model, and every threshold are pinned and recorded in the golden. Changing the
  grading model is treated with the same suspicion as a blind `jest -u`: a new
  model silently moves every pass/fail boundary, so it must be a deliberate,
  reviewed change that re-baselines the ledger. (Pinning the grading model is also
  the only `expect`-side defense against the grading model's *own* drift over
  time — the program under test is pinned by its own source/version.)
- **Scrub volatile content** (timestamps, UUIDs, ports, temp paths, run-specific
  ids) out of evidence before comparison — the proven approval-testing lever —
  so the ledger is stable without masking real changes.

```
src/checkout/coupon.expect.md                     # spec, colocated with the feature
.expect/
  config.toml                                     # pinned grading/embedder models + thresholds
  goldens/src/checkout/coupon.golden.json         # approved verdict-with-tolerance (committed)
  received/src/checkout/coupon.received.json       # last run (gitignored)
```

The golden and received trees mirror each spec's repo-relative path, so the spec's
location *is* its identity and moving a spec is a visible rename of its golden.

### Agent Execution Model

`expect check`, after the static `doctor` pass passes, spawns a sub-agent (the
`Expectations` use-case agent) and hands it
one expectation plus a **surface adapter** that exposes a small, fixed tool
vocabulary for that surface — the tool-calling-binding mechanism (Hercules /
ZeroStep), not code-gen, because a fixed tool set is auditable and far cheaper to
make deterministic:

- **cli** — run the command, capture stdout/stderr/exit-code (deterministic by
  construction; the easiest, highest-value surface to ship first).
- **http** — issue requests, assert on status/body/headers (deterministic).
- **browser** — drive via the **accessibility tree** (role + accessible name),
  the most refactor-robust target available; reserve pixel/vision for last
  resort. This is where Playwright MCP / Stagehand patterns plug in.
- **file / db** — assert on filesystem or end-of-run DB state (the τ-bench
  pattern: compare final state to an annotated goal).

The loop is perceive → act → observe, then judge against the criteria. Two
hardening rules from the research are non-negotiable:

1. **The acceptance check is tamper-resistant.** The agent under evaluation can
   *run* `expect` but cannot edit the expectation file or the golden ledger
   within the run — closing the 13.8%-reward-hacking hole. (Mechanically: the
   expectation + golden are read-only inputs to the run; mutations only happen
   through `expect approve`, a separate human-gated op.)
2. **Resolved actions are cached for deterministic replay** (Stagehand's model).
   The first run resolves "apply the coupon" to a concrete action sequence and
   records it; subsequent runs replay without an LLM call, re-resolving only on
   cache miss or failure. This converts fuzzy authoring into a fast, mostly-
   deterministic CI gate and answers the cost critique ($1.05/run doesn't scale
   to a CI suite run on every push).

### Reliability and Non-Determinism

- **`pass^k` is the headline metric**, not average pass rate. `reliability:
  pass^3` means all three runs must pass; the verdict reports the per-run spread
  so a 2-of-3 flake is visible, not hidden behind an average.
- **Repeat runs default to ≥2** where the surface is non-deterministic;
  deterministic surfaces (cli/http/file/db with cached actions) can run once.
- **Grading hardening**: bounded binary criteria over free-form scoring (a
  rubric grades one observable criterion at a time, not a vibe), and — when a
  criterion is borderline — an optional small **panel** of named models, where
  disagreement is itself a signal the criterion is vaguely worded. The
  self-preference/own-family concern is dropped on purpose: the subject is a
  program, not a sibling model.
- **Human escalation queue**: criteria the ladder resolves with low confidence
  are surfaced for human review rather than auto-passed, on empirically tuned
  thresholds (LLM confidence is miscalibrated, so the threshold is per-surface,
  not a constant).

## Config Schema

```toml
# .expect/config.toml
[model]
default = ""                       # named sah model for grading; empty => sah model default
panel = []                         # optional: extra named models for borderline criteria

[embedder]
model = "text-embedding-3-large"   # pinned; checkpoint matters for reproducibility
similarity_threshold = 0.80

[reliability]
default = "pass^1"
nondeterministic_surfaces = ["browser"]  # these default to >=2 repeats

[approval]
ci_autoapprove = false             # CI=true => unapproved drift is a hard failure
escalate_below_confidence = 0.6    # route to human queue

[agent]
use_case = "expectations"          # the agent that DRIVES the system (perceive/act);
                                   # resolves via AgentUseCase (see rule_agent.md).
                                   # distinct from [model], which only GRADES criteria.
```

Two model roles, kept separate: the **driving agent** (`[agent].use_case`)
perceives and acts on the system under test; the **grading model**
(`model:` / `[model].default`) only renders verdicts on criteria. They can be the
same named model or different ones — a cheap fast model often drives while a
stronger one grades, or vice versa.

## Comparison to Prior Art

| Aspect | Cucumber/Gherkin | Runtime-agent vendors (QA.tech, Hercules) | Anti-runtime (Octomind, Meticulous) | **expect** |
|--------|------------------|-------------------------------------------|-------------------------------------|------------|
| Author in | Gherkin + step defs | NL / Gherkin | NL (authoring) | NL + optional G/W/T, **intent mandatory** |
| Glue layer | hand-written regex defs | none (LLM binds) | generated code | none (tool-calling binding) |
| Runtime AI | no | yes (every step) | no (deterministic replay) | **hybrid: cached replay, LLM on miss** |
| Validation | code assertions | LLM judge | literal diff | **tiered ladder, judge gated** |
| Drift control | none built in | none | snapshot/visual diff | **approval ledger + pinned grading model + pass^k** |
| Reward-hacking guard | n/a | weak | n/a | **tamper-resistant check** |
| Human role | write specs (rarely) | review flakes | approve visual diffs | **own intent + approve every drift** |

## Phased Plan

### Phase 1 — Spec format, doctor, scaffold
1. Expectation file parser (front matter + intent body + criteria; G/W/T optional).
2. Frontmatter schema = the closed enumeration; `expect doctor` static health
   check (unknown-key rejection, `description`/`surface` present, body states
   intent, ≥1 criterion, `model` exists), **registered into the sah doctor
   framework** so plain `sah doctor` covers expectations.
3. `expect` MCP tool skeleton, op-dispatched, registered in the tool registry.
4. `expect init` — scaffold the `.expect/` tree (config + README + example +
   goldens/received/expectations dirs); surface auto-detection via
   `detected-projects`.

### Phase 2 — CLI surface, deterministic check
5. **cli** surface adapter (run command, capture stdout/stderr/exit).
6. Tier 1 deterministic verdicts only (exact/regex/schema/exit-code).
7. `expect check` (doctor pass → execute → structured `ExpectationVerdict`);
   scope resolution (path/folder/glob); teaching error messages.

### Phase 3 — The ledger and the human gate
8. Golden verdict-with-tolerance format + scrubbers.
9. `expect check` golden compare + CI-gate (no auto-approve),
   `expect approve`, `expect status`.
10. Drift detection: compare run verdict to golden, queue unapproved drift.

### Phase 4 — Authoring + semantic tiers
11. `expect create` — agent authors specs from intent (schema + rules as its
    instructions), `--from-session` capture; leaves the file + an unapproved
    golden (ledger state `new`) for a human to edit and `approve`.
12. Tier 2 embedding tolerance bands (pinned embedder).
13. Tier 3 model judgment against stated intent (named `model:`, binary
    criteria), gated behind tiers 1–2.
14. `AgentUseCase::Expectations` wired through `ToolContext`.

### Phase 5 — Non-determinism + more surfaces
15. `pass^k` reliability, repeat runs, escalation queue.
16. Resolved-action cache for deterministic replay.
17. **http** surface, then **browser** (accessibility-tree) surface.
18. Panel option for borderline criteria.

## Open Questions

1. **Where do goldens live for non-deterministic surfaces?** A verdict-with-
   tolerance is committable; a browser trajectory may not be. Probably: commit
   the verdict + criteria outcomes, gitignore the raw trajectory, keep the last
   one locally for `expect explain`.
2. **Single expectation file vs. directory of small ones.** Following the project
   convention (one fact per memory, small rules), lean toward one expectation per
   file for clean PR diffs and per-file goldens.
3. **Re-baselining on a deliberate grading-model change.** When the team
   intentionally changes the pinned `model:`, every golden's pass-boundary may
   move. Need an `expect rebaseline` that re-runs and presents the full diff for
   a single bulk human approval — explicitly, never silently.
4. **Reusing `rules` infrastructure.** `rules check` is already agent-driven over
   the same `ToolContext.agent_config`. How much of its caching (cache-by-source-
   and-rule) and progress-reporting machinery can `expect` share rather than
   reimplement?
5. **Authoring loop.** `expect record` should turn a working session into a
   draft expectation + a proposed golden, which a human then edits for *intent*
   (the part the machine can't infer) before first approval. How much can be
   auto-drafted without baking in the agent's happy-path bias (the known weakness
   at failure/edge scenarios)?
6. **Default grading model choice.** When an expectation omits `model:`, it
   falls back to the sah model default. Is that the right default for *grading*,
   or should `expect` carry its own default (the way `review` defaults to
   claude-code-haiku) so a global model change doesn't silently shift every
   ungoverned expectation's verdict?
7. **One model or two.** Is the split between a driving agent and a grading model
   worth the extra config, or should `expect` default them to the same named
   model and only let advanced users separate them?
