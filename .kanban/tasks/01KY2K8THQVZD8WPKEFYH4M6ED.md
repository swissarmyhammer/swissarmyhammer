---
assignees:
- claude-code
position_column: todo
position_ordinal: b380
title: 'Flaky: review progress notifications go non-monotonic under full-suite load (streamable HTTP)'
---
crates/swissarmyhammer-tools/tests/review_progress_notifications_test.rs:200

`review_working_emits_progress_notifications_per_pair_when_token_supplied` FAILED in the full scoped run (`cargo nextest run -E 'rdeps(swissarmyhammer-tools) | rdeps(swissarmyhammer-validators) | rdeps(swissarmyhammer-agent)'`):

```
notifications/progress regressed between index 61 and 62 (MCP spec violation): 2 -> 1
(messages: Some("Reviewed src/orphan.rs against command-safety") -> Some("Reviewed src/live.rs against command-safety"))
```

What I tried: re-ran the test in isolation (`cargo nextest run -p swissarmyhammer-tools --test review_progress_notifications_test`) — PASSES in 11.7s. So it is a load-dependent flake, not deterministic.

Analysis: server-side emission is strictly ordered — a single `ReviewProgressState` in one mapping task feeds one unbounded channel drained sequentially by `spawn_drain_task` (`crates/swissarmyhammer-tools/src/mcp/progress.rs`), so the server cannot emit 2 then 1. The regression must arise from receipt-side reordering across the rmcp streamable-HTTP transport under contention (progress=1 was the first PairDone, progress=2 the second — the client observed them swapped). Suspect: notifications for one tool call being split across the POST SSE stream and the standalone GET stream, or client-side concurrent stream processing in `StreamableHttpClientTransport`. Root-cause at the transport layer before adding any test-side tolerance (spec says progress must be monotonic, so a real fix is required, not a looser assertion). #test-failure