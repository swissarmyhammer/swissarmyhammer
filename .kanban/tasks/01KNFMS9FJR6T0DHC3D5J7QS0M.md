---
assignees:
- claude-code
position_column: todo
position_ordinal: 9a80
title: UndoStack fields are public -- allows external corruption of pointer invariant
---
swissarmyhammer-store/src/stack.rs\n\n`UndoStack` has three public fields: `entries`, `pointer`, and `max_size`. External code can set `pointer` to an invalid value (beyond `entries.len()`) or directly mutate `entries` without adjusting the pointer, breaking the invariant that `entries[0..pointer)` are done and `entries[pointer..len)` are redo-able.\n\nThe `load()` method defensively clamps the pointer, which suggests this concern was partially anticipated, but the public API should not require external defense.\n\nSuggestion: Make fields private. The `entries` and `pointer` fields are only accessed via serde for load/save and via the existing methods (`push`, `record_undo`, etc.). The serde access can use `#[serde(with)]` or a private inner struct. #review-finding