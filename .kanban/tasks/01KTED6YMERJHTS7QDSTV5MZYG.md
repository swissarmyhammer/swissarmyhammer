---
depends_on:
- 01KTED5F8DQ2XH5BB0WK1MRR3P
position_column: todo
position_ordinal: d680
project: ui-command-cleanup
title: Card C — Move grid.* (11) commands to a plugin + handler bus
---
## What
Move the 11 `grid.*` commands defined client-side in `apps/kanban-app/ui/src/components/grid-view.tsx` into a PLUGIN, with webview behaviors routed through the handler bus (Card B).

The 11 ids: `grid.moveToRowStart`, `grid.moveToRowEnd`, `grid.firstCell`, `grid.lastCell`, `grid.edit`, `grid.editEnter`, `grid.exitEdit`, `grid.toggleVisual`, `grid.deleteRow`, `grid.newBelow`, `grid.newAbove`. Their current execute closures manipulate the live grid handle, dispatch `${entityType}.archive` (deleteRow), or `addNewEntity` (newBelow/newAbove) — all WEBVIEW behaviors.

- New plugin `builtin/plugins/grid-commands/index.ts` (mirror `builtin/plugins/file-commands/index.ts`): each id with its `name`/`keys`/`scope` (grid scope) and, where appropriate, `menu` placement. No backend op — mark each "handled in webview".
- In `grid-view.tsx`, replace the client-side command DEFINITIONS with handler registrations via `registerWebviewCommandHandler(id, handler)` (Card B), keyed by the plugin id. The grid component still owns the grid handle and the actual manipulation logic — only the DEFINITION moves to the plugin; the behavior registers as a handler.
- `grid.deleteRow` / `grid.newBelow` / `grid.newAbove` may still dispatch `${entityType}.archive` / add-entity through `useDispatchCommand` from inside their handlers (those targets are already plugin commands) — that is fine; the point is grid.* are no longer DEFINED in React.

## Acceptance Criteria
- [ ] All 11 `grid.*` ids are defined by `grid-commands` plugin (id/name/keys/scope, menu where applicable); none are defined in grid-view.tsx.
- [ ] grid-view.tsx registers 11 webview handlers keyed by the plugin ids; dispatching each id runs the corresponding grid behavior via the bus, not a removed client-side def.
- [ ] No regression in grid keyboard navigation/editing behavior.
- [ ] GUARD (presentation-only invariant): each handler is pure presentation. Any durable effect (archive, add-entity) routes back through `useDispatchCommand` to a backend-op plugin command — never inline. `grid-view.tsx` must NOT import `@/lib/mcp-transport`. The mechanical guard `apps/kanban-app/ui/src/lib/webview-command-bus.guard.node.test.ts` must stay green.

## Tests
- [ ] Plugin e2e (mirror `builtin_ui_commands_e2e.rs`): the grid-commands plugin registers the 11 expected ids with their metadata.
- [ ] UI: extend `apps/kanban-app/ui/src/components/grid-view.keyboard-nav.spatial.test.tsx` and `grid-view.spatial-nav.test.tsx` to assert each grid.* id dispatches through the bus to the live grid behavior (edit/exitEdit/newBelow/deleteRow/etc.). Add a focused test that grid.* ids are NOT present in any client-built CommandDef list.
- [ ] `webview-command-bus.guard.node.test.ts` is green with grid-view.tsx as a registration site (the guard proves the file reaches durable effects only via `useDispatchCommand`).
- [ ] Relevant vitest files green.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.