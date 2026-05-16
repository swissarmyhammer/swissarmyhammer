---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
project: plugin-arch
title: 'swissarmyhammer-js: convert rquickjs engine to deno_core'
---
## What
Convert `swissarmyhammer-js` from `rquickjs` (QuickJS-NG) to `deno_core` so the workspace has exactly one JavaScript engine. The plugin platform depends on this; field validation must keep working unchanged.

- `crates/swissarmyhammer-js/Cargo.toml`: drop `rquickjs`, add `deno_core` (and `deno_ast` only if needed for transpile — not required for plain JS eval).
- `crates/swissarmyhammer-js/src/lib.rs`: keep the public API **exactly** — `JsState::global()`, `set(name, expr)`, `get(expr)`, `GetAllVariables`, `SetModuleBase`. The dedicated worker-thread + `mpsc` channel model carries over directly; `deno_core::JsRuntime` is single-threaded and wants that pattern.
- Replace the `worker_loop` rquickjs `Runtime`/`Context` with a `deno_core::JsRuntime`; replace `SandboxedResolver` (rquickjs `Resolver`) with a `deno_core::ModuleLoader` that enforces the same base-dir sandbox (reject absolute paths and `..` escape).
- Preserve env injection (`env`, `process.env` globals), memory/stack limits where deno_core exposes equivalents, and the pending-job draining (deno_core: `run_event_loop`).
- The single consumer is `swissarmyhammer-fields`' `ValidationEngine` — it must compile and pass unchanged.

## Acceptance Criteria
- [ ] `swissarmyhammer-js` has no `rquickjs` dependency; `deno_core` is the engine.
- [ ] Public API (`JsState`, `set`, `get`, module base) unchanged — no signature changes.
- [ ] `swissarmyhammer-fields` compiles against the converted crate with no edits to its call sites.
- [ ] Module imports outside the configured base directory are still rejected.

## Tests
- [ ] All existing `swissarmyhammer-js` tests pass after conversion (port any rquickjs-specific test internals; keep behavior assertions).
- [ ] Keep/port the sandbox-escape test: an import resolving outside the base dir errors.
- [ ] Run: `cargo test -p swissarmyhammer-js -p swissarmyhammer-fields` — all green.

## Workflow
- Use `/tdd` — keep the existing test suite as the RED/GREEN gate; port tests before swapping the engine.