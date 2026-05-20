---
assignees:
- claude-code
depends_on:
- 01KS36MCQECBSW48YG7YXQM9N3
position_column: todo
position_ordinal: '8280'
project: command-service
title: Implement `register` + `unregister command` verbs (with callback markers)
---
## What

Fill in the `register command` and `unregister command` verb handlers. The interesting work is callback marker handling — the SDK strips `available`/`execute` functions before sending the registration payload across the boundary, replacing them with `{ "$callback": "cb_..." }` markers (the universal plugin-platform primitive). The service stores those markers; later `execute`/`available` verb handlers send `notifications/callbacks/invoke` back to the registering isolate.

Files:
- `crates/swissarmyhammer-command-service/src/service.rs` — implement `register` and `unregister` arms in the dispatch
- `crates/swissarmyhammer-command-service/src/callbacks.rs` — `CallbackHandle { caller, callback_id }`, helpers to extract and store markers from incoming JSON

Behavior:
- `register`: validate payload (id non-empty, name non-empty, execute callback present), extract `available` + `execute` markers, push `StackEntry { caller, reg }` into the registry, schedule a `commands/changed` notification, return the active entry's snapshot. Idempotent for same-caller-same-id (replaces in place).
- `unregister`: pop the calling-caller's entry for this id; schedule notification. If the caller has no entry for that id, return a no-op success (don't error — plugin unload purges may race with explicit unregister).

`CallerId` comes from the rmcp `RequestContext::extensions` (per the architecture doc's CallerId propagation pattern). The platform stuffs it in; the service reads it.

Callback dispose: the architecture's per-plugin ledger expects an `Opaque(Box<dyn FnOnce>)` dispose-fn. This task does NOT wire that up (that lives in the platform-integration task). It just stores the markers so later tasks can invoke them.

## Acceptance Criteria
- [ ] `register` with a valid payload returns success and stores the entry; `list` shows it
- [ ] `register` with same `(id, caller)` replaces the entry in place — stack height stays the same
- [ ] `unregister` removes that caller's entry; `list` no longer shows it (unless a different caller's entry remains)
- [ ] `unregister` for an id the caller never registered returns success (no error)
- [ ] `register` with missing `execute` callback returns a structured `MissingExecuteCallback` error
- [ ] Both verbs schedule a `commands/changed` notification (debounced by the notifications module)
- [ ] `CallerId` from `RequestContext::extensions` is recorded with each entry

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/register_unregister.rs` — end-to-end via the service (not the registry directly): construct a `CommandService`, invoke the rmcp tool with `op: "register command"`, then `op: "list command"`, then `op: "unregister command"`, assert the state after each.
- [ ] `crates/swissarmyhammer-command-service/tests/register_callback_markers.rs` — payload with `"available": { "$callback": "cb_a" }, "execute": { "$callback": "cb_b" }`; assert the markers are stored on the `StackEntry` and the response confirms registration.
- [ ] `crates/swissarmyhammer-command-service/tests/register_caller_isolation.rs` — caller A registers `foo`; caller B unregisters `foo` → no-op (A's entry remains). Only A's unregister removes A's entry.
- [ ] `cargo test -p swissarmyhammer-command-service` passes

## Workflow
- Use `/tdd` — write the e2e service tests first; they exercise the verb dispatch and serde paths together.