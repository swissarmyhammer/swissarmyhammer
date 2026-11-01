---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz6ytrctxwppt0yv8ggc5w5q
  text: 'Superseded by ^d525k4k. That card investigated the same three files (plus six symlink siblings under `.claude/skills/` and `.zed/skills/` in `apps/kanban-cli/` and `apps/code-context-cli/`) and decided to untrack them instead of regenerating: `SkillResolver` only ever reads `{git_root}/.skills`, and for every crate in this workspace `{git_root}` is the repository root, not an `apps/*` subdirectory — so these were dead deploy artifacts, never read at runtime, not just drifted ones. All nine paths are now removed from git, `.gitignore` was widened to catch a future accidental commit from any `apps/*-cli/` directory, and a regression test (`crates/swissarmyhammer-skills/tests/no_committed_skill_deploy_artifacts.rs`) fails if one is ever re-added. This card''s acceptance criteria ("no tracked file under any `.skills/` directory mentions `llama-agent`") are trivially satisfied since no such file is tracked anymore. Safe to close.'
  timestamp: 2026-08-04T17:58:34.138031+00:00
position_column: todo
position_ordinal: f380
project: drop-llama-agent
title: Regenerate the deployed .skills/ copies so they lose the llama-agent wording
---
## What

`apps/kanban-cli/.skills/kanban/SKILL.md` still ships this line:

> This is how work is tracked across both Claude Code and llama-agent sessions, so it must be the single source of truth.

The source it is generated from, `builtin/skills/kanban/SKILL.md`, was already
corrected to "This is the single source of truth across every agent session."
Only the deployed copy is stale.

`.skills/` is GENERATED — never edit a file there by hand. Regenerate the
deployed copies from `builtin/skills/` and commit the result.

## Where this came from

Found while finishing `^hm82t0z` (the last card of `drop-llama-agent`). The
drift predates that card: commit 5edda8286 changed the source and left the
deployed copy behind. It is recorded as its own card so the fix is a
regeneration, not a hand edit smuggled into an unrelated diff.

### Subtasks

- [ ] Find the command that deploys `builtin/skills/` into `.skills/`.
- [ ] Regenerate every tracked `.skills/` tree in the repository.
- [ ] Confirm `grep -rn llama .skills/ apps/*/.skills/` is empty.

## Acceptance Criteria

- [ ] No tracked file under any `.skills/` directory mentions `llama-agent`.
- [ ] The deployed copies match their `builtin/skills/` sources byte for byte
      (whatever the deploy step produces), with no hand edits. #bug #cleanup #llama-agent #skills