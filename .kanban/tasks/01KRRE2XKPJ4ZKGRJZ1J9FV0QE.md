---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffee80
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
- [x] `swissarmyhammer-js` has no `rquickjs` dependency; `deno_core` is the engine.
- [x] Public API (`JsState`, `set`, `get`, module base) unchanged — no signature changes.
- [x] `swissarmyhammer-fields` compiles against the converted crate with no edits to its call sites.
- [x] Module imports outside the configured base directory are still rejected.

## Tests
- [x] All existing `swissarmyhammer-js` tests pass after conversion (port any rquickjs-specific test internals; keep behavior assertions).
- [x] Keep/port the sandbox-escape test: an import resolving outside the base dir errors.
- [x] Run: `cargo test -p swissarmyhammer-js -p swissarmyhammer-fields` — all green.

## Workflow
- Use `/tdd` — keep the existing test suite as the RED/GREEN gate; port tests before swapping the engine.

## Implementation Notes
- deno_core 0.400.0 (v8 147.4.0); added deno_error =0.7.1 for `JsErrorBox` (= `ModuleLoaderError`).
- `worker_loop` now owns a `deno_core::JsRuntime` driven by a current-thread Tokio runtime; `drain_event_loop` calls `run_event_loop` for Promise/dynamic-import settling.
- `SandboxedResolver` replaced by `SandboxedModuleLoader` (`deno_core::ModuleLoader`) holding the base in a `RefCell` so `SetModuleBase` mutates in place (the loader is fixed at runtime construction).
- env injected by reconstructing a JSON object via `JSON.parse` of an escaped literal.
- `bridge` now does V8 conversion: `v8_to_json` stringifies under a `TryCatch`, mapping `undefined`/empty/`"undefined"`/functions/symbols to JSON null.
- V8 heap capped at 10 MB via `CreateParams::heap_limits`; V8 exposes no embedder stack-size knob.

## Review Findings (2026-05-16 16:10)

### Blockers
- [x] `crates/swissarmyhammer-js/src/lib.rs:180-183` — The `file://` fast-path in `SandboxedModuleLoader::resolve` bypasses the sandbox entirely. Any specifier starting with `file://` is parsed and returned without a base-directory containment check, so JS evaluated after `set_module_base` can do `import('file:///etc/passwd')` and read arbitrary files. This directly violates the acceptance criterion "module imports outside the configured base directory are still rejected" and the security-boundary requirement that absolute-path rejection be airtight. The branch is also unnecessary for legitimate use: deno_core passes relative/bare import-statement specifiers (e.g. `./util.js`) to `resolve`, never the `file://` referrer URL (that arrives only as the ignored `_referrer` argument). Fix: remove the `file://` branch, or route `file://` specifiers through a containment check (parse to a path, canonicalize, verify `starts_with` the canonicalized base) before accepting. Add a regression test importing a `file:///` URL outside the base and asserting rejection.

### Warnings
- [x] `crates/swissarmyhammer-js/src/lib.rs:131-162` — Nested relative imports resolve against the configured module base instead of the importing module's directory. `resolve_sandboxed` always does `base.join(specifier)` and ignores `_referrer`, so a module at `<base>/helpers/math.js` that imports `./util.js` resolves to `<base>/util.js` rather than `<base>/helpers/util.js`. This breaks standard ES module resolution semantics for any multi-directory module tree. The current consumer (`ValidationEngine`) does not use `set_module_base`, so impact is latent, but the public API exposes module imports. Fix: when the referrer is a `file://` URL inside the sandbox, join the specifier against the referrer's parent directory (still canonicalizing and re-checking sandbox containment); fall back to the base only for top-level imports. Add a nested-relative-import test.

### Nits
- [x] `crates/swissarmyhammer-js/src/lib.rs:1031-1056` — `test_backslash_path_import_rejected` only asserts `bs_err` was set by the `.catch`, but a backslash-prefixed specifier would also be rejected later by `canonicalize()` failing on a nonexistent path, so the test does not actually prove the up-front backslash-prefix check (line 139) is what rejected it. Strengthen the assertion to check the error message contains the "absolute import path rejected" text, or assert on the specific rejection reason.