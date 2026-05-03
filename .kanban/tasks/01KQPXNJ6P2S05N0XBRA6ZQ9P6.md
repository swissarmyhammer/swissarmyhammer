---
assignees:
- claude-code
position_column: todo
position_ordinal: c980
title: Reject unknown moniker in setFocus instead of writing to store
---
## What

The vitest test `setFocus(moniker) for an unknown moniker leaves the store untouched and logs an error` in `kanban-app/ui/src/lib/entity-focus.kernel-projection.test.tsx:229` is currently `it.skip`. When unskipped, it fails:

```
AssertionError: store must remain at its previous value when the kernel rejects the moniker:
  expected 'task:does-not-exist' to be 'task:known'
```

The React adapter's `setFocus(asFq("task:does-not-exist"))` writes the new value into the store optimistically, even though the kernel simulator never emits a `focus-changed` event (mirroring the kernel's `tracing::error!` rejection path). Production behavior should be: kernel rejects → `console.error` is logged → store stays at the previous focus.

## Where

- Test (currently skipped): `kanban-app/ui/src/lib/entity-focus.kernel-projection.test.tsx:229`
- Likely fix site: the `setFocus` reducer / dispatch path in the entity-focus React adapter that calls `spatial_focus_by_moniker`. The store update needs to be gated on the kernel `Ok(_)` response (or driven solely by the `focus-changed` event), not applied unconditionally before the await resolves.

## Acceptance Criteria

- Remove `.skip` from the test in `entity-focus.kernel-projection.test.tsx:229`.
- `pnpm vitest run` reports zero skipped tests in that file.
- `setFocus` for an unknown moniker leaves `focusedFq` at its previous value and surfaces a `console.error`.
- The known-moniker happy path remains green (other 5 tests in the same file still pass).

## What was tried

Unskipping the test in isolation reproduces the failure deterministically (single run, 101 ms). The kernel simulator at `installKernelSimulator` correctly mirrors the kernel's no-op for unknown monikers — the bug is on the React side which optimistically updates the store before awaiting the kernel result.

## Notes

This was discovered while running the full test suite to verify the spatial-nav refactor that removed `FocusActions.broadcastNavCommand` and migrated `board-view.tsx` to direct `spatialActions.navigate` calls. The refactor itself is green; this skipped test is pre-existing and predates the broadcast removal (it dates to the FQM Layer 2b path-monikers migration, commit 7169b4519). #test-failure