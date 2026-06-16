---
assignees:
- claude-code
position_column: todo
position_ordinal: a180
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