---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m53gkmsdgjsfhbvymgwz2tsd
  text: |-
    ### note — from the Swift port check
    - Also send `didClose` when `documentSymbol` or a call-hierarchy request fails. Do not send it only on success.
    - Do a check in `crates/swissarmyhammer-lsp/src/client.rs`: when a reply arrives after its request timed out, the client must discard it. It must not keep it in a buffer for all time (memory leak). If a buffer keeps late replies, remove the entry on timeout. Add a test.
    - Requests from the server (they have `id` and `method`) must not be taken as the response to a pending request with the same id. Also, the client must not drop string ids.
  timestamp: 2026-10-16T23:26:28.653779+00:00
position_column: todo
position_ordinal: ffff8180
title: Close LSP documents that sah opened only to index them
---
## Problem
The LSP worker (`crates/swissarmyhammer-code-context/src/lsp_worker.rs`) never sends `didClose`. All the files that sah indexes stay open in the LSP while the process runs. sourcekit-lsp does work for each open document when a file changes. With thousands of open files, this costs much CPU. Also, sah sends no `$/cancelRequest` when a request times out (`crates/swissarmyhammer-lsp/src/client.rs`), so the server continues work that nobody will use.

## Fix
- After the worker is done with a file (`documentSymbol` and call hierarchy), send `didClose`. Do not close a file that a live tool op (hover, definition, diagnostics) uses at that time. Track the opener, or keep a small LRU of open documents with a fixed limit (for example 50).
- On a request timeout, send `$/cancelRequest` with the request id.

## Acceptance criteria
- After the worker indexes N files, the count of open documents in the session is not more than the limit.
- A request that times out causes a `$/cancelRequest` for its id.

## Tests
- A unit test with a fake LSP client that records the requests. Check that `didClose` follows the indexing requests for each file.
- A unit test for the timeout path that checks for `$/cancelRequest`. #lsp-fix #bug