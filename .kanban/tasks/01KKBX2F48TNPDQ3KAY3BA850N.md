---
position_column: done
position_ordinal: f780
title: 'Test: sah doctor runs clean without LSP/code-context noise'
---
## What

Verification card for the constructor fix (01KKBWZ7NXHTW2CA86ZFK8GHD2). After code-context background work is moved out of the `McpServer` constructor, `sah doctor` should run without LSP/indexing noise.

## Tests
- [ ] `sah doctor` completes without LSP error/warn log messages
- [ ] `sah doctor` completes without code-context indexing log messages
- [ ] `sah doctor` output shows only actual health check results
- [ ] `sah doctor --verbose` adds detail but still no LSP noise
- [ ] `sah doctor` completes in under 2 seconds (no server warmup)
- [ ] All existing health checks still pass (tools, configuration, system)

## Acceptance Criteria
- [ ] Doctor output is clean and believable — no logs contradicting passing checks
- [ ] `collect_all_health_checks()` still works (creates its own lightweight ToolRegistry)