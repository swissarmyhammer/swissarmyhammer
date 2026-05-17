---
assignees:
- claude-code
depends_on:
- 01KRRE6XMJAK3WH3EVMPTMZX8M
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffff880
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
- [x] Relative imports inside the plugin dir load and transpile; an import escaping the plugin dir is rejected.
- [x] A bare import produces a clear, specific error (not a panic, not a silent empty module).
- [x] `@swissarmyhammer/*` specifiers resolve to in-memory virtual modules.

## Tests
- [x] Integration test: a plugin entry `.ts` that imports `./helper.ts` (also in a temp dir) — assert the helper's export is usable from the entry.
- [x] Test: an entry importing `../outside.ts` (above the plugin dir) is rejected.
- [x] Test: an entry importing `lodash` fails with the bare-import error.
- [x] Test: an entry importing `@swissarmyhammer/plugin` resolves to the virtual module.
- [x] Run: `cargo test -p swissarmyhammer-plugin` — all green.

## Workflow
- Use `/tdd` — write the resolution tests (relative ok, escape rejected, bare rejected, virtual ok) first, then implement.

## Depends on
deno_core runtime.

## Review Findings (2026-05-17 13:30)

### Warnings
- [x] `crates/swissarmyhammer-plugin/src/runtime/module_loader.rs:271-289` — An absolute-path specifier (e.g. `/etc/passwd`, or a `\`-prefixed path) is not classified up front and falls through to the bare-import branch, where it is *rejected with a misleading error*: "the plugin host does not resolve npm packages — bundle '/etc/passwd' into your plugin bundle yourself". The specifier is rejected, so this is not a sandbox hole — but the diagnosis is wrong and confusing for a plugin author. The reference loader this code explicitly claims to mirror (`swissarmyhammer-js`'s `SandboxedModuleLoader`, lib.rs:157-159) has an explicit up-front guard `if specifier.starts_with('/') || specifier.starts_with('\\') { return Err("absolute import path rejected: ...") }`. Add the same up-front absolute/backslash-prefix check in `resolve` (before the bare-import fallthrough) so an absolute import gets an honest, specific error.

### Nits
- [x] `crates/swissarmyhammer-plugin/src/runtime/module_loader.rs:210-213` — The doc comment on `load_relative` states "The `specifier` has already passed `resolve`, so it is a `file://` URL inside the bundle root." This is not true for the **entry/main module**: `resolve` returns early for `ResolutionKind::MainModule` (lines 263-267) without any canonicalize-and-contain check, so the main module's `file://` URL reaches `load_relative` having skipped the sandbox check. The entry path is host-derived (`bundle_dir.join(entry_file)` in `mod.rs`), so this is safe in scope — but the comment overstates the guarantee. Reword to note the main module is host-chosen and trusted, only *imports* are sandbox-verified, mirroring the accurate explanation already in `resolve`'s own doc comment (lines 240-252).
- [x] `crates/swissarmyhammer-plugin/src/runtime/module_loader.rs:323-329` — `referrer_directory` is a verbatim copy of the identically-named private fn in `swissarmyhammer-js/src/lib.rs:199-205` (same signature, same body, same doc intent). Two private copies of a path-sandboxing helper is a quiet drift hazard: a future fix to one (e.g. handling a `file://` URL with a host component, or a percent-encoded path) will not reach the other. Not blocking — the crates are independent and neither depends on the other today — but worth a note for whoever owns the shared `swissarmyhammer-js` / plugin-runtime boundary, in case a small shared path-utility crate is warranted.