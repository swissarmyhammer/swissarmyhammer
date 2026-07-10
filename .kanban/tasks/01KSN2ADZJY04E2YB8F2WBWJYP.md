---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb480
project: command-service
title: Round-trip sentinel execute callbacks in override-stack tests once SDK callbackDispatch lands
---
## What

`crates/swissarmyhammer-command-service/tests/integration/override_stack_e2e.rs` currently verifies override-stack semantics via `active_caller` + `stack_depth` snapshots only. Sentinel-file execute callbacks were skipped because the SDK does not yet expose the `callbackDispatch` helper for command registration.

When the SDK gap closes, extend the override-stack tests to:

- Have each registered command's `execute` callback write a sentinel (e.g. the registering caller's id) to a temp file.
- After each stack mutation (host -> A -> B -> unload B -> A -> unload A -> host), invoke `execute command` and assert the sentinel file matches the expected top-of-stack caller.

This closes the gap the reviewer flagged on task 01KS36PZK9K6PHTRB9M7YPWTF2: override semantics are currently a pure-state assertion, and the dispatch path is covered separately by `HostCallbackDispatcher` tests, but a full round-trip across the SDK boundary at the override boundary would tighten the integration coverage.

## Acceptance Criteria

- [x] `override_stack_e2e.rs` includes a test that registers `execute` callbacks via the SDK's `callbackDispatch` helper from a plugin isolate.
- [x] After each unload/unwind step, `execute command` is invoked and the resulting sentinel side-effect proves the correct caller's callback ran.
- [x] The existing `active_caller`/`stack_depth` assertions stay (they remain the more direct test for the registry invariant).

## Workflow

Blocked on SDK exposing `callbackDispatch` for command registration. Track that work in the SDK-completion thread.

## Implementation

New test `override_stack_round_trips_execute_callbacks_through_sdk_isolates` added to `crates/swissarmyhammer-command-service/tests/integration/override_stack_e2e.rs`. It mirrors the existing host -> A -> B -> unload B -> unload A -> host sequence, but each probe plugin loads via the SDK convention (`ensureServices` + `registerCommands`) with a per-plugin sentinel string in its execute callback. At each plugin-on-top step the test invokes `execute command` via the bootstrap-wired service and asserts the dispatcher returned that plugin's sentinel verbatim. At each host-on-top step (start and end) the test asserts execute fails with `CallbackFailed` — `HostCallbackDispatcher` rejects non-plugin callers because they have no isolate to invoke.

### Sentinel mechanism

The task description suggested a temp-file side-effect, but plugin isolates have no filesystem surface — host traffic crosses through `op_host_dispatch` only. The execute callback's return value already round-trips verbatim through `handle_execute` (`{ ok: true, result: <value> }`), so the return value IS the sentinel signal: the test reads it through the same wire a production caller would. This is strictly more direct than an out-of-band file write.

### Helpers

- `support::write_sentinel_probe_plugin(plugins_dir, id, command_id, sentinel)` — writes a TypeScript bundle whose `load()` calls `ensureServices` + `registerCommands` with an `execute` callback returning `sentinel`.
- `support::try_call_command(...)` — `call_command` variant that surfaces the verb dispatcher's `McpError` instead of panicking, used to assert the host-on-top step fails with `CallbackFailed`.
- `support::execute_args(id)` and `support::execute_result(response)` — small ergonomics helpers for building and reading execute payloads.

The existing `active_caller`/`stack_depth` test and the reload-cycle test stay untouched — they remain the most direct test for the registry invariant.

## Tests

- [x] `cargo test -p swissarmyhammer-command-service --test integration override_stack` — all 3 tests pass (existing 2 + new 1).
- [x] `cargo test -p swissarmyhammer-command-service` — full crate suite passes (no regressions).
- [x] `cargo test -p swissarmyhammer-plugin` — SDK side stays green.
- [x] `cargo clippy -p swissarmyhammer-command-service --tests` — clean.