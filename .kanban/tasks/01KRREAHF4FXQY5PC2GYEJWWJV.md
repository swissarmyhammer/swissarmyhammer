---
assignees:
- claude-code
depends_on:
- 01KRRE3SEEAGGN81NR9VFDRMK0
- 01KRRE9W2NX504BFWMXRTJZ3NF
position_column: todo
position_ordinal: '9080'
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
- [ ] `PluginHost` reloads a plugin when its active-layer source changes; the new behavior is observable in the same host.
- [ ] Reload disposes the old ledger (old servers/callbacks gone) before the new isolate runs.
- [ ] A failed v2 load leaves the plugin unloaded with a surfaced error — no zombie isolate, no half-registered servers.
- [ ] Removing the active layer falls back to the next layer; removing the last layer unloads.

## Tests
- [ ] Integration test (`PluginHost::for_tests`): write a probe plugin, observe behavior A; rewrite its source, let the watcher fire, observe behavior B in the SAME host.
- [ ] Test: rewrite source with a `load()` that throws; assert the plugin ends up unloaded, the error is surfaced, and no server from either version is registered.
- [ ] Test: remove the project-layer copy of a plugin that also exists in the user layer; assert the user copy becomes active.
- [ ] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the observe-A → rewrite → observe-B reload test first, then implement.

## Depends on
Stacked Watcher + plugin discovery.