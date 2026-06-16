---
assignees:
- claude-code
position_column: todo
position_ordinal: a280
project: diagnostics
title: Capture publishDiagnostics and add in-process fan-out in swissarmyhammer-lsp
---
## What
The session must capture push diagnostics and fan them out to subscribers — code-context and diagnostics are sibling consumers of ONE diagnostics stream. Today `publishDiagnostics` notifications from the server are dropped on the floor; the only diagnostics path is the pull `textDocument/diagnostic` in code-context's `ops/get_diagnostics.rs`.

- In `swissarmyhammer-lsp`'s session, register a notification handler for `textDocument/publishDiagnostics`; parse with the existing parser shape (`parse_publish_diagnostics` logic currently in code-context `ops/get_diagnostics.rs` — relocate/share it) into `lsp_types::Diagnostic` records keyed by uri.
- Maintain a latest-per-uri cache (derived state, **never persisted** — see design "the cache is derived state").
- Expose an in-process subscribe channel (`tokio::sync::broadcast` of `{uri, Vec<Diagnostic>}`) so consumers receive updates as the server re-flows analysis.
- Also support the pull model (`textDocument/diagnostic`) through the same session API, so servers without push still work — unify both into the one fan-out.

## Depends on
- "Add single owned LSP session with shared open-document set in swissarmyhammer-lsp" — the handler lives on the session.

## Acceptance Criteria
- [ ] Session captures `publishDiagnostics`, maintains a latest-per-uri in-memory cache, and exposes a `subscribe()` broadcast of diagnostic updates.
- [ ] Pull (`textDocument/diagnostic`) and push (`publishDiagnostics`) both feed the same cache/fan-out.
- [ ] Cache is never written to disk.

## Tests
- [ ] `cargo test -p swissarmyhammer-lsp`: feed a scripted `publishDiagnostics` notification through the session's handler, assert the per-uri cache updates and a subscriber receives it (model-free, mocked transport, <1s).
- [ ] Reuse the existing diagnostic-parsing unit tests (moved from code-context `ops/get_diagnostics.rs`) to guard the lsp_types mapping.

## Workflow
- Use `/tdd` — write the capture+fan-out test first. #diagnostics