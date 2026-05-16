---
assignees:
- claude-code
depends_on:
- 01KRRE967SBZ5TH2JPDMSV21BY
position_column: todo
position_ordinal: 8f80
project: plugin-arch
title: 'plugin: manifest parsing and stacked discovery via swissarmyhammer-directory'
---
## What
Make `PluginHost` discover plugins on disk through `swissarmyhammer-directory`, using builtin → user → project layer stacking — the same model every other resource uses. The platform stays **host-agnostic**: it does not hardcode a directory config or fixed paths; the host supplies its layer roots (via `PluginHost::new`).

In `crates/swissarmyhammer-plugin/src/` (a `discovery.rs` + `manifest.rs`):
- `Manifest` struct deserialized from `plugin.json`: `id`, `name`, `version`, `entry` (path to the entry `.ts`), `provides` (Vec<String> — server names this plugin will register). The on-disk directory name need NOT match `id`; `id` is authoritative for identity across layers.
- Discovery operates over the layer roots the host passes to `PluginHost::new`: a builtin set (`include_dir!`, read-only, compiled into the host) and writable layer roots (user, optionally project). Use `swissarmyhammer-directory` (`ManagedDirectory<C>` / `VirtualFileSystem<C>`) to resolve and load directories under the `plugins/` subdirectory — the platform is generic over `C: DirectoryConfig`. The doc's `SwissarmyhammerConfig` is one such config; the kanban host supplies its own (`KanbanConfig` → `~/.config/kanban/plugins`, see the kanban-app bootstrap task). No `sah/`-specific path is baked into the platform.
- Precedence: project shadows user shadows builtin. A plugin id resolves to its highest-precedence layer; that copy is the active one. Removing a higher layer re-emerges the lower one.
- `provides` validation: at load, reject a `this.register(name, ...)` for a name not listed in `provides`; reject `provides` names colliding with reserved host server names.
- `PluginHost` gains `discover_and_load_all()` (point-in-time scan of all configured layers) on top of the explicit `load(dir)` from the host task.

Scope boundary: reacting to file changes (hot reload) is the next task; this task is point-in-time discovery + stacking + manifest.

## Acceptance Criteria
- [ ] `Manifest` parses a real `plugin.json`; missing required fields error clearly.
- [ ] Discovery finds plugin dirs across the supplied layer roots (builtin + user, optionally project); the highest-precedence copy wins when an id exists in multiple layers.
- [ ] Discovery is generic over `C: DirectoryConfig` — no host-specific directory name or path is hardcoded in the platform.
- [ ] `this.register` for a name not in `provides` is rejected; a `provides` name colliding with a reserved host name is rejected.
- [ ] The disk directory name not matching `id` still resolves correctly by `id`.

## Tests
- [ ] Integration test with `PluginHost::for_tests`: a temp project/user layer with `plugins/probe/plugin.json` + entry `.ts`; `discover_and_load_all()` loads it and `load()` runs.
- [ ] Layering test: same `id` in two temp layers; assert the higher-precedence copy is active (observe a behavior difference between the two copies).
- [ ] Test: a plugin whose `load()` registers a server name absent from `provides` fails with a clear error.
- [ ] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the manifest + stacked-discovery + provides-validation tests first, then implement.

## Depends on
PluginHost lifecycle.