---
assignees:
- claude-code
depends_on:
- 01KRRE6XMJAK3WH3EVMPTMZX8M
position_column: todo
position_ordinal: 8b80
project: plugin-arch
title: 'plugin: module loader — relative, bare, and @swissarmyhammer/* imports'
---
## What
Wire a `deno_core::ModuleLoader` so multi-file plugins work without bundling, and host built-ins resolve to virtual modules.

In `crates/swissarmyhammer-plugin/src/runtime/` (module loader submodule):
- **Relative imports** (`./util`, `../shared/foo`) — resolved against the plugin's bundle directory; each loaded module is transpiled the same way as the entry (via the runtime's `deno_ast` path). Imports that resolve OUTSIDE the plugin's directory are rejected (canonicalize + prefix check, same sandbox rule as `swissarmyhammer-js`'s converted resolver).
- **Bare imports** (`lodash`, `zod`) — NOT resolved by the host. Resolution fails with a clear error; the plugin author is expected to bundle npm deps themselves. The host is not an npm client.
- **`@swissarmyhammer/*` imports** — resolved to host-provided virtual modules served from memory: `@swissarmyhammer/plugin` (the SDK), `@swissarmyhammer/app` (generated app types — a runtime no-op stub here; codegen is a separate task). The SDK module's actual contents come from the SDK task; this task provides the resolution + virtual-module plumbing and can serve a stub for `@swissarmyhammer/plugin` until the SDK lands.

## Acceptance Criteria
- [ ] Relative imports inside the plugin dir load and transpile; an import escaping the plugin dir is rejected.
- [ ] A bare import produces a clear, specific error (not a panic, not a silent empty module).
- [ ] `@swissarmyhammer/*` specifiers resolve to in-memory virtual modules.

## Tests
- [ ] Integration test: a plugin entry `.ts` that imports `./helper.ts` (also in a temp dir) — assert the helper's export is usable from the entry.
- [ ] Test: an entry importing `../outside.ts` (above the plugin dir) is rejected.
- [ ] Test: an entry importing `lodash` fails with the bare-import error.
- [ ] Test: an entry importing `@swissarmyhammer/plugin` resolves to the virtual module.
- [ ] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the resolution tests (relative ok, escape rejected, bare rejected, virtual ok) first, then implement.

## Depends on
deno_core runtime.