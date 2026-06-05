---
assignees:
- claude-code
depends_on:
- 01KTBN9E9FD9X1PY1ARY9SMN99
- 01KTBNKCZ2JRRX514XWHPFB7V1
position_column: todo
position_ordinal: 8a80
project: local-review
title: 'Engine stage 2 — fan-out: one agent per (validator × file), file-grain, in parallel'
---
## What
The fleet. The shard is the validator; the **grain is the file**. Produce one agent task per `(validator, file)` pair and submit them to the shared `AgentPool` — each task reviews ONE file against ONE validator's rules, armed with engine-run probe evidence, and returns `Vec<Finding>` tagged with the validator name. Bounded context per task = no attention decay; the probes supply the cross-file facts a single file view would otherwise lack.

**Fan-out unit:** `(validator, file)`. To bound task count on large diffs, BATCH a few files per task (chunked to fit context comfortably) — but never the whole diff. The grain is the file; the batch is just packing. `log()` the batching applied.

**Parallelism is NOT controlled here.** Every task is submitted to the shared `AgentPool` (task 5), which owns the single concurrency control (worker count: local→1, remote→N/AIMD, `review.concurrency` override). Fan-out submits all `(validator, file)` tasks; the pool queues and drains them. As each task returns, its findings flow to the verify stage's inline guard, which may enqueue verify tasks onto the **same** pool — so fan-out and verify pipeline together (no stage barrier until synthesis).

**Prompt payload** (the renderer assembles exactly this per task; reuse the existing validator prompt-render path, do not invent a new template engine):
1. **Change purpose** (from `WorkList.change_purpose`) — why this change exists, so the agent judges intent (e.g. scaffolding vs. dead code).
2. **Validator instructions** — the mandate (`description`), the `rules/*.md` bodies verbatim (incl. carve-outs), the severity default, and the output contract. The output contract requires each finding to emit: `rule` (which rule of this validator fired), `claim` (what's wrong AND why it matters), `evidence` (the proof — cite the injected probe result, e.g. "per `duplicates`: 0.94 at `bar.rs:88`"), `suggestion` (the fix). One concern per finding; `file:line` citation. Matches the `Finding` type exactly.
3. **The file under review** — `path`; the structured semantic diff (changed entities, before→after); the bounded `source_slice` (header + changed entities + window, NOT the whole file); and the `probe_results` rendered as evidence blocks ("Inbound callers of new `fn foo`: none"; "Duplicate candidates for L42: `bar.rs:88` @ 0.94"; "Similar existing code: `util::parse` @ 0.88").
- Excluded by design: other files' diffs, other validators' rules, whole-codebase dumps. The agent also HAS the code_context tools to dig deeper, but the mandatory facts are pre-injected so a lazy agent can't miss them.

**Result handling:** parse each returned task with `parse_findings`; tag findings with the validator; hand them to the verify guard. A task that errors/times out yields zero findings for that `(validator, file)` and is logged via `tracing`, never panics the run.

## Acceptance Criteria
- [ ] Fan-out builds one task per `(validator, file)` (batched) and submits to the shared `AgentPool`; findings are tagged by validator and `rule`.
- [ ] The rendered prompt contains exactly the payload above; the output contract instructs the agent to emit `rule` + `claim`(what+why) + `evidence`(cited probe proof) + `suggestion`.
- [ ] No concurrency logic here — all execution goes through the pool; backend pass-through honored; batching logged.
- [ ] A failing/slow task degrades to zero findings for its `(validator, file)` without aborting the others.

## Tests
- [ ] Mock-agent test (`PlaybackAgent`/`SessionRecordingAgent`): a `WorkList` with 2 validators × 2 files → ≤4 tasks submitted, scripted responses parsed into the merged `Vec<Finding>` with correct `validator`/`rule` tags; assert the rendered prompt for one pair contains the change purpose + that file's probe evidence and NOT the other file's content.
- [ ] Batching test: many small files collapse into fewer tasks, logged.
- [ ] Resilience test: one task errors → its findings empty, the rest still return (no deadlock/panic).
- [ ] `cargo test -p swissarmyhammer-validators review::fleet` green.

## Workflow
- Use `/tdd` — script the mock agents and assert the merged findings + the exact prompt payload (incl. the `rule`/`evidence` contract) first, then implement the renderer + task submission. Reuse the `AgentPool` and the validator prompt-render path; do not reimplement concurrency or templating.