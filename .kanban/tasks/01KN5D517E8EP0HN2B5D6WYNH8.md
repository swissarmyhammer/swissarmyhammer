---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffff980
title: Test store stack.rs load with over-capacity and corrupted pointer (76.3%)
---
**File**: `swissarmyhammer-store/src/stack.rs` (76.3% -- 45/59 lines)\n\n**What**: Uncovered lines:\n- L54-55: `Default::default()` impl (calls `Self::new()`) -- trivial but uncovered\n- L122, L124-129: Inside `push()` -- the max_size trimming branch when `entries.len() > max_size` after truncation of redo side\n- L178-181: Inside `load()` -- the over-capacity trimming branch (stack loaded from YAML exceeds max_size)\n- L191, L194: Inside `save()` -- parent dir creation and YAML write\n\n**Acceptance criteria**: Coverage above 85% for stack.rs\n\n**Tests to add**:\n- Use `UndoStack::default()` to cover the Default impl\n- Test `load()` with a YAML file containing more entries than max_size (triggers L178-181 trimming)\n- Test `load()` with a corrupted pointer value > entries.len() (triggers L176 clamping)\n- The `save()` and parent dir creation are already tested in `save_creates_parent_directories` but may need tarpaulin re-check" #coverage-gap