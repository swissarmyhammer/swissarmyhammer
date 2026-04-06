---
assignees:
- claude-code
position_column: todo
position_ordinal: 8f80
title: 'Fix: tag cut/untag not working from body #tag pills or tags badge-list'
---
## What

The `task.untag` / `entity.cut` commands for tags are not visibly working. The user expects:
1. Click or right-click a `#tag` pill in the **body** field → "Remove Tag" option → tag vanishes from body text
2. Click or right-click a `#tag` pill in the **tags badge-list** field → same behavior

### Root Cause

Two issues:

**Issue 1: Body markdown pills don't pass `taskId` to MentionPill**

In `kanban-app/ui/src/components/fields/displays/markdown-display.tsx:96-109`, the `mentionComponents` factory creates `MentionPill` instances for `#tag` mentions in the body, but does **not** pass `taskId`. Without `taskId`, `MentionPill` (line 106 of `mention-pill.tsx`) skips building the `task.untag` extra command, so there's no "Remove Tag" in the context menu.

Fix: Thread the `entity` (or at least `entity.id` when `entity.entity_type === "task"`) from `MarkdownDisplay` → `MarkdownFull` → the pill component factory so each tag pill gets `taskId={entity.id}`.

**Issue 2: Verify badge-list tag pills dispatch correctly**

In `kanban-app/ui/src/components/fields/displays/badge-list-display.tsx:135`, `taskId` IS passed (`taskId={isComputedSlug ? entity.id : undefined}`). The `task.untag` command is registered in the backend (`swissarmyhammer-kanban/src/commands/mod.rs:50`). However, after the backend executes `UntagTask` (which removes `#tag` from the body text), the UI may not be re-rendering because the entity store isn't getting the updated entity. Need to verify the event flow: command dispatch → entity write → store update → re-render.

### Files to modify

1. `kanban-app/ui/src/components/fields/displays/markdown-display.tsx` — Pass `entity` through to `MarkdownFull`, then pass `taskId` to each tag `MentionPill`
2. Possibly `kanban-app/ui/src/components/fields/displays/badge-list-display.tsx` — Verify the entity.cut scope chain works after untag

## Acceptance Criteria

- [ ] Right-clicking a `#tag` pill in the **body** markdown field shows "Remove Tag" in the context menu
- [ ] Executing "Remove Tag" on a body `#tag` pill removes `#tag` from the body text and the tag disappears from the rendered view
- [ ] Right-clicking a `#tag` pill in the **tags badge-list** shows "Remove Tag" in the context menu
- [ ] Executing "Remove Tag" on a badge-list tag pill removes the tag from the task (body updated, tag gone from badge list)
- [ ] The `entity.cut` keybinding (Ctrl/Cmd+X) works when a tag pill is focused in either location

## Tests

- [ ] `kanban-app/ui/src/components/fields/displays/markdown-display.test.tsx` — Add test: when entity is a task, body `#tag` pills render with `taskId` prop, enabling `task.untag` in context menu
- [ ] `kanban-app/ui/src/components/mention-pill.test.tsx` — Existing tests cover `task.untag` presence/absence — verify they still pass
- [ ] `kanban-app/ui/src/components/fields/displays/badge-list-display.test.tsx` — Add test: tag pills in computed-slug mode pass `taskId` and have "Remove Tag" in context menu
- [ ] Run `cd kanban-app/ui && npx vitest run` — all tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.