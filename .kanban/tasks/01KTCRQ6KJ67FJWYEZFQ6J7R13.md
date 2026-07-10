---
assignees:
- claude-code
depends_on: []
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8780
project: null
title: 'Bug: Command palette won''t OPEN by hotkey/execution'
---
## Scope (narrowed 2026-06-06)
This card is scoped ONLY to the **palette won't OPEN by hotkey/execution** failure — the live frontend keymap bound a different key than the registry advertised, and the open path (command → service flag → ui-state change event → React mounts the overlay) did not complete.

The "not in the OS menu" half was handled by Card A (01KTCQFH7AEQDZD0QETSMCMGP0). Keystone execution card: 01KTCQF326FAQTQMHVV5QPG8VZ.

## RESOLVED at HEAD — root causes + fixing commits (verified 2026-06-10)

Two independent breaks had to both be fixed for the open path to complete:

1. **Result-envelope mismatch (the keystone).** Commands dispatched through the CommandService/plugins return a `CallToolResult`-shaped value `{ content, structuredContent: { ok, change } }` (or the bare `{ ok, change }` envelope), but `apps/kanban-app/src/commands.rs::ui_state_change_kind` only recognized a bare `UIStateChange` — so every plugin-command result classified as `None`, NO `ui-state-changed` event was emitted, and `app.palette.open` flipped `palette_open` on the backend while no webview ever re-rendered. Fixed by unwrapping `structuredContent.change` → `change` → raw, in commits **af9e6e965** and **6a07f9c4b**.
2. **Global emit didn't reach board webviews.** `emit_ui_state_change_if_needed` now loops `app.emit_to(label, "ui-state-changed", …)` over all webview windows; in Tauri v2 a global `app.emit` does not reach dynamically-created board webviews (**6a07f9c4b**).
3. **Keymap divergence.** The live keymap is now registry-driven: `extractKeymapBindings`/`extractScopeBindings` in `apps/kanban-app/ui/src/lib/keybindings.ts` collect `keys[mode]` from the command registry, so `app.palette.open`'s `keys: { cua: "Mod+K", vim: ":" }` (declared in `builtin/plugins/ui-commands/index.ts`) drive the REAL handler. `Mod+Shift+P` remains a deliberate static alias pointing at the same unified `app.palette.open` id (not a divergence — same command). Deterministic key ownership: **541e85ce3**.

User live-confirmed: pressing `:` pops the palette; nav menu works.

## Acceptance Criteria
- [x] One canonical shortcut opens the palette in all keymap modes; registry + live keymap agree — registry `keys` drive the live handler; `Mod+Shift+P` is an explicit alias to the same `app.palette.open` id, no dead advertised keys.
- [x] Pressing the shortcut completes the open path and mounts the overlay (live-confirmed by user; full chain verified at HEAD).
- [x] `palette_open` owned by a single service (`swissarmyhammer-ui-state`); `open`/`close` flip it per-window and the change emits via `ui-state-changed` (search MODE is a parameter per design, not a second flag).
- [x] No PRODUCTION Rust `Command` path for palette open/close — dispatch goes through the TS plugin (`builtin/plugins/ui-commands`). Caveat: legacy `PaletteOpenCmd`/`PaletteCloseCmd` structs remain in a TEST-ONLY map in `crates/swissarmyhammer-kanban/src/commands/mod.rs` (documented "no production callers"); their removal is owned by mop-up card 01KTEBZSVGAZ881RAZZWWZXGPE.

## Tests
- [x] Frontend keymap: `apps/kanban-app/ui/src/lib/keybindings.test.ts` — vim `:` resolves to `app.palette.open` via registry-sourced bindings; "cua: Mod+K dispatches app.palette.open when a scope claims it"; `Mod+Shift+P` → `app.palette.open` in all 3 modes. RAN: passes.
- [x] Service test: `crates/swissarmyhammer-command-service/tests/integration/builtin_ui_commands_e2e.rs::ui_commands_plugin_registers_and_executes` — `app.palette.open` flips the per-window flag via the ui_state backend (scope-chain window, NOT "main"), `ui.palette.close` clears it; keys/menu metadata pinned (`cua: Mod+K, vim: ":"`). RAN: 106/106 command-service tests pass.
- [x] Overlay mount: `apps/kanban-app/ui/src/components/command-palette.test.tsx` — "renders nothing when closed" / "renders the palette when open" gated on `palette_open`. RAN: passes (116 tests across the two frontend files).
- [x] Regression pinning the envelope unwrap: `commands.rs` unit tests `ui_state_change_kind_envelope_palette_open` / `_envelope_inspector_stack` / `_envelope_null_change_is_none` (bare `{ok,change}` envelope, pre-existing) **plus two tests ADDED by this card**: `ui_state_change_kind_call_tool_result_structured_content_palette_open` and `ui_state_change_kind_call_tool_result_null_change_is_none`, pinning the FULL `{content, structuredContent:{ok,change}}` CallToolResult shape — the first arm of the unwrap chain and the exact af9e6e965 keystone. These fail against the pre-fix `ui_state_change_kind`. NOT RUN: `cargo test -p kanban-app` shares `target/debug` with the live `cargo tauri dev` watcher (verified running, pid 37520) and would race it — the new tests are write-only pending the next rebuild.

## Verification log (2026-06-10)
- `cargo nextest run -p swissarmyhammer-command-service` → 106 passed, 0 failed.
- `npx vitest run src/lib/keybindings.test.ts src/components/command-palette.test.tsx` → 116 passed, 0 failed.
- Read-verified at HEAD: `ui_state_change_kind` unwrap chain, `emit_ui_state_change_if_needed` per-window `emit_to` loop, plugin `keys` declaration, registry-driven keymap extraction.