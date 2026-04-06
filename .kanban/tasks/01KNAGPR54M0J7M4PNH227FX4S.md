---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffc780
title: Remove inspect special-casing in entity-commands.ts — dispatch like any other command
---
## What

`entity-commands.ts` special-cases `ui.inspect` and `entity.inspect` commands, routing them through a client-side `InspectProvider` React context instead of dispatching to the backend like every other command. This is wrong — `ui.inspect` is a real Rust command that already works end-to-end:

1. Rust `InspectCmd` (`swissarmyhammer-kanban/src/commands/ui_commands.rs:34`) pushes moniker onto `UIState.inspector_stack` → returns `UIStateChange::InspectorStack`
2. Tauri `dispatch_command` handler (`kanban-app/src/commands.rs:1238`) deserializes result as `UIStateChange` → emits `ui-state-changed` event to the correct window
3. `InspectorSyncBridge` (`App.tsx:76`) listens to `ui-state-changed` → reads `inspector_stack` from UIState → updates `panelStack` state

The client-side `InspectProvider`, `useInspect()`, and the `inspectEntity` callback in `App.tsx:167` are redundant plumbing that bypasses the standard command flow.

### CRITICAL: Verify event-driven inspect works before deleting anything

Before removing `InspectProvider`, **prove** the backend→event→bridge path works:

1. In a running app, open the browser devtools console
2. Manually invoke: `window.__TAURI__.core.invoke("dispatch_command", { cmd: "ui.inspect", args: {}, target: "task:SOME_REAL_ID", scopeChain: ["window:main"] })`
3. Verify: the `InspectorSyncBridge` fires, `panelStack` updates, and the inspector panel appears
4. If it does NOT work, fix the bridge before removing `InspectProvider`

The `ui-state-changed` event flow at `kanban-app/src/commands.rs:1238` handles this — it checks `serde_json::from_value::<UIStateChange>(result)` and emits to `app.emit("ui-state-changed", state.ui_state.to_json())`. The `InspectorSyncBridge` at `App.tsx:76` reads `winState?.inspector_stack` from UIState and calls `setPanelStack`. This pipeline is already wired.

### Files to modify

- **`kanban-app/ui/src/lib/entity-commands.ts:73` and `:142`** — Remove the `if (cmd.id === "ui.inspect" || cmd.id === "entity.inspect")` branches. All commands should just call `dispatch(cmd.id, { target: entityMoniker })`.
- **`kanban-app/ui/src/lib/entity-commands.ts`** — Remove the `inspect` parameter from `buildEntityCommandDefs()` (line ~48) and `useEntityCommands()` (line ~128). They no longer need it.
- **`kanban-app/ui/src/lib/inspect-context.tsx`** — Delete this file entirely. `useInspect()`, `useInspectOptional()`, `useInspectDismiss()`, and `InspectProvider` are all dead code after the above changes.
- **`kanban-app/ui/src/App.tsx:167-199`** — Remove `inspectEntity`, `dismissTopPanel`, `dismissAllPanels` callbacks. Remove `InspectProvider` from the JSX tree (line ~585). The `InspectorSyncBridge` already handles everything.
- **`kanban-app/ui/src/App.tsx`** — Remove `useInspect` import and any remaining callers.
- **`kanban-app/ui/src/components/board-view.tsx:80`** — Remove `useInspect()` call, stop passing `inspect` to any child.
- **Any other `useInspect()` callers** — grep for `useInspect` and remove. The command palette and context menus already dispatch `ui.inspect` correctly via `useDispatchCommand`.

### Dismiss handling

`InspectProvider` also provides `useInspectDismiss()`. Check all callers — they should dispatch `ui.inspector.close` via `useDispatchCommand("ui.inspector.close")` instead. The Rust `InspectorCloseCmd` already exists and returns `UIStateChange::InspectorStack`, triggering the same event flow.

## Acceptance Criteria

- [ ] No `if (cmd.id === "ui.inspect")` or `if (cmd.id === "entity.inspect")` anywhere in the frontend
- [ ] `inspect-context.tsx` deleted
- [ ] `InspectProvider` removed from App.tsx JSX tree
- [ ] `useInspect()` not imported anywhere
- [ ] **Clicking "Inspect" on an entity card opens the inspector panel** (via backend dispatch → `ui-state-changed` event → `InspectorSyncBridge` → `panelStack` update)
- [ ] **Closing the inspector** works via `useDispatchCommand("ui.inspector.close")` → same event pipeline
- [ ] Inspector works correctly in multi-window setup (each window gets its own inspector stack via window-scoped UIState)

## Tests

- [ ] Update `kanban-app/ui/src/lib/entity-commands.test.ts` — remove tests for inspect special-case, add test that inspect dispatches like any other command
- [ ] Update `kanban-app/ui/src/App.test.tsx` — remove InspectProvider from test setup
- [ ] Add test: dispatching `ui.inspect` with a target moniker triggers `ui-state-changed` event (Rust integration test in `swissarmyhammer-kanban/src/commands/mod.rs`)
- [ ] Run `cd kanban-app/ui && pnpm test` — all pass
- [ ] Run `cd kanban-app/ui && pnpm tsc --noEmit` — no type errors