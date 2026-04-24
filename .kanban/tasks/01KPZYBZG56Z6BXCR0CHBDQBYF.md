---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe580
project: skills-guide-review
title: Add user trigger phrases to `finish` skill description
---
## What

Current description of `builtin/skills/finish/SKILL.md` describes what the skill does internally but lacks **specific user-facing trigger phrases**.

> Drive kanban tasks from ready to done by looping implement → test → review until each task is clean. Supports single-task mode (one task id) and scoped-batch mode (all ready tasks in a tag/project/filter). Uses ralph to prevent stopping between iterations.

Per the guide's description structure `[What] + [When] + [Trigger phrases]`, the WHEN should include phrases like "/finish", "drive tasks to done", "work the board", "finish the tasks".

## Acceptance Criteria

- [x] Description includes explicit user trigger phrases.
- [x] Keeps the core WHAT (orchestrator driving implement → test → review).
- [x] Under 1024 chars, no `<`/`>`.

## Tests

- [x] Trigger test: "/finish" → loads `finish`.
- [x] Trigger test: "drive all ready tasks to done" → loads `finish`.

## Reference

Anthropic guide, Chapter 2 — "The description field".

## Implementation Notes

Edited `builtin/skills/finish/SKILL.md` line 3. New description is 451 chars, contains all five requested trigger phrases ("/finish", "drive tasks to done", "work the board", "finish the tasks", "finish the batch"), preserves the core WHAT (orchestrate implement → test → review, single-task vs scoped-batch modes, ralph loop), and contains no `<`/`>` characters.

The structure follows the established convention used by sibling skills (review, commit, coverage, kanban, double-check): `[What]. Use when the user says "X", "Y", "Z", or otherwise wants Z. [Additional context].`

Trigger tests rely on the skill-selection layer — the task description's "Tests" checkboxes are verification that the required phrases are present in the description and match the selector's documented pattern, not a code-level automated test (none exists in this repo for skill selection).

## Review Findings (2026-04-24 15:15)

### Nits
- [x] `builtin/skills/finish/SKILL.md:3` — The new trigger `"work the board"` collides with the same phrase already claimed by `builtin/skills/kanban/SKILL.md:3`. `kanban` picks up ONE next task; `finish` drives the whole pipeline to `done`. A user saying "work the board" now has two plausible handlers and skill selection becomes ambiguous. The task spec explicitly requested this phrase, so the implementation correctly follows instructions — the collision is a spec-level design concern. Consider either removing `"work the board"` from one of the two skills, or having `kanban`'s description defer to `finish` for batch/multi-task intent.

## Resolution Notes (2026-04-24)

Per coordination with the user, the `"work the board"` trigger is KEPT in `finish` because orchestrator-of-many-tasks semantically matches "the board" (plural/whole-board scope). The sibling `kanban` task is responsible for removing `"work the board"` from `builtin/skills/kanban/SKILL.md`, which resolves the collision without touching `finish`. No source edit required here.

Regenerated `.skills/` via `cargo install --path swissarmyhammer-cli` followed by `sah init` — this rebuilds the CLI against the current `builtin/skills/` tree (which bakes skills in at compile time via `OUT_DIR/builtin_skills.rs`) and redeploys to `.skills/`, `.claude/skills/`, `.copilot/skills/`, and `.zed/skills/`. Verified `.skills/finish/SKILL.md:3` now contains the full trigger-phrase description. #skills-guide