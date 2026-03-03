---
position_column: done
position_ordinal: f5
title: Deduplicate default column definitions (3 copies)
---
**Done.** Unified default column definitions to `Board::default_column_entities()` as single source of truth.\n\n- [x] Remove `fn default_columns()` from `board/init.rs`\n- [x] Remove inline column tuple from `processor.rs`\n- [x] Have both call `Board::default_column_entities()`\n- [x] Verify tests pass (226 tests, clippy clean)