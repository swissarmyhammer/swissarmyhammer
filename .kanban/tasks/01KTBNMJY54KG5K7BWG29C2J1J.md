---
assignees:
- claude-code
depends_on:
- 01KTBNM0YGVRJQJSCTQBDHR68H
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffff780
project: local-review
title: 'Engine stage 3 — verify: adversarial refute pass over candidate findings'
---
## What
The differentiator from ultra-review: a candidate finding is reported only after it survives refutation against ground-truth evidence. Two layers — a cheap deterministic guard, then an adversarial agent — and both are **pipelined on the shared agent pool**, not a separate batch after fan-out.

1. **Probe guard (deterministic, no worker).** As each fan-out finding comes back, run the guard inline. It **reuses** the `probe_results` already attached to the work item (no re-run) and acts only on **`fact` probes**:
   - `callers` (fact) → auto-refute a `dead-code` finding whose symbol has non-empty inbound callers.
   - `duplicates` (fact) → auto-refute a `duplication` finding with no matching duplicate block.
   - `similar` is a **candidate** probe, not a fact → findings backed by `similar` (reuse-miss) get NO deterministic guard; they go straight to the agent. (Anything the guard can't decide passes through.)
2. **Adversarial verifier (agent, pipelined).** For each finding that survives the guard, submit a verify task to the **same `AgentPool`** (so verification of early findings runs while later validators are still fanning out). The verifier prompt is the inverse of fan-out: the specific finding + that file's `source_slice` + the relevant `probe_results` + "try to DISPROVE this claim." It returns `VerifiedFinding { finding, confirmed, reason }` and DEFAULTS TO `confirmed = false` on uncertainty or tool failure — only positively-substantiated findings survive.

- No separate concurrency control here — submission goes to the shared pool (task 5), worker count is the single knob.
- Record which layer refuted (guard vs agent) on the `VerifiedFinding` so synthesis/summary can report confirmed/refuted counts and reasons.

## Acceptance Criteria
- [ ] The deterministic guard reuses work-item `probe_results` (no re-run), auto-refutes only via `fact` probes (`callers`/`duplicates`), and passes `similar`-backed and undecidable findings through to the agent.
- [ ] Surviving findings are verified by an adversarial agent submitted to the shared `AgentPool` (pipelined with fan-out), refute-by-default; uncertain/tool-failure → `confirmed = false`.
- [ ] `VerifiedFinding` carries the verdict + reason + refuting layer; confirmed/refuted counts available to the caller.

## Tests
- [ ] Guard test (no agent): a planted `dead-code` finding whose `callers` result shows callers is auto-refuted; a real one passes; a `similar`-backed reuse-miss finding always passes the guard to the agent.
- [ ] Mock-agent test: 3 guard-surviving findings, scripted verifier responses (2 confirm, 1 refute) → correct verdicts; a 4th whose verifier errors resolves to refuted.
- [ ] Pipelining test: verify tasks submitted while fan-out tasks are still in flight are processed by the same pool (no separate stage barrier before synthesis).
- [ ] `cargo test -p swissarmyhammer-validators review::verify` green.

## Workflow
- Use `/tdd` — write the deterministic guard test first (needs no agent), then the scripted confirm/refute/error verifier test, then the pipelining test. Reuse the `AgentPool` and the probe `fact`/`candidate` kinds; the only new content is the guard logic and the adversarial verifier prompt.