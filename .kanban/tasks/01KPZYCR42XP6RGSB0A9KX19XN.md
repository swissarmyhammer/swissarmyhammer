---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe680
project: skills-guide-review
title: Add user trigger phrases to `kanban` skill description
---
## What

Current description of `builtin/skills/kanban/SKILL.md`:

> Execute the next task from the kanban board. Use when the user wants to make progress on planned work by implementing the next available todo item.

No explicit user trigger phrases. Guide requires specific phrases a user would say.

## Acceptance Criteria

- [x] Description includes phrases such as "kanban", "next task", "pick up work", "/kanban".
- [x] Under 1024 chars, no `<`/`>`.

## Tests

- [x] Trigger test: "what's the next task?" -> loads `kanban`.
- [x] Trigger test: "work the board" -> loads `kanban`.

## Reference

Anthropic guide, Chapter 2 — "The description field".

## Resolution

Updated the `description:` field in `builtin/skills/kanban/SKILL.md` (source of truth) to add explicit user trigger phrases:

> Execute the next task from the kanban board. Use when the user says "kanban", "/kanban", "next task", "what's the next task", or "pick up work". Picks up the next ready task from the board and drives it through doing to review.

Final length: 227 chars. No `<`/`>`. Trigger phrases "kanban", "/kanban", "next task", "what's the next task" (covers the first trigger test verbatim), and "pick up work" are all present. "work the board" was intentionally dropped per the review-finding resolution below.

Did not touch the H1 heading — that is covered by a separate task.

## Review Findings (2026-04-24 10:09)

### Warnings
- [x] `builtin/skills/kanban/SKILL.md:3` — The trigger phrase `"work the board"` collides verbatim with `builtin/skills/finish/SKILL.md:3`, which already claims the same phrase for a different workflow (`finish` loops implement->test->review across a batch; `kanban` picks up a single task and stops at review). A user who says "work the board" will get nondeterministic skill selection between two skills with different behavior. Either (a) drop `"work the board"` from the kanban description and rely on "kanban", "/kanban", "next task", "what's the next task", "pick up work" — which are all kanban-unique — or (b) coordinate with the `finish` skill's description to remove the collision there. Option (a) is the smaller change and preserves `finish`'s existing, presumably-tested trigger set.
  - Resolution: Applied option (a). Dropped `"work the board"` from the kanban description; `finish` retains the phrase. New kanban description lists only kanban-unique triggers: "kanban", "/kanban", "next task", "what's the next task", "pick up work". Trigger test "work the board -> loads kanban" is therefore intentionally no longer expected to match kanban; the collision is resolved by design.

### Nits
- [x] `builtin/skills/kanban/SKILL.md:3` — The description contains "Use when the user..." twice (once introducing the trigger phrases, once introducing the semantic condition). The second sentence ("Use when the user wants to make progress on planned work by implementing the next available todo item without specifying which one.") duplicates intent already conveyed by "Picks up the next ready task from the board and drives it through doing to review." Consider collapsing to a single "Use when..." clause — e.g., drop the trailing sentence — for a tighter description. Non-blocking; current form still reads clearly.
  - Resolution: Dropped the trailing "Use when the user wants to make progress..." sentence. Description now has a single "Use when..." clause. Final length: 227 chars.
- [x] `kanban-cli/.skills/kanban/SKILL.md:3` — The generated copy under `kanban-cli/.skills/` still carries the old description. Per project convention (`.skills/` is generated from `builtin/skills/`), this will be fixed by whatever regenerates the `.skills/` tree; flagging as a nit so it is not lost — ensure the generator is re-run before release so the compiled artifact matches the source of truth.
  - Resolution: Re-ran the project's standard deploy command (`kanban init` from `kanban-cli/`) to regenerate `kanban-cli/.skills/kanban/SKILL.md` from `builtin/skills/kanban/SKILL.md`. Did not hand-edit. The root-level `.skills/kanban/SKILL.md` was also refreshed by running `kanban init` at the repo root. Both generated copies now match the source of truth. #skills-guide