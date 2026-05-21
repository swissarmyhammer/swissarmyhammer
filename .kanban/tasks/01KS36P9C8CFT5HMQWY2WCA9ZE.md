---
assignees:
- claude-code
depends_on:
- 01KS36NMBH23RR2HSYNHX9AZK2
position_column: todo
position_ordinal: '8480'
project: command-service
title: Implement `execute` + `available command` verbs (callback round-trip + latency budget)
---
## What

The two verbs that cross the host/plugin boundary, plus the transaction/action-event bracketing that makes a command a coherent unit. Both look up the active stack entry for the id, then invoke `available` then `execute` callbacks in the registering caller's isolate.

Files:
- `crates/swissarmyhammer-command-service/src/service.rs` — `execute` and `available` arms
- `crates/swissarmyhammer-command-service/src/invoke.rs` — `invoke_callback(caller, callback_id, args)` via the platform callback dispatcher
- `crates/swissarmyhammer-command-service/src/latency.rs` — soft latency budget for `available`

### available
Look up the active entry; no `available` callback → `{ ok: true }`. Else invoke with `ctx`, enforcing the budget: >5ms warn, >50ms force `{ ok:false, reason:"available timeout" }` and cancel. Return boolean | `{ ok:false, reason }`.

### execute (with transaction bracketing + action event)
1. Recheck `available` unless `force:true`; reject `CommandUnavailable { reason }` if false.
2. **Open a transaction**: generate a `txn` id and stamp it into the call context (`RequestContext::extensions`, alongside `CallerId`) so every downstream store write the callback makes shares one undo group AND tags its emitted change events with this `txn` (see the `store` server + notification-surface tasks). Also set `origin` from `CallerId` (user/agent).
3. Invoke the `execute` callback with `ctx`.
4. **Close the transaction** after the callback resolves (success or error).
5. On success, **emit the action event** `notifications/commands/executed { id, ctx, result, txn, origin }` via the notification surface — the semantic plane reactive plugins subscribe to. The data changes the command produced were already emitted as `store/changed` carrying the same `txn`, so consumers can correlate action → data.

Bracketing is generic and automatic: any command, hitting any combination of stores/servers, produces one undo group + one correlated event batch with zero plugin effort. A command that writes nothing (e.g. a pure `ui.palette.open`) just yields an empty group (free) and still emits `commands/executed`.

The callback dispatcher and the `txn`-in-context propagation come from the plugin platform; the service receives an `Arc<dyn CallbackDispatcher>` and a handle to set/clear the ambient `txn`. Tests can fake both.

## Acceptance Criteria
- [ ] `execute` invokes `execute` callback and returns its result; rechecks `available` unless `force:true`
- [ ] `execute` opens a `txn` before and closes it after the callback; all store writes during the callback share that `txn` (one undo group)
- [ ] On success, `commands/executed { id, ctx, result, txn, origin }` is emitted; its `txn` matches the `store/changed` events the command produced
- [ ] `origin` reflects the caller (user vs agent)
- [ ] `available` budget: >50ms → forced `{ok:false, reason:"available timeout"}` + warn; 5–50ms → warn but real result
- [ ] `execute`/`available` for unknown id → `UnknownCommand`; no `available` callback → `{ok:true}`

## Tests
- [ ] `execute_happy_path.rs` — echo `execute` callback; verify result + a `commands/executed` event with matching `txn`
- [ ] `execute_transaction_grouping.rs` — a command whose callback makes two store writes; assert both share one `txn` and undo reverts them as one group
- [ ] `execute_rechecks_available.rs` — `available:false` blocks execute (reason surfaced); `force:true` runs anyway
- [ ] `available_latency_budget.rs` — 60ms fake → forced false + warn
- [ ] `execute_unknown.rs`, `available_no_callback.rs`
- [ ] `cargo test -p swissarmyhammer-command-service` passes

## Workflow
- Use `/tdd` — write `execute_transaction_grouping.rs` first; it pins the command-as-unit contract (one action → one txn → one undo group → correlated events).

Depends on the list/schema verbs task, and (for txn + action emission) the `store` server and MCP notification-surface tasks.