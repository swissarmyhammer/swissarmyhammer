---
assignees:
- claude-code
depends_on:
- 01KRRE3C4RDD999B43BRWJMA3J
position_column: todo
position_ordinal: 8a80
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
- [ ] A plugin runtime type exists that creates a fresh `deno_core::JsRuntime` isolate and tears it down cleanly.
- [ ] A `.ts` source string is transpiled via `deno_ast` and evaluated in the isolate; TS-only syntax (types, interfaces) does not cause a runtime error.
- [ ] Source maps are produced and registered for the inspector.
- [ ] No type-checking is performed at load (a type error in TS still runs).

## Tests
- [ ] Unit/integration test: feed a `.ts` snippet with type annotations + an exported function, evaluate it in a fresh isolate, assert the function's observable result (e.g. it returns a value or sets a global the test reads back).
- [ ] Test that two isolates are independent — a global set in one is not visible in the other.
- [ ] Test that deliberately type-incorrect-but-syntactically-valid TS still transpiles and runs.
- [ ] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the transpile-and-run isolate tests first, then implement.

## Depends on
swissarmyhammer-plugin crate scaffold.