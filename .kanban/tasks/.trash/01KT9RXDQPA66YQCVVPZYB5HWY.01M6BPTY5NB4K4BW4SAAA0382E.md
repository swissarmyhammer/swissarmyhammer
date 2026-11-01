---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe980
project: ai-panel
title: llama-agent MCP client must abort the turn on session teardown (quit_reason=Closed / 404) instead of looping on a dead session forever
---
## P1 — after the stall, the client retried a dead MCP session indefinitely

### Evidence
At 11:43:53 the streamable-HTTP serve loop ended: `input stream terminated` / `serve finished quit_reason=Closed`, and rmcp dropped session `12849919-…`. From 11:43:56 onward the llama-agent MCP client kept issuing `GET /mcp` on that now-gone session and getting `404 Not Found` → `session_terminate`, retrying with backoff (3s,2s,4s,8s,16s,32s,64s…) and **never aborting**. The panel appears "stuck forever" because the client is reconnecting to a session the server already deleted.

### Problems
1. The client does not treat `quit_reason=Closed` / repeated `404 Not Found` on a session as terminal — it should abort the in-flight turn and surface an error to the UI, not loop.
2. **Pin down the exact 300s stream-close.** 11:38:53 → 11:43:53 is exactly 300.000s — a timeout fired (likely an axum/rmcp HTTP request or idle/keepalive timeout in `unified_server.rs`, or the client read side). Find the knob; the transport closing a request mid-tool-call is itself a design smell once the watchdog card lands.

### Fix
- In the llama-agent MCP client reconnect path: detect terminal session loss (404 on a previously-open session id, or `quit_reason=Closed`) → stop reconnecting, fail the active tool call/turn with a clear error, propagate to the ACP client / panel.
- Document/justify the 300s transport timeout; once the per-tool watchdog (sibling card) exists, the loop should fail long before this fires.

### Notes
The `Failed to forward MCP notification: channel closed` warn (mcp_client_handler.rs:146) is a benign broadcast-with-no-subscribers case — fine to leave, but consider demoting to debug to reduce noise.

---

## Implementation notes (completed)

**Root cause (Problem 1).** `UnifiedMCPClient::with_streamable_http_and_handler` built the rmcp transport via `StreamableHttpClientTransport::from_uri(url)`, which uses `StreamableHttpClientTransportConfig::default()` → `retry_config: ExponentialBackoff { max_times: None }` (**unbounded**). The background standalone `GET /mcp` SSE stream reconnects via `SseAutoReconnectStream` using that policy, so on a torn-down session every reconnect 404s and the loop runs forever — exactly the "retrying with backoff and never aborting" symptom. (Note: rmcp's reqwest `get_stream` maps a 404 to a generic reqwest error, not `SessionExpired`, so the standalone-stream path never self-terminates; only the `post_message` path maps 404→`SessionExpired`, and with `reinit_on_expired_session: true` it self-heals once or returns a terminal error to the in-flight `call_tool`.)

**Fix.** Added `streamable_http_transport_config(url)` in `crates/llama-agent/src/mcp.rs` that builds the config via `StreamableHttpClientTransportConfig::with_uri` and swaps in a **bounded** `ExponentialBackoff` (`max_times: Some(6)` ≈ 63s of reconnect effort), then builds the transport via `StreamableHttpClientTransport::from_config(...)` (preserves rmcp's tuned default reqwest client, incl. `pool_max_idle_per_host(0)`). `reinit_on_expired_session` stays enabled so transient expiry still self-heals. All streamable/SSE/HTTP client paths funnel through this single chokepoint. Unit test `streamable_http_config_uses_bounded_reconnect_policy` pins the bound.

**Notes cleanup.** Demoted the `mcp_client_handler.rs` broadcast-no-subscribers `warn!` to `debug!` with an explanatory comment.

**Problem 2 — 300s knob: investigated, NOT a knob in our code.** Server-side rmcp `StreamableHttpServerConfig` default in `unified_server.rs` is `sse_keep_alive: 15s`, `sse_retry: 3s` (the 3s matches the first observed backoff), `stateful_mode: true` — there is no 300s value in our server or client construction. The 300s is a hyper/reqwest connection-level default outside our config surface, not a tunable we set. The card's own framing says the real fix is the sibling per-tool watchdog card, which should fail the turn long before any 300s transport close — so no server-side timeout change is made here. The actionable client deliverable (stop the infinite reconnect loop) is done.

**Verification.** `cargo check -p llama-agent` clean; `cargo clippy -p llama-agent --all-targets` clean (no warnings); `cargo test -p llama-agent --lib` → 1024 passed, 0 failed.