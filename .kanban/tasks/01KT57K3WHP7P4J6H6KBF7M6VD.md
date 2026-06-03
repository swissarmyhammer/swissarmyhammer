---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffdb80
project: command-cutover
title: 'window/app server: native context-menu render op (unblocks show_context_menu removal)'
---
## What

Blocks the final item of `01KS36Y4NBDZMGH6QF963MD6FE` (frontend invoke migration). That task's REALITY-CHECK said `invoke("show_context_menu")` should migrate to "native context-menu render (the window/app server)". The Stage-3 frontend migration (commit a0966d71b) already moved `get_entity`→entity, `spatial_*`→focus, and deleted `log_command`, and removed those dead Rust handlers. The grep guardrail `apps/kanban-app/ui/src/lib/no-direct-invoke.node.test.ts` passes.

However `show_context_menu` could NOT be migrated: the `swissarmyhammer-window-service` MCP server (crates/swissarmyhammer-window-service/src/service.rs) exposes only window ops (new/activate/set position/get position/get monitors/close window/open path/reveal path/switch board/close board). There is NO context-menu / NSMenu render op on any MCP server. Mounting a native NSMenu needs an ambient Tauri AppHandle, which has no MCP-wire equivalent today — so the Stage-3 implementers deliberately kept `show_context_menu` as an allow-listed AppHandle-bound Tauri native (documented in the grep test's ALLOWED_INVOKE_HANDLERS comment).

## To unblock
1. Add a context-menu render op to the window/app MCP server (e.g. `op: "show context menu"` taking the `ContextMenuItem[]` payload), wired to the AppHandle-backed shell so it can mount the NSMenu.
2. Migrate `apps/kanban-app/ui/src/lib/context-menu.ts:156` `invoke("show_context_menu", { items })` → `callMcpTool("window"/"app", "show context menu", { items })`.
3. Remove `show_context_menu` from `commands.rs` and from `generate_handler!` (apps/kanban-app/src/main.rs:67), and drop it from ALLOWED_INVOKE_HANDLERS in no-direct-invoke.node.test.ts.

## Acceptance Criteria
- [x] window/app MCP server has a context-menu render op backed by AppHandle
- [x] context-menu.ts routes through the MCP transport (no direct invoke)
- [x] `show_context_menu` Tauri handler removed from commands.rs + generate_handler!
- [x] `show_context_menu` removed from no-direct-invoke allow-list; grep test still green
- [x] `cargo check -p kanban-app` green; `npm test --prefix apps/kanban-app/ui` green
- [x] right-click context menu still appears in the UI

## Review Findings (2026-06-03 11:55)

All 6 ACs met; behavior-preserving port. Two warnings + two nits.

### Warnings
- [x] `crates/swissarmyhammer-window-service/src/shell.rs:336-344` — `focused_window()` falls back to `webview_windows().values().next()` (nondeterministic `HashMap` order) when no window reports focus; the old native popped on the *calling* `tauri::Window` (always correct). Fix: have the frontend pass its own window label in the op payload (the webview knows it via `getCurrentWebviewWindow().label`); the shell targets that window, falling back to focused-then-any only when absent. Restores deterministic targeting matching the old behavior.
- [x] `crates/swissarmyhammer-window-service/src/shell.rs` / `tests/integration/window_e2e.rs` — the window-targeting decision (the focus of this card) has no coverage; op-routing tests prove items reach the shell but not which window. Add coverage for the targeting/label-resolution path (mock Tauri runtime, or at least the label-resolution logic), or document untested-by-design.

### Nits
- [x] `crates/swissarmyhammer-window-service/src/shell.rs:484` — `serde_json::to_string(item).unwrap_or_default()` silently degrades a (impossible) serialize failure to an empty id; add a `tracing::warn!` on the error arm to surface it.
- [x] Two `ContextMenuItem` structs (window-service payload vs `commands.rs` menu-event decoder) are justified (opposite sides of the JSON-id wire) but coupled only by a comment; add a field-parity test to lock the encode→decode round-trip against drift.