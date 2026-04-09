---
assignees:
- claude-code
position_column: todo
position_ordinal: ac80
title: Wire perspective filter to task list_entities so filtering actually works
---
## What

The filter bar dispatches `perspective.filter` which persists the filter string
on the perspective entity (Rust side works), and `refreshBoards` accepts a
`taskFilter` param that passes it to `list_entities` which calls
`apply_filter` using chumsky — but **nobody ever passes the filter**.

`refreshEntities` in `rust-engine-container.tsx` calls `refreshBoards(boardPath)`
without the `taskFilter` argument. When a perspective's filter field changes
via `entity-field-changed`, `PerspectiveProvider` re-fetches the perspective
list (so the UI knows the filter string changed), but no task re-fetch with
the new filter is triggered.

### Fix

1. **`rust-engine-container.tsx`** — `refreshEntities` needs to accept an
   optional `taskFilter` parameter and pass it through to `refreshBoards`.
   Alternatively, expose a `refreshTasksWithFilter(filter)` method.

2. **The component tree needs to re-fetch tasks when `activePerspective.filter`
   changes.** The right place is likely a `useEffect` in `window-container.tsx`
   (or a new hook) that watches `activePerspective.filter` and calls
   `refreshEntities(boardPath, filter)` when it changes. The `PerspectiveContainer`
   already has access to `activePerspective`, but it renders inside
   `RustEngineContainer`, so it can access `useRefreshEntities`.

3. **`refresh.ts`** — `refreshBoards` already accepts `taskFilter` and wires it
   to `list_entities`. No change needed here.

### Files to modify

- `kanban-app/ui/src/components/rust-engine-container.tsx` — add `taskFilter`
  param to `refreshEntities`, pass to `refreshBoards`
- `kanban-app/ui/src/components/window-container.tsx` or
  `kanban-app/ui/src/components/perspective-container.tsx` — add `useEffect`
  that watches `activePerspective?.filter` and triggers task re-fetch

### Critical detail

The event-driven entity-field-changed handler for perspective entities only
updates the perspective list in `PerspectiveProvider`. It does NOT re-fetch
tasks. The `entity-field-changed` handler in `RustEngineContainer` patches
individual entity fields — but a perspective filter change means the *task*
set needs to be re-queried, not patched.

## Acceptance Criteria

- [ ] Entering a valid filter (e.g. `#bug`) in the filter bar and pressing Enter
      causes the board to show only matching tasks
- [ ] Clearing the filter (× button or empty submit) restores the full task list
- [ ] Switching perspectives with different filters applies the correct filter
- [ ] Invalid filters do not crash — backend returns an error string (error
      display is a separate card)

## Tests

- [ ] `kanban-app/ui/src/lib/refresh.test.ts` — add test that `refreshBoards`
      passes `taskFilter` to `list_entities` invoke call
- [ ] `kanban-app/ui/src/components/rust-engine-container.test.tsx` — add test
      that `refreshEntities(boardPath, filter)` passes filter through
- [ ] Integration: verify that after `perspective.filter` command, a subsequent
      `refreshEntities` call includes the filter in `list_entities`
- [ ] Run: `cd kanban-app/ui && npx vitest run src/lib/refresh.test.ts src/components/rust-engine-container.test.tsx`

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.