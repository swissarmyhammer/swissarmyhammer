---
assignees:
- claude-code
depends_on:
- 01KN9C5394341SWFR5E65YZV4W
position_column: doing
position_ordinal: '80'
title: Push active perspective into UIState scope chain for command palette visibility
---
## What

8 of 11 perspective commands have `scope: "entity:perspective"` and are invisible in the command palette because the scope chain never contains a `perspective:{id}` moniker. The scope chain is built by React's `CommandScopeProvider` nesting — the perspective needs to be part of that tree.

### Architecture: window > view > perspective > entities

The scope chain should be: `window:{label}` > `view:{viewId}` > `perspective:{perspectiveId}` > entity monikers. The perspective tab bar already renders inside each view. It needs to:

1. **Always have a selected perspective** — even if no user-created perspectives exist, synthesize a default no-op perspective and select it
2. **Wrap the view body in a `CommandScopeProvider`** with `moniker="perspective:{activeId}"` so everything inside (entity cards, command palette, context menus) gets the perspective in their scope chain automatically

### Files to modify

- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — wrap the `children` (view body) in `<CommandScopeProvider moniker={moniker("perspective", activePerspectiveId)}>`. The tab bar must always have an active perspective — if `perspectives` is empty, auto-create a default and select it.
- `kanban-app/ui/src/lib/perspective-context.tsx` — ensure `activePerspectiveId` is never empty. If no perspectives exist, create a default. If the active one is deleted, fall back to another.

### What success looks like

With an active perspective, pressing Cmd+K shows perspective commands (Filter, Clear Filter, Group, Clear Group, Sort, etc.) in the command palette because the scope chain contains `perspective:{id}`.

## Acceptance Criteria

- [ ] `perspective:{id}` moniker present in scope chain for all components inside the view body
- [ ] Scoped perspective commands appear in command palette when a perspective is active
- [ ] Switching perspectives updates the scope chain moniker
- [ ] A default perspective is always selected — never an empty state
- [ ] Right-click context menus on entities inside a view also include perspective-scoped commands

## Tests

- [ ] `perspective-tab-bar.test.tsx` — verify CommandScopeProvider wraps children with perspective moniker
- [ ] `pnpm test` from `kanban-app/ui/` — all pass