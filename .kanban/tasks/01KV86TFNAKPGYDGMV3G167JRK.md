---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv8jbv9e03wk540y5b07bzwv
  text: 'Picked up as task 2/15 in the diagnostics /finish batch. Foundation b3ahkva is DONE and committed (6e73c2787): swissarmyhammer-lsp now owns LspJsonRpcClient/SharedLspClient + the new LspTransport trait with an in-memory FakeTransport (in crates/swissarmyhammer-lsp/src/client.rs) — use that fake to unit-test the session''s open-doc state machine model-free (no real rust-analyzer).'
  timestamp: 2026-06-16T15:56:24.238349+00:00
- actor: claude-code
  id: 01kv8jqz3v669dxqfgawc0rnez
  text: |-
    Implemented LspSession in swissarmyhammer-lsp.

    - New src/session.rs: LspSession<C: LspTransport = LspJsonRpcClient>, Arc-based cloneable handle. Inner holds the daemon's single SharedLspClient (Arc<Mutex<Option<C>>>) plus a shared Mutex<HashMap<uri, DocState{version,text_hash}>>. open/change/save/close are idempotent against the open-doc set (no duplicate didOpen; change bumps version, no-op when text unchanged; save/close no-op when not open; close removes uri). request/notify route through the one client, returning LspError::NotRunning when the client is None.
    - LspDaemon now owns exactly one session per server, built over its existing client Arc (no second client storage). Added daemon.session() returning a clone. lib.rs re-exports LspSession.
    - Extracted the existing FakeTransport into src/test_support.rs (#[cfg(test)] crate-wide) so client.rs tests and the new session test share ONE fake rather than duplicating. No new fake invented.

    Tests:
    - Unit (model-free, FakeTransport, <1s): scripted open/change/save/close asserting open-doc transitions + duplicate-open suppression + no-op change; plus clones-share-one-set, change-before-open errors, request-with-absent-client => NotRunning, documents-stay-open-across-requests. Verified RED by flipping the dedup guard (saw 2 != 1 didOpen) then restored.
    - Integration (tests/session_rust_analyzer.rs, runtime-gated on which::which("rust-analyzer")): opens a cargo fixture, fires textDocument/documentSymbol twice with no re-open, asserts both succeed and the doc stays open. rust-analyzer present here => actually ran green.

    Verification (all green): cargo build --workspace; cargo clippy --workspace --all-targets -- -D warnings (0 warnings); cargo test -p swissarmyhammer-lsp (175 lib + 1 integration + 1 doctest).

    Note for the consumer-migration task: the session does NOT clear its open-doc set on daemon restart/shutdown; consumers re-opening after a restart is the intended follow-up, out of scope here.
  timestamp: 2026-06-16T16:03:01.371315+00:00
- actor: claude-code
  id: 01kv8k2y9vks3xr1xmcaesxwev
  text: |-
    Addressed both double-check (REVISE) findings:

    1. Stale open-doc set on restart (high severity): added LspSession::reset_documents() (clears the set WITHOUT sending didClose — the pipe is gone). Wired into LspDaemon at every client-clearing/repopulating site: start() (before storing the new client), both health_check() exit/error arms, and shutdown(). So after a restart the next open emits a fresh didOpen instead of being suppressed as a stale duplicate. New tests (RED-verified by no-op'ing reset): session::reset_documents_lets_a_reopen_emit_a_fresh_did_open and daemon::test_shutdown_resets_session_open_documents (uses the cat-echo live-client path to open via the session, then asserts shutdown empties the set).

    2. TOCTOU in open/change idempotency under concurrent shared clones: open/change/save/close now hold the docs mutex ACROSS the wire send (lock order docs -> client, never reverse, so deadlock-free; notify locks the separate client mutex). Two concurrent opens of the same uri can no longer both pass the contains_key check and both fire didOpen; change can no longer compute the same next_version twice. As a bonus, close now removes the uri only AFTER didClose succeeds, so a failed notify leaves the open set consistent (new test close_keeps_document_open_when_the_notify_fails, via FakeTransport.fail_notifications).

    Re-verified green: cargo build --workspace; cargo clippy --workspace --all-targets -- -D warnings (0 warnings); cargo test -p swissarmyhammer-lsp (178 lib + 1 integration + 1 doctest, 0 failed). Unit suite still model-free and <1s.
  timestamp: 2026-06-16T16:09:00.987189+00:00
- actor: claude-code
  id: 01kv8k5wfcmzpvd0rm7fpgrrr8
  text: |-
    Adversarial re-check disposition (bounded, one pass per really-done):

    Both concrete findings from the first REVISE pass are fixed and RED-verified (stale-set-on-restart via reset_documents wiring; TOCTOU via holding the docs lock across the wire send). The re-check's only remaining thread was acceptance criterion (c) "no other type spawns a client" — it noted that swissarmyhammer-code-context still has ~10 non-test LspJsonRpcClient::new / send_did_open callsites (layered_context.rs, lsp_communication.rs, ops/*, lsp_worker.rs).

    Justification for proceeding: those callsites are PRE-EXISTING and UNTOUCHED by this change (git diff --stat for crates/swissarmyhammer-code-context is empty). They are precisely the legacy didOpen->request->didClose churn path the task description names as living in code-context ("every live op does an atomic didOpen -> request -> didClose under one mutex hold in code-context"), to be migrated onto LspSession by the FOLLOW-UP consumer-migration task. Criterion (c) is scoped to swissarmyhammer-lsp's own design, where it holds: exactly one production LspJsonRpcClient::new (daemon.rs start()), LspDaemon owns exactly one session per server, and the session is the single Arc-shared open-doc handle. Migrating code-context here would be scope creep into a different task. Proceeding to review.
  timestamp: 2026-06-16T16:10:37.420518+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb780
project: diagnostics
title: Add single owned LSP session with shared open-document set in swissarmyhammer-lsp
---
## What
Promote `swissarmyhammer-lsp` from process supervisor to the "one LSP system" by introducing a single owned **session** per `(workspace, server)` that owns a **shared open-document set** — the invariant that forbids a second client. Today there is no open-doc tracking: every live op does an atomic `didOpen → request → didClose` under one mutex hold (`LayeredContext::lsp_multi_request_with_document` in code-context), and only `didOpen`/`didClose` exist — no `didChange`/`didSave`.

- In `swissarmyhammer-lsp`, add an `LspSession` (owned by `LspDaemon`, built on the relocated `SharedLspClient`) that tracks open documents (uri → version/text-hash) and exposes:
  - `open(path)` / `change(path, text)` / `save(path)` / `close(path)` issuing `textDocument/didOpen|didChange|didSave|didClose`, idempotently against the open-doc set (no duplicate didOpen).
  - `request(method, params)` and `notify(method, params)` against the one stdio client (the request API consumers will call).
- Documents stay open across requests (no open/close churn); the session is the single source of truth for what the server believes is open.
- Provide a cloneable session handle (`Arc`-based) so multiple in-process consumers (code-context indexer + query ops, diagnostics) share ONE open-doc set.

## Depends on
- "Invert lsp↔code-context dependency: relocate LSP client + server specs into swissarmyhammer-lsp" (b3ahkva) — needs the client living in swissarmyhammer-lsp first.

## Acceptance Criteria
- [ ] `swissarmyhammer-lsp` exposes an `LspSession` owning a shared open-document set with didOpen/didChange/didSave/didClose + request/notify.
- [ ] Opening an already-open document is a no-op (no duplicate didOpen); changing updates version; the set is observable for tests.
- [ ] `LspDaemon` owns exactly one session per server; no other type spawns a client.

## Tests
- [ ] `cargo test -p swissarmyhammer-lsp` unit test: scripted open/change/save/close sequence asserts the open-doc set transitions and that duplicate opens are suppressed (model-free; mock the client transport at a trait seam — do NOT require a real rust-analyzer for this unit test, keep it <1s).
- [ ] Integration test (gated on rust-analyzer present): open a fixture file, request `textDocument/documentSymbol` twice without re-open, assert both succeed against the persistent session.

## Workflow
- Use `/tdd` — write the open-doc-set state-machine test first, then implement the session. #diagnostics