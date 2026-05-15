---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffb380
title: Add tauri-plugin-drag dependency and configure dragDropEnabled
---
## What
Add `tauri-plugin-drag` (from crabnebula) as a Rust dependency and configure Tauri's `dragDropEnabled` setting on webview windows.

**Key config decision:** We need `dragDropEnabled: true` (the default) so Tauri's `DragDropEvent` fires in target windows when an OS-level drag enters. This is the receiving side. The `tauri-plugin-drag` crate handles the sending side (initiating OS drags from Rust).

**Files:**
- `kanban-app/Cargo.toml` — add `tauri-plugin-drag = \"2\"` (or latest compatible version from crates.io)
- `kanban-app/src/main.rs` — register the drag plugin with `.plugin(tauri_plugin_drag::init())`
- `kanban-app/tauri.conf.json` — verify `dragDropEnabled` is not explicitly set to false (default true is what we want)
- `kanban-app/ui/package.json` — add `@crabnebula/tauri-plugin-drag` JS bindings if needed for frontend initiation

**Approach:**
- Check crates.io for the latest `tauri-plugin-drag` version compatible with Tauri v2
- The plugin provides `drag::start_drag()` which we'll call from a new Tauri command
- Verify the plugin doesn't conflict with existing window setup

## Acceptance Criteria
- [ ] `tauri-plugin-drag` compiles and is registered as a Tauri plugin
- [ ] App launches without errors
- [ ] Existing drag-and-drop behavior (intra-window @dnd-kit) is unaffected
- [ ] `DragDropEvent` still fires on webview windows (dragDropEnabled remains true)

## Tests
- [ ] `cargo nextest run` — compilation succeeds, no regressions
- [ ] `cargo build` — full app builds without errors
- [ ] Manual test: app launches, existing board DnD still works