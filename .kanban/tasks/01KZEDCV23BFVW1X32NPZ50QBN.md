---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9780
title: 'tool rules: cache the doctor/fixture verdict and overlap the tool run with the fan-out'
---
Every review re-proves tool health from scratch, serially. The 2026-08-07 dogfood run (../swissarmyhammer-main/.sah/mcp.7341.log): ~50s of doctor fixture verification (the fixtures run real cargo clippy twice, fail + pass) plus 42s of workspace clippy for the actual judgment — about 100 of 368 seconds, all before the LLM fan-out starts. The health verdict cannot change until the tool version changes, and the tax grows with every tool rule (six missing-docs rules today; the complexity rules will add more cargo runs).

Two changes:
1. Cache the doctor + fixture verdict, keyed on (tool version string, rule content hash). A hit skips the fixture runs. A miss, a version change, or a rule edit re-verifies. Store beside the review engine's other state; `sah doctor` always re-verifies and refreshes the cache — doctor stays the ground truth.
2. Run the tool `run` scripts concurrently with the LLM fan-out. The suppression plan needs only the HEALTH verdict up front (it decides which prompt rules are skipped); the tool FINDINGS are only needed at synthesis. Keep the plan step; move the execution to overlap the fleet.

Acceptance: a second `review working` on an unchanged toolchain shows no fixture clippy runs in the log, and the tool run no longer delays the first fleet task.

#tool-validators