---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw3msqb33ydmyp7v8fwkh6ca
  text: |-
    Picked up. Research done. Design:
    - Stall/idle is fully delegated to AgentPool (PoolConfig idle_timeout/turn_ceiling already implement idle→abandon→session/cancel→PoolError::TurnIdle). drive.rs only needs to PRESERVE that typed outcome instead of flattening it: add ExpectError::Pool(#[from] PoolError) and stop stringifying the PoolError in drive_scope.
    - Soft stop + max-turns: thread the ACP stop_reason out of the drive seam. Add DrivenObservation.goal_reached (= stop_reason==EndTurn); change GoalDriver::drive_goal to return a DriverTurn { claim, goal_reached }. observe_with_driver now drives over a bounded helper drive_until_goal_reached (loops up to MAX_PROMPT_TURNS=15 until the agent declares end_turn; cap-exhausted = clear error). Production AcpGoalDriver always end_turns in one scoped session, so the loop runs once there.
    - Timeout + re-validation: new public fn drive_and_revalidate(expectation, adapter, config, driver) wraps observe_with_driver in tokio::time::timeout(spec.timeout)->ExpectError::Timeout, then evaluate_spec over the adapter-observed Observation -> ExpectationVerdict. A self-declared done (goal_reached) whose observed state fails the criteria yields a FAIL verdict (rejected).
    Tests (stub agent/driver, mirror existing): idle via AcpGoalDriver+StallingAgent+short idle config -> ExpectError::Pool(TurnIdle); max-turns via scripted driver goal_reached=false -> Err naming MAX_PROMPT_TURNS; timeout via scripted driver that sleeps > spec.timeout -> ExpectError::Timeout; revalidation via scripted driver goal_reached=true + StubAdapter state failing the criterion -> fail verdict.
  timestamp: 2026-06-27T04:18:25.763629+00:00
- actor: claude-code
  id: 01kw3p4241km7rqyrp1sjeh1p0
  text: |-
    Implemented + adversarial double-check run (verdict REVISE, advisory). Resolution:

    FIXED (Finding 1, HIGH): the bounded re-prompt loop was fictional for the production single-use AcpGoalDriver — a non-EndTurn first turn would hit "already used" on iteration 2 instead of a clear cap error. Replaced the loop with single-drive `drive_for_goal`: one scoped session (AcpGoalDriver's contract), and a turn that ends WITHOUT end_turn is a clear terminal max-turns error (never mistaken for success). MAX_PROMPT_TURNS is now genuinely imposed by stating it in the goal prompt (build_driver_goal) — the only channel the pool transport carries — and asserted in the prompt test + the max-turns test.

    DEFERRED to follow-up card ^z8rwhhj (Finding 2, MEDIUM — no production caller / fixture-only): drive_and_revalidate is library API exercised by stub driver/adapter; the live expect tool still calls run_expect_over_agent. This card was scoped to drive.rs + stub tests per the spec; wiring the tool layer needs Expectation+adapter threaded through expect_op (bigger change). The idle stop IS tested over the real AcpGoalDriver + real pool.

    ACCEPTED w/ justification (Finding 3, LOW — timeout teardown via Drop): the timeout path drops the AgentPool, whose workers are aborted on Drop (pool docs); the stall path additionally sends session/cancel. A real-session timeout-teardown test is in ^z8rwhhj.

    NOT APPLICABLE (Finding 4, LOW — claim graded by text criteria): evaluate/CompiledAssertion read only Observation.checkpoints[*].state, never trajectory.steps where the claim lands, so the claim cannot leak into the verdict. A text-criterion pin test is in ^z8rwhhj.

    Verification (all GREEN): cargo nextest run -p swissarmyhammer-expect (129 passed); targeted -E 'test(drive) or test(stop) or test(driver)' (21 passed); cargo check --workspace; cargo clippy -p swissarmyhammer-expect -- -D warnings (clean); cargo fmt. Red-green confirmed: reverting the typed-PoolError preservation makes stop_conditions_idle_turn_is_abandoned_as_turn_idle fail.
  timestamp: 2026-06-27T04:41:33.057782+00:00
depends_on:
- 01KW26600YD3PM22S8F4VJJTE5
position_column: doing
position_ordinal: '8280'
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