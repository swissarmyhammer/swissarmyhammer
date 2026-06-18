---
assignees:
- claude-code
depends_on:
- 01KV86XXZRGN3RYPFVDREF6NJ4
position_column: todo
position_ordinal: b280
project: diagnostics
title: Wire the request multiplexer into the live MCP path (leader binds/serves socket; follower diagnostics route via SessionRequestClient)
---
## What
Follow-on to ^ref6nj4 (Request/response multiplexer over the election socket). That card built and tested the multiplexer MECHANISM end-to-end (RequestServer/RequestClient in `swissarmyhammer-leader-election/src/request_ipc.rs`; `serve_session_requests` + `SessionRequestClient` in `swissarmyhammer-diagnostics/src/request_api.rs`; gated integration test with a real rust-analyzer leader + follower over a real socket). It deliberately DEFERRED the live production rewiring as separate cross-crate scope. This card captures that deferred wiring so the follower diagnostics path actually rides the multiplexer in `sah serve`.

## Why
Today the live MCP path does NOT use the multiplexer:
- `crates/swissarmyhammer-tools/src/mcp/server.rs` does not call `serve_session_requests` anywhere — the leader never binds/serves the request socket at startup.
- `crates/swissarmyhammer-tools/src/mcp/tools/diagnostics/mod.rs` `produce_outcome()` still returns `settled_empty()` for followers instead of routing through `SessionRequestClient`.

So a follower process running the diagnostics MCP tool silently returns empty rather than round-tripping to the leader's single rust-analyzer. The mechanism exists but is unplugged.

## Acceptance Criteria
- [ ] Leader spawns `serve_session_requests(...)` at startup in `server.rs` `do_initialize_code_context`'s Leader branch, binding the `RequestServer` at the election `socket_path()`.
- [ ] Expose `socket_path()` (and the lock path) on `CodeContextWorkspace` (the election already knows it) so the tool layer can construct a `SessionRequestClient`.
- [ ] `produce_outcome()` falls back to `SessionRequestClient::connect(socket, lock)` when there is no in-process session, instead of `settled_empty()`; connect failure surfaces the typed not-leader/leader-pid error.
- [ ] Integration: a follower `sah` process running the diagnostics tool gets a real DiagnosticsReport from the leader over the socket, with only the leader's single rust-analyzer running (gated on rust-analyzer).

## Notes
- Reuse the existing transport/API — do NOT introduce a second envelope, election, or socket. This is wiring only.
- The double-check on ^ref6nj4 noted a related design item (blocking `session.request` inside an async task) to address in this wiring card.
#diagnostics