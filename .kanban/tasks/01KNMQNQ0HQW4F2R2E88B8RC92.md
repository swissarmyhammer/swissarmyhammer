---
assignees:
- claude-code
position_column: todo
position_ordinal: a280
title: Context menu commands must dispatch through useDispatchCommand, not bypass it
---
## What

Context menu command selections are dispatched entirely on the Rust side (`menu.rs:378-393` calls `dispatch_command_internal` directly via `tauri::async_runtime::spawn`). This bypasses `useDispatchCommand` — no busy tracking, no client-side command resolution, a completely separate dispatch path from keybindings/palette/drag.

Meanwhile, **non-context-menu** menu bar commands already do this correctly: `menu.rs:400` emits `\"menu-command\"`, `app-shell.tsx:65` catches it, calls `executeCommand(commandId)` which goes through `useDispatchCommand`. Context menus should follow the same pattern.

### Fix — emit event, dispatch on frontend

**1. `kanban-app/src/menu.rs` — context menu block (~line 373-396)**
- Instead of calling `dispatch_command_internal` directly, emit a `\"context-menu-command\"` event with the full `ContextMenuItem` payload (cmd, target, scope_chain)
- This mirrors the existing `\"menu-command\"` emit at line 400 but carries richer payload

**2. `kanban-app/ui/src/lib/command-scope.tsx` — `DispatchOptions` (~line 293-299)**
- Add optional `scopeChain?: string[]` to `DispatchOptions`
- In the dispatch callback (~line 346), use `opts.scopeChain ?? scopeChainFromScope(effectiveScope)` — explicit chain wins over React context
- This lets the context menu handler pass the right-click-point scope chain rather than whatever happens to be focused when the event arrives

**3. `kanban-app/ui/src/components/app-shell.tsx` — `KeybindingHandler` (~line 60-75)**
- Add a listener for `\"context-menu-command\"` alongside the existing `\"menu-command\"` listener
- Parse the `ContextMenuItem` payload, call `dispatch(item.cmd, { target: item.target, scopeChain: item.scope_chain })`
- This goes through `useDispatchCommand` → `inflightCount` tracking, client-side resolution, the whole bus

### Why not events

Adding `command-started`/`command-finished` events would create a parallel busy-tracking system. The real fix is to have one dispatch path — `useDispatchCommand` — for all command execution. The context menu should feed into it, not around it.

## Acceptance Criteria
- [ ] Context menu commands dispatch through `useDispatchCommand` (verify via `inflightCount` — progress bar shows during execution)
- [ ] Context menu commands carry the right-click scope chain (not the current focus scope)
- [ ] `menu.rs` no longer calls `dispatch_command_internal` for context menu items
- [ ] Keybinding/palette/drag commands unchanged (no regression)

## Tests
- [ ] Add test in `kanban-app/ui/src/components/app-shell.test.tsx`: emit `\"context-menu-command\"` event with `{ cmd, target, scope_chain }` → assert `invoke(\"dispatch_command\")` is called with matching args
- [ ] Add test in `kanban-app/ui/src/lib/command-scope.test.tsx`: dispatch with explicit `scopeChain` in options → assert it overrides the context-derived chain
- [ ] Run `pnpm vitest run app-shell command-scope` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#bug