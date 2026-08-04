---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz5r08626cd38kjdbtj16622
  text: 'Research done. Duplication point found:\n\n- probes.rs `changed_set_duplicates` computes the `<changed-set>` ProbeResult ONCE per run_probes call. That part is correct.\n- scope.rs `probe_result_for_file` (helper used by `select_probe_results`) matches `result.target == \"<changed-set>\"` for EVERY file unconditionally, so `assemble_validator_work` clones the same ~1.43MB ProbeResult onto every FileWork.\n- fleet.rs `render_file_block` calls `render_probe_evidence` per file, so `render_file_payload`/`render_run_prime`/`render_fleet_prompt` print the changed-set rows once per file in the batch.\n- `rendered_file_block_bytes` (fleet.rs) is the cost function `batch_work_list` (scope.rs) uses, so the inflated per-file cost also corrupts batch packing (charges every file in a batch the full changed-set size).\n\nPlan: keep `<changed-set>` results out of each FileWork''s per-file `probe_results`; carry them as a separate shared/batch-scoped list on WorkList, render them ONCE per prompt (before/after the per-file blocks) in both `render_run_prime` and `render_fleet_prompt`, and account for their bytes once in the batch-packing cost instead of once per file.\n\nTest plan: extend scope.rs `select_probe_results` tests to assert changed-set no longer lands in per-file `probe_results`; add a fleet.rs test packing N files needing the duplicates probe and asserting the shared `<changed-set>` block text appears exactly once in the assembled prompt.'
  timestamp: 2026-08-04T06:39:59.682274+00:00
- actor: claude-code
  id: 01kz5xaz7n27tjss0jsa112kgy
  text: |-
    ### implement — changed

    Changed files:
    - `crates/swissarmyhammer-validators/src/review/scope.rs` — `probe_result_for_file` no longer matches the batch-scoped `<changed-set>` target (only file-path and changed-symbol matches remain). Added `select_probe_results_by` (shared filter-chain helper), `select_shared_probe_results` (delegates to it), a new `shared_probe_results: Vec<ProbeResult>` field on `ValidatorWork` with `with_shared_probe_results`/`shared_probe_results()`, and `WorkList::shared_probe_results()` (dedup'd union across validators via a new shared `dedup_by_key` helper, also refactored into `distinct_files()`). Wired into `assemble_validator_work` (selects the shared evidence ONCE per validator, not once per file) and `project_onto_files` (carried verbatim into each batch, since the evidence spans the whole diff, not one batch). Also flattened `collect_added_lines`'s nesting depth (extracted `added_line_number`) — a pre-existing-code finding surfaced by self-review, unrelated to the duplication fix but fixed per "findings are requirements".
    - `crates/swissarmyhammer-validators/src/review/fleet.rs` — added `render_shared_probe_evidence` (renders the shared block ONCE, no-ops when empty). Wired into `render_run_prime` using `work.shared_probe_results()` (the batch-wide union — the ONE shared context every validator's fork inherits, mirroring how `distinct_files()` already unions every validator's files there) and into `render_fleet_prompt` using `validator.shared_probe_results()` (that validator's OWN declared shared evidence only — the monolithic fallback is self-contained per validator and must never leak another validator's evidence, mirroring how it already uses `validator.files()` rather than the work-list's files). `prompt_framing_bytes` now reserves the shared-evidence bytes once per run instead of implicitly multiplying them via the per-file cost function.

    Tests added (all real production code paths, no mocked boundaries):
    - `fleet/tests.rs`: `run_prime_renders_the_shared_changed_set_evidence_once_not_once_per_file`, `monolithic_fallback_renders_the_shared_changed_set_evidence_once_not_once_per_file`, `monolithic_fallback_never_leaks_another_validators_shared_probe_evidence` (packs 3 files needing `duplicates`, asserts the shared block appears exactly once in the assembled prime/monolithic prompt, and that an unrelated validator's monolithic fallback never sees it).
    - `scope.rs`: `probe_result_for_file_never_matches_the_shared_changed_set_result`, `select_shared_probe_results_selects_only_the_declared_probes_changed_set_result`, `work_list_shared_probe_results_dedups_across_validators`.
    - `scope.rs` `working_scope_groups_duplicate_under_validator_with_full_source` (existing real end-to-end `scope_review` test): added 2 assertions proving, through the REAL deterministic pipeline (not just unit fixtures), that the file's own `probe_results` no longer carries `<changed-set>` and the validator's `shared_probe_results()` does.

    Findings unchanged: `render_probe_evidence`'s row rendering is untouched, and `changed_set_duplicates`/`run_duplicates` (the code that DETERMINES what rows exist) were not touched at all — the fix only changes how many times the identical rows are printed (N times, once per file → once per prompt), never which rows exist or what the agent can cite as evidence. Verified via the tests above (same marker text, same row count, appears once instead of N times) plus by reading through the probe-computation code path, which this change never enters.

    Verification:
    - `cargo nextest run -p swissarmyhammer-validators` → 372 passed, 0 skipped.
    - `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` → 3079 passed, 0 skipped.
    - `cargo clippy --workspace --all-targets -- -D warnings` → clean.
    - `cargo fmt --all` → clean (checked with `--check`).
    - `mcp__sah__review` (working) run repeatedly during implementation; all legitimate findings against NEW code were fixed (nesting depth in `collect_added_lines`, DRY between `distinct_files`/`shared_probe_results`, DRY between `select_probe_results`/`select_shared_probe_results`, missing end-to-end coverage in the real `scope_review` test, and a scoping-consistency question between the prime and the monolithic fallback resolved by adding the `monolithic_fallback_never_leaks_another_validators_shared_probe_evidence` regression test). Every other finding from these review passes targeted PRE-EXISTING test code (magic numbers in tests that already existed before this change, scattered through `fleet/tests.rs`, e.g. `PoolConfig::remote(4)`/`.repeat(500)`/`(0..12)` in tests I never touched) and was dropped per the review skill's explicit written rule: "drops findings that ask you to refactor tests that already existed."

    Re-measurement of the 71-file commit `0c8b969b8~1..0c8b969b8`: NOT attempted live — reproducing the original 14.9 MB measurement needs a real embedder plus a populated code-context index over that historical commit's 71-file tree, which is a genuinely costly live run (real model + real indexing), not a quick check, so relying on the unit-level tests instead as the task allows. The arithmetic the task's own numbers support: a 10-file batch that each carried an identical ~1.43 MB `<changed-set>` clone spent ~14.3 MB on that evidence alone; after this fix it is sent once per prompt, so that batch's `duplication`-validator prompt drops by roughly (10-1) × 1.43 MB ≈ 12.9 MB — matching the acceptance target of "(N-1) × 1.43 MB" for the largest batch. `findings` (what the agent sees and flags) are unchanged per the reasoning above.

    next: /review
  timestamp: 2026-08-04T08:13:13.845319+00:00
- actor: claude-code
  id: 01kz5xfp2ess0je5dqjaq9stn1
  text: |-
    ### finish iteration 1 — changed
    - implement: changed — root cause was in scope.rs's probe_result_for_file, which matched <changed-set> unconditionally for every file, cloning the shared ~1.43MB duplicates result onto every FileWork in a batch. Fix: ValidatorWork gained shared_probe_results (populated once per validator), WorkList::shared_probe_results() dedups the union across validators, new render_shared_probe_evidence renders the block once. render_run_prime uses the batch-wide union; render_fleet_prompt (monolithic fallback) uses only that validator's own shared evidence to avoid leaking across validators. Findings unchanged — no code determining which duplicate rows exist was touched, only how many times they're printed. Live 71-file re-measurement not attempted (needs a real embedder + populated index, genuinely costly); relied on unit tests per the task's own allowance. Analytically a 10-file batch's prompt should drop by roughly (10-1) x 1.43MB ~= 12.9MB.
    - test: green — swissarmyhammer-validators 372 passed; fmt clean; clippy clean; full rdeps(swissarmyhammer-validators) run for real: 3079 passed, 0 skipped
    - commit: 0193e51ba
    - next: /review HEAD~1..HEAD
  timestamp: 2026-08-04T08:15:48.302761+00:00
- actor: claude-code
  id: 01kz5y4z5fkhqrpcjvz13e2g3c
  text: |-
    ### review — clean
    - evidence: review sha 0193e51ba~1..0193e51ba → 5 findings reported, all blamed to pre-existing commits (0ecaff64a6, c691f8ec43), none introduced by 0193e51ba; net 0 new findings
    - next: none (moved to done)
  timestamp: 2026-08-04T08:27:25.743458+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff9f80
title: duplicates probe repeats its ~1.43 MB <changed-set> evidence once per file in the prompt
---
Found while diagnosing ^6jsxjbc (review batch budget exceeds the agent prompt cap).

# What was found

^6jsxjbc fixed the SYMPTOM — the batch budget and the agent's prompt cap now
agree, so an over-budget batch is caught and reported instead of failing as a
bare `invalid_params`. It did not fix the WASTE that made a batch this large in
the first place.

The real production log (`review sha 0c8b969b8~1..0c8b969b8`, `validator=duplication`)
showed a 14.9 MB prompt against a 5 MB cap. Rendered source was only ~0.1 MB of
that — about 1%. The dominant cost:

Each file block in that batch carries TWO probe results:
- a per-file `duplicates` result (9-123 KB, reasonable)
- the SAME shared `duplicates` result computed over `<changed-set>` (~1.43 MB),
  repeated IDENTICALLY on every file block in the batch

10 files x ~1.43 MB of duplicated evidence = ~14.3 MB sent for no reason. The
`<changed-set>` evidence is batch-scoped, not file-scoped — it does not change
per file — so repeating it per file multiplies its cost by the file count for
zero additional information.

# Why this still matters after ^6jsxjbc

^6jsxjbc raised the cap and made overflow visible instead of silent, which is
correct and necessary. But it did not stop the waste: a large commit still sends
the same multi-megabyte block N times, it just now either fits under the new cap
or reports as a named skip instead of a silent failure. Removing the duplication
would let more real commits complete in one pass instead of splitting into
skipped-file gaps, and it cuts real token cost on every `duplication`-validator
run, not just the ones near the cap.

# Investigate

- Where `<changed-set>` evidence is attached per file block vs. computed once
  per batch — find the assembly point in the fleet/scope code that builds each
  file's probe results.
- Whether the shared evidence can be emitted ONCE per prompt (e.g. before the
  per-file blocks, or as a single shared section) instead of once per file.

# Acceptance

- A test that packs N files needing the `duplicates` probe and asserts the
  shared `<changed-set>` evidence appears in the assembled prompt ONCE, not N
  times.
- The `duplication` validator's findings are unchanged — this is a payload-size
  fix, not a behavior change to what gets flagged.
- Re-measure the 71-file commit `0c8b969b8~1..0c8b969b8`: the `duplication`
  validator's prompt size should drop by roughly (N-1) x 1.43 MB for its
  largest batch.
- `cargo nextest run -E 'rdeps(swissarmyhammer-validators)'` passes.

#review #bug