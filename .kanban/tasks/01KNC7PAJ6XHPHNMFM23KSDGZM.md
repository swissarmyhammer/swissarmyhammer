---
assignees:
- claude-code
depends_on:
- 01KNC7NQA00AZNR027JPJTQKWD
position_column: todo
position_ordinal: '8380'
position_swimlane: container-refactor
title: Extract ViewsContainer and ViewContainer
---
## What

Extract two containers that manage the view system:

1. **ViewsContainer** — owns `ViewsProvider`, renders `LeftNav` as a presenter, wraps children. Owns the `ViewCommandScope` logic (dynamic `view.switch:{id}` commands) that's currently a standalone component in App.tsx lines 673-693.

2. **ViewContainer** — owns the active view routing (`ActiveViewRenderer` logic from App.tsx lines 707-725). Provides the active view to children. Owns a `CommandScopeProvider moniker="view:{activeViewId}"`.

**Files to create/modify:**
- `kanban-app/ui/src/components/views-container.tsx` (NEW) — wraps `ViewsProvider`, includes `ViewCommandScope` commands, renders `LeftNav` as a sidebar presenter
- `kanban-app/ui/src/components/views-container.test.tsx` (NEW) — TDD: tests written first
- `kanban-app/ui/src/components/view-container.tsx` (NEW) — owns active view routing, `CommandScopeProvider moniker="view:{id}"`, renders the correct view component
- `kanban-app/ui/src/components/view-container.test.tsx` (NEW) — TDD: tests written first
- `kanban-app/ui/src/App.tsx` — remove `ViewCommandScope` and `ActiveViewRenderer` components, replace with containers

**Current state:**
- `ViewCommandScope` (App.tsx:673-693): Generates `view.switch:{id}` command defs from the views list and wraps children in a CommandScopeProvider
- `ActiveViewRenderer` (App.tsx:707-725): Reads `activeView.kind` and renders `BoardView`, `GridView`, or placeholder
- The layout div (App.tsx:571-592) that creates the `LeftNav + content` flex layout is inline in App

**Target layout:**
```
ViewsContainer (ViewsProvider + view.switch commands)
  ├── LeftNav (sidebar presenter)
  └── ViewContainer (active view routing)
       └── PerspectivesContainer > PerspectiveContainer > [BoardView | GridView]
```

## TDD Process
1. Write `views-container.test.tsx` and `view-container.test.tsx` FIRST with failing tests
2. ViewsContainer tests: view.switch commands are registered, LeftNav renders, ViewsProvider context is available
3. ViewContainer tests: renders BoardView when activeView.kind === "board", renders GridView when "grid", renders placeholder for unknown, CommandScopeProvider moniker matches active view ID
4. Implement until tests pass
5. Refactor

## Acceptance Criteria
- [ ] `ViewsContainer` exists, owns ViewsProvider + view.switch commands
- [ ] `views-container.test.tsx` exists with tests written before implementation
- [ ] `ViewContainer` exists, owns view routing + CommandScopeProvider
- [ ] `view-container.test.tsx` exists with tests written before implementation
- [ ] LeftNav renders correctly as a sidebar
- [ ] View switching works (clicking icons in LeftNav)
- [ ] `ViewCommandScope` and `ActiveViewRenderer` removed from App.tsx

## Tests
- [ ] `views-container.test.tsx` — all pass (written first, RED → GREEN)
- [ ] `view-container.test.tsx` — all pass (written first, RED → GREEN)
- [ ] Run `cd kanban-app && pnpm vitest run` — all tests pass
- [ ] Manual: switch between Board and Grid views via LeftNav