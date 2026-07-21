---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01ky29c1stnag26gh9agrxn7w2
  text: |-
    Approach locked (user: "I want CI to pass, 'related' means nothing" + "I also want useful tests"). Persistent failure = ONLY the diagnostics real-RA IPC tests (leader_follower_request_ipc: rename+definition) — they run at FULL --workspace parallelism (not in any nextest test-group) so a real rust-analyzer is CPU-starved and misses the fixed 30s LSP_REQUEST_TIMEOUT. Same class the repo already solved for fsevents-watcher/treesitter-embedding via capped test-groups. The kanban ai_panel_e2e real-model test is NOT persistent (passed on the lighter 311s prior run; only blew its existing 10-min budget on my 3x-loaded 902s run) — leave it unless a normal-load run still fails it.

    Fix (keeps tests USEFUL — they still drive real rust-analyzer through the real leader/follower socket and still assert real edits):
    1. Make LSP_REQUEST_TIMEOUT (crates/swissarmyhammer-lsp/src/client.rs:30) env-overridable via SAH_LSP_REQUEST_TIMEOUT_SECS, default 30 (a real product-config improvement — a cold RA on a slow/loaded machine legitimately needs more than a hard 30s). Read once (OnceLock).
    2. Serialize the diagnostics real-RA IPC test binaries in .config/nextest.toml (new test-group, max-threads=1, generous slow-timeout) — matches the fsevents/treesitter precedent, reduces concurrent rust-analyzers.
    3. Grant a load-tolerant timeout to CI+local test runs (e.g. .cargo/config.toml [env] or CI workflow env) without changing the 30s production default.
    Reproduce the diagnostics IPC tests locally to confirm green before pushing.
  timestamp: 2026-07-21T12:10:52.602340+00:00
position_column: doing
position_ordinal: '8380'
title: 'CI red ~2 weeks: real-rust-analyzer LSP request timeouts (rename/definition) + real-model e2e >600s on the self-hosted runner'
---
## What

CI's `test` job (`cargo nextest run --no-fail-fast`, `.github/workflows/ci.yml`) has failed on **every commit since 2026-07-07** (last green: `fa71ba0d3`; first red: `b9ddb00e9` on 2026-07-08 — 9+ consecutive failures). Because every other CI job (`fmt`, `clippy`, `frontend`, `apps`, `docs`, `examples`) declares `needs: test`, the whole pipeline has been red for two weeks. This is NOT caused by recent feature work — it was verified that a clean 15-commit batch (the review-notification work, HEAD `39a0bfb9e`) adds ZERO new failures and reproduces only these pre-existing ones.

Failing tests (all real-external-dependency timeouts, all in crates unrelated to recent feature work):
- `swissarmyhammer-diagnostics::leader_follower_request_ipc::follower_multi_step_rename_gets_real_leader_edits_under_one_lock` — `textDocument/rename` request times out after 30s (`crates/swissarmyhammer-diagnostics/tests/leader_follower_request_ipc.rs:422`). The persistent one (fails even on a lightly-loaded run).
- `swissarmyhammer-diagnostics::leader_follower_request_ipc::follower_request_with_document_gets_real_definition_without_leader_preopen` — `textDocument/definition` request times out after 30s (same file:301). Trips additionally when the runner is loaded.
- `kanban-app::ai_panel_e2e::test_ai_panel_e2e_qwen_generates_tokens_and_second_prompt_succeeds` — real-model Qwen e2e, TERMINATED at >600s (nextest terminate-after). Same class as the tracked ^nmms6bb real-model flake.

## Root cause (evidence)

The regression window `fa71ba0d3..b9ddb00e9` contains exactly ONE commit — a validator-DESCRIPTION-only change — which cannot make rust-analyzer's `rename` request time out. So the redness is **environmental, not a code regression**:
- CI uses `dtolnay/rust-toolchain@stable` + `rustup component add rust-analyzer` with **no `rust-toolchain.toml` pin** (confirmed: no pin file exists) — every run floats to the latest stable rust-analyzer, whose timing/behavior can drift the real-RA integration tests past their fixed 30s ceiling.
- The 30s ceiling is `const LSP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30)` in `crates/swissarmyhammer-lsp/src/client.rs:30`.
- Failure count scales with runner load: a 311s test run had 1 failure; a 902s (~3× loaded) run had 3 — the signature of request timeouts under contention, consistent with the documented rust-analyzer-thrashing issues ([[project_lsp_spawn_not_leader_gated]]).

## Acceptance Criteria
- [ ] CI's `test` job is green for a current-HEAD commit (the pipeline's downstream jobs then run instead of skipping)
- [ ] The chosen remedy does NOT suppress coverage (no `#[ignore]`, no deleting the leader/follower assertions); it either makes the real-RA tests robust to runner conditions or removes the environmental nondeterminism
- [ ] The real-model Qwen e2e (`kanban-app::ai_panel_e2e`) either completes under its cap reliably or is moved to the manually-triggered/real-model lane the way other real-model coverage is (cf. coverage.yml split), not left to terminate at 600s in the gating `test` job

## Investigate / candidate fixes (owner decision — pick and implement)
- [ ] Pin the toolchain: add `rust-toolchain.toml` (channel + a known-good rust-analyzer), so CI stops floating rust-analyzer version run-to-run. Likely the highest-leverage single change.
- [ ] Re-evaluate `LSP_REQUEST_TIMEOUT` (client.rs:30) for real-RA-on-loaded-runner reality — a cold/contended rust-analyzer rename can legitimately exceed 30s; a higher ceiling (or a warm-up gate before the timed request) may be correct rather than a weakening. Justify whichever way.
- [ ] Gate/serialize the real-RA leader/follower IPC tests so multiple rust-analyzers don't thrash the runner during the run (ties into [[project_lsp_spawn_not_leader_gated]] — is LSP spawn leadership-gated in the test harness?).
- [ ] Move the real-model `kanban-app::ai_panel_e2e` Qwen test out of the gating `test` job into the manual real-model lane.

## Tests
- [ ] Reproduce locally: `cargo nextest run -p swissarmyhammer-diagnostics --test leader_follower_request_ipc` on a loaded machine, confirm the rename/definition timeout, then confirm the chosen remedy makes it deterministically green across repeated runs
- [ ] After the fix, a full CI run for current HEAD shows `test` green and the downstream jobs (fmt/clippy/frontend/apps/docs/examples) actually run (not skipped)

## Workflow
- Diagnose the specific remedy first (toolchain pin vs timeout vs serialization vs lane-move), then implement and prove with repeated runs. Do NOT paper over with ignore/retry. #ci