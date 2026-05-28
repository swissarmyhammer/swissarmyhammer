---
assignees:
- claude-code
depends_on:
- 01KS36P9C8CFT5HMQWY2WCA9ZE
- 01KS5F7BR6850RKT67X4CNHPAZ
- 01KS5G3AKZXDN7K6YR415E0V4K
position_column: review
position_ordinal: '80'
project: command-service
title: '`execute` transaction bracketing + `commands/executed` action event'
---
## What

Add transaction bracketing and the action-event plane to the Command engine's `execute` (which already does the callback round-trip — see `01KS36P9C8…`). Split out from `execute` because both pieces hard-depend on the `store` server (txn/undo-group) and the MCP notification surface (`commands/executed`), which are higher in the build order.

Wraps the existing `execute` arm (`crates/swissarmyhammer-command-service/src/service.rs`):
1. **Open a transaction** before invoking the `execute` callback: generate a `txn` id and stamp it into the call context (`RequestContext::extensions`, alongside `CallerId`) so every downstream store write the callback makes shares one undo group AND tags its emitted change events with this `txn` (see the `store` server transaction-grouping + the notification-surface tasks). Also set `origin` from `CallerId` (user / agent:id).
2. Invoke the `execute` callback (existing behavior).
3. **Close the transaction** after the callback resolves (success or error).
4. On success, **emit the action event** `notifications/commands/executed { id, ctx, result, txn, origin }` via the notification surface — the semantic plane reactive plugins subscribe to. The data changes the command produced were already emitted as `store/changed` carrying the same `txn`, so consumers can correlate action → data.

Bracketing is generic and automatic: any command, hitting any combination of stores/servers, produces one undo group + one correlated event batch with zero plugin effort. A command that writes nothing (e.g. a pure `ui.palette.open`) yields an empty group (free) and still emits `commands/executed`.

The service receives a handle to set/clear the ambient `txn` (from the store transaction API) and the notification sink. Tests can fake both.

## Acceptance Criteria
- [ ] `execute` opens a `txn` before and closes it after the callback; all store writes during the callback share that `txn` (one undo group)
- [ ] On success, `commands/executed { id, ctx, result, txn, origin }` is emitted; its `txn` matches the `store/changed` events the command produced
- [ ] `origin` reflects the caller (user vs agent:id)
- [ ] A write-nothing command still emits `commands/executed` with an empty group
- [ ] An `execute` whose callback errors still closes the txn (no leaked open transaction)

## Tests
- [ ] `execute_transaction_grouping.rs` — a command whose callback makes two store writes; assert both share one `txn` and a single `store.undo` reverts them as one group
- [ ] `execute_emits_action_event.rs` — echo `execute` callback; verify a `commands/executed` event with a `txn` matching the produced `store/changed` events
- [ ] `execute_error_closes_txn.rs` — callback errors; assert no open transaction remains
- [ ] `cargo test -p swissarmyhammer-command-service` passes

## Workflow
- Use `/tdd` — write `execute_transaction_grouping.rs` first; it pins the command-as-unit contract (one action → one txn → one undo group → correlated events).

Depends on: the `execute`/`available` verbs task (`01KS36P9C8…`), the `store` server (txn grouping), and the MCP notification surface (`01KS5G3AKZ…`).