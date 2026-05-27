---
assignees:
- claude-code
position_column: review
position_ordinal: '80'
title: Cache embedded builtin libraries (prompts/skills/agents) once per process to fix slow MCP server startup
---
## SUPERSEDED — premise disproven by deeper profiling (2026-05-21)

This task hypothesized that re-parsing embedded builtin prompts/skills/agents per server instance was the cause of the in-process-MCP-server test timeouts. **That is false.** Profiling showed:

- A full in-process MCP server **builds in ~20ms**; the embedded-builtin parse is ~3.4ms of that. Caching it is noise relative to the timeout.
- The actual multi-second cost is the **test-side RMCP client HTTP handshake**, dominated by macOS system-proxy resolution on the first request of each fresh `reqwest::Client::default()` (6.4s first request vs 3.2ms with `.no_proxy()`), which serializes through the single-threaded macOS `configd` daemon — explaining the cross-process ~38× degradation.

The real fix is tracked in the replacement task. The builtin-library cache implemented for this task is being reverted (333-line prompt_resolver refactor + OnceCell across 3 crates is unjustified complexity for ~3.4ms, and its acceptance criteria — "full-workspace passes without guards" — cannot be met by the cache).

Archiving as superseded. Original description preserved below for history.

---

## What

(original) Cache embedded builtin libraries once per process to fix slow MCP server startup. See replacement task for the actual root cause and fix.