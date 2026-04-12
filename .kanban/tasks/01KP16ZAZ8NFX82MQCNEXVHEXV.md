---
assignees:
- claude-code
position_column: todo
position_ordinal: c28180
title: Fix progress bar rendering 0 despite correct computed data in entity store
---
## What

The progress field on entity cards shows 0 (empty bar) even when the entity store contains the correct computed value — e.g. `{completed: 14, percent: 100, total: 14}`. The clipboard (which reads directly from `entity.fields.progress`) confirms the data is correct; the display is simply not rendering it.

### Observed Behavior
- Entity store has `progress: {completed: 14, percent: 100, total: 14}` (verified via entity.copy clipboard)
- Progress bar renders as 0 / empty
- The `[x]` checkboxes in the body ARE correctly parsed by the backend's `parse-body-progress` derivation

### Verified Working
- Backend `parse_checklist_counts` correctly matches `- [x]` items (3 Rust tests pass)
- Backend `ComputeEngine.derive_all()` correctly produces the progress object
- Frontend entity store contains the correct progress value (clipboard proves it)
- Entity-card progress tests pass (5 tests with mock data in `entity-card.test.tsx`)

### Investigation Focus — the display chain

The data reaches `useFieldValue(\"task\", id, \"progress\")` correctly. The bug is somewhere between that hook and the visual output. Possible causes:

1. **`ProgressDisplay` receives wrong value or type** — add `console.warn` instrumentation to log the actual `value` and `mode` props received by `ProgressDisplay` at `kanban-app/ui/src/components/fields/displays/progress-display.tsx:4`. Compare with what the entity store contains.

2. **Display registration shadowed** — verify only one `registerDisplay(\"progress\", ...)` call exists and it maps to `ProgressDisplay`. Check for duplicate registrations.

3. **CSS/rendering issue** — the bar renders but the fill color matches the background (invisible). Check `bg-primary` vs `bg-muted` in the progress display classes.

4. **Mode mismatch** — entity card passes `mode=\"compact\"` which shows `{percent}%`. If the user sees `0/N` format, that's the \"full\" mode format (`{completed}/{total}`) — check whether the inspector rather than the card is showing the bug.

5. **Snapshot cache returning stale value** — `getFieldValue` in `entity-store-context.tsx:183` uses a `snapshotCache` with `fieldValuesEqual` deep comparison. If the cache holds a stale `{total:0, completed:0, percent:0}` and `fieldValuesEqual` incorrectly matches it against the real value, the stale value would be returned.

### Approach

Run the kanban-app, open the inspector for a card with checkboxes, and instrument `ProgressDisplay` to log what it receives. The gap between what the store has and what the display renders IS the bug.

### Subtasks

- [ ] Add `console.warn` instrumentation to `ProgressDisplay` to log received `value`, `typeof value`, and `mode` — then check via `log show` what arrives at render time
- [ ] Verify no duplicate `registerDisplay(\"progress\", ...)` calls exist
- [ ] If value arrives correctly at ProgressDisplay, check CSS — does `bg-primary` resolve to a visible color in the current theme?
- [ ] If value does NOT arrive correctly, trace from `useFieldValue` → `getFieldValue` → `snapshotCache` to find where it diverges

### Also: enrich entity-created events (separate improvement)

The `enrich_computed_fields` pipeline in `kanban-app/src/commands.rs` only enriches `EntityFieldChanged` events, not `EntityCreated`. This means newly created entities arrive at the frontend without computed fields. This is worth fixing too but is NOT the cause of the current bug (since the clipboard proves the store has correct data).

## Acceptance Criteria

- [ ] Progress bar renders the correct fill percentage when entity store has a valid progress object
- [ ] Progress text shows correct values (e.g. \"100%\" in compact mode, \"14/14\" in full mode)
- [ ] Existing entity-card progress tests continue to pass
- [ ] Root cause identified and documented

## Tests

- [ ] `kanban-app/ui/src/components/entity-card.test.tsx` — existing 5 progress tests still pass
- [ ] Add test with exact production data shape: `progress: {completed: 14, percent: 100, total: 14}` — verify `aria-valuenow=\"100\"`  
- [ ] `npx vitest run` full UI suite passes
- [ ] Manual verification: open kanban board, inspect a card with checkboxes, confirm progress bar matches checkbox state

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.</description>
</invoke>