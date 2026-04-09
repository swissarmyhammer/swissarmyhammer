---
assignees:
- claude-code
depends_on:
- 01KNHCC6VZW17D66ED224GS1A5
position_column: done
position_ordinal: ffffffffffffffffffffe180
title: 'Frontend: single code path for entity-field-changed — patch from changes array only'
---
## What

Collapse the three-path `entity-field-changed` handler in `rust-engine-container.tsx` into a single path: patch individual fields from the `changes` array. Remove the `fields` full-replacement path and the `get_entity` re-fetch fallback.

### Architecture rule (from memory: event-architecture)

The frontend patches individual fields from the event's `changes` array. Never falls through to a `get_entity` re-fetch. The only `get_entity` call is for `entity-created` (new entities).

### Current problem (3 paths)

In `kanban-app/ui/src/components/rust-engine-container.tsx` (lines 300-341):

1. **Path 1** (line 300): `if (fields)` — replaces all entity fields wholesale from enriched payload
2. **Path 2** (line 307): `else if (changes.length > 0)` — patches individual fields from changes array
3. **Path 3** (line 319): `else` — re-fetches entire entity via `invoke(\"get_entity\", ...)`

After Card 1 removes the `fields` option from `EntityFieldChanged`, Path 1 is dead code. Path 3 is the re-fetch we're eliminating.

### Fix

1. Remove the `fields` branch entirely (Path 1)
2. Remove the `get_entity` re-fetch fallback (Path 3)
3. Keep only the `changes` patch path (Path 2) — this is the one correct path
4. If `changes` is empty, log a warning and skip (this means the backend sent a no-op event)
5. Update `EntityFieldChangedEvent` interface to remove `fields` property

### Files to modify

1. **`kanban-app/ui/src/components/rust-engine-container.tsx`** (entity-field-changed listener, ~line 279)
   - Remove `if (fields)` branch
   - Remove `else` re-fetch branch
   - Keep only `changes` patching logic
   - Add `console.warn` when changes is empty (defensive, should not happen)

2. **`kanban-app/ui/src/components/rust-engine-container.tsx`** (`EntityFieldChangedEvent` interface, ~line 59)
   - Remove `fields?: Record<string, unknown>` property
   - Keep `changes: Array<{ field: string; value: unknown }>`

## Acceptance Criteria

- [ ] `entity-field-changed` handler has exactly ONE code path: patch from changes
- [ ] No `invoke(\"get_entity\", ...)` calls in the `entity-field-changed` handler
- [ ] No `fields` property on `EntityFieldChangedEvent` interface
- [ ] Task drag-and-drop still updates the board (field-level diffs carry position changes)
- [ ] External file edits still propagate (watcher produces FieldChange diffs)
- [ ] `cd kanban-app/ui && npx vitest run` passes

## Tests

- [ ] `kanban-app/ui/src/components/rust-engine-container.test.tsx` — update entity-field-changed tests to only test the changes-patch path
- [ ] `kanban-app/ui/src/components/rust-engine-container.test.tsx` — remove any test that expects full-fields replacement or re-fetch
- [ ] Run `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #events