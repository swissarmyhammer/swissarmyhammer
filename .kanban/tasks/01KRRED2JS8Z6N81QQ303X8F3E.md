---
assignees:
- claude-code
depends_on:
- 01KRRE7WP7YY56R7MTBHJHZD12
- 01KRRE8D4712C7785TRN3GGR4H
- 01KRREC7YF5ENG2M2E7DQYSDGS
position_column: todo
position_ordinal: '9580'
project: plugin-arch
title: 'plugin: SDK e2e tests — operation _meta round-trip and callbacks'
---
## What
Capability integration tests for the SDK's operation-tool path sugar and the callback primitive, following the `files_dispatch_e2e.rs` reference shape.

`crates/swissarmyhammer-plugin/tests/integration/`:
- `operation_meta_e2e.rs` — register a real operation tool (one carrying `_meta["io.swissarmyhammer/operations"]`). A probe plugin calls `this.<server>.<tool>.<noun>.<verb>({...})`; assert the tool receives a `tools/call("<tool>", { op: "<verb> <noun>", ... })` — the SDK read `_meta` and built `op` from the path. Also assert the direct form `this.<server>.<tool>({op:..., ...})` works and an unknown verb path raises `UnknownOperation`.
- `callback_e2e.rs` — a probe plugin passes a function across the boundary; the host invokes it via `notifications/callbacks/invoke`; assert the function ran and its return value flowed back to where the plugin awaits it (observe via a value the plugin writes through the `files` server or returns).

Each test: own `TempDir`, fresh `PluginHost`, no shared/`static` state.

## Acceptance Criteria
- [ ] `operation_meta_e2e.rs` proves the `_meta` round-trip: path-form call → `tools/call(tool, {op, ...})`; direct form works; unknown verb → `UnknownOperation`.
- [ ] `callback_e2e.rs` proves a plugin-supplied function is invoked by the host and its return value flows back.
- [ ] Both follow the reference-test isolation model; no mocked dispatcher/registry.

## Tests
- [ ] Run: `cargo test -p swissarmyhammer-plugin` — the two new `*_e2e.rs` tests and the whole suite green.
- [ ] Each test must genuinely fail if the SDK `_meta` resolution or the callback primitive is broken.

## Workflow
- Tests are the deliverable; no `/tdd` cycle. Reuse the harness/helpers from `files_dispatch_e2e.rs`.

## Depends on
TypeScript SDK, callback primitive, and the reference `files_dispatch_e2e.rs` harness.