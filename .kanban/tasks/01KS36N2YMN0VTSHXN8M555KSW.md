---
assignees:
- claude-code
depends_on:
- 01KS36MCQECBSW48YG7YXQM9N3
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffaf80
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
- [x] `register` with a valid payload returns success and stores the entry; `list` shows it
- [x] `register` with same `(id, caller)` replaces the entry in place — stack height stays the same
- [x] `unregister` removes that caller's entry; `list` no longer shows it (unless a different caller's entry remains)
- [x] `unregister` for an id the caller never registered returns success (no error)
- [x] `register` with missing `execute` callback returns a structured `MissingExecuteCallback` error
- [x] Both verbs schedule a `commands/changed` notification (debounced by the notifications module)
- [x] `CallerId` from `RequestContext::extensions` is recorded with each entry

## Tests
- [x] `crates/swissarmyhammer-command-service/tests/register_unregister.rs` — end-to-end via the service (not the registry directly): construct a `CommandService`, invoke the rmcp tool with `op: "register command"`, then `op: "list command"`, then `op: "unregister command"`, assert the state after each.
- [x] `crates/swissarmyhammer-command-service/tests/register_callback_markers.rs` — payload with `"available": { "$callback": "cb_a" }, "execute": { "$callback": "cb_b" }`; assert the markers are stored on the `StackEntry` and the response confirms registration.
- [x] `crates/swissarmyhammer-command-service/tests/register_caller_isolation.rs` — caller A registers `foo`; caller B unregisters `foo` → no-op (A's entry remains). Only A's unregister removes A's entry.
- [x] `cargo test -p swissarmyhammer-command-service` passes

## Workflow
- Use `/tdd` — write the e2e service tests first; they exercise the verb dispatch and serde paths together.

## Implementation Notes

- Acceptance criterion "`list` shows it" was verified through the read-only `with_registry` accessor (which is what `list command` will project from). The public `list command` verb is still a `todo!()` stub from the prior service-skeleton task and gets implemented in the follow-up `list` + `schema` task (01KS36NMBH23RR2HSYNHX9AZK2). The integration-through-the-`list`-verb path is exercised there.
- Added three structured `CommandError` variants to validate the registration payload: `EmptyId`, `EmptyName { id }`, `MissingExecuteCallback { id }`. The corresponding rmcp errors carry a `data` field of shape `{ "kind": "<VariantName>", ...fields }` so callers branch on the discriminant.
- Created `src/callbacks.rs` with `CallbackHandle { caller, callback_id }` per the task spec, plus the `ensure_callback_present` validator used by `validate_registration`.
- A shared test harness lives at `tests/common/mod.rs` — it mints an inert `Peer<RoleServer>` via `serve_directly` against a closed transport, then builds a `RequestContext` with `CallerId` planted in its extensions. This mirrors what the in-process transport does in production.

## Review Findings (2026-05-27 14:35)

### Warnings
- [x] `crates/swissarmyhammer-command-service/src/service.rs:227-242` — `validate_registration` checks `req.execute.callback_id` non-empty but not `req.available`'s. The same SDK serializer bug that produces an empty `execute` id could produce an empty `available` id. Unlike `execute` (which fails fast with `MissingExecuteCallback`), an empty `available` marker is stored silently and then fails opaquely at dispatch time when the platform tries to route `cb_` back to the originating isolate. Add a parallel branch — when `req.available.is_some()` and its `callback_id` is empty, return a new `MissingAvailableCallback { id }` variant (or extend `MissingExecuteCallback` into a `MissingCallback { id, role }` shape) with the same `kind` discriminant convention.
- [x] `crates/swissarmyhammer-command-service/tests/register_unregister.rs` — No e2e test pins the "both verbs schedule a `commands/changed` notification" acceptance criterion. The notifier module has its own tests, but the *wiring* from `handle_register` / `handle_unregister` through to `notifier.notify()` is currently verified only by code-reading. Add a test that constructs the service via `CommandService::with_notifier_sink(move || counter.fetch_add(1, ...))`, runs a register + an unregister, calls `notifier.flush()` (or sleeps past the debounce), then asserts the counter went to 2. Add a paired test that asserts the no-op `unregister_for_unknown_id` does NOT bump the counter (this pins the intentional `if removed { notify }` guard in `handle_unregister`).

### Nits
- [x] `crates/swissarmyhammer-command-service/src/callbacks.rs:67-73` — `ensure_callback_present` returns `Result<(), ()>` but the `Err(())` carries no information; the only caller immediately discards it and constructs a different error. Change the signature to `fn ensure_callback_present(marker: &CallbackMarker) -> bool` (returning `true` on a present marker), and update the caller to `if !ensure_callback_present(&req.execute) { return Err(...) }`. The `Result<(), ()>` shape suggests the function might grow into a proper validator, but the function body is a single emptiness check — a `bool` predicate is the honest signature.
- [x] `crates/swissarmyhammer-command-service/tests/common/mod.rs:110-123` — `call_tool` takes `op: &str` as a parameter but only uses it as a comment-via-`let _ = op;` — the actual op comes from `arguments["op"]`. The redundant parameter invites callers to pass mismatched values (op `"register command"` with arguments declaring `"unregister command"`) without any check. Either remove the parameter (call sites already self-document via the arguments) or `debug_assert_eq!(arguments.get("op").and_then(Value::as_str), Some(op))` to make it load-bearing.

## Review Response (2026-05-27)

All four findings addressed:

- **Warning 1** (parallel `available` validation): added `CommandError::MissingAvailableCallback { id }` variant and a parallel branch in `validate_registration`. When `req.available.is_some()` and its `callback_id` is empty, registration is rejected with a structured `invalid_params` error whose `data` field is `{ "kind": "MissingAvailableCallback", "id": "<id>" }`. Pinned by the new test `register_with_empty_available_callback_returns_structured_error`.
- **Warning 2** (notification wiring): added `register_and_unregister_each_schedule_a_change_notification` (asserts counter = 2 after a register + unregister with debounce sleeps in between) and `no_op_unregister_does_not_schedule_a_notification` (asserts counter stays at 0 when `pop_caller` returns `false`, pinning the `if removed { notify }` guard).
- **Nit 3** (`Result<(), ()>` signature): renamed `ensure_callback_present` → `is_callback_present` with signature `fn is_callback_present(marker: &CallbackMarker) -> bool`. Caller updated to `if !is_callback_present(&req.execute) { return Err(...) }` for both the `execute` and the new `available` branch.
- **Nit 4** (redundant `op` parameter): the `op: &str` parameter in `common::call_tool` is now load-bearing via `debug_assert_eq!(arguments.get("op").and_then(Value::as_str), Some(op))`. A typo at the call site will fire the assert in debug builds rather than silently running the wrong verb.

`cargo test -p swissarmyhammer-command-service` and `cargo clippy -p swissarmyhammer-command-service --tests -- -D warnings` both pass with zero failures and zero warnings.
