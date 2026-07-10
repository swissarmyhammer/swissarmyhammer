---
assignees:
- claude-code
depends_on:
- 01KS36PZK9K6PHTRB9M7YPWTF2
- 01KS371KNY4YARZ67KWVSXPDFP
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb380
project: command-service
title: 'SDK helpers: `ensureServices` + `registerCommands` (plugin convention)'
---
## What

Add two SDK helpers in the plugin platform's TypeScript runtime that codify the convention every command-registering plugin follows:

1. `ensureServices(plugin, serviceNames[])` — registers each named service from the host's exposed-modules table, idempotently. Relies on the plugin-arch idempotency work: if another plugin already registered the same `(name, source)`, this call is a no-op.
2. `registerCommands(plugin, commands[])` — loops over an array of `CommandRegistration` objects calling `plugin.commands.command.command.register(...)` for each. Returns disposables (auto-tracked by the platform ledger).

The convention: a plugin's `load()` first calls `ensureServices(this, ["commands"])` (plus `"window"`, `"app"` if needed), THEN calls `registerCommands(this, [...])`. This makes plugins self-contained — they declare which services they depend on rather than assuming the host pre-registered them.

```ts
// Convention for any command-registering plugin's load():
async load() {
  await ensureServices(this, ["commands", "window"]);
  await registerCommands(this, [
    { id: "task.move", name: "Move Task", /* ... */, execute: async (ctx) => { ... } },
    /* ... */
  ]);
}
```

Files (in `crates/swissarmyhammer-plugin/` — virtual `@swissarmyhammer/plugin` module served to plugins):
- `crates/swissarmyhammer-plugin/src/sdk/services.ts` — `ensureServices(plugin, names[])`. Calls `plugin.register(name, { rust: <host-side id> })` for each. Looks up the rust-side id from a small lookup table provided by the host (`commands` → `command_service`, `window` → `window_service`, `app` → `app_service`, etc.). The lookup table is part of `@swissarmyhammer/plugin` and updated as services are added.
- `crates/swissarmyhammer-plugin/src/sdk/commands.ts` — `registerCommands(plugin, commands[])`

Why `ensureServices` exists rather than letting plugins call `this.register("commands", { rust: "command_service" })` directly: the rust-side id (`command_service` vs `commands` the public name) is a host-internal detail; the helper lets the host evolve the underlying module name without breaking plugins. Plugins just say "I need the commands service."

## Implementation

The plan-stipulated files are created and the convention helpers are added to the embedded SDK:

- `crates/swissarmyhammer-plugin/src/sdk/services.ts` — exports `RUST_MODULE_IDS` (`commands` → `commands` for now), `UnknownService`, and `async function ensureServices(plugin, names[])` which loops calling `plugin.register(name, { rust: lookup[name] })`. Idempotency comes from the plugin-arch registry change: same-`(name, source)` registrations refcount-merge instead of erroring.
- `crates/swissarmyhammer-plugin/src/sdk/commands.ts` — exports `CommandRegistration` interface and `async function registerCommands(plugin, commands[])` which loops `plugin.commands.command.command.register(...)` for each entry. Returns the array of host responses.

The three SDK files (`plugin.ts`, `services.ts`, `commands.ts`) are concatenated at build time into one virtual `@swissarmyhammer/plugin` module via `crate::sdk::combined_sdk_source()`, so the helpers share scope with `Plugin` / `PluginThis` without needing `import` statements.

Closing the previous task's noted SDK gap (callback marshalling for command registration): the SDK's `HostBridge.toolsCall` now runs each `arguments` payload through `marshalCallbacks`, replacing any function value with a `{$callback: "cb_..."}` marker before dispatch. The marshalling is a no-op for plain-data tool calls (the URL/CLI verbatim path is unchanged), but the in-process command service now receives the marker shape it expects for `execute`/`available` fields. The host's `tools_call` envelope handler scans args for callback markers and records each id in the plugin's ledger as a `RegistrationHandle::Callback`, so the isolate's callback table is drained on unload.

Example bundles cover both forms:
- `examples/plugins/ensure-services-a/` and `examples/plugins/ensure-services-b/` — both call `ensureServices(this, ["commands"])` then `registerCommands(this, [...])` with a distinct command id each.
- `examples/plugins/command-sdk-direct/` — uses `registerCommands` for one command AND the direct `this.commands.command.command.register(...)` form for another, exercising both paths in one bundle.

## Acceptance Criteria
- [x] `ensureServices(this, ["commands"])` registers the `commands` server if not yet registered; no-op if already registered with the same source
- [x] After `ensureServices`, `this.commands.command.command.register(...)` works (path sugar from `_meta`)
- [x] `registerCommands(this, [...])` loops register calls; auto-cleanup via the ledger
- [x] Calling `ensureServices` from two different plugins in the same host works — second is idempotent no-op
- [x] If a different plugin registered `commands` with a DIFFERENT source, `ensureServices` returns the `ServerNameTaken` error (per the plugin-arch idempotency task's contract) — `ensureServices` does not catch, so the error propagates verbatim; the contract itself is pinned by `tests/registry_source_mismatch.rs` and the per-call path through `connect_and_register` surfaces it the same way for a plugin caller.

## Tests
- [x] `crates/swissarmyhammer-plugin/tests/integration/ensure_services_e2e.rs` — two probe plugins both call `ensureServices(this, ["commands"])` in `load()`; both succeed; both register a unique command; the registry surfaces both; unload one — its commands purged but `commands` server remains live for the other; unload the other — both commands purged, refcount drops to zero, server torn down.
- [x] `crates/swissarmyhammer-plugin/tests/integration/command_sdk_e2e.rs` — probe plugin uses `ensureServices` + `registerCommands` (the convention) AND the direct `this.commands.command.command.register(...)` form; both produce the same observable state (both ids land on the registry); unload purges both via the per-plugin ledger.
- [x] `cargo test -p swissarmyhammer-plugin --test integration` passes (2/2)
- [x] `cargo test -p swissarmyhammer-plugin` passes (full suite green, no regressions)
- [x] `cargo test -p swissarmyhammer-command-service` passes (full suite green, no regressions)

## Workflow
- Use `/tdd` — write the two-plugin shared-service test first; it pins the convention.

Depends on the Command service being live in the host (previous task) AND on plugin-arch's `ServerRegistry::register` idempotency task.

## Review Findings (2026-05-27 11:29)

### Nits
- [x] `crates/swissarmyhammer-plugin/tests/callbacks.rs:296` — Test name `tool_call_payloads_are_not_scanned_for_callbacks` is now stale. The SDK's `toolsCall` path DOES run payloads through `marshalCallbacks`; what the test actually pins is that a plain-data payload (no function values) passes through unchanged. The docstring above the test was updated to reflect this, but the function name still asserts the opposite of the new behavior. Rename to something like `plain_tool_call_payloads_pass_through_unchanged` so the name matches the contract. **Fixed**: renamed to `plain_tool_call_payloads_pass_through_unchanged`; only the kanban history references the old name.
- [x] `crates/swissarmyhammer-plugin/tests/integration/ensure_services_e2e.rs:128-148` — Step 4's comment ("a `list_via_host` call afterward would surface `ServerUnavailable`, and that distinction is exactly what the refcount design exists for") describes a stronger property than the assertions actually check — the test only inspects the service-side registry, never re-invokes `list_via_host`. Adding `assert!(matches!(list_via_host(&bootstrap.host).await, Err(Error::ServerUnavailable)))` (or equivalent) would tighten the test to match the stated contract; right now the refcount-to-zero teardown of the `commands` server is only inferred indirectly via the empty service registry. **Fixed**: added the `matches!(..., Err(Error::ServerUnavailable))` assertion after Step 4's purge checks; imported `swissarmyhammer_plugin::Error` for it. Integration suite still 2/2.
- [x] `crates/swissarmyhammer-plugin/src/sdk/services.ts:56-62` — `RUST_MODULE_IDS` currently has one entry where the public name and internal id are identical (`commands` → `commands`). The indirection's value is real (lets the host evolve internal ids) and the documentation explains it well, but the present table can read as redundant. Consider a brief reminder comment in the table itself (e.g. `// the public/internal split is structural — entries may diverge as services migrate`) so a future maintainer does not "simplify" by collapsing the map. **Fixed**: added an in-table `NOTE` warning that the public/internal split is structural and must not be collapsed even when every key currently equals its value.