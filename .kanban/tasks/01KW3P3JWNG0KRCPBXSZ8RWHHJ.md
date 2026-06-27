---
assignees:
- claude-code
depends_on:
- 01KW266VBY2KC9XYMVDGG00RXF
position_column: todo
position_ordinal: c480
project: expect
title: Wire drive_and_revalidate into the live expect tool + real-path test
---
## What
The stop-conditioned engine `swissarmyhammer_expect::drive_and_revalidate` (bounded drive + spec-timeout + independent re-validation, from task ^gg00rxf) is implemented and unit-tested with stub drivers/adapters, but the LIVE expect MCP tool still calls `run_expect_over_agent` directly (`crates/swissarmyhammer-tools/src/mcp/tools/expect/expect_op.rs`). So the stop conditions + re-validation are not yet on the production path.

This is the documented fixture-only / real-path-test gap: the deliverable must run over a real `AcpGoalDriver` + real `SurfaceAdapter`, not only stubs.

## Acceptance Criteria
- [ ] The live expect pipeline drives expectations through `drive_and_revalidate` (thread the `Expectation` + chosen `SurfaceAdapter` through `expect_op` so the verdict is produced over the adapter-observed state, not just `Vec<DrivenObservation>`).
- [ ] A real-pipeline integration test (mirror `semantic_search_e2e` style) exercises the spec-timeout teardown over a REAL/mock-ACP session (Finding 3 from ^gg00rxf review: confirm `AgentPool` Drop cancels the in-flight session on timeout, not just aborts workers).
- [ ] A re-validation test with a trajectory/stdout TEXT criterion (not just a JSON field) confirms the agent's claim cannot satisfy the verdict (Finding 4 from ^gg00rxf review). Note: today `evaluate` reads only `Observation.checkpoints[*].state`, never `trajectory.steps` where the claim lands, so this should already hold — the test pins it.

## Notes
Surfaced by the adversarial double-check on ^gg00rxf. That card was deliberately scoped to `drive.rs` + stub tests; this card carries the production wiring + real-path coverage.