---
assignees:
- claude-code
depends_on:
- 01KRRE6XMJAK3WH3EVMPTMZX8M
position_column: todo
position_ordinal: 8c80
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
- [ ] `Plugin` base class and the `makeDispatcher`/`makePluginThis` Proxies exist in the SDK and are served as `@swissarmyhammer/plugin`.
- [ ] Path-form operation calls compile to `tools/call(tool, {op, ...args})` using `op` looked up from `_meta`.
- [ ] Flat tools dispatch as `tools/call(tool, args)`.
- [ ] An unknown noun/verb path raises `UnknownOperation`; an unknown server raises `UnknownServer` at dispatch time.
- [ ] `RESERVED` names are not treated as path segments.

## Tests
- [ ] Integration test through a real isolate: load a plugin whose `load()` exercises the Proxy against a registered fake/real server with a known `_meta` tree; assert the wire call shape observed by the transport (tool name + `{op, ...}` map) for both a path-form operation call and a flat call.
- [ ] Test: an unknown verb path produces `UnknownOperation`; assert the error lists the valid verbs.
- [ ] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — assert the produced wire-call shapes first, then implement the Proxy.

## Depends on
deno_core runtime (provides the isolate + the SDK→host seam).