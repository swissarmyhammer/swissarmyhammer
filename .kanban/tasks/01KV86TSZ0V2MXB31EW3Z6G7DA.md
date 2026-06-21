---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kv8kta5yjaddeqehq7zaz7p8
  text: 'Picked up as task 3/15 in the diagnostics /finish batch. Builds on g167jrk (DONE, c78686245): the LspSession lives in crates/swissarmyhammer-lsp/src/session.rs with request/notify + the shared open-doc set; register the publishDiagnostics notification handler ON the session. Shared FakeTransport is in crates/swissarmyhammer-lsp/src/test_support.rs — use it to drive a scripted publishDiagnostics notification through the handler in a model-free unit test. Relocate/share the parse_publish_diagnostics logic currently in code-context ops/get_diagnostics.rs.'
  timestamp: 2026-06-16T16:21:46.814917+00:00
- actor: claude-code
  id: 01kv8kxgcaq85nvd3qr2ex4217
  text: |-
    Research done. Plan:
    - New module crates/swissarmyhammer-lsp/src/diagnostics.rs = the ONE parser, producing Vec<lsp_types::Diagnostic> (lenient hand-rolled per-item skip preserving the existing code-context test semantics). Defines DiagnosticUpdate { uri, diagnostics } for the broadcast. Ports the parse unit tests.
    - LspSession gains: latest-per-uri in-memory cache (Mutex<HashMap<String, Vec<lsp_types::Diagnostic>>>, derived state, never persisted), tokio broadcast::Sender<DiagnosticUpdate>, subscribe(), handle_publish_diagnostics(params), diagnostics_for(uri), and pull_diagnostics(path) which issues textDocument/diagnostic and feeds the SAME cache/fan-out. Push + pull unify.
    - code-context get_diagnostics.rs: replace its hand-rolled JSON parsing with the lsp-crate parser, mapping lsp_types::Diagnostic -> its enriched Diagnostic; keep re-exports compiling.
    TDD: capture+fan-out test first (scripted publishDiagnostics via FakeTransport, assert per-uri cache + subscriber receives, model-free <1s, assert no disk writes).
  timestamp: 2026-06-16T16:23:31.466955+00:00
- actor: claude-code
  id: 01kv8mdphb9s73p4fcx5g8fb9k
  text: |-
    Implemented and verified.

    Files:
    - NEW crates/swissarmyhammer-lsp/src/diagnostics.rs — the ONE shared parser (parse_publish_diagnostics, parse_diagnostics_from_result) producing Vec<lsp_types::Diagnostic>, lenient per-item skip; + DiagnosticUpdate { uri, diagnostics } broadcast payload; ported the parsing unit tests.
    - crates/swissarmyhammer-lsp/src/session.rs — LspSession now owns a latest-per-uri in-memory cache (Mutex<HashMap<String, Vec<lsp_types::Diagnostic>>>, derived state, never persisted) + a tokio::sync::broadcast::Sender<DiagnosticUpdate>. New API: subscribe(), diagnostics_for(uri), handle_publish_diagnostics(params) [push], pull_diagnostics(path) [pull via textDocument/diagnostic]. Both push and pull funnel through one private store_and_broadcast() so cache + fan-out stay in lockstep. reset_documents() also clears the cache (derived state must not outlive the gone process).
    - crates/swissarmyhammer-lsp/src/lib.rs — pub mod diagnostics + re-exports.
    - crates/swissarmyhammer-code-context/src/ops/get_diagnostics.rs — dropped the duplicate hand-rolled parser; its parse_* now delegate to swissarmyhammer_lsp::parse_* and convert lsp_types::Diagnostic -> the enriched code-context Diagnostic. Re-exports and its parsing tests stay green.

    Verification (all fresh, green):
    - cargo build --workspace: clean (exit 0)
    - cargo clippy --workspace --all-targets -- -D warnings: zero warnings (exit 0)
    - cargo test -p swissarmyhammer-lsp: 206 passed, 0 failed
    - cargo test -p swissarmyhammer-code-context: 624 + 26 + 2 passed, 0 failed
    The capture+fan-out test (publish_diagnostics_updates_cache_and_fans_out_to_subscriber) is model-free via FakeTransport, finishes in 0.00s, and asserts a temp dir is unchanged before/after (no disk writes). Pull-feeds-same-cache covered by pull_diagnostics_feeds_the_same_cache_and_fan_out.

    double-check agent verdict: PASS (one informational note: numeric code bound is i32, dictated by lsp_types::NumberOrString::Number(i32) — intentional, unreachable for real LSP codes).

    Note: wiring the daemon read loop to call handle_publish_diagnostics is intentionally out of scope (this task is the session-level capture API + fan-out); the daemon has no notification-draining read loop yet.
  timestamp: 2026-06-16T16:32:22.059390+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb880
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