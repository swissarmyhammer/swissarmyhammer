---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffc980
title: 'Refactor view/perspective into container hierarchy: ViewContainer > PerspectiveScope > PerspectiveTabBar + ViewDisplay'
---
## What

The current structure has `PerspectiveTabBar` doing double duty — it's both a tab strip UI and a `CommandScopeProvider` wrapper for the view body. The views (`BoardView`, `GridView`) each embed `PerspectiveTabBar` internally with their content as children. `ActiveViewRenderer` in `App.tsx:707` does the view switching.

Refactor into a clean container hierarchy where each concern has one job:

```
ViewCommandScope               ← existing, provides view.switch commands
  ActiveViewRenderer (rename → ViewContainer)
    PerspectiveScope           ← NEW: CommandScopeProvider with perspective:{id} moniker
      PerspectiveTabBar        ← SIMPLIFIED: pure tab strip UI, no children, no scope wrapping
      ViewDisplay              ← NEW: picks BoardView/GridView based on activeView.kind
```

### Files to modify

- **`kanban-app/ui/src/App.tsx`** — Rename `ActiveViewRenderer` to `ViewContainer`. Move the perspective scope wrapping and view selection logic here. The component becomes:
  1. Reads `activeView` from `useViews()`, `activePerspective` from `usePerspectives()`
  2. Wraps children in `CommandScopeProvider` with `moniker={moniker("perspective", activePerspective.id)}` (the `PerspectiveScope`)
  3. Renders `PerspectiveTabBar` (pure tab UI)
  4. Renders `ViewDisplay` which switches on `activeView.kind` to pick `BoardView` or `GridView`

- **`kanban-app/ui/src/components/perspective-tab-bar.tsx`** — Revert to pure tab UI: remove `children` prop, remove the outer `CommandScopeProvider` wrapper around children. Keep the per-tab `CommandScopeProvider` for right-click context menus on individual tabs.

- **`kanban-app/ui/src/components/board-view.tsx`** — Remove `PerspectiveTabBar` import and usage (line 45, 513). The tab bar is now rendered by `ViewContainer`, not by each view.

- **`kanban-app/ui/src/components/grid-view.tsx`** — Remove `PerspectiveTabBar` import and usage (line 29, 522). Same as above.

- **`kanban-app/ui/src/components/board-view.test.tsx`** — Remove the `PerspectiveTabBar` mock (line 24-26). Views no longer embed the tab bar.

### Key constraint

The scope chain order must be: `window:{label}` > view commands > `perspective:{id}` > view-internal scopes (board, columns, tasks). The perspective scope sits between the view command scope and the board/grid FocusScope.

## Acceptance Criteria

- [ ] `PerspectiveTabBar` is pure UI — no `children` prop, no `CommandScopeProvider` wrapping the view body
- [ ] `perspective:{id}` moniker is in the scope chain for all components inside the view (entity cards, command palette, context menus)
- [ ] `BoardView` and `GridView` do not import or render `PerspectiveTabBar`
- [ ] View switching (board/grid) still works correctly
- [ ] Perspective-scoped commands (filter, group, sort) appear in command palette

## Tests

- [ ] `perspective-tab-bar.test.tsx` — update: no children rendering, just tab UI
- [ ] `board-view.test.tsx` — remove PerspectiveTabBar mock, verify board still renders columns
- [ ] `pnpm vitest run perspective-tab-bar board-view` — all pass
- [ ] `cd kanban-app/ui && pnpm test` — no regressions