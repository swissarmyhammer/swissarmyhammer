---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffcb80
title: Add tests for indexing worker loop and chunk parsing
---
indexing.rs\n\nCoverage: 68.4% (54/79 lines)\n\nUncovered lines: 60-61, 80, 90, 95, 97, 102, 108-109, 111-113, 121, 127, 133, 136, 141, 145, 151, 155, 163, 211-212, 215, 223\n\nTwo functions:\n1. `run_indexing_worker` (lines 73-165) - main worker loop: queries dirty files, processes in parallel, writes chunks, marks indexed\n2. `parse_and_extract_chunks` (lines 198-234) - chunk-splitting logic for large files\n\nTest scenarios:\n- Set up shared DB with dirty files, call run_indexing_worker (or exercise via spawn_indexing_worker with short sleep), verify files marked indexed and ts_chunks written\n- parse_and_extract_chunks with file > 1000 bytes → verify chunk splitting\n- parse_and_extract_chunks with empty file → Ok(vec![])\n- parse_and_extract_chunks with normal file → single chunk returned\n\n#coverage-gap #coverage-gap