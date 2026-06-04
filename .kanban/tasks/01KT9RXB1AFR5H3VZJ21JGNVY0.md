---
assignees:
- claude-code
position_column: todo
position_ordinal: '8180'
project: ai-panel
title: Agent tool execution needs a watchdog timeout so a slow/hung tool returns an error instead of stalling the whole turn
---
## P1 — a single blocked tool call wedged the entire agent turn

### Evidence
When `grep_files` blocked walking `/` (see sibling P0 card), the agentic loop had **no timeout**. It sat idle for a full 5 minutes (11:38:53 → 11:43:53, only UI `nav.focus` events in between) until the MCP transport itself gave up and closed the stream. The model was healthy (~31 tok/s, sub-second tool calls before this) — it was wedged, not slow.

### Problem
There is no per-tool-call watchdog in the agentic loop. A tool handler that hangs (infinite walk, network stall, deadlock) blocks generation indefinitely, with no error surfaced to the model or the UI.

### Fix
- Wrap each tool dispatch in a bounded `tokio::time::timeout` (configurable; default well under the 300s transport timeout — e.g. 30–60s).
- On timeout: **cancel/abort** the tool future if possible, return a structured error tool-result into the conversation so the agentic loop can continue/retry or report, and emit a user-visible notice in the panel.
- Ensure the timeout is independent of the MCP transport's own idle/keepalive timeout so the loop fails *gracefully* before the session is torn down.

### Verification
Inject a deliberately-blocking fake tool (fits the `llama-coverage` scripted-fake-model harness); assert the turn returns a timeout error result within the bound and the loop stays alive.