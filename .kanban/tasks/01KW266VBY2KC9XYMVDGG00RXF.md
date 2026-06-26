---
assignees:
- claude-code
depends_on:
- 01KW26600YD3PM22S8F4VJJTE5
position_column: todo
position_ordinal: b380
project: expect
title: Stop conditions + independent re-validation of agent "done"
---
## What
Bound the agent driver loop with two independent stops and never trust the agent's self-declared completion. Per `ideas/expect.md` §"Stop conditions" and hardening rule 2.

- In `crates/swissarmyhammer-expect/src/drive.rs`:
  - **Soft stop**: agent declares goal reached (ACP `stopReason: end_turn`).
  - **Hard caps**: a max-prompt-turns cap (anchor a sensible default, e.g. ~10-15) AND the spec `timeout` wall-clock.
  - **Stall/idle**: reuse `AgentPool`'s `idle_timeout` → `abandon_turn` → `session/cancel` (the deterministic floor) so a wedged turn is abandoned, not hung.
- **Independent re-validation**: the agent's self-declared COMPLETE is re-checked by `expect` (the deterministic verdict via `evaluate` over the adapter-observed state) and can REJECT a self-declared done. The verdict lives in `expect`, never in the agent — delegate exploration, not the pass/fail call.

## Acceptance Criteria
- [ ] A driver turn that idles past `idle_timeout` is abandoned via `session/cancel` (no hang); surfaced as a `PoolError::TurnIdle`-style outcome.
- [ ] Exceeding max-turns or the spec `timeout` terminates the loop with a clear error, not a hang.
- [ ] An agent that returns "done" but whose adapter-observed state fails the criteria yields a FAIL verdict (self-declared done rejected).

## Tests
- [ ] Stub-agent tests (mirror review/pool tests): idle→cancel, max-turns cap, timeout cap.
- [ ] Test: agent claims success but observation fails criteria ⇒ verdict is fail.
- [ ] `cargo nextest run -p swissarmyhammer-expect stop_conditions` passes.

## Workflow
- Use `/tdd`. Reuse `AgentPool` liveness rather than reimplementing it.