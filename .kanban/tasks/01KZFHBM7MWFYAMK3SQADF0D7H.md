---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzgst7jcrj0kykwbhtsgcwgc
  text: |-
    Research done. Facts:

    - `find_duplicates_in` splits the corpus into `source_chunks_list` and `other_chunks` on `chunk.file_path == file`. Only cross-file comparison occurs.
    - `find_duplicates` (connection path) delegates to `find_duplicates_in`, so one change corrects both paths.
    - Callers of `find_duplicates_in`: `run_duplicates` and `run_clone_siblings` in `crates/swissarmyhammer-validators/src/review/probes.rs`. `run_clone_siblings` filters out files under review, so intra-file rows do not change its output. `run_duplicates` will now carry intra-file rows — the goal of this card.
    - `changed_set_duplicates` does not call `find_duplicates_in`. It embeds changed blocks and compares them pairwise across files. Its behavior does not change. Its tests confirm the two-new-files case.
    - `compared_chunks` has no consumer outside `find_duplicates.rs`. New meaning: the count of candidate chunks each source chunk is ranked against (all eligible chunks except itself). With one source chunk and one other chunk, the count stays 1, so the existing assertion holds.
    - Plan: identity check with `std::ptr::eq` keeps a chunk from matching itself. Thresholds stay at min_similarity 0.85, min_chunk_bytes 100, max_per_chunk 5.
    - Note: `dump validators` is not an op in this build of the `review` tool. Used `list validators` to load the rules instead.
  timestamp: 2026-08-08T13:43:18.348998+00:00
- actor: claude-code
  id: 01kzgt4f59k9wykkcvqqj8jdwy
  text: |-
    ### implement — changed
    - evidence: 2 files — crates/swissarmyhammer-code-context/src/ops/find_duplicates.rs, crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs (doc only). Proving test: `ops::find_duplicates::tests::test_finds_duplicate_within_same_file` (RED first: failed at compared_chunks 0 vs 1 before the fix; GREEN after).
    - what changed: `find_duplicates_in` no longer splits the corpus into source vs other files. It builds one `eligible` pool (size floor `min_chunk_bytes` unchanged) and ranks each source chunk against the whole pool minus itself, with identity by `std::ptr::eq`. Intra-file duplicates now surface. Thresholds held: min_similarity 0.85, min_chunk_bytes 100, max_per_chunk 5. `compared_chunks` now means "candidates each source chunk is ranked against" (pool size − 1; 0 when the file has no eligible chunks) — it has no consumer outside the file, and the existing assertions still hold. Stale "other files" docs corrected in both files.
    - test fixture: modeled on `apps/swissarmyhammer-cli/src/signal_handler.rs` (block at lines 6-25 repeats at 38-57), with default thresholds. Asserts each chunk matches only the OTHER chunk, never itself.
    - confirmations: `changed_set_duplicates` unchanged; `review::probes::tests::duplicates_flags_a_block_pasted_into_two_new_unindexed_files` passes — a block pasted into two new files is still reported. `run_clone_siblings` filters files under review, so its output is unchanged by intra-file rows.
    - verification: swissarmyhammer-code-context lib 644 passed; swissarmyhammer-validators 526 + 2 doc-tests passed; swissarmyhammer-tools lib 1313 passed. `cargo clippy -p swissarmyhammer-code-context --all-targets -- -D warnings` clean; `cargo fmt --check` clean; LSP diagnostics on the working set: 0 errors, 0 warnings.
    - next: ready for /review.
  timestamp: 2026-08-08T13:48:53.801761+00:00
position_column: doing
position_ordinal: '8380'
title: duplicates probe cannot see duplication inside one file
---
`find_duplicates_in` in `crates/swissarmyhammer-code-context/src/ops/find_duplicates.rs` splits the corpus into `source_chunks_list` and `other_chunks` by `chunk.file_path == file` (lines 136-140). It compares the source file only against other files. It can never report two duplicate blocks in the same file.

Measured on this repo: 0 intra-file pairs out of 91480 reported pairs.

`jscpd` found 470 prod-to-prod duplicate pairs inside a single file. Examples:

- `apps/swissarmyhammer-cli/src/error.rs` lines 83-96 against lines 140-153
- `apps/swissarmyhammer-cli/src/signal_handler.rs` lines 6-25 against lines 38-57
- `crates/claude-agent/src/acp_error_conversion.rs`, four repeats of 20 lines, first at line 183

The duplication prompt rule asks the model to find verbatim and near-verbatim copies. It receives the probe as its machine evidence. Today that evidence is blind to the whole intra-file class.

Do this:

- Let a chunk of the source file compare against the other chunks of the same file. Keep a chunk from matching itself.
- Hold the existing thresholds: `min_similarity 0.85`, `min_chunk_bytes 100`, `max_per_chunk 5`.
- Add a test that proves an intra-file duplicate is reported. Use one of the three files named above as the fixture shape.
- Confirm `changed_set_duplicates` still reports blocks pasted into two new files.

Found while evaluating jscpd for ^3b49ewn. jscpd was rejected; this gap is the one true finding of that evaluation.

#tool-validators #objectivity