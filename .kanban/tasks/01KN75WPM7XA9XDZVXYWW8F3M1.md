---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffff580
title: Test treesitter chunk edge cases (parsed source variants, byte-range extraction)
---
File: swissarmyhammer-treesitter/src/chunk.rs (70.3% coverage, 149/212 lines)\n\nUncovered code (~63 lines):\n- SemanticChunk byte_len(), content(), path() for Parsed variant (lines 74-162)\n- Various ChunkSource::Parsed branch handling\n- chunk_with_context() - context expansion around chunks (lines 375-393)\n- Database record conversion methods (lines 421-429, 462-501)\n\nThe Parsed variant of ChunkSource has several uncovered accessors and the context expansion logic needs testing with real parsed files." #coverage-gap