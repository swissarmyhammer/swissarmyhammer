---
assignees:
- claude-code
position_column: todo
position_ordinal: a280
project: diagnostics
title: 'code-context get_diagnostics: honor session readiness (is_ready) so a still-loading rust-analyzer isn''t reported as clean'
---
## Problem

The readiness gate added in 1j79z6d (short_id 1j79z6d) covers the `diagnostics` MCP tool / inline-on-edit fold-in path (`swissarmyhammer-diagnostics::diagnose_with_outcome` now reports `pending` when `session.is_running() && !session.is_ready()`).

A SEPARATE surface was NOT covered: the code-context `get diagnostics` op, `crates/swissarmyhammer-code-context/src/layered_context.rs::lsp_diagnostics`. It still collapses a not-ready rust-analyzer's error/empty answer into a clean `Some(vec![])` reported as `SourceLayer::LiveLsp`, so a caller of `code_context get diagnostics` reads "no diagnostics" as "clean" while the server is merely still loading.

This is PRE-EXISTING (not a regression introduced by 1j79z6d — that path already collapsed error envelopes to empty before the change). Flagged by the adversarial double-check of 1j79z6d as observation #3.

## What exists to reuse

- `LspSession::is_ready()` (swissarmyhammer-lsp/src/session.rs) — the session-global readiness flag, set by `pull_diagnostics` from rust-analyzer's ServerCancelled (-32802) / ContentModified (-32801) / `retriggerRequest` answer.
- The pattern to mirror: `diagnose_with_outcome` in swissarmyhammer-diagnostics.

## Proposed change

In `layered_context.rs::lsp_diagnostics` (and/or the `get diagnostics` op that wraps it), when the live LSP session `is_running() && !is_ready()`, do NOT return a clean `Some(vec![])` as authoritative — surface a "pending / not-ready" signal (or fall back / mark the layer accordingly) so the consumer can distinguish "analyzed, clean" from "still loading". Match whatever shape the `get diagnostics` op result uses to carry that distinction.

## Verify

- Unit test at the `LspSession` seam (FakeTransport scripted with a -32802 answer → `lsp_diagnostics` must not report an authoritative clean empty).
- Confirm no regression for servers that never signal not-ready (e.g. tsserver): `is_ready()` defaults true, so those are unaffected.

## Related
- 1j79z6d (the diagnostics/inline-fold readiness gate this extends).
- Layer 2 single-warm-leader work: "Lease-based leadership takeover" (short_id nfprqm9). #diagnostics