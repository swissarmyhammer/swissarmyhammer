---
assignees:
- claude-code
depends_on:
- 01KY0N94MJ938XTK2K5SQMQ62V
position_column: todo
position_ordinal: a680
title: Surface model-download progress as MCP notifications/progress (code_context + review)
---
## What

Consume the model-loader `DownloadObserver` seam (built by ^sqmq62v — this task depends on it) so the model-download phase streams MCP `notifications/progress` instead of minutes of silence. The silence matters operationally: a first-run `code_context` `rebuild index` or `review` op downloads the multi-hundred-MB qwen-embedding model before emitting anything, and an MCP client with a tool-call inactivity timeout kills the connection — progress notifications are what keep it alive.

Two entry points, both with progress plumbing already in place:

1. **code_context rebuild index** — `crates/swissarmyhammer-code-context/src/progress.rs`: add `IndexProgress::DownloadingModel { file: String, downloaded_bytes: u64, total_bytes: u64 }` to the event enum. In `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`, the rebuild-index path resolves the embedder via `swissarmyhammer_embedding::Embedder::default()` (around the `index_discovered_files_async` setup, near line 1881) with the `reporter` already in scope — attach a `DownloadObserver` (via `Embedder::with_download_observer`) that forwards each `DownloadEvent` as `reporter.report(IndexProgress::DownloadingModel { .. })`, so download events flow through the existing `McpProgressReporter` → `notifications/progress` pipeline before `Discovering`.
2. **review ops** — extend `ReviewProgressEvent` (added by ^jn2wjd5 in `crates/swissarmyhammer-validators/src/review/fleet.rs`) with a `DownloadingModel { file, downloaded_bytes, total_bytes }` variant. In `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs`, forward a `DownloadObserver` into the embedder load so first-run reviews emit download notifications through the existing `spawn_review_progress_bridge`: either widen `EmbedderFactory` to accept an `Option<model_loader::DownloadObserver>`, or (less invasive) have `run_review_request_inner` construct the default embedder with `with_download_observer` wired to the run's progress sender. Note `DEFAULT_EMBEDDER` is a process-global `OnceCell` — only the first run downloads, so events are naturally first-run-only.

3. **Wire mapping** — `crates/swissarmyhammer-tools/src/mcp/progress.rs` `McpProgressReporter::build_param`: map `DownloadingModel` keeping the wire `progress` monotonic (downloads precede chunking; never let byte counts regress the cumulative counter — carry the byte detail in `message`, e.g. `Downloading <file>: <downloaded>/<total> bytes`, full filename, never truncated). Same for the review bridge's `review_progress_param`.

Subtasks:
- [ ] `IndexProgress::DownloadingModel` variant + `build_param` mapping (monotonic wire progress, byte-carrying message)
- [ ] rebuild-index path attaches the observer to `Embedder::default()` and forwards into `reporter`
- [ ] `ReviewProgressEvent::DownloadingModel` variant + emission from the review embedder load + `review_progress_param` mapping
- [ ] Tests (below)

## Acceptance Criteria
- [ ] `code_context` `op: "rebuild index"` with a `progressToken`, when the embedder load reports download events, emits `notifications/progress` messages naming the file and byte counts, before any `Discovering`/`Chunking` notification
- [ ] `review` ops with a `progressToken` emit download notifications through the review progress bridge when the embedder factory's load downloads
- [ ] Wire `progress` remains monotonically non-decreasing across DownloadingModel → Discovering → Chunking → Embedding → Done (existing `rebuild_index_progress_notifications_test.rs` monotonicity assertion still passes)
- [ ] Download messages carry the full untruncated filename and full byte counts
- [ ] No progress token and no sink → zero notifications, unchanged results (both tools)

## Tests
- [ ] Unit tests in `crates/swissarmyhammer-tools/src/mcp/progress.rs`: `DownloadingModel` mapping — message format, byte counts, monotonicity when interleaved with `Chunking`/`Embedding` events
- [ ] Unit test in `crates/swissarmyhammer-tools/src/mcp/tools/review/review_op.rs`: a `ReviewProgressEvent::DownloadingModel` through the bridge produces a token-echoing param whose message names file + bytes and whose wire progress never regresses
- [ ] Integration test (mock embedder reporting synthetic `DownloadEvent`s — no network, no real model): rebuild-index path forwards observer events as `IndexProgress::DownloadingModel` through the reporter, asserted alongside the existing progress-event tests in `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs`
- [ ] Run: `cargo nextest run -p swissarmyhammer-tools -E 'test(progress)'` and `cargo nextest run -p swissarmyhammer-tools --test rebuild_index_progress_notifications_test` — green

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass. #mcp #progress #review