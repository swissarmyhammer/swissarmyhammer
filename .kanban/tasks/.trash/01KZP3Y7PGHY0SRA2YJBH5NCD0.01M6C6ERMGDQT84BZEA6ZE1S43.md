---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kztw1rgeyjj6sj6ja1k9rhp2
  text: |-
    ### All four tests pass at HEAD — measured 2026-08-12

    Found during the test step of ^wwb6hk7. `cargo nextest run --workspace` at commit 59bd9ae5c: 14136 passed, 0 failed. Each of the four named tests was checked by name in the raw output:

    - `review_e2e_report_lands_on_a_kanban_task_in_the_dated_gfm_format` — PASS
    - `review_e2e_sha_range_confirms_the_same_defects` — PASS
    - `review_e2e_working_confirms_real_defects_and_refutes_both_red_herrings` — PASS
    - `review_progress_is_received_by_a_real_client_over_a_byte_stream_transport` — PASS

    **Cause: the rule deletion on ^wwb6hk7.** The test files did not change between 0de4a3936 and HEAD — `git diff` over them is empty — so no test-code edit explains it. The fix is in `builtin/validators/duplication/VALIDATOR.md`. This card recorded the root cause as a report that carried a `reuse` finding but not the `duplication` claim the tests check (`CLAIM_DUP`). Before, VALIDATOR.md said a tool rule owned duplicate detection for parsed languages. ^wwb6hk7 removed the `duplication-parsed` tool rule, and VALIDATOR.md now states that the `duplication` prompt rule owns the job and reads the `duplicates` probe directly. That restores the missing claim.

    This card was not driven through the review gate, so it is left open for a person to close.
  timestamp: 2026-08-12T11:34:46.542565+00:00
position_column: todo
position_ordinal: ffc980
title: 'review_e2e duplication claim is missing: three review_e2e tests and the stdio streaming test fail on main'
---
`cargo nextest run --workspace` has four failures on the `review` branch. They are not caused by the change on ^2r35j9t. I proved this: I put the five deleted orphan files back with `git cat-file blob`, ran the fastest failing test again, and it failed the same way. The run did not recompile anything, which shows the orphan files never reached the compiler.

Failing tests:

- `swissarmyhammer-tools::tools_tests integration::review_e2e::review_e2e_sha_range_confirms_the_same_defects`
- `swissarmyhammer-tools::tools_tests integration::review_e2e::review_e2e_working_confirms_real_defects_and_refutes_both_red_herrings`
- `swissarmyhammer-tools::tools_tests integration::review_e2e::review_e2e_report_lands_on_a_kanban_task_in_the_dated_gfm_format`
- `swissarmyhammer-tools::review_progress_stdio_test review_progress_is_received_by_a_real_client_over_a_byte_stream_transport`

The three `review_e2e` tests fail on the same assertion at `crates/swissarmyhammer-tools/tests/integration/review_e2e.rs:161` — `report_has_claim(markdown, CLAIM_DUP)`. The report has five findings, but none of them is the duplication claim the test wants:

```
- [ ] `src/orphan.rs:3` — orphan_never_called has no inbound callers and is dead. fix it.
- [ ] `src/payments.rs:5` — STRIPE_KEY is a hardcoded live secret. fix it.
- [ ] `src/payments.rs:16` — fee_for_tier hardcodes a tier if-chain that should be a table. fix it.
- [ ] `src/payments.rs:16` — fee_for_tier returns a bare f64 where a typed Money would be safer. fix it.
- [ ] `src/util_reuse.rs:3` — my_mean_squared reimplements the shared mean_squared_error util. fix it.
```

The last line is the reuse claim, not the duplication claim. Look at `CLAIM_DUP` and at the recent validator work (`ab778d1dc feat(validators): split magic-numbers into tool rules, narrow data-driven`) to find which side moved.

The stdio test times out after 70 s at `crates/swissarmyhammer-tools/tests/review_progress_stdio_test.rs:168` with "timed out waiting for: streamed review findings + verdicts received by the client". Check whether it has the same root cause before you treat it as a separate defect.

#test-failure