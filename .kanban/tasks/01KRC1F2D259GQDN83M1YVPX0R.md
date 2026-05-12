---
assignees:
- claude-code
depends_on:
- 01KRC1C93CD73746F4C0Q2PP86
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffd280
title: Switch frontend perspective tab bar filter to view_id with kind fallback
---
## What

Make the formula-bar perspective tabs show **only** the perspectives scoped to the currently active view (by id), with a documented fallback so legacy kind-scoped perspectives still appear. Newly created perspectives must be saved with the active view's id so they don't leak to sibling views of the same kind.

### Files modified

- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — replaced the kind-only filter in `usePerspectiveTabBar` with view_id-first / kind-fallback. Updated `AddPerspectiveButton` to take a `viewId` prop and pass `view_id` alongside `view` on `perspective.save`.
- `kanban-app/ui/src/components/perspective-tab-bar.view-id-scoping.test.tsx` — new regression test file covering view_id scoping + creation path.

### Behavior

- Two grid-kind views with different ids show **different** perspective tab sets in the formula bar.
- Existing perspectives loaded from disk that lack `view_id` continue to appear across every view of their kind — the legacy compat rule from the data-model task is honored on the client too.
- New perspectives created from a specific view are pinned to that view's id and do not appear when switching to a sibling view of the same kind.

### Out of scope

- Backend filter changes — covered by "Switch backend perspective filters to view_id with kind fallback" (parallel task).
- Migrating existing `.kanban/perspectives/*.yaml` files — see "Migrate existing perspective YAMLs to carry view_id where unambiguous".
- The `useAutoCreateDefaultPerspective` auto-create path in `perspective-context.tsx` was intentionally left untouched — the task scope is "from this component" (PerspectiveTabBar). If the auto-create path needs view_id propagation, that's a follow-up.

## Acceptance Criteria

- [x] `perspective-tab-bar.tsx` filters by `view_id` when set, and falls back to `view` (kind) otherwise.
- [x] New perspective creation paths pass `view_id` derived from the currently active view's id.
- [x] Switching between two `kind: "grid"` views with different ids shows two distinct perspective tab sets in the formula bar — verified by a regression test.
- [x] Existing perspectives without `view_id` still appear in every view whose kind matches — verified by a regression test.
- [x] `npx vitest run perspective-tab-bar` is green (75/75 across 12 files).

## Tests

- [x] New regression test file: `perspective-tab-bar.view-id-scoping.test.tsx` covering:
  - active view-a shows P1 (view_id match), not P2.
  - active view-b shows P2 (view_id match), not P1.
  - legacy P3 (no view_id) appears in both grid views via kind fallback.
  - legacy grid-kind P3 does NOT appear in the board view (kind fallback respects kind boundaries).
  - view_id-scoped P1 does NOT appear in the board view.
  - "+" on view-a dispatches `perspective.save` with `view_id: "view-a"`.
  - "+" on view-b dispatches `perspective.save` with `view_id: "view-b"`.
- [x] Run: `npx vitest run perspective-tab-bar` — green (75/75).
- [x] Run: `npm test` (full UI suite, includes tsc) — green (2107/2107 across 221 files).

## Workflow

- Used /tdd — wrote the regression tests first, watched 4 fail (RED), then swapped the filter + creation-path argument and watched them pass (GREEN).
- Matched the existing test harness in `perspective-tab-bar.test.tsx` (same mock pattern for `usePerspectives`, `useViews`, schema, entity store, UI state) — did not invent a new mount helper. #perspective-view-id