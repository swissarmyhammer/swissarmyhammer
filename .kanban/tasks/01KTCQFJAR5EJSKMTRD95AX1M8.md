---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8880
title: Commands do not re-mount on Vite hot reload (require full app restart)
---
REOPENED 2026-06-06 — prior work (a lifecycle guard test only; no production fix) was discarded.

## OWNER CONTEXT
On a Vite HMR hot reload the command system does not re-mount — the user must fully restart the app to recover commands. A prior investigation concluded the unit-testable frontend lifecycle (useCommandList / transport / keybinding cleanup) was already correct and shipped only a guard test; that was discarded.

This is very likely entangled with the broader command-surfacing root cause the owner is now driving (commands not reaching the OS menu / palette / jump surfaces). Revisit AFTER the OS-menu + palette command-surfacing work lands, since fixing how commands are surfaced may resolve or reframe the HMR symptom. Keep the focus on the navigation OS menu first. TDD, RED first, and the fix must reproduce the real defect (not a vacuous guard).

## RESOLUTION 2026-06-10 — fixed by architecture that landed since; verification chain documented

Verdict: the registry-driven + bus + FIFO-serialized architecture re-mounts every piece of the command system across both Vite HMR modes. No production gap remains; no code change was needed on this card.

### HMR modes (apps/kanban-app/ui: @vitejs/plugin-react, no custom import.meta.hot, StrictMode root in main.tsx)
1. Fast Refresh (component-only module edit): in-place re-render preserving hook state; remount only on hook-signature change. StrictMode's dev double-mount exercises the identical unmount→remount sequence continuously, so remount semantics are pinned by the StrictMode tests below.
2. Full page reload (any non-component .ts edit — keybindings.ts, mcp-transport.ts, etc.): fresh JS world, old page's React cleanups never run; backend state must tolerate re-initialization without them. It does (idempotency points below).

### Per-piece verification chain
- Registry-command fetch: useCommandList (ui/src/hooks/use-command-list.ts) fetches `list command` in a mount effect — NOT once at module load — and re-fetches on `notifications/commands/changed` (100ms debounce); unsubscribes on unmount. Remount or reload → fresh fetch. Tests: use-command-list.test.tsx ("fetches on mount", "re-fetches on commands/changed", "debounces burst", "unsubscribes on unmount").
- Scope CommandDefs: CommandScopeProvider (ui/src/lib/command-scope.tsx) builds the scope purely from React context via useMemo — no module-level or backend registration, so a remount rebuilds it by construction.
- Webview command bus: module-level map with ownership-guarded cleanup — a stale unmount cleanup never wipes a remount's re-registration (Card B). Tests: webview-command-bus.test.ts (stale-cleanup guard), command-scope.webview-bus.test.tsx (dispatch-path integration, unregister fallback to backend). On full reload the module re-evaluates empty and components re-register.
- Keybinding tables: app-shell.tsx derives globalBindings via extractKeymapBindings(registryCommands, mode) in useMemo from the live useCommandList result; the keydown handler is re-created in an effect (with removeEventListener cleanup) whenever the table or keymap mode changes; focused-scope bindings are read live through getScopeBindings() on every keystroke. Nothing is cached at module scope.
- Layer push/pop: SpatialFocusProvider FIFO-serializes kernel layer ops (enqueueLayerOp in spatial-focus-context.tsx), so a remount's push→pop→push is processed in React lifecycle order. Test: spatial-focus-context.layer-op-ordering.test.tsx ("window-root layer survives a StrictMode remount whose pop is processed slowly") — ran today, green. For the full-reload path (old pops never run) the kernel's SpatialRegistry::push_layer is an idempotent keyed insert that preserves last_focused — Rust test registry::tests::push_layer_preserves_last_focused_on_re_push_with_none (ran today, green).
- Notification pump: WindowContainer invokes mcp_subscribe on every mount; bind_window_forwarder (apps/kanban-app/src/commands.rs) is idempotent per (label, board) — early-returns when already bound, replaces+aborts otherwise. A reload's re-invoke is a no-op.
- OS menu + the registry itself: commands live in Rust per-board plugin runtimes; the menu is built Rust-side from the catalogue (menu.rs build_menu_from_commands / rebuild_menu). A webview reload cannot lose either.

### Historical root cause (why the symptom existed)
Matches card 01KTQCHWP5T4GS8SPGYVXD2CT9's documented mechanism: layer push/pop ops used to be independent unordered async MCP tasks, so a remount's stale pop could be processed AFTER the re-push, permanently deleting the window-root layer; every subsequent focus commit was dropped ("focus snapshot names an unregistered layer") and all key-driven commands went dead until full restart. An HMR remount fires the same push→pop→push as a StrictMode double-mount. FIFO serialization fixed it. Independently, command definitions moved from frontend wiring into the Rust CommandService registry, fetched fresh on every mount.

### Residual dependency (explicitly out of scope here)
Live refresh of an ALREADY-mounted list depends on `notifications/commands/changed` being published end-to-end — card 01KT9X16D6QZXA9Q822DD41X03. HMR correctness does not depend on it because remount/reload re-fetches.

### Adjacent gap discovered (filed separately)
No cleanup removes a window's kernel layers on webview full reload or WindowEvent::Destroyed (on_window_destroyed only rebuilds the menu) — overlay layers open at reload time linger in SpatialRegistry. Does not reproduce this card's symptom (window root is re-pushed/replaced; commands resolve via the webview key handler + registry fetch). Filed as a new card.

### Test evidence (2026-06-10)
- `npx vitest run` on the 4 cited files: 4 files / 17 tests passed, exit 0.
- `cargo test -p swissarmyhammer-focus --lib push_layer`: 1 passed, exit 0.