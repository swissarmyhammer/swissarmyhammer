---
assignees:
- claude-code
depends_on:
- 01KN79G8KWH8B0K3SB2CS1EPH1
position_column: todo
position_ordinal: 9b80
title: 3. PerspectiveProvider must follow ViewsProvider pattern — consistent frontend store architecture
---
## What

The PerspectiveProvider (card 1 in the GUI plan) must follow the same pattern as ViewsProvider, not EntityStoreProvider. Both are self-contained list providers. This card establishes the consistency requirement and updates the existing PerspectiveProvider card.

### The pattern (from ViewsProvider)
1. **Self-contained state** — provider manages its own `useState` for the list
2. **Own data fetching** — `invoke("dispatch_command", { cmd: "perspective.list" })` on mount
3. **Own event listeners** — listen for `entity-field-changed`, `entity-created`, `entity-removed` filtered by `entity_type === "perspective"`, plus `board-changed`
4. **Re-fetch on event** — events are signals to re-fetch the full list, not data carriers
5. **Active selection from UIState** — active perspective ID comes from `uiState.windows[label].active_perspective_id` (needs backend UIState field added)
6. **Setter dispatches to backend** — `backendDispatch({ cmd: "ui.perspective.set", args: { perspective_id } })`

### What EntityStoreProvider does differently (NOT the model)
- Receives pre-fetched data via props from App.tsx
- Event handling in App.tsx, not the provider
- Surgical patches via `setEntitiesFor()` updater

### Files to modify
- `kanban-app/ui/src/lib/perspective-context.tsx` — (card 1 creates this) follow ViewsProvider pattern exactly
- `kanban-app/ui/src/lib/ui-state-context.tsx` — add `active_perspective_id` to `WindowStateSnapshot`

### Consistency checklist
- [ ] PerspectiveProvider is stateful (own useState), not prop-driven
- [ ] Fetches via `backendDispatch({ cmd: "perspective.list" })`
- [ ] Listens for `entity_type === "perspective"` events → calls `refresh()`
- [ ] Active perspective from UIState, not local state
- [ ] Setter dispatches backend command
- [ ] `usePerspectives()` hook returns `{ perspectives, activePerspective, setActivePerspectiveId, refresh }`
- [ ] Placed in App.tsx provider tree after ViewsProvider (or alongside)

## Acceptance Criteria
- [ ] PerspectiveProvider follows the exact same architecture as ViewsProvider
- [ ] Event-driven refresh (not enriched events)
- [ ] UIState owns active perspective selection
- [ ] Hook API matches ViewsProvider pattern

## Tests
- [ ] `kanban-app/ui/src/lib/perspective-context.test.tsx` — renders, provides default values, refresh on perspective event, active perspective from UIState
- [ ] `pnpm test` from `kanban-app/ui/` passes