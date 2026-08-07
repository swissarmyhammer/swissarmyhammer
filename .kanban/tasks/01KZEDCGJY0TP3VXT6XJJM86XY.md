---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9580
title: 'review: retry a fleet reply that does not parse before failing the pair'
---
One malformed LLM reply invalidates a whole review. Seen 2026-08-07 in ../swissarmyhammer-main (`.sah/mcp.7341.log`, 15:16:31): the duplication validator's reply failed JSON parse ("expected `,` or `}` at line 7 column 361"), the engine yielded zero findings, marked the task failed, and the 6-minute review ended INCOMPLETE. The only recovery is a full re-run.

Requirements:
- When a fleet task reply does not parse into findings, re-ask that one task once before it is declared failed. Log the retry.
- Before the re-ask, try a cheap repair parse first (strip text before `[` and after `]`; a reply is a JSON array by contract). A repaired parse counts as success and needs no retry.
- Only a second parse failure marks the task failed. The INCOMPLETE banner behavior stays — honesty is correct.
- Test: a fleet task whose first reply is malformed and whose second reply is valid produces findings and a complete report; a task with two malformed replies fails the pair as today.

#tool-validators