---
assignees:
- claude-code
depends_on:
- 01KRGMK1VTXCFZRKTMM8ESSVM2
- 01KRGMKMWEC139S450D2SBRMYG
position_column: done
position_ordinal: ffffffffffffffffffffffffc680
project: rebuild-index
title: MCP `notifications/progress` reporter for tool calls
---
Convert `IndexProgress` events into MCP `notifications/progress` messages so any MCP client (Claude Code, the CLI, the inspector) can render progress for long-running tool calls.

## Background

`model-context-protocol-extras` already has `ProgressNotificationParam` (re-exported from rmcp), `McpNotification::Progress`, and the `NotifyingServer` wrapper. We just need to bridge from `IndexProgress` to that channel — and we need to plumb the request's `progressToken` from the JSON-RPC `_meta` field through to the op handler.

## Implementation

### Capture the progress token

In `mod.rs::execute(...)`, before dispatching the op, extract `progressToken` from the incoming request's `_meta` (if present). rmcp puts this on the request envelope. Pass it (and a handle to the notification sink) into the op dispatch.

### Reporter impl

New module `swissarmyhammer-tools/src/mcp/progress.rs`:

```rust
pub struct McpProgressReporter {
    token: ProgressToken,
    sink: NotificationSink,  // mpsc::Sender<McpNotification> or whatever NotifyingServer exposes
}

impl ProgressReporter for McpProgressReporter {
    fn report(&self, event: IndexProgress) {
        let (progress, total, message) = match &event {
            IndexProgress::Discovering { found } => (0, 0, format!("Discovering ({found} files)")),
            IndexProgress::Chunking { file, done, total } => (*done, *total, format!("Chunking {}", file.display())),
            IndexProgress::Embedding { batch, batches, .. } => (*batch, *batches, "Embedding".into()),
            IndexProgress::Done { files, chunks, elapsed } => (1, 1, format!("Done: {files} files, {chunks} chunks in {elapsed:?}")),
        };
        let _ = self.sink.try_send(McpNotification::Progress(ProgressNotificationParam {
            progress_token: self.token.clone(),
            progress: progress as f64,
            total: Some(total as f64),
            message: Some(message),
        }));
    }
}
```

Note: the formatted `message` strings are part of the MCP payload, not the renderer — they're a fallback that any client can display verbatim. The structured data (`progress`, `total`) is what TUIs use. Both flow through.

### Plumb into `rebuild index`

In `execute_rebuild_index`, build either an `McpProgressReporter` (if a progress token was supplied) or a `NoopReporter` (if not). Pass it to `index_discovered_files_async`.

## Tests

- Unit: in-memory `NotificationSink` (vec), call `McpProgressReporter::report` with each event variant, assert the resulting `ProgressNotificationParam` shape.
- Integration: end-to-end MCP call to `rebuild index` with a `progressToken`, collect notifications, assert the final notification's `progress == total`. Pattern after `agent-client-protocol-extras/src/test_mcp_server.rs`.

## Out of scope

- The CLI-side consumer (next card)

#mcp #code-context #rebuild-index

## Review Findings (2026-05-13 16:30)

### Warnings
- [x] `swissarmyhammer-tools/src/mcp/progress.rs` — Cross-phase progress is now monotonic on the wire. The reporter tracks cumulative `files_done` / `batches_done` counters (each maintained with `.max()` so out-of-order events cannot regress them) and emits `progress = files_done + batches_done`, `total = files_total + batches_total` (floored at `progress`). Verified by a new unit test `cross_phase_progress_is_strictly_monotonic_on_the_wire` walking the realistic Discovery → interleaved Chunking+Embedding → Done sequence, a new `out_of_order_events_cannot_regress_progress` defensive test, and a new monotonicity assertion in the e2e test asserting `progress[i+1] >= progress[i]` and `total >= progress` for every adjacent pair.
- [x] `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` (rebuild_index drain await) — `let _ = handle.await;` replaced with `if let Err(err) = handle.await { tracing::warn!(error = ?err, "rebuild_index: progress drain task did not join cleanly"); }` so a panic in the drain task surfaces in production logs instead of being silently swallowed. Comment explains the rationale alongside.

### Nits
- [x] `swissarmyhammer-tools/src/mcp/progress.rs` — Constructor now returns a named struct `McpProgressReporterBuild { reporter, receiver }` instead of a 2-tuple. Method renamed to `McpProgressReporter::build` (avoids the `clippy::new_ret_no_self` lint that the old `new` returning `Self`'s pair would trigger).
- [x] `swissarmyhammer-tools/src/mcp/progress.rs` — `format!("Done: ... in {elapsed:?}")` switched to `{elapsed:.2?}` which renders e.g. `1.23s` instead of `1.234567891s` for a user-visible status line.
- [x] `swissarmyhammer-tools/tests/rebuild_index_progress_notifications_test.rs` — The 100ms `tokio::time::sleep` to flush the client receive loop is kept (the comment already documents that the server-side drain handle is `.await`ed and the sleep covers transport delivery). Acknowledged as pragmatic per the nit; not a blocker.
- [x] `swissarmyhammer-tools/tests/rebuild_index_progress_notifications_test.rs` — Awkward `let _ = token;` removed. `make_token("rebuild-progress-test-token")` is now inlined into `params.set_progress_token(...)`; a comment block above the call explains rmcp's `progress_token_provider` contract.
- [x] `swissarmyhammer-tools/src/mcp/progress.rs` — Test helper `drain` renamed to `take_buffered` so it no longer shadows the production `spawn_drain_task` naming when grep'ing for "drain".