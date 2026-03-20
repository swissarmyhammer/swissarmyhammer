---
depends_on:
- 01KKKP1BTZ4GZXMC18WSHZXBHZ
position_column: done
position_ordinal: ffab80
title: Remove js tool usage from test skill
---
## What

The test skill instructs agents to use the `js` tool to record `are_tests_passing` as a boolean variable. This is write-only — nothing ever reads it back via `get expression`. Remove this step entirely; the pass/fail result is already communicated in step 7 ("Report back").

**Source of truth to edit:**
- `builtin/skills/test/SKILL.md` — remove step 6 ("Record the overall result") which references `js` with `op: "set expression"`

**Generated files (will auto-update, but can patch now):**
- `.skills/test/SKILL.md`
- `.agents/test/AGENT.md`

Renumber remaining steps (current step 7 becomes step 6).

## Acceptance Criteria
- [ ] `builtin/skills/test/SKILL.md` has no references to `js` tool, `set expression`, or `are_tests_passing`
- [ ] Steps are renumbered correctly (no gap)
- [ ] Generated `.skills/test/SKILL.md` matches after regeneration

## Tests
- [ ] `grep -r "are_tests_passing" builtin/` returns no results
- [ ] `grep -r "js.*set expression" builtin/` returns no results
- [ ] Skill still reads coherently with the step removed