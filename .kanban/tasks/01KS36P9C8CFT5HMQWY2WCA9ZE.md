---
assignees:
- claude-code
depends_on:
- 01KS36NMBH23RR2HSYNHX9AZK2
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb180
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
Look up the active entry; no `available` callback → `{ ok: true }`. Else invoke with `ctx`, enforcing the budget: >5ms warn, >50ms force `{ ok:false, reason:\"available timeout\" }` and cancel. Return boolean | `{ ok:false, reason }`.

### execute
1. Recheck `available` unless `force:true`; reject `CommandUnavailable { reason }` if false.
2. Invoke the `execute` callback with `ctx`.
3. Return its result.

(The txn open/close around step 2 and the `commands/executed` emission after step 3 are added by the follow-up task `01KS613VPH2G4ZWKZPGW9ZCJAA` once `store` + notification surface exist. This task deliberately does NOT open a txn or emit an action event — keeping it free of higher-tier deps.)

The callback dispatcher comes from the plugin platform; the service receives an `Arc<dyn CallbackDispatcher>`. Tests can fake it.

## Acceptance Criteria
- [x] `execute` invokes the `execute` callback and returns its result; rechecks `available` unless `force:true`
- [x] `available` budget: >50ms → forced `{ok:false, reason:\"available timeout\"}` + warn; 5–50ms → warn but real result
- [x] `execute`/`available` for unknown id → `UnknownCommand`; no `available` callback → `{ok:true}`
- [x] No txn/event coupling in this task (those are the follow-up's concern); this task has no `store`/notification-surface dependency

## Tests
- [x] `execute_happy_path.rs` — echo `execute` callback; verify result returned
- [x] `execute_rechecks_available.rs` — `available:false` blocks execute (reason surfaced); `force:true` runs anyway
- [x] `available_latency_budget.rs` — 60ms fake → forced false + warn
- [x] `execute_unknown.rs`, `available_no_callback.rs`
- [x] `cargo test -p swissarmyhammer-command-service` passes

## Workflow
- Use `/tdd` — write `execute_rechecks_available.rs` first; it pins the available→execute gating.

Depends on the list/schema verbs task. The txn-bracketing + action-event follow-up (`01KS613VPH2G4ZWKZPGW9ZCJAA`) depends additionally on the `store` server and the MCP notification-surface task.

## Review Findings (2026-05-27 15:10)

### Nits
- [x] `crates/swissarmyhammer-command-service/src/service.rs:741-763` — `interpret_available` treats a `Value::Object` lacking an `ok` field as Available via `unwrap_or(true)`, silently discarding any `reason` field. A callback that returns `{ \"reason\": \"no selection\" }` without `ok: false` is silently treated as Available. The docstring documents the rule, but it is a subtle gotcha for SDK authors. Consider either flagging \"object without `ok` field\" as malformed (defensive) or strengthening the docstring with an explicit example of the gotcha. Low priority — current behavior is documented and stable.
  - Resolution: strengthened the docstring on `interpret_available` with an explicit `# Gotcha for SDK authors` section that calls out the `{ \"reason\": \"no selection\" }` example, explains why `reason` is silently discarded when `ok` is missing, and documents why the defensive default (treat bare-object as Available) is intentional — it matches the boolean default for forward compatibility.
- [x] `crates/swissarmyhammer-command-service/tests/available_latency_budget.rs:72-93` — the \"warn band\" test sleeps a real 10ms and asserts the result is the real `ok: true`. The 50ms hard deadline has 40ms of slack, which is comfortable, but the test is real wall-clock and could theoretically flake on a heavily loaded CI runner where a tokio sleep slips past 50ms. Not observed yet; worth noting only if the test ever flakes. Optional: shorten to e.g. 7ms or use `tokio::time::pause` + virtual clock to make the assertion deterministic.
  - Resolution: switched `available_callback_just_past_warn_threshold_still_returns_real_result` to `#[tokio::test(start_paused = true)]` so tokio's virtual clock drives the dispatcher's `sleep(10ms)` and the latency budget's `timeout(50ms)`. The test is now deterministic — wall-clock load cannot slip the sleep past the deadline. Added `test-util` to the dev-dependency tokio features to enable `start_paused`. Added a comment in the test explaining the virtual-clock rationale.
