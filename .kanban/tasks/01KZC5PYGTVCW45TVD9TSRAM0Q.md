---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzd035ym1fc2ky852yerr3zd
  text: |
    ### Research

    Root cause confirmed. `run_review` (synthesize.rs) computes one number and uses it for two different jobs:

    ```
    let framing = prompt_framing_bytes(&work, loader);
    let budget  = fleet_config.file_payload_budget(framing);   // = batch_size.min(AGENT_PROMPT_CAP - framing)
    let (batches, skipped) = batch_work_list(&work, budget, rendered_file_block_bytes);
    ```

    `batch_work_list` uses that ONE number both to decide where batch boundaries fall AND to decide the over-cap verdict (`size > budget` -> `SkippedFile`).

    `prompt_framing_bytes` (fleet/render.rs) grows with the diff:
    - `render_shared_probe_evidence` over the WHOLE changed set (the `<changed-set>` duplicates rows) — the dominant term, ~18 KB per changed file in the reported rounds;
    - `render_focus_files` inside `render_validator_suffix` — one line per matched file.

    So more files -> more framing -> smaller budget -> more files over cap. The card's loop.

    Numbers: `AGENT_PROMPT_CAP = 512 * 1024 = 524288` (claude_agent `MAX_PROMPT_LENGTH`). Round 1 budget 164176 -> framing 360112. Round 2 budget 54338 -> framing 469950.

    Plan: split the one number into two.
    - constant per-file cap decides the over-cap verdict — `min(batch_size, AGENT_PROMPT_CAP / 2)`, independent of the diff;
    - measured per-batch budget still decides batch boundaries, so prompts keep fitting.
  timestamp: 2026-08-07T02:16:05.332233+00:00
- actor: claude-code
  id: 01kzd0xfc9ngtzvm6hhefp8h5g
  text: |
    ### Implementation

    Split the one number into two.

    `crates/swissarmyhammer-validators/src/review/scope/batch.rs` now takes a `BatchBudget { file_cap, batch_bytes }` built from the `FileCapBytes` and `BatchBytes` newtypes (the `TasksAttempted`/`TasksFailed` pattern, so the two byte counts cannot be transposed):
    - `file_cap` decides the over-cap verdict — `size > budget.file_cap()` is the `SkippedFile`;
    - `batch_bytes` decides where batch boundaries fall.

    `FleetConfig` gained `MAX_FILE_BLOCK_BYTES` (`AGENT_PROMPT_CAP / PROMPT_SHARES_PER_FILE_BLOCK`, so 256 KiB), `file_block_cap()` (`batch_size.min(MAX_FILE_BLOCK_BYTES)`), and `batch_budget(framing)` which pairs the constant cap with the framing-sensitive `file_payload_budget(framing)`. Building both in one place is what stops a call site collapsing them again.

    `SkippedFile::budget()` became `cap()`, and the report now says "over the N-byte per-file cap ... (split the file)" instead of "over the N-byte batch budget ... (narrow the scope)". Narrowing the scope can no longer change the verdict, so the old advice was wrong.

    RED -> GREEN was verified by toggling `batch_budget` back to the pre-fix cap. With the defect restored, `a_file_inside_the_cap_stays_inside_it_when_the_change_around_it_grows` failed with exactly the reported symptom:

        SkippedFile { path: "src/subject.rs", validator: "bulk", size: 197707, cap: 50946 }

    The subject renders 197707 bytes and satisfies the 262144-byte cap, but the bigger change's shared probe evidence dropped the old threshold to 50946.

    ### Discovery, recorded as ^x8z9hgf

    `prompt_framing_bytes` is 360112 bytes on the 9-file round and 469950 on the 15-file round, against a 524288-byte cap — 69% then 90% of every prompt. It is dominated by the `<changed-set>` shared probe evidence, which `project_onto_files` carries verbatim into every batch. That squeeze is a separate defect; new card `^x8z9hgf` covers it.
  timestamp: 2026-08-07T02:30:26.953880+00:00
- actor: claude-code
  id: 01kzd0xt1ftrt6cs8a1mst01mt
  text: |
    ### implement — changed
    - evidence: 12 files — crates/swissarmyhammer-validators/src/review/scope/batch.rs, review/scope.rs, review/scope/tests.rs, review/fleet.rs, review/fleet/render.rs, review/fleet/tests.rs, review/synthesize.rs, review/test_support.rs, review/mod.rs, crates/swissarmyhammer-tools/src/mcp/tools/review/mod.rs, review/review_op/response.rs, review/tests.rs. `cargo test -p swissarmyhammer-validators` 455 passed, 0 failed; `cargo test -p swissarmyhammer-tools --lib` 1313 passed, 0 failed; clippy `-D warnings` clean on both crates; `cargo check --workspace --all-targets` clean.
    - next: /review
  timestamp: 2026-08-07T02:30:37.871171+00:00
position_column: doing
position_ordinal: '8480'
title: Review batch budget shrinks as the diff grows — over-cap splits cannot converge
---
Found while watching ^cbnfe97. The per-batch prompt budget is not stable between review runs on the same code:

- Round 1 (checkpoint 716951039, 9-file diff): batch budget 164176 bytes. 4 files over cap.
- Round 2 (checkpoint 503b74346, 15-file diff): batch budget 54338 bytes — one third of round 1. 13 files over cap, INCLUDING files the round-1 fix just split (fleet.rs 82→52KB source, scope.rs 177→49KB source) and files untouched by the diff logic (doctor.rs).

The feedback loop: an over-cap finding says "split the file" (by design — c45ba2d40 made over-cap a confirmed finding). Splitting adds files to the next diff. A bigger diff shrinks the per-batch budget. A smaller budget puts MORE files over cap. Each fix round makes the next round worse. This cannot converge.

Work:
- Find where the batch budget is computed (batch_work_list / cost math, now in review/scope/batch.rs) and confirm the budget depends on diff size or batch count.
- Make the over-cap threshold stable per file: a constant cap, independent of how many files the diff carries.
- An over-cap verdict must be reproducible: the same file content gets the same verdict on every run.

Acceptance:
- Two consecutive review runs over the same content report the same set of over-cap files.
- A file that satisfied the cap in run N cannot be over cap in run N+1 without growing.

#tool-validators