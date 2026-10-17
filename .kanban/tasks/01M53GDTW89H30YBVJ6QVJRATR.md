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
- actor: claude-code
  id: 01m553ag8vgvtbyk7nv5qz7s1y
  text: |-
    ### research — discoveries
    - The worker (`lsp_worker.rs::index_single_file`) calls `session.sync_open` and sends no `didClose`. The collection functions in `lsp_communication.rs` take `&mut LspJsonRpcClient` but use only `send_request`. They can be generic over `LspTransport`, so a fake client can drive the worker in a unit test (the existing worker tests spawn a python mock, which is an integration-style test).
    - `LspSession` keeps `uri -> DocState` with no limit and no owner. All tool ops (hover, definition, diagnostics watcher, IPC `lsp_request_with_document`) use `sync_open` and never close.
    - `client.rs`: the read loop is synchronous. There is NO buffer of late replies: a reply that arrives after its timeout is read by the next request and skipped as a mismatch. So there is no leak today, but this is not tested. The test needs a seam: the read loop must be generic over `BufRead`/`Write`.
    - `classify_response` takes any message with a matching numeric `id` as the response, also a server request (`id` + `method`). A string id `"5"` gives a mismatch.
    - No `$/cancelRequest` is sent on timeout.
    - Policy choice: owner tracking (Tool vs Indexer) for the worker close, plus a fixed LRU limit of 50 open documents for all opens. Both live in one private type in the session. The worker closes only a document that it opened and that no tool op synced after it.
  timestamp: 2026-10-17T14:12:46.491918+00:00
- actor: claude-code
  id: 01m553m1f0756avv7cnqnxa4hr
  text: |-
    ### progress — parts 1 and 2 landed
    - Session: new private policy type `session/open_documents.rs` (owner + LRU tick), `DocumentOwner {Tool, Indexer}`, `sync_open_for`, `release`, `MAX_OPEN_DOCUMENTS = 50`. A tool sync makes the owner Tool (sticky). At the limit the least recently used document closes before a new didOpen; a failed eviction didClose fails the open.
    - Worker: `index_single_file` syncs as Indexer and calls `release` on every exit path after the sync (success, request failure, persist failure, sync failure). The collection functions in `lsp_communication.rs` and the worker functions are now generic over `LspTransport`, so a recording fake (`test_fixtures::RecordingTransport`) drives them.
    - Tests RED then GREEN: 7 session tests, 5 worker tests.
    - Next: client.rs — `$/cancelRequest` on timeout, discard of late replies, server requests vs responses, string ids.
  timestamp: 2026-10-17T14:17:59.008046+00:00
- actor: claude-code
  id: 01m553v8se86q11affw73v7ynn
  text: |-
    ### progress — parts 3 and 4 landed
    - `client.rs`: the read loop is now `read_reply` over generic `Write`/`BufRead` with a `ready` closure (unit-testable with `Cursor`/`Vec<u8>`, no process). On timeout it sends `$/cancelRequest` `{id}` (write failure is logged). A late reply to an earlier id is classified `Stale` and discarded; nothing is buffered. A message with `method` + `id` is `ServerRequest`, never the reply; it gets a `MethodNotFound` (-32601) error reply that echoes the id unchanged (string ids stay strings). A reply whose id is the pending id as a string (`"5"`) matches.
    - `stdout_ready` is a free function that polls the fd of `reader.get_ref()`, so the `stdout_fd` field is gone. New `write_message` does the Content-Length framing for requests, notifications, cancels and server-request answers.
    - Behavior notes for review: (1) notification write errors now read `write failed: ...` (was `write notification failed: ...`); (2) a mismatched reply id was a `warn!`, now a `debug!` because late replies after a cancel are normal; (3) the old framing copies in `initialize`, `send_did_open`, `send_did_close`, `shutdown` are unchanged (pre-existing).
    - `ARCHITECTURE.md`: updated the LSP paragraph (collection functions take any `LspTransport`; owner/limit policy; cancel and server-request handling).
  timestamp: 2026-10-17T14:21:55.886293+00:00
- actor: claude-code
  id: 01m553vhntc97hsr4byth1pha7
  text: |-
    ### implement — changed
    - evidence: 9 files — crates/swissarmyhammer-lsp/src/session.rs, crates/swissarmyhammer-lsp/src/session/open_documents.rs (new), crates/swissarmyhammer-lsp/src/client.rs, crates/swissarmyhammer-lsp/src/lib.rs, crates/swissarmyhammer-code-context/src/lsp_worker.rs, crates/swissarmyhammer-code-context/src/lsp_communication.rs, crates/swissarmyhammer-code-context/src/test_fixtures.rs, ARCHITECTURE.md. New tests: 7 session, 5 worker, 5 client (each RED then GREEN). `cargo nextest run -p swissarmyhammer-lsp -p swissarmyhammer-code-context -p swissarmyhammer-diagnostics`: 1102 passed, 0 failed, 0 skipped. `cargo clippy -p swissarmyhammer-lsp -p swissarmyhammer-code-context -p swissarmyhammer-diagnostics -p swissarmyhammer-tools --all-targets -- -D warnings`: clean. `cargo fmt` applied. All four parts done; nothing left undone.
    - next: /review
  timestamp: 2026-10-17T14:22:04.986528+00:00
- actor: claude-code
  id: 01m5546jzka57pskh5bvq7wx0v
  text: |-
    ### test — green
    - evidence: `cargo nextest run --no-fail-fast -E 'rdeps(swissarmyhammer-lsp) | rdeps(swissarmyhammer-code-context)'` ran 5288 tests. 5288 passed, 0 failed, 0 skipped (65 slow). The output has no LEAK marker. The known flaky validators test passed in this run.
    - evidence: `cargo clippy --workspace --all-targets -- -D warnings` has no warnings. Exit code is 0.
    - evidence: `cargo fmt --check` found no format differences. Exit code is 0.
    - next: review.
  timestamp: 2026-10-17T14:28:06.771192+00:00
position_column: doing
position_ordinal: '8480'
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