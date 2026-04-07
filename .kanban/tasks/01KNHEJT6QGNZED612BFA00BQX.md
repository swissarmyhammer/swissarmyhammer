---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffe680
title: BoardData not patched on entity-field-changed for board/column types
---
rust-engine-container.tsx + window-container.tsx\n\nThe old App.tsx entity-field-changed handler updated `setBoard` for board/column/swimlane entity types so that BoardData (columns list, board name) stayed in sync without a full refresh. The new RustEngineContainer only patches entitiesByType, but board-view reads from `board.columns` (BoardData prop from window-container), not from entitiesByType.\n\nThis means column renames, column reorder changes, and board name edits that arrive as entity-field-changed events will update the entity store but NOT the board view until the next full refresh.\n\nSuggestion: Either (a) make board-view derive columns from entitiesByType instead of BoardData, or (b) add a board-data patching path in the entity-field-changed handler for structural types (board, column), or (c) treat column field-changes the same as column creates and trigger a full refresh.\n\nVerification: Rename a column and verify the board view header updates without a manual refresh. #review-finding