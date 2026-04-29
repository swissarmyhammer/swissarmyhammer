---
assignees:
- claude-code
position_column: todo
position_ordinal: fc80
title: 'Validator runner: 30s per-rule timeout is too aggressive — qwen times out on ~100% of rules'
---
## Symptom

After the per-rule timeout + parallel-rules-within-ruleset shipped (sibling task `01KQAFFZDX40GSKXQVS0MTNDWV`, marked done), a Stop-hook test run on the evening of 2026-04-28 against `swissarmyhammer-common/src/sample_avp_test.rs` produces:

- **Stop hook total runtime: ~60s** (was 10+ minutes yesterday) ✅
- **`validator result ... hook_type="Stop"` line per rule** ✅
- **Stop hook event closes cleanly** ✅
- **9 of 10 rules dispatched in parallel within 700ms** ✅
- **But: 10 of 10 rules hit the 30s wall-clock timeout and got logged as `passed=true ... reason="timeout"`** ❌

`grep` across all evening runs for non-timeout verdicts: **exactly 1 rule completed in real analysis** (`security-rules:input-validation` at 21:36:23). Every other rule on every other run timed out.

This means the validator is now structurally working but produces **zero real signal**. Effectively a quality-of-result regression — qwen is too slow to actually analyze code within 30s, so all rules pass-with-warning regardless of whether the code has issues.

## Why

Likely a combination:

1. **qwen reasoning + tool calls take longer than 30s for non-trivial files.** The fixture file is ~95 lines with multiple functions, magic numbers, commented code — qwen's reasoning + `read_file` round-trip + final JSON is observably 60–90s in yesterday's sequential logs.

2. **Parallel execution may make per-rule time *worse*, not better.** Yesterday with sequential execution, single rule wall time was 30–90s. Today with 9 rules in parallel, all sharing one llama-agent process / one GPU / one model context, contention may push each rule's individual wall time even higher — but they're now bounded at 30s.

3. **Model warm-up + per-session MCP connect overhead** repeats per rule. Today's 21:40:17 logs show 9 `new_session` calls in 250ms — each followed by MCP client creation, tool discovery, session registration. If that's 1–2s per rule before the model even starts inference, 30s is even tighter.

4. **Thinking mode may still be on for Stop runs.** Yesterday's PostToolUse recordings had empty `<think></think>` blocks; Stop runs didn't have recordings (see task `01KQAFT5H1CYQ8YDNAM4J0HD1Q`) but the 1484-token first turn pattern matches \"reasoning then tool call\" rather than \"tool call directly.\"

## What to investigate / tune

### Tier 1 — verify the diagnosis

- Set `RULE_TIMEOUT_SECS=120` env var (or whatever the knob is) and re-run. Do rules complete? If yes, knob fits. If no, deeper issue.
- Set parallelism cap to 1 (`max_in_flight=1`) and re-run with 30s timeout. If rules complete sequentially, contention is the issue.
- Compare the `Agentic loop completed: N tokens generated` line for a parallel run vs. sequential run for the same rule — if the parallel run takes 2-3× the tokens, the model is producing more output (e.g. retrying reads, exploring more) under contention.

### Tier 2 — tunable defaults

The right defaults depend on the model:
- **qwen-3.6-27B**: probably 60–90s per rule, max_in_flight=2–3 (memory pressure from concurrent contexts)
- **claude-code as validator**: 5–10s per rule, max_in_flight=8+
- **smaller qwen (0.6B/1.5B)** per task `01KQ4WEHKG6E3X6ZPPBGJNRA5T`: 5–15s per rule

Wire the timeout + parallelism caps to model identity, not a global default. The validator config already knows whether it's claude-code or llama-agent + which model.

### Tier 3 — make rules cheaper per-rule

- **Disable thinking** on the validator chat template (per task `01KQAFFZDX40GSKXQVS0MTNDWV` Problem 1 option). PostToolUse recordings show this is already happening on that path; replicate for Stop.
- **Cache the rule prompt** — the rule body itself is static, only the file content changes. If the chat template engine supports prompt caching for the system + rule sections, the per-rule cold cost should go down a lot.
- **Reuse one shared session for all rules in a ruleset** instead of fresh sessions. The per-rule fresh session was for prompt-bleed isolation, but maybe a ruleset-shared session with explicit \"forget previous rule\" prompting is cheaper.

## Acceptance

- For the same `sample_avp_test.rs` fixture, a Stop-hook run produces real `passed=(true|false)` verdicts with actual analysis for at least 80% of rules (not `reason=\"timeout\"`).
- Stop hook total runtime stays under 3 minutes (we have headroom now that we know the floor is ~60s and the ceiling needs to allow qwen its real thinking time).
- For at least one rule that should fail (e.g. `no-magic-numbers` against the fixture's `7777`, `0xCC`, `11250` literals), the verdict is `passed=false` with a message referencing the actual literals.
- A regression test in `avp-common/tests/` exercises a small mock-validator-agent that returns its verdict at exactly `timeout - 1` seconds, asserting it's logged as a real verdict (not a timeout). And another that returns at `timeout + 1` seconds, asserting it's logged as a timeout.

## Pairs with

- `01KQAFFZDX40GSKXQVS0MTNDWV` (parent — shipped the timeout + parallelism). This is the tuning follow-up.
- `01KQ4WEHKG6E3X6ZPPBGJNRA5T` (smaller model option). One mitigation path.

#avp #performance #tuning