---
assignees:
- claude-code
depends_on:
- 01KRRE3SEEAGGN81NR9VFDRMK0
- 01KRRE9W2NX504BFWMXRTJZ3NF
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffd80
project: plugin-arch
title: 'plugin: hot reload driven by the directory watcher'
---
## What
Wire hot reload: `PluginHost` subscribes to the `swissarmyhammer-directory` stack-aware watcher on the `plugins/` subdirectory and turns `StackedEvent`s into load/reload/unload decisions.

In `crates/swissarmyhammer-plugin/src/` (extend `discovery.rs`/`host.rs`):
- On startup, `PluginHost` starts `Watcher::<SwissarmyhammerConfig>::watch("plugins")` and consumes its `StackedEvent` receiver.
- Translate each event by the layer currently active for that plugin id:
  - `Added { layer }` → if `layer` becomes the highest-precedence layer for the id, load it; else just refresh the override stack.
  - `Modified { layer }` → if `layer` is the active layer, reload that plugin.
  - `Removed { layer }` → if `layer` was active, fall back to the next layer (or unload if none).
- Reload mechanics: tear down the plugin's V8 isolate; walk the per-plugin ledger and dispose every registration; create a fresh isolate; re-transpile + reload source; call `load()`. Latency target tens of ms.
- Edge cases the doc fixes: in-flight calls into a reloading plugin reject with `PluginReloaded`; a failed v2 load leaves the plugin UNLOADED (no fallback to v1 — v1 is already torn down); crashed plugins do NOT auto-restart (surface via a status/notification); class-field state is intentionally lost on reload.
- `provides` expansion on reload pauses for re-approval (a hook/callback the host exposes; the actual UI prompt is out of scope — provide the seam + a default-deny-or-allow policy a test can drive).

## Acceptance Criteria
- [x] `PluginHost` reloads a plugin when its active-layer source changes; the new behavior is observable in the same host.
- [x] Reload disposes the old ledger (old servers/callbacks gone) before the new isolate runs.
- [x] A failed v2 load leaves the plugin unloaded with a surfaced error — no zombie isolate, no half-registered servers.
- [x] Removing the active layer falls back to the next layer; removing the last layer unloads.

## Tests
- [x] Integration test (`PluginHost::for_tests`): write a probe plugin, observe behavior A; rewrite its source, let the watcher fire, observe behavior B in the SAME host.
- [x] Test: rewrite source with a `load()` that throws; assert the plugin ends up unloaded, the error is surfaced, and no server from either version is registered.
- [x] Test: remove the project-layer copy of a plugin that also exists in the user layer; assert the user copy becomes active.
- [x] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the observe-A → rewrite → observe-B reload test first, then implement.

## Depends on
Stacked Watcher + plugin discovery.

## Review Findings (2026-05-17 14:32)

### Warnings
- [x] `crates/swissarmyhammer-plugin/src/host.rs:1024-1050` (`reconcile_id`) — A `Modified` event on a SHADOWED (non-winning) lower-layer copy spuriously reloads the active copy. `handle_stacked_event` discards the event's per-layer attribution and `reconcile_all` reconciles every id; for an id whose shadowed copy changed, `reconcile_id` sees `(Some(active), Some(winner))` with `winner.source == active.layer` (the higher layer is unchanged and still the winner), so it falls into the "Modified of the active layer — reload in place" branch and runs `reload_active`. The active plugin is torn down and re-`load()`ed even though its own source did not change — losing class-field state and re-running `load()` for no reason. The task's SCRUTINIZE item states explicitly: "a `Modified` to a SHADOWED (non-active) layer must NOT reload the active copy." `reconcile_id`'s own docstring also claims a shadowed-copy change "does not touch the active copy", which the code contradicts. Suggested fix: guard `reload_active` so it only reloads when the active layer's source actually changed — e.g. record the winning copy's bundle directory (or a content/mtime fingerprint) in `ActivePlugin` and skip the reload when the re-resolved winner is byte-for-byte the same copy that is already active.
- [x] `crates/swissarmyhammer-plugin/src/host.rs:725-737` (`rollback_loaded`) — A failed discovery scan leaves stale `reload_status` entries behind. `discover_and_load_all` calls `record_active` for every plugin that loads before the failing one, and `record_active` inserts a `ReloadStatus::Healthy` into `state.reload_status` (host.rs:692-694). When a later plugin fails, `rollback_loaded` unloads the earlier successes and calls `forget_active_by_plugin_id` to drop their `active_plugins` records — but never removes their `reload_status` entries. So after a failed scan, `PluginHost::reload_status(id)` returns `Some(Healthy)` for a plugin that is not loaded. The `rollback_loaded` docstring claims the host is left "exactly as the scan found it ... so a failed scan leaves no stale hot-reload state behind" — `reload_status` is hot-reload state and is not cleaned up. Suggested fix: in `rollback_loaded` (or `forget_active_by_plugin_id`), also remove the `reload_status` entry for each rolled-back plugin id so a failed scan truly leaves no stale state.
