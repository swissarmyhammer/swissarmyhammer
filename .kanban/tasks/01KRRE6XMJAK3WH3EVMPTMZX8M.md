---
assignees:
- claude-code
depends_on:
- 01KRRE3C4RDD999B43BRWJMA3J
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff780
project: plugin-arch
title: 'plugin: deno_core runtime — per-plugin isolate and TypeScript transpile'
---
## What
Build the deno_core runtime layer of `swissarmyhammer-plugin`: one V8 isolate per plugin, with TypeScript transpiled at module-load time.

In `crates/swissarmyhammer-plugin/src/runtime.rs` (or a `runtime/` module):
- A `PluginRuntime` (or `Isolate`) wrapper around `deno_core::JsRuntime` — one per plugin, for fault isolation and clean teardown. Single-threaded; follow the dedicated-thread + channel pattern (mirror `swissarmyhammer-js`'s worker model — that conversion is the reference).
- TypeScript: transpile `.ts` → JS + source maps at load time using `deno_ast` (wraps `swc_core`). Syntactic TS-to-JS only — NO type-checking at load time. Register source maps with the V8 Inspector so DevTools/stack traces show original TS lines.
- A way to load and evaluate a plugin entry module and call exported lifecycle functions; expose an op or channel for the SDK→host bridge (the dispatcher hook is wired in the SDK / PluginHost tasks — here just provide the seam).
- V8 Inspector support (`--inspect[=PORT]`) gated to dev mode.

Scope boundary: this task delivers the runtime that can transpile + run a `.ts` module in an isolated V8 isolate. The module loader (relative/bare/`@swissarmyhammer/*` resolution) and the host load/unload lifecycle are separate tasks.

## Acceptance Criteria
- [x] A plugin runtime type exists that creates a fresh `deno_core::JsRuntime` isolate and tears it down cleanly.
- [x] A `.ts` source string is transpiled via `deno_ast` and evaluated in the isolate; TS-only syntax (types, interfaces) does not cause a runtime error.
- [x] Source maps are produced and registered for the inspector.
- [x] No type-checking is performed at load (a type error in TS still runs).

## Tests
- [x] Unit/integration test: feed a `.ts` snippet with type annotations + an exported function, evaluate it in a fresh isolate, assert the function's observable result (e.g. it returns a value or sets a global the test reads back).
- [x] Test that two isolates are independent — a global set in one is not visible in the other.
- [x] Test that deliberately type-incorrect-but-syntactically-valid TS still transpiles and runs.
- [x] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the transpile-and-run isolate tests first, then implement.

## Depends on
swissarmyhammer-plugin crate scaffold.

## Review Findings (2026-05-17 13:05)

### Warnings
- [x] `crates/swissarmyhammer-plugin/src/runtime/transpile.rs:84-93` — The inner comment block describes `SourceMapOption::Inline` behavior ("`SourceMapOption::Inline` makes the emitter both append the `//# sourceMappingURL=` comment ... and still hand us the raw map"), but the `emit_options` two lines below sets `source_map: SourceMapOption::Separate`, and the later comment at lines 104-110 correctly describes `Separate`. A reader gets two contradictory explanations of the same code. Rewrite the lines 84-93 comment to describe what the code does: `Separate` returns the map alongside the code, and `with_inline_source_map` then inlines it manually; `inline_sources` embeds the original TS text in the map.
- [x] `crates/swissarmyhammer-plugin/src/runtime/mod.rs:59-65` and `mod.rs:319-322` — The `HEAP_MAX_BYTES` doc and the `worker_loop` inline comment both claim "V8 enforces this as a hard ceiling, so a runaway plugin aborts its own script instead of exhausting host memory." This is false as written: `v8::CreateParams::heap_limits()` alone does not abort just the offending script — when the limit is reached V8 raises a fatal OOM and aborts the entire host process unless an `add_near_heap_limit_callback` is registered that calls `terminate_execution` (or raises the limit). The module's stated purpose is fault isolation (mod.rs:6-11: "a plugin that exhausts memory ... cannot corrupt another plugin's state"), so this is a load-bearing false guarantee. Either register a near-heap-limit callback that terminates the offending isolate, or correct the comments so they no longer promise fault isolation the code does not deliver.
- [x] `crates/swissarmyhammer-plugin/src/runtime/mod.rs:265-282` — Teardown can hang indefinitely on a wedged isolate. `join_worker` (called from both `shutdown` and `Drop`) closes the command channel then calls `handle.join()`, which blocks until the worker thread exits. If the worker is executing a non-terminating plugin script (infinite loop), it never returns to its `recv()` loop, so `join()` blocks the dropping thread forever. The runtime retains no `v8::IsolateHandle`, so there is no way to call `terminate_execution` from another thread to interrupt it; `COMMAND_TIMEOUT` only bounds the caller's `await`, not the worker. This contradicts the "no hang path" / "tears down cleanly with no leaked thread" criterion and the module doc framing at mod.rs:67-71. Retain the `IsolateHandle` and call `terminate_execution` before `join`, or explicitly document that interrupting a wedged isolate is deferred to a later watchdog task.

### Nits
- [x] `crates/swissarmyhammer-plugin/src/runtime/mod.rs:341-346` — `JsRuntime::new` is constructed with `inspector: config.inspect_port.is_some()`, and deno_core initializes the in-isolate inspector at construction time when that flag is true. The subsequent `runtime.maybe_init_inspector()` call is therefore a redundant no-op (`maybe_init_inspector` early-returns when the inspector already exists), but the comment claims it "initialize[s] the in-isolate inspector." Drop the call and keep just the `tracing::info!`, or adjust the comment to note the call is defensive/idempotent.
- [x] `crates/swissarmyhammer-plugin/src/runtime/mod.rs:95-101` — `PluginRuntime`'s doc explicitly states the handle is `Send`; RUST_REVIEW.md calls for compile-time assertions for such guarantees. Add a `const _: fn() = || { fn assert_send<T: Send>() {} assert_send::<PluginRuntime>(); };` (or equivalent) so an accidental `!Send` field is caught at compile time.