---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffff780
title: 'Coverage: read_source_range in ops/lsp_helpers.rs'
---
crates/code-context/src/ops/lsp_helpers.rs

Coverage: 0% (0/4 lines)

File reading helper that extracts a line range from a source file. Test with: valid file and range, missing file, range beyond EOF, single-line range. Use a temp file with known content. #coverage-gap