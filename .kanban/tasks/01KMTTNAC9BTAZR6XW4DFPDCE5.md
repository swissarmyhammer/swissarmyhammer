---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffa980
title: 'Review finding: UndoStack max_size enforced only on push -- load can exceed limit'
---
**Severity**: Low (defensive edge case)\n**File**: `swissarmyhammer-entity/src/undo_stack.rs` lines 168-177\n\nWhen `UndoStack::load()` deserializes from YAML, it does not validate that `entries.len() <= max_size` or that `pointer <= entries.len()`. A hand-edited or corrupted YAML file could produce an out-of-bounds pointer or an over-capacity stack.\n\nFor example, if someone edits `undo_stack.yaml` and sets `pointer: 999` with only 2 entries, `undo_target()` would return `entries[998]` which would panic on index-out-of-bounds.\n\nSuggested fix: add validation after deserialization in `load()`:\n```rust\nlet mut stack: Self = serde_yaml_ng::from_str(&contents)?;\n// Clamp pointer to valid range\nstack.pointer = stack.pointer.min(stack.entries.len());\n// Trim if over capacity\nif stack.entries.len() > stack.max_size {\n    let excess = stack.entries.len() - stack.max_size;\n    stack.entries.drain(0..excess);\n    stack.pointer = stack.pointer.saturating_sub(excess);\n}\nOk(stack)\n```\n\nThis is low priority since the file is machine-managed and gitignored, but defensive validation is cheap. #review-finding