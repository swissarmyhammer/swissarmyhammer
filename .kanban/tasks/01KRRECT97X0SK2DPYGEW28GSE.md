---
assignees:
- claude-code
depends_on:
- 01KRRE5VD7WS8HQX12KG2CA398
- 01KRRE634FJBXSDSK4HXH1F2VF
- 01KRREC7YF5ENG2M2E7DQYSDGS
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8480
project: plugin-arch
title: 'plugin: transport e2e tests — CLI subprocess and URL server'
---
## What
Capability integration tests for the two out-of-process transports, following the `files_dispatch_e2e.rs` reference shape (real isolate, real registered server, observe an effect that only happens if the platform works).

`crates/swissarmyhammer-plugin/tests/integration/`:
- `cli_server_e2e.rs` — a probe plugin's `load()` does `this.register("x", { cli: [...] })`; the host spawns the subprocess; a `tools/call` goes through stdio and returns. Use a real tiny stdio MCP server as the subprocess. Assert the call's effect/return value.
- `url_server_e2e.rs` — a probe plugin does `this.register("x", { url: ... })`; the host calls it; a mock HTTP MCP endpoint records the request shape. Assert the recorded request (tool name + arguments map + auth header) and that the response reached the plugin.

Each test: own `TempDir`, fresh `PluginHost`, no shared/`static` state.

## Acceptance Criteria
- [x] `cli_server_e2e.rs` registers a CLI source through a real plugin and proves a `tools/call` round-trips over stdio.
- [x] `url_server_e2e.rs` registers a URL source and proves a `tools/call` round-trips over HTTP with auth headers.
- [x] Both follow the reference-test isolation model; no mocked dispatcher/registry.

## Tests
- [x] Run: `cargo test -p swissarmyhammer-plugin` — the two new `*_e2e.rs` tests and the whole suite green.
- [x] Each test must genuinely fail if its transport is broken (verify by temporarily breaking the transport locally).

## Workflow
- Tests are the deliverable; no `/tdd` cycle. Reuse the harness/helpers established by `files_dispatch_e2e.rs`.

## Depends on
CliServer, UrlServer, and the reference `files_dispatch_e2e.rs` harness.

## BLOCKED — host-side defect surfaced by these tests (2026-05-18)

Both e2e tests were written exactly to plan (flat `tests/cli_server_e2e.rs` + `tests/url_server_e2e.rs`, reference shape, real isolate, real fixture, observation via the real `files` tool, bounded timeouts). They compile cleanly. They FAIL — and the failure is a genuine platform bug in the host, not a test defect:

- Both tests fail identically: `this.register({cli|url})` SUCCEEDS (the `CliServer`/`UrlServer` connects), but the immediately-following `tools/call` fails with `ServerUnavailable` at `HostBridge.toolsCall`.
- Root cause: `HostBridge::block_on` in `crates/swissarmyhammer-plugin/src/host.rs` (~line 1607) spawns a **brand-new ephemeral current-thread Tokio runtime on a scratch thread for every single bridge call**, and that runtime is dropped when the scratch thread exits.
- `CliServer`/`UrlServer` hold an `rmcp` `RunningService` whose background service loop is a task on the runtime live during `connect`. When `register`'s scratch runtime is dropped, that service loop dies. The next bridge call (`toolsCall`) runs on a *different* ephemeral runtime and finds a dead peer → `ServerUnavailable`.
- The `HostBridge` doc comment at host.rs:1584 even says `cli`/`url` connects "are sent to a fresh task on **the host's runtime**" — but the implementation builds a throwaway runtime instead. Doc and code disagree; the doc describes the correct design.
- `cli_server.rs` / `url_server.rs` (T8/T9) pass only because they connect AND invoke on the *same* `#[tokio::test]` runtime — they never cross the bridge, so they never hit the per-call-runtime teardown.

Fixing this requires a host-code change (make the bridge use a single long-lived host runtime so transport background tasks survive across bridge calls) — that is production code, outside this tests-only task's scope. Per the task plan's "STOP and report a genuine blocker rather than working around it", stopped here.

Deliverables in place and ready once the host bug is fixed:
- `crates/swissarmyhammer-plugin/tests/cli_server_e2e.rs` — `discovered_plugin_round_trips_a_tools_call_over_the_cli_subprocess`
- `crates/swissarmyhammer-plugin/tests/url_server_e2e.rs` — `discovered_plugin_round_trips_a_tools_call_over_the_url_endpoint`

Suggested follow-up task: "plugin: HostBridge must drive cli/url transports on a long-lived host runtime" — fix `HostBridge::block_on` to dispatch onto one persistent runtime, then unblock this task.

## Resolved (2026-05-18)

The host-side defect was fixed under task `01KRXP69S5QHC13V1PK28EXJYA` ("plugin: HostBridge must drive cli/url transports on a long-lived runtime", now `done`): `HostBridge` now routes every bridge call onto one persistent `BridgeRuntime` so transport background tasks survive across calls. With that fix in place, both T21 e2e tests pass — verified GREEN and stable across 3 consecutive `cargo test -p swissarmyhammer-plugin` runs. The RED (broken HostBridge) → GREEN (fixed) flip is itself the proof that each test genuinely fails when its transport path is broken. This task's deliverable — the two `*_e2e.rs` files — is complete.