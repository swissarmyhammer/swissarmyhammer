---
assignees:
- claude-code
depends_on:
- 01KNC7PAJ6XHPHNMFM23KSDGZM
position_column: todo
position_ordinal: '8480'
position_swimlane: container-refactor
title: Extract PerspectivesContainer and PerspectiveContainer
---
## What

Extract two containers that manage the perspective system:

1. **PerspectivesContainer** — owns `PerspectiveProvider`, renders `PerspectiveTabBar` as a presenter above the content well. Wraps children.

2. **PerspectiveContainer** — owns the active perspective application (filter/sort/group evaluation). Provides filtered/sorted entities to children via context. Owns `CommandScopeProvider moniker="perspective:{activePerspectiveId}"`.

**Files to create/modify:**
- `kanban-app/ui/src/components/perspectives-container.tsx` (NEW) — wraps `PerspectiveProvider`, renders `PerspectiveTabBar`
- `kanban-app/ui/src/components/perspective-container.tsx` (NEW) — applies active perspective filter/sort/group, provides results to children
- `kanban-app/ui/src/App.tsx` — remove inline `PerspectiveProvider` wrapping

**Current state:**
- `PerspectiveProvider` is wrapped directly in App.tsx line 569
- `PerspectiveTabBar` is rendered inside `board-view.tsx` (line 46, imported)
- `evaluateFilter` / `evaluateSort` from `perspective-eval.ts` are called inside both `BoardView` and `GridView` directly

**Target layout:**
```
PerspectivesContainer (PerspectiveProvider + tab bar presenter)
  ├── PerspectiveTabBar (tab bar presenter)
  └── PerspectiveContainer (active perspective applied)
       └── [BoardView | GridView]  (receive pre-filtered data)
```

**Key design decision:** The perspective evaluation (filter/sort) currently happens inside each view. Moving it to `PerspectiveContainer` means views receive already-filtered entities, eliminating duplicated filter logic in BoardView and GridView.

## Acceptance Criteria
- [ ] `PerspectivesContainer` exists, owns PerspectiveProvider + renders tab bar
- [ ] `PerspectiveContainer` exists, applies filter/sort/group, provides results via context
- [ ] PerspectiveTabBar renders between the NavBar and the content well
- [ ] Switching perspectives updates the content correctly
- [ ] Filter/sort/group changes in the tab bar popover apply immediately

## Tests
- [ ] Existing `perspective-tab-bar.test.tsx` still passes
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: switch perspectives, apply filters, verify content updates