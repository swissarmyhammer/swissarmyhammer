---
assignees:
- claude-code
depends_on:
- 01KRREAHF4FXQY5PC2GYEJWWJV
- 01KRREC7YF5ENG2M2E7DQYSDGS
position_column: todo
position_ordinal: '9680'
project: plugin-arch
title: 'plugin: lifecycle e2e tests — discovery/layering, hot reload, unload, failed load'
---
## What
Capability integration tests for the plugin lifecycle, following the `files_dispatch_e2e.rs` reference shape.

`crates/swissarmyhammer-plugin/tests/integration/`:
- `discovery_layering_e2e.rs` — same plugin id in user + project temp layers; assert the project copy is active; remove the project copy; assert the user copy re-emerges.
- `hot_reload_e2e.rs` — write a probe plugin, observe behavior A; rewrite its source; the watcher fires; observe behavior B in the SAME `PluginHost`.
- `unload_disposal_e2e.rs` — load a plugin that registers a server; `unload`; assert calls into that server fail with `ServerUnavailable` and its callbacks no longer fire.
- `failed_load_e2e.rs` — a plugin whose `load()` throws; assert the host surfaces the error, leaves no zombie isolate, and registers no half-built servers.

Each test: own `TempDir`, fresh `PluginHost`, no shared/`static` state — hot-reload tests especially need a fresh host since reload behavior depends on the prior load's ledger. Assert observable effects, never intermediate registry/ledger state.

Note: the override-stack capability (two plugins registering the same command id) belongs to the Command service (separate `command-service.md`) and is out of scope here.

## Acceptance Criteria
- [ ] Four `*_e2e.rs` files covering discovery/layering, hot reload, unload disposal, and failed load.
- [ ] Each asserts observable effects only; each owns its `TempDir` + fresh `PluginHost`.
- [ ] No mocked dispatcher/registry; real isolates and real registered servers.

## Tests
- [ ] Run: `cargo test -p swissarmyhammer-plugin` — the four new `*_e2e.rs` tests and the whole suite green.
- [ ] Each test must genuinely fail if its lifecycle behavior is broken.

## Workflow
- Tests are the deliverable; no `/tdd` cycle. Reuse the harness/helpers from `files_dispatch_e2e.rs`.

## Depends on
Hot reload (which transitively covers discovery, unload) and the reference `files_dispatch_e2e.rs` harness.