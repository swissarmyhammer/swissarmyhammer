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
2. **The `expect` tool** — provisions the system under test, **observes** an
   authoritative outcome, **evaluates** it against the criteria through a tiered
   ladder, and compares the result to a golden.
3. **The drift ledger** — an approved *observation* per expectation, committed to
   the repo, that a human must approve every change to.

Three verbs do the work, and keeping them separate is the spine of the design:
**`observe`** produces an authoritative observation of the running system,
**`evaluate`** is a pure function `(observation, criteria) → verdict`, and
**`approve`** promotes an observation to the golden. `check` is just
`doctor + observe + evaluate + compare-to-golden`.

### Relationship to existing tools

| Tool | Axis | Subject |
|------|------|---------|
| `rules check` | static | does the *code* follow our rules? |
| `review` | static | is this *diff* correct/clean? |
| **`expect`** | **dynamic** | does the running *system* do what a human intended, and is that still true? |

`expect` is the missing runtime/behavioral axis, and architecturally it is the
closest sibling of `review`: both fan a scoped task out to a delegated agent over
ACP, capture structured output, and verify it before recording a verdict. `expect`
should reuse `review`'s ACP machinery wholesale rather than re-derive it (see
[Delegation over ACP](#delegation-over-acp-dont-rebuild-the-agent)), and slots a
new `AgentUseCase::Expectations` into the use-case-based agent assignment proposed
in [rule_agent.md](./rule_agent.md).

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
- A freshly created cart with one $50 item (arranged per run, so `pass^3` stays independent)
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
- **State invariants, not scenarios.** A good `Then` says how the system *should be*
  in domain language — "every column header's count equals its cards," "a task is in
  exactly one column" — not a scripted example with incidental data ("card *X*,
  count 2→3"). Invariants catch a *class* of failures and don't drift on incidental
  data; the authoring skill pushes for them. (See *How `evaluate` turns prose into a
  check* for why invariant assertions beat frozen literals.)

### Operations

Op-dispatched, and — like every sah CLI built on the shared `Operation` trait —
the user-facing grammar is **noun-first: `expect <noun> <verb>`**. The generator
(`swissarmyhammer-operations::cli_gen`) builds a `noun → verb → args` tree, exactly
as kanban does — `kanban board init`, `kanban task add`, `kanban tasks list`. (The
`verb noun` string `add task` is only the internal MCP op id; the command you type
is noun-first.) **The noun's number follows cardinality**: singular for one
(`expectation check`), plural for the collection or a batch (`expectations list`,
`expectations check`).

**Nouns** (four):

- **expectation** — the `*.expect.md` spec (frontmatter + intent + criteria).
- **observation** — one authoritative capture of a run: a timeline of checkpoints
  (one per `When` step + a final) plus the driver trajectory (the `received`).
  Produced by `expectation observe`, addressed by its expectation's path.
- **golden** — an approved, scrubbed observation; the committed baseline. Produced
  by `observation approve`.
- **surface** — the adapter catalog (cli/http/browser/gui/file/db), read-only.

The *verdict* is derived by `evaluate`, never stored, and is not a command noun.

**Domain grid** — read each cell left-to-right as `<noun> <verb>`:

| noun ↓ \ verb → | create | get | list | delete | observe | check | evaluate | approve |
|---|:-:|:-:|:-:|:-:|:-:|:-:|:-:|:-:|
| **expectation**  | ✓ | ✓ |   | ✓ | ✓ | ✓ |   |   |
| **expectations** |   |   | ✓ |   | ✓ | ✓ |   |   |
| **observation**  |   | ✓ |   | ✓ |   |   | ✓ | ✓ |
| **observations** |   |   | ✓ |   |   |   | ✓ | ✓ |
| **golden**       |   | ✓ |   | ✓ |   |   | ✓ |   |
| **goldens**      |   |   | ✓ |   |   |   | ✓ |   |
| **surface**      |   | ✓ |   |   |   |   |   |   |
| **surfaces**     |   |   | ✓ |   |   |   |   |   |

`expectation observe`/`expectation check` *produce* observations; `observation
approve` *produces* a golden; `observation evaluate` / `golden evaluate` re-judge a
stored observation or re-grade the baseline (no re-run). `list` is plural-always;
`get`/`create`/`delete` are single-item; `observe`/`check`/`evaluate`/`approve`
take singular or plural.

**Trait verbs** — the two special cases. They are *not* `<noun> <verb>` ops; they
are top-level (like kanban-cli's hand-written `doctor`), nounless, and roll up to
the matching `sah` command:

| command | trait | rolls up to |
|---|---|---|
| `expect init` | `Initializable` | `sah init` |
| `expect doctor [scope]` | `Doctorable` | `sah doctor` |

**Example flows** (`<noun> <verb>` throughout; `init`/`doctor` are the top-level
exceptions):

```
# set up, capture from a conversation, baseline it
expect init                                       # trait verb, top-level
expect expectation create --from-chat             # drafts src/checkout/coupon.expect.md (doctor'd, `new`)
expect expectation observe src/checkout/coupon    # provision SUT, drive, capture the observation
expect observation approve src/checkout/coupon    # promote received → golden, commit

# inner dev loop
expect expectation check src/checkout/coupon      # doctor + observe + evaluate + compare → pass / fail / drift

# a drift, triaged and accepted
expect expectation check src/checkout/coupon      # → drifted
expect observation get src/checkout/coupon        # what happened (checkpoint timeline + trajectory)
expect observation evaluate src/checkout/coupon   # the reasoned verdict (why)
expect observation approve src/checkout/coupon    # intended change → accept the new golden

# edited a criterion — re-grade without re-running
expect golden evaluate src/checkout/coupon        # re-judge the approved observation vs new criteria

# CI gate, survey, retire
expect expectations check                         # all specs; strict; non-zero on bad spec / unmet / drift
expect expectations list                          # all specs + ledger state (new/approved/drifted/stale)
expect doctor                                     # whole-suite static health (also via sah doctor)
expect expectation delete src/checkout/coupon     # remove spec + its observation + golden
```

**`check` decomposes into three separable verbs.** `observe` produces an
authoritative observation of the running system (and stores it as `received`);
`evaluate` is a *pure* function over that observation — `(observation, criteria)
→ verdict` — touching no system and re-runnable for free; the compare then holds
the verdict against the golden. They are separate ops because each is
independently useful: `observe` alone records a candidate baseline for `approve`;
`evaluate` alone re-judges a stored observation against edited criteria or a
changed `model:` *without re-running the system*. In CI, a bare `expect expectations check`
exits non-zero on a malformed spec *or* an unmet expectation *or* an unapproved
drift.

**Two different things are being checked — `doctor` is the static half of
`check`.** Checking the *expectation file* and checking the *code against the
expectation* are distinct in cost, inputs, and what a failure means — but they are
not separate workflows: `check` is `doctor` plus execution. `doctor` exists on its
own only because the static half is cheap enough to run constantly (on save, in
`create`, inside `sah doctor`) without paying to drive the system.

| | `expect doctor` (the static half) | `expect expectation check` (doctor + observe + evaluate + compare) |
|---|---|---|
| Question | Is this *spec* well-formed? | Does the *code* meet this spec? |
| Reads | the `.expect.md` file only | the file **and** the running system |
| Cost | instant, no agent, no model | drives the system, may call a model |
| Runs | every save / pre-commit / in `create` / `sah doctor` | inner loop + CI gate |
| A failure means | the author wrote a bad spec | the program is wrong (or drifted) |

`expect doctor` is a pure static health check on the spec files themselves —
sah's existing diagnostic verb, applied to expectations. It parses the
frontmatter against the closed enumeration (below), requires `description` and
`surface`, requires a body that states intent and at least one criterion, and
rejects unknown keys. Crucially it validates **dynamic** fields against live
reality, not just a static schema: `model:` must name a model in the **current**
sah registry, `setup:` must reference things that exist. No system is driven and
no model is consulted. It returns **structured, per-field** diagnostics — for each
field `{status, message, allowed?, suggestion?}` — next to the human rendering, so
an authoring agent can patch exactly the red fields (the same `ok`/`warning`/
`error`-with-fix-hint shape `sah doctor` speaks). A pinned `model:` that has gone
missing is a **warning, not an error**: doctor flags it and grading falls back to
the default — safe, because if the fallback model grades the approved observation
differently, the golden compare catches it as drift.

**The tool is doctorable.** Rather than living only behind `expect doctor`, the
expectation diagnostics register into the sah doctor framework, so a plain `sah
doctor` includes "are the expectation specs valid?" alongside every other system
check. `expect doctor` is just the scoped entry point into the same diagnostic
provider. `expect expectations check` always runs the doctor pass first and refuses to run a
malformed spec, so a CI failure is never ambiguous between "bad spec" and "bad
code."

**Scope resolution.** Every op that takes a `<scope>` (`doctor`, `observe`,
`evaluate`, `check`, `approve`) accepts the same three forms, resolved in this
order:

1. **a specific expectation** — by path to one `*.expect.md` file, or by its
   repo-relative path with the extension dropped (`src/checkout/coupon`);
2. **a folder** — every `*.expect.md` under it, recursively
   (`expect expectations check src/checkout/`);
3. **a glob** — shell-style (`expect expectations check 'src/**/*pricing*.expect.md'`,
   or by tag via `--tag pricing`).

With no scope at all, `doctor`/`check` discover every `**/*.expect.md` in the repo
— the default CI invocation is a bare `expect expectations check`.

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
  goldens/               # approved scrubbed observations, mirroring each spec's repo-relative path
  received/              # last run per spec (gitignored)
  .gitignore             # ignores received/, keeps goldens/
```

A feature-local spec at `src/checkout/coupon.expect.md` keeps its golden at
`.expect/goldens/src/checkout/coupon.golden.json` — the golden tree mirrors the
repo-relative path of each spec, so identity needs no `id`. `init` also detects
the project's surfaces (CLI binary, HTTP server, desktop app, etc. — reusing the
`detected-projects` machinery) and writes sensible `surface` defaults into
`config.toml` so the first `expect expectation create` has context to work from.

#### `expect expectation create`

The authoring op, and the one a coding agent drives on your behalf — because the
most valuable expectations are captured *from intent at the moment it's
expressed*, not hand-written later. `create` is **context-hungry**: it reads
whatever intent-bearing artifact it is pointed at, drafts one or more
`*.expect.md` files with explicit intent + bounded criteria, loops them through
`doctor` until every field is green, records a candidate observation, and leaves
the result **unapproved** for a human to confirm.

```
expect expectation create "a valid coupon reduces the order total by its discount, once"
expect expectation create --from-chat        # mine the conversation for stated acceptance criteria
expect expectation create --from-task <id>   # draft from a kanban task's acceptance criteria (seed)
expect expectation create --from-spec <path> # mine a design doc / PRD for should/must/example
expect expectation create --from-session     # turn a hand-verified run into an expectation
```

Sources, all feeding one draft → doctor → confirm pipeline:

- **chat** (the default in an interactive session) — the authoring **skill**
  watches the conversation and **proactively offers** to capture
  acceptance-criteria-shaped statements ("the coupon should only apply once").
  This behavior lives in the skill/agent layer, not the tool: recognizing intent
  mid-conversation is the agent's job; `create` is what it calls once you accept.
- **task** — a kanban task's description usually *is* acceptance criteria. The
  draft links back to the task as **provenance only** (a tag/comment); the
  expectation then stands on its own and is not coupled to the task lifecycle.
- **spec** / **session** — mine an existing design doc, or capture a hand-verified
  run.

The agent is handed the **resolved** frontmatter schema — the closed enums *plus*
the live values for dynamic fields like the available `model:` set — and the
**intent-is-mandatory / keep-criteria-bounded / state-the-right-reason** rules as
its authoring instructions. Because every draft round-trips through `doctor`'s
structured per-field diagnostics, the agent can't emit an invalid spec: it patches
exactly the red fields and re-checks. A human still owns the result — `create`
leaves the file and an unapproved candidate observation (ledger state `new`) for a
person to edit for *intent* and then `approve`.

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
  frontmatter: model `qwen-flash` (line 4)
    → not an available model. available now: claude-sonnet-4-6, qwen-coder-flash,
      claude-haiku-4-5. suggestion: qwen-coder-flash
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
is also the schema handed to `expect expectation create`.

| Key | Required | Type / allowed values | Default | Meaning |
|-----|----------|-----------------------|---------|---------|
| `description` | **yes** | string (one line) | — | what this expectation is, like a skill's `description`; shown in `list`/reports, retrieval hook for `create` |
| `surface` | **yes** | `cli` \| `http` \| `browser` \| `gui` \| `file` \| `db` | — | how the agent perceives and acts on the system under test |
| `model` | no | named sah model, **validated against the live registry** | `[model].default`, else sah model default | the model that **grades** criteria (Tier 3); missing ⇒ doctor warns + falls back |
| `reliability` | no | `pass^N` where N ≥ 1 (e.g. `pass^1`, `pass^3`) | `pass^1` | all N repeated runs must pass |
| `repeat` | no | integer ≥ 1 | derived from `reliability` and surface | how many times to run before judging reliability |
| `tiers` | no | subset of `[deterministic, tolerance, judgment]` | all three | which verdict-ladder tiers may decide a criterion |
| `similarity_threshold` | no | float 0.0–1.0 | `[embedder].similarity_threshold` (0.80) | Tier-2 cosine cutoff, per-expectation override |
| `timeout` | no | duration (`30s`, `5m`) | `60s` | wall-clock budget for one run |
| `tags` | no | list of kebab-case strings | `[]` | grouping for `list --tag` / glob-by-tag scope |
| `setup` | no | string or list | — | **provisioning** declaration for the surface — how `expect` builds/launches the SUT and arranges fixtures (and tears down) |
| `isolation` | no | `shared` \| `fresh` | `shared` | `fresh` gives this expectation its own provision instead of the shared per-check instance |

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
- **`isolation`** — `shared` (default; run against the one instance provisioned
  per `check`) or `fresh` (provision a dedicated instance for this expectation when
  it needs a pristine SUT).

**Static vs dynamic validation.** `surface`, `tiers`, `reliability`, and
`isolation` are *static* closed enums. `model` and `setup` are *dynamic* — `doctor`
checks them against the live registry / the surface adapter at author time, which
is why authoring must round-trip through `doctor`, not a frozen schema.

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

`observe` produces an `Observation` (the authoritative capture); `evaluate` is the
pure function `(Observation, &[Criterion]) -> ExpectationVerdict`. The observation
is a **timeline of checkpoints**, not a single final snapshot — the adapter captures
state (and timing) after *each* `When` step, because real criteria are multi-step
("after the *first* apply… after the *second*…"), relational ("drops by the
discount"), and temporal ("under 500ms"). A locator addresses a checkpoint. The
verdict is structured, never a bare boolean — sparse pass/fail is too weak to drive
the next agent edit:

```rust
pub struct Observation {
    pub path: String,                 // repo-relative path of the spec — its identity
    pub checkpoints: Vec<Checkpoint>, // one per When step + a final — the authoritative timeline
    pub trajectory: Trajectory,       // what the driver did, for `observation get` — never the verdict source
}

pub struct Checkpoint {
    pub after: String,                // the When step this snapshot follows (or "final")
    pub state: SurfaceState,          // adapter's authoritative read: a11y tree / json body / db rows / stdout
    pub duration: Duration,           // for temporal criteria
}

// evaluate is pure and re-runnable: no system touched.
pub fn evaluate(obs: &Observation, criteria: &[Criterion]) -> ExpectationVerdict;

pub struct CriterionVerdict {
    pub criterion: String,
    pub tier: VerdictTier,          // which layer decided it
    pub pass: bool,
    pub score: Option<f32>,         // continuous, for tolerance bands / judge
    pub evidence: Vec<Evidence>,    // the slice of the observation that justifies the call
    pub reason: String,             // why — especially the judge's reasoning
    pub confidence: Option<f32>,    // for the human-escalation queue
}

pub struct ExpectationVerdict {
    pub path: String,
    pub criteria: Vec<CriterionVerdict>,
    pub reliability: Reliability,   // pass^k result across repeated observations
}
```

### How `evaluate` turns prose into a check

`evaluate` doesn't re-interpret each `Then` line every run. It works off a
**compiled assertion** per criterion — and how that compilation works is where the
"natural language, no step definitions" promise is kept rather than hand-waved.

**1. Compile the criterion into a typed assertion.** A `Then` line binds to a
checkpoint, a **locator** (where in that checkpoint's state the value lives), an
operator, and an expected — or, for a judgment, a rubric + an anchor:

```
"after the first apply, the total is $40"
  → { checkpoint: 1, locate: $.total, op: equals, expected: 40.00 }            (Tier 1)
"an error explains the coupon is already applied"
  → { checkpoint: 2, locate: $.message, op: judge,
      rubric: "conveys already-applied", anchor: <approved text>, sim: 0.85 }  (Tier 3)
```

**The *kind* of assertion that compiles sets the tier** — locator + exact/regex/
numeric → Tier 1; numeric or semantic tolerance → Tier 2; rubric + anchor → Tier 3.
The author never picks a tier; the cheapest faithful one wins.

**2. Locators are a per-surface dialect, ranked by robustness.** A locator resolves
a path into a checkpoint's state; each surface has its own:

| surface | locator | robustness |
|---|---|---|
| cli | stream regex-capture / json-path if JSON / `exit` | regex brittle, json-path stable |
| http | `status` / `header:<name>` / json-path | json-path stable |
| db | a SQL query + projection | very stable (the locator *is* SQL) |
| file | path + content (+ sub-locator if structured) | stable |
| browser / gui | `role[name=…]` + tree relationship (`within` / `ancestor`) | a11y-stable; pixel/offset brittle |

The compiler prefers the most durable locator that captures the value (json-path
over text-regex, role+name over DOM position). A locator that **stops binding** (the
`Total:` line moved, the column was renamed) is itself **structural drift** —
surfaced loudly, never a silent mis-read.

**3. Compilation needs a real observation, so it freezes into the golden.** You
can't write `$.total` without seeing the output's shape — so compilation happens at
`observation approve`, bound against the approved observation, and the compiled
assertion set is **frozen into the golden** alongside it. `evaluate` over a later
observation **replays the frozen assertions** (no recompile) — apples-to-apples,
mostly deterministic. A freshly compiled assertion must **bind and pass against the
very observation it was compiled from**, or it's rejected as a hallucinated locator
before it ever reaches the approve diff — compilation is self-verifying.

**4. The compiled assertion is reviewable and hand-editable — but prose-bound.** The
`observation approve` diff shows the binding ("$40 ← `$.total`"), not just the value,
so a mis-compiled locator is caught at review. A reviewer can hand-edit a tricky
locator. The guardrail that keeps this from becoming Cucumber step-definition glue:
a hand-edit is **bound to the criterion's prose** — change the criterion text and
the edit is discarded, recompiled, and re-reviewed. An assertion can never silently
check something the prose no longer says.

**5. Prefer invariants over frozen literals.** A criterion compiles to one of two
deterministic flavors:

- **literal-match** — `$.total equals 40` — freezes a specific value (example-style).
- **invariant-holds** — *for each* column: `header_count == count(cards)` — freezes
  a *relationship*; the expected is derived from the observation each run.

Invariants are how you say "this is how things should be," and they're strictly
better where they exist: they catch a *class* of failures (a count that lies on
*any* board, not the one scenario you scripted), and they **don't drift on incidental
data** (different tasks, different totals next month → still green, no re-approval
noise). The authoring skill should push for invariants in the system's domain
language; frozen literals are the fallback when there genuinely is only a specific
expected value. This is the existential `Given`/`When`/`Then` — `Given` the essential
precondition, `When` the essential action, `Then` the invariant that must then hold,
not a fixture with a name and a magic number.

**6. Tier 3 is the residual-of-the-residual.** A judgment criterion does *not* call
the model every run. At `evaluate`: locate the evidence → first take **embedding
similarity to the anchor** (the approved text). ≥ threshold → it's essentially the
approved evidence → **pass, no model call**. Only on *divergence* does the judge wake:
"does this *new* evidence still satisfy the rubric?" Yes → passes the rubric but the
evidence changed → **drift**, surfaced for re-approval. No → fail. A stable message
never touches the model; a changed one touches it once and shows as drift.

**"Pure" means no SUT — not uniformly deterministic.** `evaluate` touches no system
and is re-runnable, but determinism is *per-criterion*: Tier 1/2 frozen assertions
run with no model (deterministic); a Tier 3 criterion that diverged calls the model
live (fuzzy — hence `pass^k` / panel for those lines). A spec whose `Then` items all
compile to Tier 1 / invariants is fully deterministic.

### The Drift Ledger (controlling drift)

This is the heart of the design, and we model it directly on **snapshot UI
testing** (Jest snapshots, Playwright `toHaveScreenshot`, Chromatic, approval
tests), adapted for non-determinism.

- **The golden is an approved *observation*, not a frozen verdict.** `approve`
  stores the full, scrubbed observation a human signed off on. The verdict is never
  the stored source of truth — it is re-derived by `evaluate` on both sides, so
  compare is `evaluate(received)` vs `evaluate(golden)`, per criterion. Storing the
  observation keeps the baseline **re-evaluable**: change a criterion or the
  grading `model:` and you can re-judge the approved observation without re-running
  the system.
- **The criteria are what a snapshot lacks — so we don't freeze raw output.** A
  pure snapshot test has no human assertions, so the *entire* output is the
  assertion (which is why broad snapshots become undiffable). Here the human wrote
  the `Then` checklist, so the criteria pre-declare *which aspects matter*. Compare
  is field-wise per criterion, by tier:
  - **deterministic** — the golden's matched value (or scrubbed hash); drift if it
    changes.
  - **tolerance** — the golden's score + band; drift if the score leaves the band.
  - **judgment** — the **approved evidence** (the actual message/state text) plus a
    similarity threshold; a reworded-but-equivalent result stays green, a
    changed-meaning result drifts and surfaces old-vs-new for the human (Chromatic
    exactly).
- **First run is strict: no approved golden ⇒ CI fails.** A `new` expectation
  cannot pass in CI (Playwright's first-run-fails; Jest `--ci`'s
  missing-snapshot-fails) — you can never mint a green baseline in CI. The golden
  is created locally by `observe` + `approve` and committed in a reviewable diff.
- **`approve` is a human gate over a diff, granular like `--update-snapshots`.**
  `expect observation approve <scope>` promotes the last received observation to
  golden, with `--missing` (only brand-new), `--changed` (only drifted), or
  `--all` (bulk). `expect expectations list` surfaces the pending old-vs-new diffs
  first; not approving = the
  drift stays red until the code is fixed (Chromatic's reject). And **`CI=true`
  never auto-approves** — an unapproved drift is always a hard failure, never a
  silent write (the anti-`jest -u` invariant).
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
  goldens/src/checkout/coupon.golden.json         # approved + scrubbed observation (committed)
  received/src/checkout/coupon.received.json       # last observation (gitignored)
```

The golden and received trees mirror each spec's repo-relative path, so the spec's
location *is* its identity and moving a spec is a visible rename of its golden.

### The Check Loop

The most important architectural decision: **`expect` owns the mechanical
drive+observe for every surface in-process, and borrows an agent only for the
*reasoning*** — interpreting a fuzzy step, or authoring one. Pressing a button by
role+name, issuing an HTTP request, running a command is mechanical UI/IO automation,
expect's to own (no Node, no Python — see *Surface adapters*). What it does *not*
rebuild is LLM planning: when a step genuinely needs interpretation it delegates that
to an existing agent over ACP (the *Delegation* section). Either way `expect` owns
the surface, the stop conditions, the capture, and the verdict.

After the static `doctor` pass, an `expect expectation check` runs each expectation as
**provision → arrange → act → observe → evaluate → teardown**, with three roles
kept strictly separate — the **driver** causes the transition, the **adapter**
observes the authoritative state, the **grader** judges — and the driver is never
trusted as observer or judge:

1. **Provision** the SUT from the spec's `setup` (build + launch the
   binary/service, open a fresh fixture/db). `expect` owns this lifecycle (see
   *Provisioning and Isolation*).
2. **Arrange (Given)** — establish the precondition state, deterministically via
   fixtures where possible, agent-driven only where necessary.
3. **Act (When)** — the **driver** causes the transition, and for every surface the
   default driver is **expect's built-in adapter** doing it mechanically: cli runs the
   command, http issues the request, browser/gui press/type by `role[name=…]` over the
   accessibility tree. Mechanical actuation is deterministic and reproducible. An LLM
   agent enters only to *author* those concrete steps (in `create`) or as a **runtime
   fallback** when a cached action stops binding — and a fallback re-resolve is
   surfaced as drift, never silently applied. When an agent does drive, it is handed
   the **goal** (intent + Given + When) but **not** the `Then` criteria.
4. **Observe** — the **surface adapter** reads the *authoritative* state at each
   checkpoint — after every `When` step and at the end — directly from the SUT (exit
   code, stdout, files, a11y tree, db rows, http response), and assembles the
   `Observation` timeline (plus the trajectory). This — not the driver's transcript —
   is the result; the *observed program/DOM/DB state is ground truth, not a
   screenshot*. If the driver was an agent, its structured output is
   captured via a schema-forced `StructuredOutput` tool call (reuse `review`'s
   contract + tolerant `extract_json_value`), but it is treated as a *claim*, never
   the observation.
5. **Evaluate** — the **grader** runs the tiered ladder (deterministic → tolerance
   → model judgment) over the observation, and the verdict is compared to the
   golden. This is the pure `evaluate` step; it never trusts the driver's claim of
   success.
6. **Teardown** the provisioned instance.

#### Surface adapters: built-in, mechanical, in-process

Every surface adapter is a built-in engine that both **drives** (causes the When)
and **observes** (captures the authoritative checkpoint) — the same mechanism does
both. None of it is delegated to Node, Python, Playwright, or Appium; it all runs
**in the `expect` process** as Rust FFI / COM / D-Bus / WebSocket:

| surface | drive | observe | in-process mechanism |
|---|---|---|---|
| **cli** | run argv | stdout/stderr/exit/files | std process |
| **http** | issue request | status/headers/body | an HTTP client |
| **db** | run statements | rows/tables | a DB client |
| **file** | write | files/dirs/content | the filesystem |
| **browser** | press/type by `role[name=…]` | snapshot the a11y tree | **CDP** `Accessibility` + `Input` via `chromiumoxide` (pure Rust, no Node) |
| **gui** | press/type by `role[name=…]` | snapshot the a11y tree | **AX** (macOS `AXUIElement`) · **UIA** (Windows `IUIAutomation`) · **AT-SPI** (Linux `atspi`+`zbus`) |

**Accessibility is the GUI's drive *and* observe channel.** The same AX / UIA /
AT-SPI (and CDP `Accessibility`) tree you read for the observation also exposes the
actions — `AXPress` / UIA `InvokePattern` / AT-SPI actions / CDP `Input` — so the
adapter presses `button[name="Complete"]` and snapshots the resulting tree through
one API. This is deliberately *not* pixels: a locator binds to `role + accessible
name + tree position`, robust to layout/styling, and a genuine control rename
surfaces as honest structural drift — not the everything-screams-on-a-cosmetic-change
noise of a screenshot diff. Sparse a11y → vision/OCR is the last resort, and a sparse
tree is itself a signal the app's accessibility (and testability) is weak.

**This makes browser/gui *deterministic* surfaces.** Mechanical a11y actuation
("press the button named Complete") is reproducible, so browser/gui reclassify
alongside cli/http: deterministic, can run once. Non-determinism only enters when an
*agent* is in the mechanical loop (the runtime fallback), which is the exception.

**Drilling into a Tauri / Electron app.** Tauri uses the OS webview (WebView2 /
WKWebView / WebKitGTK), and **every OS webview bridges its web content's accessibility
into the native a11y tree** — a `<button aria-label="Complete">` shows up as a real
`button` node named "Complete" in AX / UIA / AT-SPI. So you **don't need the webview's
debug protocol**: the `gui` (native-a11y) adapter reads and drives a Tauri app exactly
like any native app.

- **macOS** (WKWebView): the web UI appears under an `AXWebArea`; read the subtree and
  drive via `AXUIElementPerformAction` — pure AX FFI, no inspector needed.
- **Windows** (WebView2): in the UIA tree. Bonus — WebView2 is Chromium, so enabling
  `--remote-debugging-port` also exposes the raw CDP `Accessibility` tree to
  `chromiumoxide` as an escape hatch when the bridged tree is thin.
- **Linux** (WebKitGTK): bridged to AT-SPI.

So the in-repo `kanban-app` (Tauri) is checked with `surface: gui`, native a11y, **no
CDP and no Node** — macOS AX reads the bridged React tree and `AXPress` actuates it.
The one quality dependency: the bridged tree is only as good as the web app's
semantics (`<button>`/`aria-label` → rich; `<div onclick>` soup → sparse).

**Stop conditions — `expect` owns both, because the substrate won't.** Every
mature loop in the survey has two independent stops, and the hard caps are always
harness-imposed (the model APIs document none):

- **Soft stop** — the agent declares it has reached the goal (returns its
  structured result). ACP surfaces this as `stopReason: end_turn`.
- **Hard caps** — a max-prompt-turns cap (anchor on the surveyed defaults: Claude
  computer-use 10, LangChain 15, LangGraph 25, Skyvern 10/25/50) and the spec's
  `timeout` wall-clock. ACP returns `max_turn_requests` / `max_tokens`.
- **Stall detection** — the strongest pattern (Magentic-One) is a decrementing
  stall counter plus an "are we looping?" judgment, re-planning at 3 stalls;
  `review`'s pool already gives us the deterministic floor of this — an
  `idle_timeout` that abandons a wedged turn and sends `session/cancel`. Reuse it.

Three hardening rules from the research are non-negotiable:

1. **The driver never sees the acceptance criteria.** The delegated agent gets the
   goal (intent + Given + When) but **not** the `Then` checklist or the golden —
   exactly as SWE-bench withholds the held-out test from the agent. The verifier
   checks the captured result against the withheld criteria. This is the single
   biggest defense against reward-hacking: an agent that can't see the rubric
   can't optimize to it. (METR measured o3 reward-hacking 30% of RE-Bench runs;
   "don't reward hack" in the prompt only moved it 80%→70% — withholding works,
   prompting doesn't.) The body's stated intent is the driver's goal, but
   `Notes`/right-reason text and the `Then` checklist are themselves criteria —
   withheld from the driver and routed to the grader at prompt-assembly (Open
   Question 8 tracks how cleanly that split holds).
2. **The verdict is deterministic and lives in `expect`, never in the agent.** We
   delegate *exploration*, not the pass/fail call. The agent's self-declared
   "done" is re-validated, never trusted — Skyvern's independent `check-user-goal`
   that can *reject* a self-declared COMPLETE is the gold standard, and SWE-bench
   data shows 35.7% of self-verified-as-correct trajectories were still wrong.
3. **The check is tamper-resistant.** The agent under evaluation runs in a sandbox
   it cannot use to edit the expectation, the golden, or fixtures; mutations to the
   ledger happen only through `expect observation approve`, a separate human-gated op. Detect
   spec/fixture edits directly, not just by grading outcome.

**Assert on outcomes, never on action equality.** Agent trajectories are not
reproducible even at temperature 0 + fixed seed — batch-invariance alone produced
80 unique completions from 1000 identical temp-0 requests, diverging by token 103.
Pass/fail is gated on the captured *outcome* (checkpoint state + criteria), never on a
byte-identical action sequence.

**Determinism comes from not calling the model, not from temp=0.** Following
Stagehand: cache each resolved action keyed by a hash of (normalized URL/target +
state snapshot + method), and on replay execute the cached action without the
model, re-resolving via the agent only on cache miss or fingerprint drift —
"a wrong cached click is worse than a slow click." This is what turns a fuzzy
authoring step into a fast, mostly-deterministic CI gate and answers the cost
critique (a full agent loop is ~3–5× a single call; you don't want that per push).

### Provisioning and Isolation

`expect` **owns the system-under-test lifecycle** — it provisions a fresh SUT,
drives it, and tears it down — so a `check` is a true gate on *this code, built
now*, not on whatever happened to be running. How the SUT comes to exist is the
`setup` declaration, per surface, leaning on `detected-projects` for build/launch
knowledge: cli builds and spawns the binary; http builds, launches, and waits for
ready; gui launches the app; db creates a fresh database and loads a fixture; file
runs in a scratch dir. Each surface defines its own provision **and** teardown.

**Granularity: provision once per `check`, shared.** The expensive build + launch
happens once; every expectation and every `pass^k` repeat runs against that one
instance, torn down at the end. This is the fast path, and the explicit trade is
that `expect` is *not* isolating expectations for you:

- **Expectations must be order-independent**, and each **`Given` must arrange its
  own preconditions** — never assume a clean slate, because the instance is shared
  and dirty from prior expectations.
- For `pass^k` to mean anything against a shared instance, the `Given` must
  **re-establish state on each `observe`** (otherwise run 1's effects bleed into
  run 2 and the reliability number is meaningless).

**Escape hatch: `isolation: fresh`.** An expectation that genuinely needs a
pristine SUT sets `isolation: fresh` and gets its own dedicated provision +
teardown instead of the shared instance — at the cost of the rebuild/relaunch.
Default is `shared`.

### Delegation over ACP (don't rebuild the agent)

**Whenever `expect` goes out to an agent — to drive the system or to judge a
residual criterion — it speaks ACP** (the Agent Client Protocol), the same way
`review` does. This is a deliberate stance, and the research backs the claim that
the alternative is a design error.

**The thesis.** Most testing tools (browser-use, Skyvern, Magnitude, Hercules)
embed their own live-LLM loop and thereby re-own — per tool — model access,
planning, tool-calling, retries, context management, and a permission model.
That's the M×N duplication that LSP killed for editors and that ACP exists to kill
for agents: ACP is the *client-drives-agent* layer (a host launches a full coding
agent as a subprocess, JSON-RPC over stdio, sibling to LSP). By delegating, a tool
inherits an existing agent's entire capability surface for free and stays swappable
across Claude Code (via the `claude-code-acp` adapter), Gemini CLI (native), and
others. The pattern is not exotic — eval harnesses already work this way: HAL and
Terminal-Bench *run* an external agent rather than embedding one; TestSprite is the
verifier/executor while Claude Code is the delegated fixer; and **`review` already
does precisely this inside this repo.**

**The honest counter-argument, and our answer.** The one place delegation bites a
*test* runner is determinism: an external agent is non-deterministic, and a
runner's value is reproducible verdicts. The answer is the split this doc already
takes — **delegate the exploration, keep the verdict deterministic and inside
`expect`**, behind a pinned agent version. Cognition's bounded-delegation rule
fits: the subagent answers a scoped question; the runner owns the decision.

**What we reuse from `review` (verbatim, not re-derived).** The codebase already
solved the hard parts; `expect`'s engine should sit on the same side of the
tool/engine boundary (agent-construction-free, receiving a `DynConnectTo<Client>`
from the tool layer):

- **`AgentPool`** (`swissarmyhammer-validators/src/validators/pool.rs`) — the whole
  `submit` / `submit_forked` / `submit_primed` / `SessionPinGuard` / `PoolError`
  surface. Per-turn liveness, `idle_timeout` → `abandon_turn` → `session/cancel`,
  and the fork/pin warm-reuse choreography come for free.
- **`run_review_over_agent`** (`.../review/drive.rs`) — the reference ACP wiring:
  `Client.builder().with_handler(TolerantResponseRouter)`, the **single** notifier
  feed (double-feeding corrupts the JSON), **once-per-connection** `initialize`,
  and `answer_agent_request` for mid-prompt `session/request_permission` and
  `fs/read_text_file` (confined under repo root). Mirror it; do not re-discover its
  deadlock/double-feed fixes.
- **The `AgentFactory` / `AgentHandle` seam** + the process-global pipeline gate +
  the spawn-blocking-on-a-current-thread-runtime pattern (`review_op.rs`).
- **`extract_json_value`** (the tolerant fenced-JSON extractor) for the
  `StructuredOutput` capture.
- **The validator model + loader + introspection** (`types.rs` / `loader.rs` /
  `validators.rs`) — the closest existing template for the expectation file format,
  the three-layer builtin→user→project precedence, and the `list`/`get`/`check`
  read ops. The expectation/criteria model is the analog of the RuleSet/Rule model.

**What ACP does *not* give us, so we build it** (client-side, mirroring review):
ACP has no native subagent type and its prompt turn returns a control signal
(`stopReason`), not a structured payload. So the "subagent per expectation" is our
abstraction — open one scoped `session/new`, send the goal, drain `session/update`,
read `stopReason`, tear down — and the structured result is assembled by us from a
forced `StructuredOutput` tool call, not handed over by the protocol.

**The verifier can itself be an ACP delegation — and should be independent.** Most
criteria resolve deterministically (the floor catches an estimated 30–60% for
free). For the residual subjective criteria, the judge is a model call; if that
judge needs to *act* (re-run the program, inspect state) it is another ACP
subagent — an "Agent-as-a-Judge," which measured ~90% human alignment vs ~60–70%
for static LLM-as-judge. Independence matters: the grading model/agent should not
be the same one that drove the system (self-preference inflates verdicts 10–25%,
and self-verification of one's own trajectory systematically under-detects its own
errors). Since the subject under test is a *program*, the primary verifier —
program execution — is structurally independent already; the rule only constrains
the residual model-judged criteria.

### Reliability and Non-Determinism

- **`pass^k` is the headline metric**, not average pass rate. `reliability:
  pass^3` means all three runs must pass; the verdict reports the per-run spread
  so a 2-of-3 flake is visible, not hidden behind an average.
- **Repeat runs default to ≥2 only when an agent drives** (the runtime fallback).
  Every surface is *mechanically* driven by default — cli/http/file/db and
  a11y-driven browser/gui alike — so the default is deterministic and runs once;
  non-determinism (and the ≥2 default) enters only when the agent fallback resolves
  an action live.
- **`pass^k` requires a re-arranged `Given`.** Because the SUT is shared across a
  `check` (see *Provisioning and Isolation*), each repeated `observe` must
  re-establish its `Given` state — or set `isolation: fresh` for a clean instance.
  Otherwise the repeats aren't independent and `pass^k` is theater.
- **Grading hardening**: bounded binary criteria over free-form scoring (a
  rubric grades one observable criterion at a time, not a vibe), and — when a
  criterion is borderline — an optional small **panel** of named models, where
  disagreement is itself a signal the criterion is vaguely worded (a disjoint
  panel correlated with humans *better* than a single GPT-4 at ~7–8× lower cost).
  The subject under test is a program, so judge self-preference is mostly moot —
  with one carve-out: a residual criterion that is model-judged should be graded
  by a *different* model than the one that drove the run, since an agent grading
  its own trajectory both inflates the verdict and misses its own errors.
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
on_missing = "fallback"            # pinned model gone: "fallback" (warn + use default) | "error"

[provision]
granularity = "per-check"          # one shared SUT per check; `isolation: fresh` overrides per-spec
# per-surface build/launch/teardown is auto-detected (detected-projects); `setup:` overrides

[embedder]
model = "text-embedding-3-large"   # pinned; checkpoint matters for reproducibility
similarity_threshold = 0.80

[reliability]
default = "pass^1"
nondeterministic_surfaces = []           # all surfaces drive mechanically (deterministic);
                                         # only the agent runtime-fallback adds non-determinism

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
| Agent harness | n/a | **rebuilt in-house** (own loop/tools/model) | n/a | **delegated over ACP** (reuse an existing agent) |
| Validation | code assertions | LLM judge | literal diff | **tiered ladder, judge gated, criteria withheld from driver** |
| Drift control | none built in | none | snapshot/visual diff | **snapshot-style approval of observations + pass^k** |
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

### Phase 2 — CLI surface, deterministic observe + evaluate (no agent yet)
5. **cli** surface adapter + provisioning: build/launch from `setup` (via
   `detected-projects`), capture stdout/stderr/exit/files, teardown. The
   deterministic-only path needs **no agent**.
6. `expect expectation observe` → `Observation`; `expect observation evaluate` (pure) Tier 1 only
   (exact/regex/schema/exit-code).
7. `expect expectation check` = doctor + observe + evaluate; scope resolution
   (path/folder/glob); teaching error messages.

### Phase 3 — The ledger and the human gate
8. Golden = approved scrubbed **observation**; per-criterion compare by tier;
   scrubbers.
9. `expect observation approve` (granular `--missing`/`--changed`/`--all`,
   human-gated); **strict first-run** (no golden ⇒ CI fails); never auto-approve
   in CI.
10. Drift detection: `evaluate(received)` vs `evaluate(golden)`, queue unapproved
    drift.

### Phase 4 — ACP delegation (reuse review's machinery)
11. Stand up the agent seam by reusing `review`: `AgentPool`,
    `run_review_over_agent` wiring (`TolerantResponseRouter`, single notifier,
    once-per-connection `initialize`, `answer_agent_request`), the
    `AgentFactory`/`AgentHandle` seam, pipeline gate. `AgentUseCase::Expectations`
    wired through `ToolContext`.
12. One scoped ACP session per expectation; goal-only prompt (criteria withheld);
    structured-output capture via forced `StructuredOutput` + `extract_json_value`.
13. Stop conditions: max-turns cap, `timeout`, `idle_timeout`→`session/cancel`;
    independent re-validation of the agent's self-declared "done".

### Phase 5 — Semantic tiers + authoring
14. Tier 2 embedding tolerance bands (pinned embedder).
15. Tier 3 model judgment against withheld criteria (named `model:`, binary,
    different model than the driver), gated behind tiers 1–2; optional panel.
16. `expect expectation create` — agent authors specs from intent (schema + rules as its
    instructions), `--from-session` capture; leaves the file + an unapproved
    golden (ledger state `new`) for a human to edit and `approve`.

### Phase 6 — Non-determinism + more surfaces
17. `pass^k` reliability, repeat runs, escalation queue.
18. Resolved-action cache for deterministic replay (Stagehand model).
19. **http** surface, then **browser** (a11y-tree), then **gui** (OS a11y) surfaces.
20. **db** / **file** state surfaces.

## Open Questions

1. **Storing a full observation for heavy/volatile surfaces.** We approve the full
   scrubbed observation — trivial for cli/http/db, but large and noisy for
   browser/gui (a whole a11y tree). What gets committed vs. gitignored — the
   criterion-relevant slice + a digest in the golden, raw trajectory kept only
   locally for `expect observation get`? Scrubbing must be aggressive enough that the
   golden is stable yet still re-evaluable against new criteria.
2. **Single expectation file vs. directory of small ones.** Following the project
   convention (one fact per memory, small rules), lean toward one expectation per
   file for clean PR diffs and per-file goldens.
3. **Re-baselining on a deliberate grading-model change.** When the team
   intentionally changes the pinned `model:`, every golden's pass-boundary may
   move. Need an `expect rebaseline` that re-runs and presents the full diff for
   a single bulk human approval — explicitly, never silently.
4. **How much of `review` to extract vs fork.** `expect` should reuse `AgentPool`,
   `run_review_over_agent`, the `AgentFactory` seam, and `extract_json_value`. Are
   these stable enough to factor into a shared crate both `review` and `expect`
   depend on, or does `expect` start by copying `drive.rs`/`pool.rs` and we
   converge later? (The engine boundary — agent-construction-free, receives
   `DynConnectTo<Client>` — must be preserved either way.)
5. **Happy-path bias in `create`.** Auto-drafting from chat/task/spec captures the
   obvious criteria well, but agents are documented to be weak at failure/edge
   scenarios. How does the authoring skill push for the negative cases (the "and it
   does NOT do X" criteria) rather than only the stated happy path?
6. **Default grading model choice.** When an expectation omits `model:`, it
   falls back to the sah model default. Is that the right default for *grading*,
   or should `expect` carry its own default (the way `review` defaults to
   claude-code-haiku) so a global model change doesn't silently shift every
   ungoverned expectation's verdict?
7. **One model or two.** Is the split between a driving agent and a grading model
   worth the extra config, or should `expect` default them to the same named
   model and only let advanced users separate them? (The independence argument
   only bites for *model-judged residual* criteria — most criteria are
   deterministic and don't care.)
8. **Withholding criteria from the driver in practice.** The driver gets intent +
   Given + When but not the `Then` checklist. But a human writes them in one file,
   and the intent prose may leak the criteria anyway. How aggressively do we split
   the spec at prompt-assembly time, and is a leaky intent acceptable given the
   deterministic verdict is what actually gates?
9. **Replay-cache invalidation.** A resolved-action cache keyed on
   (target + state snapshot + method) speeds CI but can replay a stale action into
   a changed program and mask a regression. What's the fingerprint-drift threshold,
   and should the cache auto-invalidate on any spec edit or golden change?
10. **Sandbox boundary for the driver.** Tamper-resistance requires the driving
    agent run where it cannot edit specs/goldens/fixtures. Does `expect` reuse an
    existing sah sandbox, run the agent in a worktree, or rely on the ACP
    permission model (`answer_agent_request`) to deny writes to `.expect/`?
