---
assignees:
- claude-code
position_column: todo
position_ordinal: '8580'
title: 'Flaky: swissarmyhammer-code-context lsp_communication::tests::test_send_request_accepts_mismatched_id_response'
---
## What

`swissarmyhammer-code-context::lsp_communication::tests::test_send_request_accepts_mismatched_id_response` failed under the full `cargo nextest run --workspace` run (14663 tests, ~452s wall time, high parallel load) but passes deterministically when run in isolation (`cargo nextest run -p swissarmyhammer-code-context <name>` → ok in 0.05s) and when running the whole `swissarmyhammer-code-context` crate alone (1399/1399 pass).

Likely cause: timing/ordering assumption in the mismatched-id test path — the test probably awaits a response with a short timeout and the dispatch/wakeup is starved under heavy parallel load.

## Where

- File: `crates/swissarmyhammer-code-context/src/lsp_communication.rs` (tests module)
- Test: `lsp_communication::tests::test_send_request_accepts_mismatched_id_response`

## Acceptance Criteria

- Identify the timing/wait that flakes (likely a `tokio::time::timeout` or a `recv_timeout`) and replace it with deterministic completion (await the actual oneshot the dispatcher fulfills, or use `tokio::test(start_paused = true)` and `tokio::time::advance` for any clock-driven wait).
- No `sleep`/`timeout` for synchronization-by-hope.
- Test passes 100 runs back-to-back, including a workspace-wide concurrent build.

## Tests

- `cargo nextest run -p swissarmyhammer-code-context lsp_communication::tests::test_send_request_accepts_mismatched_id_response`
- `cargo nextest run --workspace` — full suite green #test-failure