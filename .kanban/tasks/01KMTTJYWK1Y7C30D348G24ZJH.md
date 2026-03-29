---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffcb80
title: 'Review finding: UndoStack::save is not atomic despite comment claiming it should be'
---
**Severity**: Low\n**File**: `swissarmyhammer-entity/src/undo_stack.rs` lines 185-192\n\nThe comment on `save()` says:\n> Uses atomic write (write to temp location + rename would be ideal, but for simplicity we write directly -- the file is small and non-critical).\n\nThis is honest about the tradeoff, but the comment format is misleading -- it starts by saying \"Uses atomic write\" then immediately contradicts itself. The parenthetical disclaimer is easy to miss.\n\nSuggested fix: rewrite the doc comment to lead with the truth:\n```rust\n/// Save the UndoStack to a YAML file.\n///\n/// Creates parent directories if needed. Writes directly (not atomic);\n/// the file is small and non-critical, so a partial write during a crash\n/// just means the stack resets to empty on next load.\n```\n\nThis is documentation-only, no behavior change needed. #review-finding