---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffff8f80
title: Context menu commands must dispatch through useDispatchCommand, not bypass it
---
## What

Context menu command selections are dispatched entirely on the Rust side (`menu.rs` calls `dispatch_command_internal` directly via `tauri::async_runtime::spawn`). This bypasses `useDispatchCommand` — no busy tracking, no client-side command resolution, a completely separate dispatch path from keybindings/palette/drag.

Meanwhile, **non-context-menu** menu bar commands already do this correctly: `menu.rs` emits `"menu-command"`, `app-shell.tsx` catches it, calls `executeCommand(commandId)` which goes through `useDispatchCommand`. Context menus should follow the same pattern.

### Fix — emit event, dispatch on frontend

**1. `kanban-app/src/menu.rs` — context menu block**
- Instead of calling `dispatch_command_internal` directly, emit a `"context-menu-command"` event with the full `ContextMenuItem` payload (cmd, target, scope_chain)
- This mirrors the existing `"menu-command"` emit but carries richer payload

**2. `kanban-app/ui/src/lib/command-scope.tsx` — `DispatchOptions`**
- Add optional `scopeChain?: string[]` to `DispatchOptions`
- In the dispatch callback, use `opts.scopeChain ?? scopeChainFromScope(effectiveScope)` — explicit chain wins over React context
- This lets the context menu handler pass the right-click-point scope chain rather than whatever happens to be focused when the event arrives

**3. `kanban-app/ui/src/components/app-shell.tsx` — `KeybindingHandler`**
- Add a listener for `"context-menu-command"` alongside the existing `"menu-command"` listener
- Parse the `ContextMenuItem` payload, call `dispatch(item.cmd, { target: item.target, scopeChain: item.scope_chain })`
- This goes through `useDispatchCommand` → `inflightCount` tracking, client-side resolution, the whole bus

### Acceptance Criteria
- [x] Context menu commands dispatch through `useDispatchCommand` (verify via `inflightCount` — progress bar shows during execution)
- [x] Context menu commands carry the right-click scope chain (not the current focus scope)
- [x] `menu.rs` no longer calls `dispatch_command_internal` for context menu items
- [x] Keybinding/palette/drag commands unchanged (no regression)

### Tests
- [x] Add test in `kanban-app/ui/src/components/app-shell.test.tsx`: emit `"context-menu-command"` event with `{ cmd, target, scope_chain }` → assert `invoke("dispatch_command")` is called with matching args
- [x] Add test in `kanban-app/ui/src/lib/command-scope.test.tsx`: dispatch with explicit `scopeChain` in options → assert it overrides the context-derived chain
- [x] Run `pnpm vitest run app-shell command-scope` — all pass

#bug

## Review Findings (2026-04-11 10:09)

### Nits
- [x] `kanban-app/src/commands.rs` — The doc comment on `show_context_menu` (near line 2338) still says "dispatches directly via `dispatch_command_internal` — no round-trip to the frontend." This is now factually wrong: `handle_menu_event` emits a `context-menu-command` event that round-trips through the frontend's `useDispatchCommand`. The inline comment at line 2355 ("dispatches directly — no lookup table needed") is fine for describing the JSON encoding strategy, but the function-level doc should be updated to reflect the new event-based flow. Suggested fix: change the doc to say something like "When the user selects an item, `handle_menu_event` emits a `context-menu-command` event so the frontend routes it through `useDispatchCommand`."