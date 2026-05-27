---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffac80
project: plugin-arch
title: Make `ServerRegistry::register` idempotent for same-(name, source) registrations
---
## What

Make `this.register(name, source)` smart and forgiving: if a plugin (or the host) registers a server with a name+source that already matches an existing live registration, treat it as a no-op rather than failing with `ServerNameTaken`. Plugins shouldn't have to coordinate with each other or with the host to defensively check "is this already registered?" before calling `register`.

The current architecture (plugin-architecture.md *Name collisions*) says:
> The first registration of a name wins; subsequent attempts fail with `ServerNameTaken`.

That's too strict. Two plugins that both depend on the same external MCP server (e.g., a community `weather` server) should both be able to call `register("weather", { cli: ["npx", "weather-server"] })` in their `load()` without one of them blowing up.

Files (in `crates/swissarmyhammer-plugin/`):
- `crates/swissarmyhammer-plugin/src/registry.rs` — change `register(name, source)` semantics:
  - If `name` is not present → register it (as today)
  - If `name` IS present AND the existing source is structurally equal to the new source → no-op, return success, ledger entry is appended so unregister-on-unload still works correctly (refcounting per registering caller)
  - If `name` IS present AND sources differ → return `ServerNameTaken` (today's behavior, kept for the genuinely-conflicting case)

Source equality: `{ url: same }`, `{ cli: same args, same env, same cwd }`, `{ rust: same id }`. Compare by structural equality, not pointer.

Per-caller refcounting: each registering plugin's ledger gains an entry. On unload, the entry is removed; the underlying server stays live until the last caller unregisters. Only then does the server actually shut down (subprocess killed, HTTP connection closed, etc.).

## Acceptance Criteria
- [ ] `register(name, source)` followed by `register(name, source)` (same name AND same source) from any caller returns success without panicking or erroring
- [ ] Two plugins both registering the same external `weather` server with the same CLI line both succeed; both have ledger entries; one unloading leaves the server live; both unloading shuts it down
- [ ] `register(name, source_a)` followed by `register(name, source_b)` (different sources) still returns `ServerNameTaken` — we don't silently shadow with a different implementation
- [ ] The host's startup-registered servers (`commands`, `window`, `app`, etc.) are immune to plugins registering different sources under those names (existing behavior; the name-taken error still applies for mismatched source)

## Tests
- [ ] `crates/swissarmyhammer-plugin/tests/registry_idempotent.rs` — directly test the registry: register; register again same source; assert success and only one underlying server. Register a different source → `ServerNameTaken`.
- [ ] `crates/swissarmyhammer-plugin/tests/integration/multi_plugin_shared_server_e2e.rs` — two probe plugins both register the same CLI-sourced server; both load; both call a tool on it; unload one → server still live; unload the other → server shut down (subprocess no longer in the process table or whatever observable proxy fits)
- [ ] `crates/swissarmyhammer-plugin/tests/registry_source_mismatch.rs` — register with one URL, then register same name with a different URL; assert `ServerNameTaken`
- [ ] `cargo test -p swissarmyhammer-plugin` passes

## Workflow
- Use `/tdd` — write the multi-plugin shared-server e2e first; it's the headline scenario.

Tagged plugin-arch because this is a platform property, not specific to command-service.