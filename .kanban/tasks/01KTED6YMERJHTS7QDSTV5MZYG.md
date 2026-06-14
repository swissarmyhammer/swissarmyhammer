---
depends_on:
- 01KTED5F8DQ2XH5BB0WK1MRR3P
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9580
project: ui-command-cleanup
title: Card C — Move grid.* (11) commands to a plugin + handler bus
---
## What
Move the 11 `grid.*` commands defined client-side in `apps/kanban-app/ui/src/components/grid-view.tsx` into a PLUGIN, with webview behaviors routed through the handler bus (Card B).

The 11 ids: `grid.moveToRowStart`, `grid.moveToRowEnd`, `grid.firstCell`, `grid.lastCell`, `grid.edit`, `grid.editEnter`, `grid.exitEdit`, `grid.toggleVisual`, `grid.deleteRow`, `grid.newBelow`, `grid.newAbove`. Their current execute closures manipulate the live grid handle, dispatch `${entityType}.archive` (deleteRow), or `addNewEntity` (newBelow/newAbove) — all WEBVIEW behaviors.

- New plugin `builtin/plugins/grid-commands/index.ts` (mirror `builtin/plugins/file-commands/index.ts`): each id with its `name`/`keys`/`scope` (grid scope) and, where appropriate, `menu` placement. No backend op — mark each "handled in webview".
- In `grid-view.tsx`, replace the client-side command DEFINITIONS with handler registrations via `registerWebviewCommandHandler(id, handler)` (Card B), keyed by the plugin id. The grid component still owns the grid handle and the actual manipulation logic — only the DEFINITION moves to the plugin; the behavior registers as a handler.
- `grid.deleteRow` / `grid.newBelow` / `grid.newAbove` may still dispatch `${entityType}.archive` / add-entity through `useDispatchCommand` from inside their handlers (those targets are already plugin commands) — that is fine; the point is grid.* are no longer DEFINED in React. (Review correction: archive re-dispatches the cross-cutting `entity.archive` with the row's moniker as `target` — no per-type `{type}.archive` exists.)

## Acceptance Criteria
- [x] All 11 `grid.*` ids are defined by `grid-commands` plugin (id/name/keys/scope, menu where applicable); none are defined in grid-view.tsx.
- [x] grid-view.tsx registers 11 webview handlers keyed by the plugin ids; dispatching each id runs the corresponding grid behavior via the bus, not a removed client-side def.
- [x] No regression in grid keyboard navigation/editing behavior.
- [x] GUARD (presentation-only invariant): each handler is pure presentation. Any durable effect (archive, add-entity) routes back through `useDispatchCommand` to a backend-op plugin command — never inline. `grid-view.tsx` must NOT import `@/lib/mcp-transport`. The mechanical guard `apps/kanban-app/ui/src/lib/webview-command-bus.guard.node.test.ts` must stay green.

## Tests
- [x] Plugin e2e (mirror `builtin_ui_commands_e2e.rs`): the grid-commands plugin registers the 11 expected ids with their metadata.
- [x] UI: extend `apps/kanban-app/ui/src/components/grid-view.keyboard-nav.spatial.test.tsx` and `grid-view.spatial-nav.test.tsx` to assert each grid.* id dispatches through the bus to the live grid behavior (edit/exitEdit/newBelow/deleteRow/etc.). Add a focused test that grid.* ids are NOT present in any client-built CommandDef list.
- [x] `webview-command-bus.guard.node.test.ts` is green with grid-view.tsx as a registration site (the guard proves the file reaches durable effects only via `useDispatchCommand`).
- [x] Relevant vitest files green.

## Workflow
- Use `/tdd` — failing tests first, then implement. Automated tests only.

## Review Findings (2026-06-11 07:40)

### Warnings
- [x] `apps/kanban-app/ui/src/components/grid-view.tsx:534` — `grid.deleteRow` re-dispatches `${entityType}.archive` (e.g. `task.archive`), but NO per-type `.archive` command is registered anywhere in production: the plugin catalogue has only the cross-cutting `entity.archive` (`builtin/plugins/entity-commands/index.ts:207`, confirmed by the full-baseline 88-id set), the legacy kanban map (`crates/swissarmyhammer-kanban/src/commands/mod.rs`) likewise has only `entity.archive`, and the dynamic-prefix rewriter (`apps/kanban-app/src/commands.rs:1069-1074`) handles only `board.switch:` / `entity.add:` / `window.focus:` — there is no `{type}.archive` rewrite. In production, Delete Row dispatched from the palette fails in the backend and is swallowed into `console.error("Failed to delete row:")`; the row is never archived. This is a 1:1 carry-over from the retired React def (the card text asserted "those targets are already plugin commands", which is false for archive), so the change does not make production behavior worse — but this card ENSHRINES the dead target: the new test `grid-view.keyboard-nav.spatial.test.tsx:721` pins `task.archive` against a mocked transport (mock-boundary test hides the missing registration), and `builtin/plugins/grid-commands/index.ts:28` documents it as "an existing backend-op command". Suggested fix: re-dispatch `entity.archive` with the row entity's moniker as `target` (it is registered and resolves `from: target`), and update the test + plugin/handler comments to match; alternatively add a `{type}.archive` dynamic rewrite alongside `entity.add:{type}`. **FIXED (red-first): test now pins `entity.archive` with `target: "task:t2"`; handler dispatches `entity.archive` with `entities[row].moniker`; plugin + handler comments updated.**
- [x] `apps/kanban-app/ui/src/components/grid-view.cursor-ring.test.tsx` — the 2 pre-existing failures ("renders exactly one [data-cell-cursor] when focus is on a grid_cell moniker", "clicking a cell sets entity-focus and lights the cursor ring on that cell") are confirmed pre-existing (identical failure set reproduced at HEAD `7c5015141` in a clean worktree), but NO kanban card tracks this file. Card `01KTS1C4EX8W6GZYPAYB1T431K` describes the same symptom family (synthetic `focus-changed` emission not reaching the entity-focus store in browser mode) but enumerates only `focus-scope.test.tsx` (9) + `attachment-display.test.tsx` (1). Suggested fix: extend that card's failing-set enumeration with this file (or create a sibling card) so the failures don't stay orphaned. **TRACKED: sibling card `01KTVB8096XFPVQ47MP82ME7M3` filed with exact test names, error output, repro command, and pre-existing proof at HEAD `7c5015141`.**

### Nits
- [x] `apps/kanban-app/ui/src/components/grid-view.tsx:661-667` — stale doc comment: "Thin orchestrator that delegates layout computation to useGridLayout, keyboard command definitions to useGridCommands, …" still references the `useGridCommands` hook this card retired (and a `useGridLayout` that does not exist). Update the dangling block (it also floats above `GridStatusBar`, not the component it describes) to name `useGridCommandHandlers`. **FIXED: dangling block removed from above `GridStatusBar`; `GridView` now carries an accurate doc naming `useGridData` / `useGridNavigation` / `useGridCommandHandlers` / `useGridCallbacks` / `DataTable`.**