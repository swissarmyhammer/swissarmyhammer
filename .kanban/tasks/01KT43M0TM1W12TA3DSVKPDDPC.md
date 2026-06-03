---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffdf80
project: plugin-arch
title: Host instantiates default-exported Plugin class (drop per-bundle load() boilerplate)
---
Make the plugin entry point match the design doc's Plugin Identity section AND Obsidian style: `export default class MyPlugin extends Plugin { async load() {} }` — the class IS the entry, instantiated by the host.

## Problem
The host strictly requires a named `load` *function* export (`host.rs:94` `LOAD_EXPORT="load"`; `runtime/mod.rs:938-960` fetches the `"load"` namespace key and errors if it's not a function — no default-class support). This diverges from:
- The design doc (`ideas/plugins/plugin-architecture.md`, Plugin Identity): `export default class WeatherPlugin extends Plugin`.
- Obsidian's `export default class MyPlugin extends Plugin`.
- The SDK's own `makePluginThis` docstring, which says the base instance "must be wrapped by `makePluginThis` before its `load` is run" — implying the host wraps it.

Consequence: identical boilerplate at the bottom of all 18 bundles (7 builtin + 11 examples):
```ts
export async function load(): Promise<unknown> {
  const plugin = makePluginThis(new TaskCommandsPlugin()) as TaskCommandsPlugin;
  await plugin.load();
  return null;
}
```

## Fix
Host injects a tiny bootstrap shim that imports the bundle, reads the `default` export, `new`s it, wraps with `makePluginThis`, then calls `.load()` (and `.unload()` on teardown). Reading the `"default"` namespace key is no harder than the current `"load"` lookup (`call_module_export`); the only added step is `new` + the JS-land Proxy wrap, which the shim does in JS.

## Acceptance
- A bundle authored as `export default class X extends Plugin { async load() {} }` (no module-level `load` function, no manual `makePluginThis`) loads, registers, and unloads correctly through a real V8 isolate.
- All 7 builtin plugins + examples migrated to default-class form; the boilerplate `export async function load()` deleted from each.
- `unload` lifecycle still works (host calls the instance's `unload()` method).
- e2e tests updated; full plugin test suite green.

Note: consider keeping the function-export path supported transitionally so the migration stays green per-commit, then removing it.

## Review Findings (2026-06-03 15:20)

Scope: uncommitted working-tree delta for this task. Reviewed `src/sdk/plugin.ts`, `src/runtime/mod.rs`, `src/host.rs`, `src/discovery.rs`, `src/lib.rs`, all 20 migrated bundle `index.ts` files, and the test-fixture migrations.

### Acceptance criteria
- AC1 (default-class bundle loads/registers/unloads through a real isolate): MET. `runtime/mod.rs` test `default_class_plugin_loads_and_unloads` drives Load then Unload on a real isolate, asserts `load()`'s `'loaded'` return propagates and the stored instance's `unload()` runs (sets `__unloaded`), proving the isolate-local instance slot persists across the two separate host calls. `default_class_missing_default_export_is_reported` proves a missing `default` errors clearly naming `default`.
- AC2 (all builtin + examples migrated; `export async function load()` deleted): MET. All 20 entry `index.ts` files (8 builtin + 11 examples + 1 test fixture) have `export default class`; repo-wide grep for `export async function load` / `export function load` / `makePluginThis` across `**/plugins/**/*.ts` and the test suite returns zero matches. Spot-checked cli-echo (keeps its `unload()` override + `super.unload()`), multi-module (sibling import preserved), ensure-services-a, command-sdk-direct, kanban-builtin-probe, builtin-probe — all logic preserved inside the class; old boilerplate did only new+wrap+load, so nothing was lost.
- AC3 (`unload` still works via the instance method): MET. `host.rs::run_plugin_unload` drives `PluginLifecycle::Unload`; SDK `__sahUnloadDefaultPlugin` reaches the stored wrapped instance and runs its `unload()`, then clears the slot. Unload-without-load and double-unload are clean no-ops (slot `undefined` → returns `null`). `record_crashed` correctly skips the hook for a dead isolate.

### Verification of scrutiny points
- Wrap location: `hostLoadDefaultPlugin` runs `makePluginThis(new ctor())` and stores/awaits the WRAPPED instance, so `this.<server>` dispatch works inside `load()`/`unload()`. Correct.
- Return propagation: `hostLoadDefaultPlugin` returns `await instance.load()`; host ignores the value (only checks `Ok`), matching old behavior.
- Removed fallback: `LOAD_EXPORT`/`UNLOAD_EXPORT` fully removed; no remaining `"load"`-export resolution; no bundle/test still using the old form.
- Pre-existing failure `committed_examples_coload_across_layers`: CONFIRMED independent of this change. The probe bundle's `register("kanban", { rust: "kanban" })` line is unchanged by this diff (it registers under `"kanban"` at HEAD and after), while the test (clean, no uncommitted changes) expects server name `kanban-builtin-probe`. The migration only converted the class to `export default` and dropped the boilerplate — the server-name mismatch is pre-existing, not newly broken here.

### Blockers
(none)

### Warnings
(none)

### Nits
(none)

CLEAN — no blockers, warnings, or nits. All three acceptance criteria met.