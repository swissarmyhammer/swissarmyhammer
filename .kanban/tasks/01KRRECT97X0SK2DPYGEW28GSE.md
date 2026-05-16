---
assignees:
- claude-code
depends_on:
- 01KRRE5VD7WS8HQX12KG2CA398
- 01KRRE634FJBXSDSK4HXH1F2VF
- 01KRREC7YF5ENG2M2E7DQYSDGS
position_column: todo
position_ordinal: '9480'
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
- [ ] `cli_server_e2e.rs` registers a CLI source through a real plugin and proves a `tools/call` round-trips over stdio.
- [ ] `url_server_e2e.rs` registers a URL source and proves a `tools/call` round-trips over HTTP with auth headers.
- [ ] Both follow the reference-test isolation model; no mocked dispatcher/registry.

## Tests
- [ ] Run: `cargo test -p swissarmyhammer-plugin` — the two new `*_e2e.rs` tests and the whole suite green.
- [ ] Each test must genuinely fail if its transport is broken (verify by temporarily breaking the transport locally).

## Workflow
- Tests are the deliverable; no `/tdd` cycle. Reuse the harness/helpers established by `files_dispatch_e2e.rs`.

## Depends on
CliServer, UrlServer, and the reference `files_dispatch_e2e.rs` harness.