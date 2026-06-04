---
assignees:
- claude-code
position_column: todo
position_ordinal: '8280'
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