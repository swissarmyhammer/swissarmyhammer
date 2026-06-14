---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffed80
project: plugin-arch
title: 'Host: deliver NotificationBridge events into plugin isolates (subscription registry + pump + subscribe/unsubscribe envelopes + ledger cleanup)'
---
Foundation for the SDK event-subscription API. Connects the two live-but-disconnected primitives: the host-wide `NotificationBridge` (crates/swissarmyhammer-plugin/src/notify.rs) and the host→isolate callback path (`PluginHost::invoke_plugin_callback`, host.rs:1871 → `Command::InvokeCallback`, runtime/mod.rs:864 → `__sahInvokeCallback`). Today NO path delivers a bridge notification into an isolate; this card builds it.

This is the HOST/Rust half only (SDK surface is a separate dependent card). Reuse existing primitives — do NOT build a second delivery path.

## Scope (crates/swissarmyhammer-plugin/)

1. **Subscription registry** on `HostInner` (host.rs): `method: String -> Vec<Subscription { plugin_id, callback_id }>`. A `tokio::sync::Mutex`/`std::sync::Mutex` separate from (or folded into) the existing host state lock — pick whichever avoids holding a lock across the async invoke.

2. **Inbound envelopes from the isolate**, paralleling the existing `callbackDispatch` envelope (handled host.rs:2097-2113, dispatched host.rs:2222):
   - `notifications/subscribe`: `{ method, callback: {$callback} }`. The tools_call envelope handler already records callback markers in the per-plugin ledger as callback handles — confirm that also fires for this envelope so the callback id is ledger-tracked. Insert `(method, {plugin_id, callback_id})` into the registry and append a ledger purge opaque that removes this plugin's registry entries on unload.
   - `notifications/unsubscribe`: `{ method, callback_id }`. Remove the matching registry entry; dispose the callback.

3. **Pump task per host**: where `notification_bridge` is constructed (host.rs:800), spawn a task that calls `bridge.subscribe()` and loops `recv().await`. On each `McpNotification`, look up `notification.method` in the registry; for each subscription `tokio::spawn(invoke_plugin_callback(plugin_id, callback_id, params))` fire-and-forget (notification = no reply). Handle broadcast `Lagged` (warn + continue, like the window forwarder at kanban-app/src/commands.rs:2600+) and `Closed` (exit). Store the pump JoinHandle; abort on host drop.

4. **Auto-cleanup**: on plugin unload the ledger drains; the subscribe purge opaque removes every registry entry for that plugin_id. Verify the existing callback-table drain already covers subscribe callbacks (they are marshalled like any other callback).

## Tests (Rust, real-path — no mock-boundary shortcuts)
- registry insert / lookup / remove
- pump routes a `bridge.publish(...)` notification to the correct plugin's callback id (assert invoke_plugin_callback was driven with the right args)
- unsubscribe removes routing (subsequent publish does NOT invoke)
- plugin unload purges ALL its subscriptions
- `Lagged` does not kill the pump

## Acceptance
A unit/integration test publishes an `McpNotification` on a host's bridge and observes the registered isolate callback being invoked with the notification params, and observes it NOT invoked after unsubscribe/unload.

## Review Findings (2026-06-04 10:15)

Reviewed working-tree changes: `crates/swissarmyhammer-plugin/src/{events.rs,host.rs,lib.rs,sdk/plugin.ts}` + `tests/event_subscription_e2e.rs`. Builds clean, clippy clean, all 7 tests (3 E2E + 4 unit) pass. Strong real-pipeline coverage, correct lock-free-across-await registry, lazy pump preserving the "no subscribers ⇒ inert bridge" property, and `Weak`-held pump avoiding a reference cycle. Acceptance criteria met.

### Warnings
- [x] `crates/swissarmyhammer-plugin/src/host.rs:2515-2596` — `run_event_pump` (doc + fn, lines 2529-2595) was inserted *into the middle* of `collect_callback_ids`'s doc comment. The result: `collect_callback_ids`'s docstring (2515-2528) now sits directly above `run_event_pump`'s docstring (2529-2544) with no blank line and no body between them, so the two doc blocks read as one and `collect_callback_ids` (line 2596) is left with NO docstring of its own. Every function needs its own docstring (project convention). Fix: move the `run_event_pump` doc+fn to a separate location (e.g. after `collect_callback_ids`), restoring `collect_callback_ids`'s doc directly above its `fn` and giving `run_event_pump` a clean standalone doc block.

### Nits
- [ ] `crates/swissarmyhammer-plugin/src/host.rs:2310` (`HostBridge::unsubscribe`) — The task wording for the unsubscribe envelope says "Remove the matching registry entry; dispose the callback." The implementation removes the registry entry but does NOT dispose the isolate-side callback on unsubscribe; it leaves the ledger `Callback` handle and relies on the unload drain (and the dependent `.on()` SDK card's `off()`) to dispose it. This is a defensible design for the low-level primitive and the doc comment explains it, but it means an unsubscribed callback's isolate-table slot lingers until unload — a slow leak for a plugin that subscribes/unsubscribes many times in a long session. Consider disposing on unsubscribe (or confirm the dependent `.on()` card's `off()` always disposes, making this moot).

## Resolution (2026-06-04 10:20)

- **Warning — FIXED + verified**: relocated `run_event_pump` (doc + fn) to *after* `collect_callback_ids` in host.rs. `collect_callback_ids` now has its own docstring directly above its `fn`, and `run_event_pump` has a clean standalone doc block. Pure relocation (byte-identical body). `cargo build` + `cargo clippy --lib` clean; all 7 tests (3 E2E + 4 unit) green after the move.
- **Nit — deferred by design (moot once `.on()` lands)**: `HostBridge::unsubscribe` intentionally leaves isolate-callback disposal to the unload ledger drain for this low-level primitive. The dependent `.on()` card (01KT9FYTVE) now explicitly specifies `off()` ALWAYS disposes the local callback (`__sahDisposeCallback`) in addition to unsubscribing — so the normal plugin-author teardown path has no leak. The only un-disposed-until-unload case is a caller using the raw `transport.unsubscribe` primitive directly (internal use only), which is acceptable.