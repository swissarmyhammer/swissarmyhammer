---
assignees:
- claude-code
position_column: todo
position_ordinal: a180
project: file-edit-tools
title: Diagnostics quiescence declares "settled/clean" while rust-analyzer is still loading/indexing — can't distinguish clean from not-analyzed-yet
---
## What
The diagnostics system cannot tell "the code is clean" from "rust-analyzer hasn't analyzed it yet" — both render as 0 errors with `pending: false`. After an edit that introduces a real compile error (E0308), the inline fold-in AND the explicit `diagnostics` tool both return empty-and-not-pending, so the model/user sees "looks fine" on genuinely-broken code.

### Evidence (live repro — supersedes an earlier WRONG hypothesis)
An earlier diagnosis blamed leader-election ("follower can't reach/elect a leader; NotLeader silently swallowed"). **That was wrong for the observed case and should not be chased as the root cause:**
- `list servers` showed rust-analyzer running (fresh pid), and the explicit `diagnostics` check returned `0 errors` **empty — NOT** a "could not reach leader" error. So the leader was reachable and a session existed.
- A second breaking edit produced an identical empty envelope; waiting 12s and re-checking still gave 0 errors.

The real cause: on a cold start in a large workspace (dozens of crates), rust-analyzer is still loading the project model / indexing, and **flycheck (cargo check) has not completed a pass for this crate**, so it legitimately has published zero diagnostics yet. The quiescence/`pending` machinery — designed to flag exactly this ambiguity — concluded `settled` (`pending: false`) because it only watches for the publishDiagnostics stream to go quiet, and a server that has not published anything yet is indistinguishable from a settled-clean one.

### Root cause (verified in code)
- `crates/swissarmyhammer-diagnostics/src/settle.rs` — `settle`/`settle_stream` subscribe to the diagnostic publish stream and debounce on a `settle_window` of quiet; `pending: true` is only produced at the `settle_hard_timeout` backstop. It has no notion of rust-analyzer's load/index/flycheck progress, so "no publishes yet (still loading)" reads as "settled."
- `crates/swissarmyhammer-diagnostics/src/diagnose.rs:182` `diagnose_with_outcome` returns `(records, pending)` straight from `settle`; `config.rs` holds `settle_window`/`settle_hard_timeout`.
- The test `outcome_reports_not_pending_when_settled` (diagnose.rs:628) bakes in the wrong assumption (quiet ⇒ not pending), and `produce_outcome`/the fold-in faithfully propagate the bogus `pending: false`.

## Required behavior
- An edit (or explicit diagnose) issued while rust-analyzer is still **loading/indexing**, or before **flycheck has completed at least one pass** for the target file's crate, must report **`pending: true`** (provisional), NOT a settled-empty/clean result. Only declare settled-clean once the server has actually completed a diagnostic pass and genuinely reports nothing.
- "Clean" and "not analyzed yet" must be distinguishable in the result the model sees.

## Approach (investigate, then implement)
- Make quiescence progress-aware: consume rust-analyzer's `$/progress` work-done tokens (e.g. `rustAnalyzer/Indexing`, `rustAnalyzer/cargo check`/flycheck) from the shared `swissarmyhammer-lsp` session, and/or require at least one completed flycheck pass for the file's crate, before `settle` may report not-pending. While any relevant work-done token is in progress (or no flycheck pass has completed), return `pending: true`.
- Determine in `swissarmyhammer-lsp` whether the session already surfaces `$/progress`/work-done state; if not, plumb it through to the settle engine (without spawning a second LSP client — single-client invariant).
- Keep `settle_hard_timeout` as the backstop, but premature quiet must no longer be read as settled while the server is provably still warming up.

### Secondary (keep, but lower priority — NOT the observed cause)
- The inline fold-in still swallows a genuine leader-unreachable `IpcError::NotLeader` into an empty `pending:false` envelope (`crates/swissarmyhammer-tools/src/mcp/inline_diagnostics.rs:63-72`). That path was NOT hit in this repro, but it is a real observability gap: surface a typed `diagnostics_unavailable: "could not reach LSP leader"` marker there instead of an empty outcome, so an unreachable leader is also distinguishable from clean. Do this only as a small follow-on slice; do not re-frame the whole task around leader election (that hypothesis was disproven here).

## Acceptance Criteria
- [ ] A diagnose/edit issued before rust-analyzer has completed loading/indexing or a first flycheck pass for the target crate reports `pending: true`, not settled-empty.
- [ ] Once the server completes its pass and the file genuinely has an error, the diagnostic surfaces (folded into the edit envelope and via the explicit tool).
- [ ] Once the server completes its pass and the file is genuinely clean, the result is settled `pending: false` with no diagnostics (true clean is still reported as clean).
- [ ] The result distinguishes the three states — pending / clean / has-errors — at the surface the model sees.
- [ ] (Secondary) a genuine leader-unreachable case yields a typed `diagnostics_unavailable` marker, not an empty clean-looking envelope.
- [ ] No second LSP client spawned; the edit never fails because diagnostics are pending.

## Tests (real-path, not mocks — the missing coverage)
- [ ] **Core regression (fails today):** drive `settle`/`diagnose_with_outcome` with a session that is still warming up — work-done/indexing in progress (or no flycheck pass completed) and zero publishes — and assert the outcome is `pending: true`, NOT settled-empty. Use the injectable timer/manual clock already used by the settle tests.
- [ ] Update/replace `outcome_reports_not_pending_when_settled` (diagnose.rs:628) so "quiet" alone no longer implies not-pending; a settled-not-pending result requires a completed analysis pass, not just stream silence.
- [ ] **Settled-clean still works:** a session that completes its pass with no diagnostics yields `pending: false`, empty (no false-pending regression).
- [ ] **End-to-end (gate on rust-analyzer installed; skip cleanly if absent):** edit a fixture file to an E0308 through the MCP edit path; assert the result is `pending` during warm-up and folds in the real diagnostic after the pass completes — never a silent clean on broken code.
- [ ] (Secondary) forced leader-unreachable → typed `diagnostics_unavailable` marker.
- [ ] `cargo nextest run -p swissarmyhammer-diagnostics -p swissarmyhammer-tools` green (NEVER plain `cargo test`); `cargo clippy -- -D warnings` clean.

## Notes for the next agent (don't repeat dead ends)
- The leader-election/"follower can't bootstrap a leader" hypothesis was investigated and **disproven** for this repro (rust-analyzer was running and reachable, the explicit tool returned empty-not-error). Start at the settle/quiescence layer, not leader election.
- The precise spot to fix is where `settle` decides quiescence vs the server's actual load/flycheck progress.

## Workflow
- Use `/tdd` — write the failing "warming-up ⇒ pending, not settled-empty" test first, then implement progress-aware quiescence.