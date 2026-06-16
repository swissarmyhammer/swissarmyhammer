---
assignees:
- claude-code
position_column: todo
position_ordinal: ad80
project: diagnostics
title: Watcher push notifications + subscribable diagnostics MCP resource
---
## What
Soft levers for the hardest case — a foreign host doing a **native edit** where your tool never runs. The watcher still detects the change but can only reach the host/human, not wake an idle model. Provide the two courtesy channels the design names:

1. **Watcher push** — watcher detects a change → emits `notifications/message` (host-facing; whether it renders is up to the host). Route through the existing MCP→ACP relay (`NotifyingClientHandler` in `llama-agent/src/mcp_client_handler.rs` already converts MCP progress/logging notifications into ACP `SessionUpdate`s) so llama-agent forwards it; for foreign hosts it is a plain MCP `notifications/message`.
2. **Subscribable MCP resource** — expose diagnostics as an MCP resource that emits `notifications/resources/updated`, so a host that subscribes gets diagnostics without a tool call. Back it with the session's per-uri diagnostics cache.

These are explicitly courtesy / not load-bearing (cannot make a foreign model act out of turn) — keep them best-effort.

## Depends on
- "Cross-process publishDiagnostics fan-out + leader file watcher" (the watcher is the push source)
- "Capture publishDiagnostics and add in-process fan-out in swissarmyhammer-lsp" (the resource's data source)

## Acceptance Criteria
- [ ] Watcher-detected changes emit `notifications/message`; in llama-agent these relay to the ACP client via the existing handler.
- [ ] Diagnostics exposed as a subscribable MCP resource that emits `notifications/resources/updated` on change, backed by the per-uri cache.
- [ ] Both paths are best-effort and never block analysis or edits.

## Tests
- [ ] `cargo test -p swissarmyhammer-tools`: subscribing to the diagnostics resource then pushing a cache update emits a `resources/updated`; model-free.
- [ ] `cargo test -p llama-agent`: a synthesized watcher `notifications/message` is relayed through `NotifyingClientHandler` to a broadcast `SessionNotification` (fake-model seam, <10s).

## Workflow
- Use `/tdd`. Reuse the existing notification relay; do not add a new transport. #diagnostics