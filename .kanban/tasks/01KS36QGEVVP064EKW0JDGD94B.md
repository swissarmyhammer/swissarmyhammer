---
assignees:
- claude-code
depends_on:
- 01KS36PZK9K6PHTRB9M7YPWTF2
- 01KS371KNY4YARZ67KWVSXPDFP
position_column: todo
position_ordinal: '8680'
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

## Acceptance Criteria
- [ ] `ensureServices(this, ["commands"])` registers the `commands` server if not yet registered; no-op if already registered with the same source
- [ ] After `ensureServices`, `this.commands.command.command.register(...)` works (path sugar from `_meta`)
- [ ] `registerCommands(this, [...])` loops register calls; auto-cleanup via the ledger
- [ ] Calling `ensureServices` from two different plugins in the same host works — second is idempotent no-op
- [ ] If a different plugin registered `commands` with a DIFFERENT source, `ensureServices` returns the `ServerNameTaken` error (per the plugin-arch idempotency task's contract)

## Tests
- [ ] `crates/swissarmyhammer-plugin/tests/integration/ensure_services_e2e.rs` — two probe plugins both call `ensureServices(this, ["commands"])` in `load()`; both succeed; both register a unique command; `list` returns both; unload one — its commands purged but `commands` server remains live for the other
- [ ] `crates/swissarmyhammer-plugin/tests/integration/command_sdk_e2e.rs` — probe plugin uses `ensureServices` + `registerCommands` (the convention) and the direct `this.commands.command.command.register(...)` form; both produce the same observable state; unload purges both
- [ ] `cargo test -p swissarmyhammer-plugin --test integration` passes

## Workflow
- Use `/tdd` — write the two-plugin shared-service test first; it pins the convention.

Depends on the Command service being live in the host (previous task) AND on plugin-arch's `ServerRegistry::register` idempotency task.