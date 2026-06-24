---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvwsh9nm6yfr4xb35v3xpskn
  text: |-
    Layer 1 (observability) implemented in crates/swissarmyhammer-tools/src/mcp/inline_diagnostics.rs:

    - New `MutationDiagnosis { Analyzed(DiagnoseOutcome), Unavailable(String) }` replaces the bare `DiagnoseOutcome` return of `MutationDiagnoser::diagnose`.
    - `LiveDiagnoser` now maps the `produce_outcome` error path (leader unreachable / no session) to `Unavailable(reason)` instead of swallowing it at debug! and returning an empty clean outcome.
    - New `fold_unavailable_into_result` attaches `diagnostics_unavailable: <reason>` to BOTH the structured surface and an appended text block — so the model can no longer read "no diagnostics block" as "clean".
    - New unit test `unavailable_analysis_folds_explicit_marker` asserts the marker on both surfaces and that no `diagnostics` key is emitted.

    Verified: `cargo test -p swissarmyhammer-tools --lib inline_diagnostics` -> 10 passed, 0 failed.

    REMAINING (not yet done):
    1. Cold-load case: when rust-analyzer IS reachable but the project model has not loaded the crate yet, `produce_outcome` returns a legitimately-empty report with pending:false — still indistinguishable from clean. Needs RA load-state -> pending:true mapping.
    2. Layer 2 (single loaded leader / stop the N-rust-analyzer spawn thrash) is the existing leadership work — see "Lease-based leadership takeover" (short_id nfprqm9) + spawn-gating ^7a5h2bj. Do not duplicate here.
  timestamp: 2026-06-24T12:26:31.476180+00:00
- actor: claude-code
  id: 01kvwv0408m32hd0dj0mpbfnvs
  text: |-
    Cold-load gate IMPLEMENTED (pull-response readiness, TDD).

    Empirical basis (isolated real-RA probe): rust-analyzer answers a `textDocument/diagnostic` pull issued while loading with a JSON-RPC error `ServerCancelled (-32802)` + `data.retriggerRequest:true` (the earliest pulls return a true-empty report, which is irreducibly ambiguous with clean; the -32802 is the catchable not-ready signal). `request()` previously parsed that error body into an empty (clean-looking) report.

    Change:
    - swissarmyhammer-lsp/src/session.rs: `SessionInner.ready: AtomicBool` (default true); `is_ready()`; `pull_response_is_not_ready()` (codes -32801/-32802 or retriggerRequest); `pull_diagnostics` now sets ready=false and returns Ok(empty) WITHOUT caching/broadcasting on a not-ready answer, and ready=true on a real report.
    - swissarmyhammer-diagnostics/src/diagnose.rs: `diagnose_with_outcome` ORs `pending` with `is_running() && !is_ready()`, so a running-but-loading server reports pending instead of false-clean (which the Layer-1 fold-in surfaces).

    Tests (all green):
    - session.rs: `pull_not_ready_response_marks_session_not_ready_without_caching`, `real_pull_answer_marks_session_ready_again` (FakeTransport, deterministic).
    - diagnose.rs: `outcome_pending_when_running_server_is_not_ready` (RecordingTransport scripted -32802).
    - tests/ra_pull_readiness.rs: isolated real-rust-analyzer e2e (tempdir + canonicalize + kill-on-drop daemon, PATH-gated, SERIAL). Asserts the warm pull path reports the E0308 and ends ready; observes the not-ready transition (best-effort, since a fast load can skip the brief -32802 window). Ran 2x stable, "observed not-ready: true".
    - Full suites: swissarmyhammer-lsp lib 221, swissarmyhammer-diagnostics lib 73, tools inline_diagnostics 10 — all pass.

    Binary rebuilt + installed via `just sah`. Known residual ambiguity (documented, not fixable at the response layer): the earliest pure-indexing pulls return true-empty and still look clean until RA starts emitting -32802 / a report; on a large cold workspace RA emits -32802 throughout the long load, so the gate fires. Single-warm-leader (Layer 2 / nfprqm9) remains the path to "edit → immediately see the error".
  timestamp: 2026-06-24T12:52:05.768494+00:00
- actor: claude-code
  id: 01kvxefxewdbqcf06th2ha0pvb
  text: |-
    DONE + manually verified end to end.

    The readiness gate alone was necessary-not-sufficient: the manual test exposed that diagnose_with_outcome never pulled — it did sync_open + settle and read only the in-process cache, which for rust-analyzer is populated solely by the watcher's async pulls. So check file / inline-edit returned empty even on warm broken code.

    Fix (commit f16754c75): diagnose_with_outcome now pulls each target file directly (after the already-blocking sync_open). Warm -> report cached -> surfaced; cold -> ServerCancelled/-32802 -> is_ready false -> pending.

    Manual test on the rebuilt binary: editing `render_hash(hash: u8) -> String { hash }` folded the E0308 ("expected String, found u8") straight into the edit-tool result with pending:false; reverting it produced a silent (no-diagnostics) envelope. Joy, not debugging.

    Commits: bce7d48e2 (readiness gate + Layer-1 unavailable signal + real-RA integration test), f16754c75 (pull fix). All gates green: lsp lib 221, diagnostics lib 73(+1), tools inline_diagnostics 10, clippy clean, adversarial review PASS, real-RA integration stable.

    Follow-up still open: en0jq4h (code-context get_diagnostics readiness — same class on a different surface).
  timestamp: 2026-06-24T18:32:46.300677+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe180
project: diagnostics
title: 'Inline diagnostics: distinguish "analyzed & clean" from "not analyzed / unavailable" instead of silently returning 0 errors'
---
## Problem

A successful `files` edit/write of a diagnosable source file, and the explicit `diagnostics check file` tool, both return `{"diagnostics": [], "counts": {"errors": 0}}` in THREE indistinguishable situations:

1. The code is genuinely clean.
2. rust-analyzer has not loaded this crate's project model yet (cold / partial load).
3. The follower could not reach the elected LSP leader.

So the model cannot tell "looks fine" from "nobody actually looked." This silently defeats the inline-diagnostics fold-in.

## Reproduction (observed this session)

- Introduced a hard type error (`render_hash(hash: u8) -> String { hash }`, E0308) in `crates/swissarmyhammer-hashline/src/lib.rs`.
- `cargo check` confirms the error (after mtime bump — see note below).
- The edit envelope folded in NO `diagnostics`/`pending` block.
- `mcp__sah__diagnostics check file` on the broken file returned `0 errors` — repeatedly, after 12s and after 45s waits.
- `diagnostics list servers` shows rust-analyzer Running (pid 19614). `ps`: that instance is idle (~5% CPU) with only ~602 MB RSS — far too small for a loaded project model of this multi-crate workspace, i.e. it never fully loaded this crate. AND there were FIVE rust-analyzer instances running concurrently (each + proc-macro-srv) — the not-leader-gated spawn thrash. The diagnostics tool queries the under-loaded supervisor instance.

## Root cause in code

- `crates/swissarmyhammer-tools/src/mcp/inline_diagnostics.rs` `LiveDiagnoser::diagnose`: on ANY error from `produce_outcome` (incl. the typed NotLeader / leader-pid error), it logs at `debug!` and returns `DiagnoseOutcome { report: empty, pending: false }`.
- `fold_outcome_into_result` then attaches nothing when `diagnostics.is_empty() && !pending`.
- Net: the carefully-surfaced typed "could not reach leader" error (asserted by `produce_outcome_without_a_session_surfaces_the_typed_not_leader_error` in `diagnostics/mod.rs`) is swallowed at the one place a human/model would see it.
- Separately, when rust-analyzer IS reachable but the project model isn't loaded, the outcome is a legitimately-empty report with `pending: false` — also indistinguishable from clean.

## Proposed fix

In the fold-in / outcome, introduce an explicit "diagnostics unavailable / not-yet-analyzed" state distinct from "analyzed, zero findings":
- On the `produce_outcome` error path, fold in a marker (e.g. `diagnostics_unavailable: "<reason>"`, leader-unreachable) instead of an empty clean result.
- Treat "rust-analyzer reachable but project/flycheck not yet settled for this file" as `pending: true`, not clean (quiescence detection must not declare settled when the project model hasn't loaded the crate).
- The envelope should make "we did not actually analyze this" visible to the model.

## Related / do NOT duplicate

- Leadership/multi-instance side is tracked by "Lease-based leadership takeover (^d8vae11)" (short_id nfprqm9, in review) and the known spawn-gating work (^7a5h2bj). This task is specifically about the OBSERVABILITY of an empty/unavailable diagnostics result, not the leader election itself.

## Side note (separate, smaller)

In the PREVIOUS MCP build, the edit envelope reported `metadata_preserved: true` and preserved the file mtime, which masked the change from `cargo`'s mtime-based staleness check (false green until `touch`). The fresh MCP build dropped `metadata_preserved` and mtime now advances — appears already fixed, but worth a regression test that an edit to a source file bumps mtime. #diagnostics