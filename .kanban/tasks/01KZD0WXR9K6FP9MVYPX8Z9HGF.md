---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzd6vymjk2ywxazqt1tqyehr
  text: |
    ### Measurement — framing decomposition

    Measured with the REAL builtin validator set (`load_builtins`) and the real renderers, over a 15-file change of this repo.

    Per-validator suffix (`render_validator_suffix`), largest first:

    ```
     21866 suffix ( 21036 without the 15 focus-file lines)  swift
     17426 suffix ( 16596 ... )  completeness
     16022 suffix ( 15192 ... )  code-hygiene
     11232 suffix ( 10402 ... )  duplication
     10995 suffix ( 10165 ... )  numpy
     10847 suffix ( 10017 ... )  js-ts
     10220 suffix (  9390 ... )  code-security
      9044 suffix (  8214 ... )  python
      8385 suffix (  7555 ... )  dart
      8355 suffix (  7525 ... )  test-integrity
      7424 suffix (  6594 ... )  rust
      6152 suffix (  5322 ... )  reuse
    ```

    Shared evidence (`render_shared_probe_evidence` over `<changed-set>` `duplicates`):
    - header + prose + 1 row = 450 bytes
    - 1000 rows = 201249 bytes, so about **201 bytes per row**

    Decomposition of the card's two real-run numbers:

    | term | 9-file run | 15-file run |
    |---|---|---|
    | change purpose + payload header | under 1 KB (one commit message) | under 1 KB |
    | largest validator suffix | about 16-17 KB | about 16-17 KB |
    | **shared evidence** | **about 342 KB (~1700 rows)** | **about 452 KB (~2250 rows)** |
    | total | 360112 | 469950 |

    **The shared evidence is about 95% of the framing.** The card names two terms; the measurement shows they are not comparable — the suffix is about 4% and the purpose under 1%. The 18 KB per added file the card reports is about 90 rows per file, which is what a PAIRWISE comparison of changed entities gives: `changed_set_duplicates` compares every changed block against every other, so the row count grows with the SQUARE of the changed entity count.
  timestamp: 2026-08-07T04:14:28.498540+00:00
- actor: claude-code
  id: 01kzd7f8v7a8wcpnvjwhwynxvd
  text: |
    ### How each term is bounded, and why

    **1. Shared evidence — cap the block** (the card's candidate "cap the evidence block").

    `MAX_SHARED_EVIDENCE_BYTES = MAX_FRAMING_BYTES / 2` = 131072. `render_shared_probe_evidence` renders rows through a new bounded core (`render_probe_evidence_within` in `probes.rs`), stops at a row boundary, and replaces the rest with a notice naming exactly how many rows it dropped. The notice bytes are reserved out of the cap BEFORE any row renders, so the notice can never push the section past it.

    Rejected the other candidate, "filter the shared evidence rows to the batch's files":
    - A `<changed-set>` row names TWO files that can land in different batches, so a per-batch filter drops real evidence.
    - The framing is measured ONCE before batching (it sizes the batches), so a batch-dependent section makes the measure a fixed point of the packing that consumes it.
    - Filtering bounds nothing: one batch's file can still appear in thousands of rows.
    - A constant keeps the section byte-identical across both prompt shapes and across runs over the same change — the fork-prefix reuse contract and the ^tsram0q convergence property both need that.

    **2. Focus-file list — charge it per file** (the card's third candidate).

    `render_validator_suffix` now goes through `render_suffix(name, ruleset, files)`. `prompt_framing` reserves `validator_suffix_framing_bytes` = the same render with NO files; `rendered_file_block_bytes` adds `focus_file_line_bytes`. The identity `suffix == framing + sum(per-file lines)` holds by construction, and is asserted. This removes the only file-count-dependent term from the framing.

    **3. Change purpose — measured, not truncated.** It is one commit message (`resolve.rs::commit_messages`), under 1 KB on the measured runs, so it needs no bound of its own. It is now reported as its own term so a run that gets tight says so.

    ### The invariant

    `MAX_FRAMING_BYTES = AGENT_PROMPT_CAP - MAX_FILE_BLOCK_BYTES` = 262144.

    A batch's prompt is framing plus file blocks, and the packer bounds the blocks two ways: a multi-file batch by `file_payload_budget` (cap minus framing, so it always fits), and a single-file batch by the constant `MAX_FILE_BLOCK_BYTES` (which must NOT read the framing — ^tsram0q). Only the second can overflow, and it cannot once the framing stays inside `MAX_FRAMING_BYTES`.

    ### Verified RED then GREEN

    Acceptance test `a_batch_prompt_fits_the_cap_when_one_file_fills_the_per_file_cap` drives the REAL renderers, the REAL `prompt_framing`, and the REAL `batch_work_list`: two files each sized to fill the per-file cap (sized by probing the renderer, not a hand-computed line count) plus 20000 `<changed-set>` rows.

    - RED (cap removed, premise asserts loosened): `a batch's shared prime is 1056829 bytes, over the 524288-byte cap (framing 811013, full-cap block 258584)`.
    - GREEN: every prime and every monolithic prompt inside 524288; framing bounded; the evidence truncated, not merely fitting.

    New tests: the acceptance test, evidence capped + names dropped rows, evidence that fits renders every row and no notice, the notice fits its reserve, suffix splits into framing + per-file lines, framing stops growing with the file count, and every SHIPPED builtin validator's suffix fits the authored share.

    Updated `a_file_inside_the_cap_stays_inside_it_when_the_change_around_it_grows`: its premise (the grown change's framing crowds out the subject) is now unconstructible — that is the point — so it asserts the bound instead.

    ### Also

    `run_review` logs the decomposition (`framing_purpose`, `framing_shared_evidence`, `framing_validator_suffix`, `framing_largest_validator`, `framing_cap`), so every real run reports which term is large.
  timestamp: 2026-08-07T04:25:01.543499+00:00
- actor: claude-code
  id: 01kzd7ff9k73kbfgpc88dcg47h
  text: |
    ### implement — changed
    - evidence: 6 files — crates/swissarmyhammer-validators/src/review/fleet.rs, crates/swissarmyhammer-validators/src/review/fleet/render.rs, crates/swissarmyhammer-validators/src/review/fleet/tests.rs, crates/swissarmyhammer-validators/src/review/mod.rs, crates/swissarmyhammer-validators/src/review/probes.rs, crates/swissarmyhammer-validators/src/review/synthesize.rs
    - tests: `cargo test -p swissarmyhammer-validators` 462 passed, 0 failed; `cargo clippy -p swissarmyhammer-validators --all-targets` clean; `cargo check --workspace --all-targets` clean; `cargo test -p swissarmyhammer-tools --lib mcp::tools::review` 69 passed
    - next: /review
  timestamp: 2026-08-07T04:25:08.147797+00:00
position_column: doing
position_ordinal: '8480'
title: Review prompt framing eats 70-90% of the agent prompt cap
---
Found while fixing ^tsram0q. `prompt_framing_bytes` measured 360112 bytes on a 9-file change and 469950 bytes on a 15-file change, against an `AGENT_PROMPT_CAP` of 524288. The framing alone takes 69% then 90% of every batch prompt, so file blocks get what little is left.

Two terms make it up:
- the largest validator suffix — every prompt rule body of the biggest validator, authored markdown of unbounded size;
- `render_shared_probe_evidence` over `WorkList::shared_probe_results` — the `<changed-set>` `duplicates` rows, which grew about 18 KB per added file between the two rounds. `project_onto_files` carries this block verbatim into EVERY batch, so each batch pays the whole change's evidence.

Consequences:
- `FleetConfig::file_payload_budget` returns a small number, so batches hold very few files and the run makes many more agent turns than it needs.
- ^tsram0q made the over-cap verdict a constant (`MAX_FILE_BLOCK_BYTES`, half the cap), so a file between the packing budget and the cap now takes a batch of its own. When the framing is this large that one prompt can pass `AGENT_PROMPT_CAP`, and `AgentPool::enqueue` refuses it with `PromptTooLong`.

Work:
- Measure the framing decomposition on a real run of this repo: purpose, shared evidence, and per-validator suffix.
- Decide how to bound each term. Candidates: filter the shared evidence rows to the batch's files, cap the evidence block, or charge the focus-file list per file instead of as run framing.
- Prove a batch prompt stays inside `AGENT_PROMPT_CAP` even when one file fills the per-file cap.

#tool-validators
