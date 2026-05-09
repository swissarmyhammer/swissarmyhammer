---
assignees:
- claude-code
position_column: todo
position_ordinal: be80
title: 'Vitest: 4 skipped tests need fixing or deletion'
---
## What

Four tests in the kanban-app/ui vitest suite are skipped (`↓` marker). Skipped tests are not acceptable per the test skill — they must be either fixed or deleted.

## Skipped tests

1. `src/components/focus-scope.test.tsx > FocusScope > useIsFocused ancestor: column gets data-focused when card inside is focused`
2. `src/components/focus-on-click.regression.spatial.test.tsx > focus-on-click regression suite (every component class) > toolbar action > clicking a toolbar action focuses it and renders the indicator — production has no toolbar component today`
   - Comment in title says "production has no toolbar component today" — likely a candidate for deletion
3. `src/components/board-view.spatial-nav.test.tsx > BoardView (spatial-nav) > does not wrap in FocusZone when no SpatialFocusProvider is present`
4. `src/lib/entity-focus.kernel-projection.test.tsx > EntityFocusProvider — kernel-projection invariant > setFocus(moniker) for an unknown moniker leaves the store untouched and logs an error`

## Acceptance Criteria

- [ ] Each skipped test is either un-skipped and passing, or deleted with a justification in the commit message
- [ ] Full `pnpm exec vitest run` shows zero skipped tests

## Context

Discovered while testing focus-debug-overlay (01KQJHE82FPDD1YVN7RW8ZCF3T). Out of scope for that task; filed separately. #test-failure