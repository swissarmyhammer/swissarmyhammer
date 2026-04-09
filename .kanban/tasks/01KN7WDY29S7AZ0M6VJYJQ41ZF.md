---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffaa80
title: Move PerspectiveTabBar inside view components — perspectives are scoped per view, not global
---
## What

PerspectiveTabBar is currently rendered in `App.tsx` at line 591 as a peer to NavBar — above the entire view area. This is wrong. Perspectives are scoped per view kind, so the tab bar belongs **inside** each view component, not at the App level.

### Current (wrong)
```
<NavBar />
<PerspectiveTabBar />           ← App level, peer to toolbar
<div flex>
  <LeftNav />
  <ActiveViewRenderer />       ← renders BoardView or GridView
</div>
```

### Correct
```
<NavBar />
<div flex>
  <LeftNav />
  <BoardView>
    <PerspectiveTabBar />       ← inside the view, scoped to it
    ...board content...
  </BoardView>
</div>
```

### Files to modify
- `kanban-app/ui/src/App.tsx` — Remove `<PerspectiveTabBar />` from App-level layout (line 591). Remove PerspectiveTabBar import.
- `kanban-app/ui/src/components/board-view.tsx` — Import and render `<PerspectiveTabBar />` at the top of BoardView's layout, inside its existing `CommandScopeProvider`.
- `kanban-app/ui/src/components/grid-view.tsx` — Import and render `<PerspectiveTabBar />` at the top of GridView's layout, inside its existing `CommandScopeProvider`.

### Why this matters
- Perspectives filter/sort/group within a specific view kind. They're view-scoped state.
- Placing them in App makes them appear global, outside the view's command scope chain.
- Inside the view, the tab bar naturally participates in the view's `CommandScopeProvider` — so perspective commands resolve with the correct scope.

## Acceptance Criteria
- [ ] `<PerspectiveTabBar />` removed from App.tsx
- [ ] `<PerspectiveTabBar />` rendered inside BoardView, above board content
- [ ] `<PerspectiveTabBar />` rendered inside GridView, above grid content
- [ ] Tab bar visually appears in the same position (above view content, below navbar)
- [ ] All existing perspective-tab-bar tests still pass

## Tests
- [ ] `kanban-app/ui/src/components/perspective-tab-bar.test.tsx` — all existing tests pass
- [ ] `pnpm test` from `kanban-app/ui/` — all pass
- [ ] Visual: tab bar renders above board content, not above the entire layout