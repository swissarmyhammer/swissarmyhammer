---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9680
title: 'review fleet: warm prefix reuse never engages — every fork runs cold'
---
All nine validator tasks in the 2026-08-07 dogfood run logged "fleet task fork was degraded (no warm prefix reuse); proceeding cold" (../swissarmyhammer-main/.sah/mcp.7341.log, 15:15:12–15:17:21). The prime mechanism in `crates/swissarmyhammer-validators/src/review/fleet/prime.rs` gave zero reuse against the real claude-agent path. Every validator paid a full cold context; the review took 368s for a 2-file diff. This is the dominant wall-clock and token cost.

Work:
- Find why the fork degrades: trace the prime session creation and the fork request against claude-agent. The warning fires for every validator, so the cause is systemic, not per-task.
- Fix the fork path so a warm prefix is reused, or determine that claude-agent cannot support it and remove the prime stage plus its cost — a stage that always degrades is dead weight and a misleading log line.
- Acceptance: a production-path test (or an instrumented `review file` run) shows at least one fleet task reusing the warm prefix, or the prime stage is gone and the degraded warning with it.

#tool-validators