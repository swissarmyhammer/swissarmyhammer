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

The two verbs that cross the host/plugin boundary. Both look up the active stack entry for the id, then send `notifications/callbacks/invoke` to the registering caller's isolate, then await the result. This task wires the service into the plugin platform's callback primitive.

Files:
- `crates/swissarmyhammer-command-service/src/service.rs` — `execute` and `available` arms
- `crates/swissarmyhammer-command-service/src/invoke.rs` — `invoke_callback(caller, callback_id, args) -> Result<Value>` using the platform's callback dispatcher (passed in at service construction)
- `crates/swissarmyhammer-command-service/src/latency.rs` — soft latency budget enforcement for `available`

Behavior:
- `available command`: look up the active entry; if no `available` callback, return `{ ok: true }` (commands with no precondition are always available). Otherwise invoke the callback with `ctx`. Enforce the budget — start a timer; if the call takes >5ms emit a warn-level trace; if >50ms force the result to `{ ok: false, reason: "available timeout" }` and cancel the callback. Return the callback's result (boolean or `{ ok: false, reason }`).
- `execute command`: look up the active entry; if `force` is true, skip the precondition recheck; otherwise call `available` first and reject with `CommandUnavailable { reason }` if it returns false. Then invoke `execute` with `ctx`. Return whatever the callback returned.

`CommandUnavailable` error carries the reason from `available` so the UI can show "Open a board first" etc.

The callback dispatcher lives in `swissarmyhammer-plugin` (built in the plugin-arch project). The service receives an `Arc<dyn CallbackDispatcher>` at construction. For tests this can be a fake that just returns canned results.

## Acceptance Criteria
- [ ] `execute` for a registered command invokes the `execute` callback and returns its result
- [ ] `execute` without `force` calls `available` first; if `available` returns false, returns `CommandUnavailable { reason }` and does NOT call `execute`
- [ ] `execute` with `force: true` skips the `available` recheck
- [ ] `available` with no `available` callback returns `{ ok: true }` (always available)
- [ ] `available` exceeding 50ms returns `{ ok: false, reason: "available timeout" }` and the timer logs at warn level
- [ ] `available` between 5ms and 50ms logs at warn level but returns the actual result
- [ ] `execute` for an unknown id returns `UnknownCommand`

## Tests
- [ ] `crates/swissarmyhammer-command-service/tests/execute_happy_path.rs` — register a command whose `execute` callback echoes its args; verify the response
- [ ] `crates/swissarmyhammer-command-service/tests/execute_rechecks_available.rs` — register with `available` returning `{ ok: false, reason: "no board" }`; `execute` rejects with `CommandUnavailable("no board")`; `execute` with `force: true` runs anyway
- [ ] `crates/swissarmyhammer-command-service/tests/execute_unknown.rs` — `execute { id: "does.not.exist" }` returns `UnknownCommand`
- [ ] `crates/swissarmyhammer-command-service/tests/available_no_callback.rs` — registered without `available` → `available command` returns `{ ok: true }`
- [ ] `crates/swissarmyhammer-command-service/tests/available_latency_budget.rs` — fake dispatcher that sleeps 60ms; `available` returns `{ ok: false, reason: "available timeout" }`; trace at warn level captured
- [ ] `cargo test -p swissarmyhammer-command-service` passes

## Workflow
- Use `/tdd` — write the latency-budget test first; it pins the contract that the palette can rely on, which is the whole point of the budget.