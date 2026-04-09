---
assignees:
- claude-code
depends_on:
- 01KNJ6JKAADY7CKZS0KYW2YD7E
- 01KNJ6KG15CNWMCS47PPASYQ51
position_column: done
position_ordinal: ffffffffffffffffffffff8880
title: 'Verify: tag cut/untag works end-to-end after field-moniker fix'
---
## What

Tag cut/untag commands fail silently because field-row monikers like `task:01ABC.body` and `task:01ABC.tags_field` pollute the scope chain, shadowing the real `task:01ABC` moniker.

### Root Cause

When a tag pill is rendered inside a task's inspector, the scope chain walks:
```
tag:bugId → task:01ABC.body → task:01ABC → column:todo → ...
```

The backend's `resolve_entity_id("task")` returns the **first** `task:` moniker, which is `01ABC.body` — a field-row moniker, not a real task ID. So `UntagTask::new("01ABC.body", "bug")` fails because no task with that ID exists.

The same applies to badge-list tag pills (`task:01ABC.tags_field` shadows `task:01ABC`).

### Fix approach

The field-row monikers in `entity-inspector.tsx:267` use format `{entity.moniker}.{field.name}` which produces `task:01ABC.body`. Since `parse_moniker` splits on the first `:`, this matches entity type `task` with id `01ABC.body`.

**Option A (recommended): Change field-row monikers to use a non-entity namespace.** Instead of `task:01ABC.body`, use `field:task:01ABC.body` or `task:01ABC/body` — a format that doesn't parse as entity type `task`. This is the cleanest fix because field rows are not entities.

**Option B: Make `resolve_entity_id` skip monikers with dots.** Fragile — dots could appear in real IDs.

### Files to modify

1. `kanban-app/ui/src/components/entity-inspector.tsx:267` — Change `scopeMoniker` format so field-row FocusScopes don't masquerade as entity monikers. E.g. use `field:task:01ABC.body` or a separator that `parse_moniker` won't match as `task`.
2. Any code that depends on field-row moniker format (navigation predicates in the same file, tests).

## Acceptance Criteria

- [ ] Field-row scope monikers no longer parse as the parent entity type (no `task:` prefix on field-row scopes)
- [ ] Right-clicking a `#tag` pill in the body field shows "Remove Tag" and executing it removes the tag from the body
- [ ] Right-clicking a `#tag` pill in the tags badge-list shows "Remove Tag" and executing it removes the tag
- [ ] `entity.cut` (Ctrl/Cmd+X) works when a tag pill is focused in either location
- [ ] Inspector field navigation (up/down/left/right between fields) still works correctly
- [ ] Inspector edit mode (entering/exiting field editors) still works correctly

## Tests

- [ ] `kanban-app/ui/src/components/entity-inspector.test.tsx` — Verify field-row monikers use new format, update any assertions on moniker strings
- [ ] `kanban-app/ui/src/components/fields/displays/badge-list-nav.test.tsx` — Verify pill navigation still works with new field-row moniker format
- [ ] `kanban-app/ui/src/components/mention-pill.test.tsx` — Existing task.untag tests still pass
- [ ] Run `cd kanban-app/ui && npx vitest run` — all tests pass
- [ ] Manual: right-click a tag pill in inspector body → "Remove Tag" → tag vanishes

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass. #field-moniker-fix