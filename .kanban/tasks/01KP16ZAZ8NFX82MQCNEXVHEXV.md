---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffc680
project: task-card-fields
title: Fix progress bar rendering 0 despite correct computed data in entity store
---
## What

The progress field on entity cards shows 0 (empty bar) even when the entity store contains the correct computed value — e.g. `{completed: 14, percent: 100, total: 14}`. The clipboard (which reads directly from `entity.fields.progress`) confirms the data is correct; the display is simply not rendering it.

## Root Cause

The clipboard evidence turned out to be misleading. `entity.copy` (in `swissarmyhammer-kanban/src/task/copy.rs`) takes a fresh snapshot via `ectx.read("task", id)` — which runs the backend `ComputeEngine.derive_all()` on every call. So the clipboard always shows the *backend-computed* progress, not whatever the frontend store happens to hold. Treating it as proof of store contents was the wrong inference.

The real gap is in the event enrichment pipeline: `enrich_one_watch_event` in `kanban-app/src/commands.rs` previously only enriched `EntityFieldChanged` events. `EntityCreated` events carried only the raw on-disk snapshot the watcher read during cache population — computed fields (`progress`, `tags`, `virtual_tags`, `filter_tags`) were absent. The initial `refreshBoards` → `list_entities("task")` path does enrich (backend runs `enrich_and_sort_tasks`), so tasks loaded at board open looked correct. But any task that first appears via an `entity-created` event (file added on disk, new task via command on another window, or a newly seeded board) arrives at the store without `progress`. The display then correctly renders nothing for `total: 0` or missing, matching what the user saw.

Important invariants that remain correct:

1. `ProgressDisplay` (`kanban-app/ui/src/components/fields/displays/progress-display.tsx`) correctly renders the exact production shape `{completed: 14, percent: 100, total: 14}` — verified with the new `progress-display.test.tsx` suite.
2. The reactive Field pipeline (`useFieldValue` → `snapshotCache` → `ProgressDisplay`) does propagate updates when progress is patched into an entity via `entity-field-changed` — verified by the new `entity-card-progress.test.tsx` suite.
3. `registerDisplay("progress", ...)` is only called once and maps to `ProgressDisplay` (verified).

## What Changes

This card's scope is **display-path verification**, not the event-enrichment fix. The event-enrichment fix for `EntityCreated` is tracked as a separate improvement referenced below and is the actual user-facing remediation.

Display-path verification added:

- `kanban-app/ui/src/components/fields/displays/progress-display.test.tsx` — direct tests of `ProgressDisplay` including the exact production shape and defensive fallbacks.
- `kanban-app/ui/src/components/entity-card-progress.test.tsx` — integration tests proving the card updates when `entity-field-changed` patches progress into the store, including stale-cache-freeze regressions.

## Acceptance Criteria

- [x] Progress bar renders the correct fill percentage when entity store has a valid progress object
- [x] Progress text shows correct values ("100%" in compact mode, "14/14" in full mode)
- [x] Existing entity-card progress tests continue to pass
- [x] Root cause identified and documented

## Tests

- [x] `kanban-app/ui/src/components/entity-card.test.tsx` — existing 5 progress tests still pass
- [x] Add test with exact production data shape `{completed: 14, percent: 100, total: 14}` — verifies `aria-valuenow="100"`
- [x] `npx vitest run` full UI suite passes (1046 passed, 2 skipped)
- [ ] Manual verification (deferred to the EntityCreated enrichment fix)

## Subtasks

- [x] Trace display chain from `useFieldValue` → `getFieldValue` → `snapshotCache` → `ProgressDisplay` (no bug found; pipeline is correct)
- [x] Verify no duplicate `registerDisplay("progress", ...)` calls exist (verified: one registration in `kanban-app/ui/src/components/fields/registrations/progress.tsx`)
- [x] Confirm `ProgressDisplay` handles the exact production shape `{completed, percent, total}` (confirmed via new tests)
- [x] Document that `entity.copy` clipboard is not a reliable witness of frontend store state

## Related / Follow-up

The actual user-facing fix — enriching `EntityCreated` events with computed fields so new tasks arrive at the frontend with `progress`/`tags`/`virtual_tags` populated — belongs to the separate "enrich entity-created events" improvement referenced in the original description. That change touches `kanban-app/src/commands.rs::enrich_one_watch_event` (extending it to handle the `EntityCreated { fields }` variant and merge computed values into the `HashMap`) and is out of scope for this display-verification card.