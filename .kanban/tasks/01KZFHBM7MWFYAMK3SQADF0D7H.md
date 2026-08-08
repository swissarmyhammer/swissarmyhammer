---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9a80
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