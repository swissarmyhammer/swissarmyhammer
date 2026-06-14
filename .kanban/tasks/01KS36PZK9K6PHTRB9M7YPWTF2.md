---
assignees:
- claude-code
depends_on:
- 01KS36P9C8CFT5HMQWY2WCA9ZE
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb280
project: command-service
title: Wire Command service into host bootstrap + ledger-driven auto-cleanup
---
## What

Expose the Command service as an in-process MCP server through `swissarmyhammer-tools`' bootstrap, and integrate with the per-plugin ledger so plugin unload auto-purges that plugin's registrations.

Files:
- `crates/swissarmyhammer-tools/src/mcp/server.rs` (or whichever bootstrap location plugin-arch chose) — `host.expose_rust_module("commands", CommandService::new(callback_dispatcher.clone()))`
- `crates/swissarmyhammer-plugin/src/ledger.rs` (touched, not owned by this task) — Command service hooks an `Opaque(Box::new(move || registry.purge_caller(caller)))` into the ledger at construction so the platform can dispose without knowing the service's internals

## Implementation

Bootstrap landed in `crates/swissarmyhammer-command-service/src/bootstrap.rs` rather than `swissarmyhammer-tools` — the latter is downstream and unaware of this workspace's plugin crates. The bootstrap entry point is `install_commands_module(&PluginHost) -> Arc<CommandService>`.

Three seams wire the service into the platform:

1. **Callback dispatcher** (`HostCallbackDispatcher`) — implements `CallbackDispatcher`. For a `CallerId::Plugin(plugin_id)`, routes invocations through a new `PluginHost::invoke_plugin_callback` which uses a new `CallbackInvoker` (cloned worker-channel sender) so the await happens outside the host's state mutex.
2. **Caller lifecycle** (`HostCallerLifecycle`) — implements a new `CallerLifecycle` trait on the service. For a `CallerId::Plugin(plugin_id)`, calls a new `PluginHost::record_unload_hook` which appends a `RegistrationHandle::Opaque(purge_fn)` to the calling plugin's ledger. The hook fires when `PluginHost::unload` drains the ledger.
3. **Module exposure** — wraps the service in `InProcessServer::from_arc` and registers via `host.expose_rust_module("commands", ...)`.

`CommandService::handle_register` now calls `install_unload_hook_for(&caller)` after every successful register. Hook installation is deduped through an `installed_hooks: Mutex<HashSet<CallerId>>` on the service so a plugin that registers N commands appends exactly one opaque entry to the host ledger, not N. `purge_caller` clears the caller from `installed_hooks` so a reloaded plugin (same `PluginId`) gets a fresh hook on its next register.

`CommandService::purge_caller(&CallerId)` is exposed publicly so the bootstrap's hook closure (and external test code) can drive purges directly. It schedules a debounced `commands/changed` notification and immediately flushes — so the post-unload state is visible to subscribers without waiting for the debounce window.

The override stack re-emergence works because `CommandRegistry::push` already replaces a caller's existing entry in place; `purge_caller` removes only that caller's entries from every per-id stack; and the top-of-stack semantics surface the next-most-recent entry. So A → B → unload B → A re-emerges naturally falls out.

## Acceptance Criteria
- [x] `swissarmyhammer-command-service::bootstrap::install_commands_module` exposes `commands` as an in-process server at host startup
- [x] Calling `tools/call("command", { op: "register command", ... })` from any caller (host or plugin) succeeds and lands on the override stack
- [x] When a plugin unloads, all its registrations are purged automatically via the per-plugin ledger drain (no plugin-side `unregister` required)
- [x] Override stack re-emergence works after plugin unload: A → B → B unloads → A re-emerges → A unloads → host's original re-emerges
- [x] `notifications/commands/changed` is flushed at the plugin-unload boundary (hook calls `notifier.flush()`)

## Tests
- [x] `crates/swissarmyhammer-command-service/tests/integration/host_bootstrap_e2e.rs` — three tests covering module exposure, host-caller register surfacing through `list command`, and one-shot bootstrap collision rejection
- [x] `crates/swissarmyhammer-command-service/tests/integration/unload_cleanup_e2e.rs` — two tests: real probe plugin loaded, registrations driven from Rust attributed to its `PluginId`, unload purges every entry and clears the per-plugin ledger; plus a duplicate-purge safety test
- [x] `crates/swissarmyhammer-command-service/tests/integration/override_stack_e2e.rs` — two tests: full host → A → B → unwind sequence verified by `active_caller` + `stack_depth` snapshots at every step; plus a reload-uses-fresh-caller-slot test. Sentinel-file execute callbacks were skipped pending SDK exposure of `callbackDispatch` for command registration — the active-caller snapshot proves the same override semantics deterministically without the SDK gap.
- [x] `cargo test -p swissarmyhammer-command-service --test integration` passes (7/7)

## Workflow
- Use `/tdd` — write `override_stack_e2e.rs` first; it's the headline scenario from command-service.md and exercises the full stack.

Depends on plugin-arch tasks: per-plugin ledger, callback dispatcher, `expose_rust_module`, host bootstrap.

## Review Findings (2026-05-27 15:55)

### Warnings
- [x] `crates/swissarmyhammer-command-service/src/service.rs:175-202` — Public `CommandService::purge_caller` only calls `notifier.notify()` (debounced), but the task description and the `install_unload_hook_for` doc comment both say purge "immediately flushes — so the post-unload state is visible to subscribers without waiting for the debounce window." The flush only happens inside the unload-hook closure (`install_unload_hook_for`, lines 290-311), not in the public method. Either (a) move the `flush()` call into `purge_caller` itself so direct callers get the documented behavior, or (b) tighten the doc to make clear the public method is the no-flush primitive and only the unload-hook path flushes. Today an external caller invoking `purge_caller` directly to clean up its own state (the explicitly documented use case at line 179-181) waits up to 100ms before subscribers see the change, contradicting the description.
  - **Resolution:** Took option (a). `purge_caller` now calls `notifier.notify()` followed by `notifier.flush()` when (and only when) an entry actually changed. The unload-hook closure in `install_unload_hook_for` retains its identical inline notify+flush sequence — both paths give external subscribers immediate visibility of the post-purge state. Doc comment on `purge_caller` updated to spell out the flush semantics explicitly; tests confirm idempotent (no-op) purges still skip notification.

### Nits
- [x] `crates/swissarmyhammer-command-service/src/service.rs:247-279, 290-311` — `handle_register` installs a fresh unload hook on every successful register. A plugin that registers N commands appends N `RegistrationHandle::Opaque` entries to the host ledger, each holding `Arc<Mutex<CommandRegistry>> + Arc<ChangeNotifier> + CallerId`. On unload, all N hooks run — the first does the purge, the remaining N-1 are no-ops. The trade-off ("simpler bookkeeping" per the inline comment and task description) is reasonable for early-stage plugins, but for a future plugin registering ~100 commands the ledger bloat and N-1 wasted mutex acquisitions are observable. Consider tracking installed callers in a `HashSet<CallerId>` on the service so the hook is installed at most once per caller. Not blocking — the current design is documented and bounded.
  - **Resolution:** Added `installed_hooks: Mutex<HashSet<CallerId>>` to `CommandService`. `install_unload_hook_for` now consults the set; first call for a caller inserts and installs the hook, subsequent calls return immediately. `purge_caller` removes the caller from the set so a reloaded plugin (which reuses the same `PluginId`) reinstalls a hook on its next register. Per-plugin ledger growth is now constant per caller regardless of how many commands they register. All 7 integration tests still pass — including `unload_purges_every_command_the_plugin_registered`, which registers two commands and now verifies a single hook still purges both.
- [x] `crates/swissarmyhammer-command-service/src/bootstrap.rs:178-198` — When `HostCallerLifecycle::install_unload_hook` calls `record_unload_hook` and the plugin is no longer tracked (returns `false`), the hook is silently dropped with only a `tracing::debug!`. This can happen in a narrow race window: a plugin's `register command` succeeds and the service mutates its registry, but between then and the hook installation the host's `unload` already drained the ledger. The plugin's registry entry would leak until another caller's purge or the next host bootstrap removes it. Race is unusual (register-during-unload of the calling plugin is atypical) but worth noting — consider promoting the log to `warn!` or returning an error the service can surface, so a leak is at least observable.
  - **Resolution:** Promoted `tracing::debug!` to `tracing::warn!` in `HostCallerLifecycle::install_unload_hook` and expanded the message to call out the potential leak window explicitly. The inline doc comment now spells out the race instead of claiming there is none. The leak itself is bounded (it self-heals on the next bootstrap or any other caller's purge for the same caller id) and the race is atypical, so a structured error return would be over-engineering; observability via `warn!` is the right floor.
- [x] `crates/swissarmyhammer-command-service/tests/integration/override_stack_e2e.rs:1-242` — The override-stack tests verify `active_caller` + `stack_depth` instead of executing sentinel callbacks. The implementer flagged this gap (SDK `callbackDispatch` helper not yet exposed for command registration). Acceptable for this task's scope because (a) override-stack semantics are a pure function of registry state, not callback execution, and (b) the callback dispatch path is independently covered by `HostCallbackDispatcher` and its plugin-side wiring. Worth a follow-up task in the SDK-completion thread to round-trip a sentinel `execute` callback once the SDK helper lands.
  - **Resolution:** Accepted as-is for this task's scope, per the reviewer's own assessment. Follow-up task filed as `01KSN2ADZJY04E2YB8F2WBWJYP` ("Round-trip sentinel execute callbacks in override-stack tests once SDK callbackDispatch lands") to track the SDK-completion thread work.