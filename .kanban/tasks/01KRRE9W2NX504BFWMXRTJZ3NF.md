---
assignees:
- claude-code
depends_on:
- 01KRRE967SBZ5TH2JPDMSV21BY
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffc80
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
- [x] `Manifest` parses a real `plugin.json`; missing required fields error clearly.
- [x] Discovery finds plugin dirs across the supplied layer roots (builtin + user, optionally project); the highest-precedence copy wins when an id exists in multiple layers.
- [x] Discovery is generic over `C: DirectoryConfig` — no host-specific directory name or path is hardcoded in the platform.
- [x] `this.register` for a name not in `provides` is rejected; a `provides` name colliding with a reserved host name is rejected.
- [x] The disk directory name not matching `id` still resolves correctly by `id`.

## Tests
- [x] Integration test with `PluginHost::for_tests`: a temp project/user layer with `plugins/probe/plugin.json` + entry `.ts`; `discover_and_load_all()` loads it and `load()` runs.
- [x] Layering test: same `id` in two temp layers; assert the higher-precedence copy is active (observe a behavior difference between the two copies).
- [x] Test: a plugin whose `load()` registers a server name absent from `provides` fails with a clear error.
- [x] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the manifest + stacked-discovery + provides-validation tests first, then implement.

## Depends on
PluginHost lifecycle.

## Review Findings (2026-05-17 13:00)

### Warnings
- [x] `crates/swissarmyhammer-plugin/src/host.rs:472-485` — `discover_and_load_all` loops over discovered plugins calling `load_resolved(...).await?`; if the Nth plugin fails to load, the `?` returns `Err` while plugins 1..N-1 remain loaded and live in the registry. The returned `Err` carries no list of what *did* load, so the caller cannot unload them — a partially-initialized host. This silently contradicts `PluginHost::new`, whose docs explicitly state "A host whose builtins fail to load is not returned, because a partially initialized host would silently miss tools the embedder shipped." Either roll back already-loaded plugins on a mid-scan failure (mirroring `new`), or return the successfully-loaded ids alongside the error so the caller can decide. At minimum, document the current partial-success behavior on `discover_and_load_all` so it is a deliberate, stated contract rather than an accident, and add a test covering the multi-plugin-with-one-failure path.
- [x] `crates/swissarmyhammer-plugin/src/manifest.rs:67-72` and `crates/swissarmyhammer-plugin/src/host.rs:385-388` — the manifest's `entry` field is plugin-authored and is joined onto the bundle directory with no containment check (`bundle_dir.join(entry_file)`), then handed to the runtime. `runtime/module_loader.rs:216-218` documents that the entry/main module URL is "returned unchecked because the entry path is host-derived ... and trusted, not plugin-chosen" — but with this change `entry` *is* plugin-chosen. A manifest with `"entry": "../../../etc/passwd"` (or any traversal) escapes the bundle root and bypasses the sandbox containment check by design. Validate that the resolved `entry` path stays within the plugin directory (reject `..` components / canonicalize-and-check) before loading, or update the module_loader's trust comment to reflect that `entry` is now manifest-supplied and must be contained.

### Nits
- [x] `crates/swissarmyhammer-plugin/src/discovery.rs:181-189` — `scan_layer` silently skips any immediate subdirectory of `plugins/` that lacks a `plugin.json`. This is the documented and intended behavior, but a directory that looks like a plugin (e.g. has an `entry.ts` but a missing/misnamed `plugin.json`) is dropped with no diagnostic. Consider a `tracing::debug!` when a `plugins/` subdirectory is skipped for want of a manifest, so a misconfigured bundle is observable rather than invisible.