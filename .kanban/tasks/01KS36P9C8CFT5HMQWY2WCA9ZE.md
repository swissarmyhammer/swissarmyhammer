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

The two verbs that cross the host/plugin boundary. Both look up the active stack entry for the id, then invoke `available` then `execute` callbacks in the registering caller's isolate. (Transaction bracketing + the `commands/executed` action event are SPLIT into a follow-up task — `01KS613VPH2G4ZWKZPGW9ZCJAA` — because they hard-depend on the `store` server and the MCP notification surface, which are higher up the build order. This task lands first and is Tier-0-clean: a command executes; grouping/events arrive later.)

Files:
- `crates/swissarmyhammer-command-service/src/service.rs` — `execute` and `available` arms
- `crates/swissarmyhammer-command-service/src/invoke.rs` — `invoke_callback(caller, callback_id, args)` via the platform callback dispatcher
- `crates/swissarmyhammer-command-service/src/latency.rs` — soft latency budget for `available`

### available
Look up the active entry; no `available` callback → `{ ok: true }`. Else invoke with `ctx`, enforcing the budget: >5ms warn, >50ms force `{ ok:false, reason:"available timeout" }` and cancel. Return boolean | `{ ok:false, reason }`.

### execute
1. Recheck `available` unless `force:true`; reject `CommandUnavailable { reason }` if false.
2. Invoke the `execute` callback with `ctx`.
3. Return its result.

(The txn open/close around step 2 and the `commands/executed` emission after step 3 are added by the follow-up task `01KS613VPH2G4ZWKZPGW9ZCJAA` once `store` + notification surface exist. This task deliberately does NOT open a txn or emit an action event — keeping it free of higher-tier deps.)

The callback dispatcher comes from the plugin platform; the service receives an `Arc<dyn CallbackDispatcher>`. Tests can fake it.

## Acceptance Criteria
- [ ] `execute` invokes the `execute` callback and returns its result; rechecks `available` unless `force:true`
- [ ] `available` budget: >50ms → forced `{ok:false, reason:"available timeout"}` + warn; 5–50ms → warn but real result
- [ ] `execute`/`available` for unknown id → `UnknownCommand`; no `available` callback → `{ok:true}`
- [ ] No txn/event coupling in this task (those are the follow-up's concern); this task has no `store`/notification-surface dependency

## Tests
- [ ] `execute_happy_path.rs` — echo `execute` callback; verify result returned
- [ ] `execute_rechecks_available.rs` — `available:false` blocks execute (reason surfaced); `force:true` runs anyway
- [ ] `available_latency_budget.rs` — 60ms fake → forced false + warn
- [ ] `execute_unknown.rs`, `available_no_callback.rs`
- [ ] `cargo test -p swissarmyhammer-command-service` passes

## Workflow
- Use `/tdd` — write `execute_rechecks_available.rs` first; it pins the available→execute gating.

Depends on the list/schema verbs task. The txn-bracketing + action-event follow-up (`01KS613VPH2G4ZWKZPGW9ZCJAA`) depends additionally on the `store` server and the MCP notification-surface task.