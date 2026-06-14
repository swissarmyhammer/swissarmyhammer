---
assignees:
- claude-code
depends_on:
- 01KRRE6XMJAK3WH3EVMPTMZX8M
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff980
project: plugin-arch
title: 'plugin: TypeScript SDK — Plugin base class and generic dispatch Proxy'
---
## What
Author the TypeScript SDK served as the `@swissarmyhammer/plugin` virtual module — the `Plugin` base class plus the generic dispatch Proxy that turns `this.<server>.<tool>...(args)` into MCP `tools/call`s.

SDK source lives in `crates/swissarmyhammer-plugin/src/sdk/` (TS files embedded into the binary, served as the virtual module).
- `abstract class Plugin`: optional `load()`/`unload()`; `register(name, source)` / `unregister(name)`; `log: Logger`; `track(d): Disposable`; and the dynamic `[server: string]: ServerDispatcher` index via Proxy.
- `ServerSource` union: `{url, headers?}` | `{cli, env?, cwd?}` | `{rust}`.
- `makeDispatcher(transport, server, path)` — Proxy over a function: every property access extends the call `path`; calling the leaf invokes `transport.callPath(server, path, input)`. `RESERVED` names (`on/off/once/subscribe/unsubscribe`, `then`) are not forwarded as path segments.
- `makePluginThis(transport, base)` — Proxy over the base instance: base methods pass through, unknown props become server dispatchers.
- `transport.callPath` resolves shape from the **cached `Tool` definition** fetched via `tools/list`:
  - flat tool (no `io.swissarmyhammer/operations` in `_meta`) → `tools/call(tool, args)`.
  - operation tool, path `[tool, noun, verb]` → look up `_meta…operations[noun][verb].op`, dispatch `tools/call(tool, { op, ...args })`.
  - operation tool, direct form `[tool]` with `op` already in args → pass through.
  - unknown noun/verb → `UnknownOperation` listing valid verbs from `_meta`.
- The transport's actual wire calls cross to the host via the runtime seam from the deno_core task (a host op / channel). Host-side `register`/`unregister` fully wiring into `ServerRegistry` is part of the PluginHost task — here, define the transport interface and the SDK behavior; a host bridge stub is acceptable for unit testing the Proxy logic.

## Acceptance Criteria
- [x] `Plugin` base class and the `makeDispatcher`/`makePluginThis` Proxies exist in the SDK and are served as `@swissarmyhammer/plugin`.
- [x] Path-form operation calls compile to `tools/call(tool, {op, ...args})` using `op` looked up from `_meta`.
- [x] Flat tools dispatch as `tools/call(tool, args)`.
- [x] An unknown noun/verb path raises `UnknownOperation`; an unknown server raises `UnknownServer` at dispatch time.
- [x] `RESERVED` names are not treated as path segments.

## Tests
- [x] Integration test through a real isolate: load a plugin whose `load()` exercises the Proxy against a registered fake/real server with a known `_meta` tree; assert the wire call shape observed by the transport (tool name + `{op, ...}` map) for both a path-form operation call and a flat call.
- [x] Test: an unknown verb path produces `UnknownOperation`; assert the error lists the valid verbs.
- [x] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — assert the produced wire-call shapes first, then implement the Proxy.

## Depends on
deno_core runtime (provides the isolate + the SDK→host seam).

## Review Findings (2026-05-17 14:54)

All five acceptance criteria are met and verified by the integration tests. The `PluginThis<T> = T & Record<string, ServerDispatcher>` deviation from the architecture doc's class-index-signature sketch is a sound, faithful realization — a class index signature would force every declared member (`load`, `register`, `log`, …) to be `ServerDispatcher`, which is not type-correct; the intersection type at the `makePluginThis` boundary models the exact runtime behavior (declared members pass through `Reflect.get`, all other string keys yield dispatchers). The `then`-reads-`undefined` thenable guard, the `RESERVED` handling, the `_meta`-key-driven flat-vs-operation detection, the direct-form pass-through, the per-server `tools/list` cache with its `null` negative-cache sentinel, and the hand-written `RuntimeConfig` `Debug` impl (correct — `Arc<dyn HostDispatcher>` is not `Debug`, and consistent with the existing manual `fmt::Debug` on `ServerRegistry`) are all correct. The findings below are documentation/style only.

### Warnings
- [x] `crates/swissarmyhammer-plugin/src/sdk/plugin.ts` (throughout) — The SDK is authored with Rust-style `///` doc comments rather than TypeScript `/** */` JSDoc blocks. TypeScript tooling, editors, and doc generators treat `///` as ordinary `//` line comments, so none of this documentation surfaces for plugin authors who consume `Plugin`, `register`, `ServerSource`, `Logger`, etc. — and the `{@inheritDoc Transport.register}` / `{@inheritDoc Transport.unregister}` / `{@inheritDoc Transport.callPath}` tags (lines 181, 187, 217) and `{@link}` references silently never resolve. The code transpiles fine (swc strips all comments), so this is not a correctness bug, but the developer-facing API contract is effectively invisible. Convert the SDK's doc comments to `/** */` JSDoc blocks so editor hovers, `{@link}`, and `{@inheritDoc}` work.

### Nits
- [x] `crates/swissarmyhammer-plugin/src/sdk/plugin.ts:228` — The catch clause binds `catch (err)`. The project JS/TS guideline (JS_TS_REVIEW.md) requires catch clauses to bind `error`, not `e`/`err`/`ex`. Rename to `catch (error)`.
