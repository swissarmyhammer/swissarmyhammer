---
assignees:
- claude-code
depends_on:
- 01KRREAHF4FXQY5PC2GYEJWWJV
- 01KRREBGRC9WTBRRXB7KS8WQT8
position_column: todo
position_ordinal: '9780'
project: plugin-arch
title: 'kanban-app: integrate PluginHost into AppState with builtin + user-layer plugins'
---
## What
Wire the plugin platform into the kanban app so plugins load as part of the application context (`AppState`). Two layers: builtin plugins compiled into the host, and user plugins from `~/.config/kanban/plugins`. No project layer.

- **`KanbanConfig` directory config** — add a `DirectoryConfig` impl to `crates/swissarmyhammer-directory/src/config.rs` (alongside `AvpConfig`, `ShellConfig`, `CodeContextConfig`, `RalphConfig`) whose directory name is `kanban`, so `ManagedDirectory::<KanbanConfig>::xdg_config()` resolves `$XDG_CONFIG_HOME/kanban` (i.e. `~/.config/kanban`). The plugin user layer is its `plugins/` subdirectory: `~/.config/kanban/plugins/<plugin-id>/`.
- **Builtin plugins** — bundle the host's builtin plugins into the kanban-app binary via `include_dir!` from `builtin/plugins/` under the repo's existing top-level `builtin/` tree (consistent with the other builtin resources already bundled from `builtin/`). This is the read-only builtin layer.
- **`swissarmyhammer-tools` dependency** — `apps/kanban-app` gains a dependency on `swissarmyhammer-tools`. This is how the kanban app obtains the in-process tool rust modules (`kanban`, and any others it wants to offer plugins) and the rmcp-`ServerHandler` adapters from the tool-exposure task — there is no other source for them.
- **`PluginHost` on `AppState`** — add a `plugin_host` field to `AppState` (`apps/kanban-app/src/state.rs`). Construct it in `with_ui_state`/`new` via `PluginHost::new(...)` with the builtin set + the `KanbanConfig`-derived user root. `AppState` already owns `commands_registry`, `ui_state`, `boards` — the plugin host joins them as another piece of shared application context.
- **Startup** — in Tauri `setup` (where `AppState::start_watchers` is already called after the `AppHandle` exists), call `discover_and_load_all()` and start the hot-reload watcher on `~/.config/kanban/plugins`. Plugins loaded during `auto_open_board` (before Tauri) follow the same pattern as board watchers: discover early, start the watcher once the `AppHandle` is available.
- **Expose in-process servers** — expose the kanban app's in-process tool rust modules to its `PluginHost` via `expose_rust_module` so plugins can call them — at minimum the `kanban` operation tool — reusing the `swissarmyhammer-tools` rmcp-`ServerHandler` adapters.

## Acceptance Criteria
- [ ] `KanbanConfig` exists in `swissarmyhammer-directory`; resolves `~/.config/kanban`; the plugin user layer is `~/.config/kanban/plugins/`.
- [ ] Builtin plugins are `include_dir!`-bundled into the kanban-app binary from `builtin/plugins/` as the read-only builtin layer.
- [ ] `apps/kanban-app` depends on `swissarmyhammer-tools` and exposes its tool rust modules through that dependency.
- [ ] `AppState` has a `plugin_host` field constructed with builtin + user layers; no project layer.
- [ ] At startup the kanban app discovers and loads plugins from both layers and starts the hot-reload watcher on `~/.config/kanban/plugins`.
- [ ] At least the `kanban` operation tool is exposed to plugins via `expose_rust_module`.

## Tests
- [ ] Integration test (extend `apps/kanban-app` tests, `AppState::new_for_test` pattern — parameterize it to accept temp `~/.config/kanban` + temp builtin roots): construct `AppState`, assert a builtin probe plugin and a user-layer probe plugin both load, and a plugin can call the exposed `kanban` server (observe a real kanban effect).
- [ ] Test: a plugin dropped into the temp user `plugins/` dir is picked up by the watcher and loaded (reuse the hot-reload e2e harness).
- [ ] `KanbanConfig` unit test in `swissarmyhammer-directory`: asserts the resolved config dir ends in `kanban`.
- [ ] Run: `cargo test -p kanban-app -p swissarmyhammer-directory` — all green; existing kanban-app tests still pass.

## Workflow
- Use `/tdd` — write the AppState-loads-builtin-and-user-plugins test first, then implement.

## Depends on
Hot reload (watcher-driven load/reload) + the swissarmyhammer-tools tool-exposure adapters.