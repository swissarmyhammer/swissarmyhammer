---
assignees:
- claude-code
position_column: todo
position_ordinal: f480
title: Auto-select / activePerspective fallback are kind-only and can select a perspective invisible in the active view's tab bar — route both through perspectiveVisibleInView
---
## What

Pre-existing gap surfaced during review of 01KTY6T1GPY94VYWANE9X41SKJ (iteration-4 nit): perspective *selection* still matches kind-only while perspective *visibility* (tab bar + auto-create guard) is view_id-first. The selected/active perspective can therefore be one pinned to a sibling view — i.e. a tab the user cannot see in the active view's bar.

## Exact locations (`apps/kanban-app/ui/src/lib/perspective-context.tsx`)

1. `useAutoSelectActivePerspective` (hook defined around line 208): line 218 — `const matching = perspectives.filter((p) => p.view === viewKind);`. All three repair paths (invalid/empty stored id → switch to `matching[0]`; valid-id stale-filter redispatch; validity check `matching.some((p) => p.id === active_perspective_id)`) operate on this kind-only set, so `perspective.switch` can be dispatched for a perspective pinned (`view_id`) to a different view of the same kind.
2. `activePerspective` memo in `PerspectiveProvider` (lines 354–360): the fallback `perspectives.find((p) => p.id === active_perspective_id) ?? perspectives[0] ?? null` isn't even kind-filtered — `perspectives[0]` can be any kind AND any pinned view.

## Symptom

A perspective that is invisible in the active view's tab bar (pinned via `view_id` to a sibling view, filtered out by the bar's `filteredPerspectives` memo in `perspective-tab-bar.tsx`) can be auto-selected as active or fall back as `activePerspective`. The board then filters/groups by a perspective the user has no visible tab for — no highlighted tab, no way to see or switch off it from the bar.

## Fix direction

Reuse the shared predicate `perspectiveVisibleInView(p, activeViewId, viewKind)` exported from `apps/kanban-app/ui/src/types/kanban.ts` (line 98) — the single source of truth already shared by the tab bar's filter (`perspective-tab-bar.tsx`) and `useAutoCreateDefaultPerspective`:

- In `useAutoSelectActivePerspective`, build `matching` with `perspectiveVisibleInView` instead of `p.view === viewKind` (requires threading `activeViewId` into the hook, same as was done for `useAutoCreateDefaultPerspective` in iteration 4).
- In the `activePerspective` memo, constrain the `perspectives[0]` fallback (and arguably the `find`) to visible perspectives via the same predicate.

## Tests

- Vitest in `apps/kanban-app/ui/src/lib/perspective-context.test.tsx` (existing mock/harness patterns): a perspective pinned to a sibling view must NOT be auto-selected nor returned as the `activePerspective` fallback; a visible one must win. TDD red-check: revert the predicate to kind-only and confirm the new tests fail.

## Constraints

- Scoped vitest + `npx tsc --noEmit` in apps/kanban-app/ui only; no workspace builds.