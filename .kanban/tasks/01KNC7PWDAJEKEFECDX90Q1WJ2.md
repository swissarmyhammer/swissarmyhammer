---
assignees:
- claude-code
depends_on:
- 01KNC7PAJ6XHPHNMFM23KSDGZM
position_column: done
position_ordinal: fffffffffffffffffffa80
title: Extract PerspectivesContainer and PerspectiveContainer
---
## What

Extract two containers that manage the perspective system:

1. **PerspectivesContainer** — owns `PerspectiveProvider`, renders `PerspectiveTabBar` as a presenter above the content well. Wraps children.

2. **PerspectiveContainer** — owns the active perspective application (filter/sort/group evaluation). Provides filtered/sorted entities to children via context. Owns `CommandScopeProvider moniker="perspective:{activePerspectiveId}"`.

**Files to create/modify:**
- `kanban-app/ui/src/components/perspectives-container.tsx` (NEW) — wraps `PerspectiveProvider`, renders `PerspectiveTabBar`
- `kanban-app/ui/src/components/perspectives-container.test.tsx` (NEW) — TDD: tests written first
- `kanban-app/ui/src/components/perspective-container.tsx` (NEW) — applies active perspective filter/sort/group, provides results to children
- `kanban-app/ui/src/components/perspective-container.test.tsx` (NEW) — TDD: tests written first
- `kanban-app/ui/src/App.tsx` — remove inline `PerspectiveProvider` wrapping
- `kanban-app/ui/src/components/board-view.tsx` — **REMOVE** `<PerspectiveTabBar />` import and render (line 45, 513) — the tab bar moves up to PerspectivesContainer
- `kanban-app/ui/src/components/grid-view.tsx` — **REMOVE** `<PerspectiveTabBar />` import and render (line 29, 522) — same

**Current state:**
- `PerspectiveProvider` is wrapped directly in App.tsx line 569
- `PerspectiveTabBar` is **currently rendered inside both BoardView (board-view.tsx:513) and GridView (grid-view.tsx:522)** — NOT in App.tsx
- `evaluateFilter` / `evaluateSort` from `perspective-eval.ts` are called inside both `BoardView` and `GridView` directly

**Target layout:**
```
PerspectivesContainer (PerspectiveProvider + tab bar presenter)
  ├── PerspectiveTabBar (tab bar presenter — rendered ONCE here)
  └── PerspectiveContainer (active perspective applied)
       └── [BoardView | GridView]  (receive pre-filtered data, no longer render tab bar)
```

## TDD Process
1. Write `perspectives-container.test.tsx` and `perspective-container.test.tsx` FIRST with failing tests
2. PerspectivesContainer tests: PerspectiveProvider context available, PerspectiveTabBar renders once, children render below tab bar
3. PerspectiveContainer tests: evaluateFilter applied to entities, evaluateSort applied, filtered results provided via context, CommandScopeProvider moniker matches active perspective ID
4. Implement until tests pass
5. Refactor

## Acceptance Criteria
- [ ] `PerspectivesContainer` exists, owns PerspectiveProvider + renders tab bar
- [ ] `perspectives-container.test.tsx` exists with tests written before implementation
- [ ] `PerspectiveContainer` exists, applies filter/sort/group, provides results via context
- [ ] `perspective-container.test.tsx` exists with tests written before implementation
- [ ] PerspectiveTabBar renders between the NavBar and the content well (once, not per-view)
- [ ] `PerspectiveTabBar` import/render removed from both BoardView and GridView
- [ ] Switching perspectives updates the content correctly

## Tests
- [ ] `perspectives-container.test.tsx` — all pass (written first, RED → GREEN)
- [ ] `perspective-container.test.tsx` — all pass (written first, RED → GREEN)
- [ ] Existing `perspective-tab-bar.test.tsx` still passes
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: switch perspectives, apply filters, verify content updates