---
assignees:
- claude-code
depends_on:
- 01KTCAFH74MPPZ9282P699QBW0
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffa680
project: card-comments
title: Update implement/finish/kanban skills to record a conversation log via add comment
---
## What
Teach the work-the-card skills to record progress as a conversation log on the task using the new `add comment` op. Edit the SKILL.md SOURCE files under `builtin/skills/` (NOT any generated copy — per project convention, generated skill dirs are produced from source).

CONFIRMED: sources are `builtin/skills/{implement,finish,kanban}/SKILL.md`. Generation/deploy is handled by `crates/swissarmyhammer-skills` (`deploy.rs`), which produces the generated copies at `.skills/`, `.sah/skills/`, and `apps/kanban-cli/.skills/`. Do NOT hand-edit any of those three — edit only `builtin/skills/` and let deploy regenerate.

Files:
1. `builtin/skills/implement/SKILL.md` — add a short "Record progress" step in the Process section. Guidance: after moving to `doing` and at meaningful milestones (research done, implementation done, blockers hit, before moving to `review`), call `{"op":"add comment","task_id":"<id>","text":"<what happened>"}`. The author is attributed automatically to the dispatching actor. CONFIRMED there is already a concrete hook to build on: line ~126 "Cannot complete? Do NOT move forward. Comment what happened, report back." — make "Comment what happened" concrete by pointing it at `add comment`.
2. `builtin/skills/finish/SKILL.md` — equivalent guidance so the autonomous loop records a comment per iteration / state transition.
3. `builtin/skills/kanban/SKILL.md` — the same brief guidance for the general "pick up a card" flow. Also suggest calling `list comments` first to review prior context on a card before starting.

Keep additions concise and consistent with each skill's voice. Reference ops by canonical form `add comment` / `list comments`.

After editing sources, run the skills deploy/generation (if a command exists, e.g. via the CLI or a cargo xtask) so the generated copies reflect the change; otherwise note generation happens at build/deploy time and leave generated dirs untouched.

## Acceptance Criteria
- [x] implement, finish, and kanban SKILL.md sources each instruct the agent to record a conversation log via `add comment` at defined milestones.
- [x] Guidance references the canonical op names `add comment` / `list comments`.
- [x] No edits were made directly under any generated dir (`.skills/`, `.sah/skills/`, `apps/kanban-cli/.skills/`) — only `builtin/skills/`.
- [x] If a skills-generation/deploy step is run, generated output reflects the new guidance.

## Tests
- [x] Add/extend a skill-content test in `crates/swissarmyhammer-skills` (search its tests for an existing pattern that loads a builtin skill body via the skill loader) asserting the implement/finish/kanban skill bodies contain an `add comment` instruction. If no harness exists, add a lightweight test that loads the builtin skill markdown through the crate's loader and asserts the substring.
- [x] `cargo nextest run -p swissarmyhammer-skills` — green.

## Workflow
- Use `/tdd` — write the skill-body assertion test first, then edit the SKILL.md sources.

## Implementation Notes (2026-06-12)
- New test: `crates/swissarmyhammer-skills/tests/skill_comment_guidance.rs` (TDD: watched both tests fail RED, then pass GREEN after source edits). Full crate: 118/118 passed.
- Edited only `builtin/skills/{implement,finish,kanban}/SKILL.md`.
- Deploy step run from the tree (`cargo run -p kanban-cli -- init`, project scope): "Deployed 6 skill(s) to claude-code, copilot, zed-ai"; verified `.claude/skills`, `.zed/skills`, `.skills` copies contain the new `add comment` / `list comments` guidance.
- Note: the running kanban MCP server binary predates the comment ops, so `add comment` could not be dogfooded on this card during the session (op rejected as unsupported by the live server; the ops exist in the tree source and schema).