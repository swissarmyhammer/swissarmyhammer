---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
title: 'Dead code: PerspectiveError::DuplicateName variant and helper are unreachable'
---
**Severity**: Low (code quality)\n**Layer**: Correctness\n**Files**: `swissarmyhammer-perspectives/src/error.rs`, `swissarmyhammer-kanban/src/error.rs`\n\nWith the removal of name uniqueness enforcement, the `PerspectiveError::DuplicateName` variant is now dead code. Nothing in the crate constructs it. The `duplicate_name()` helper on line 46 is also unused.\n\nThe `KanbanError::From<PerspectiveError>` impl still maps `DuplicateName` to `KanbanError::DuplicateName` (error.rs:206), and `KanbanError` still has its own `DuplicateName` variant (line 60). The kanban variant *may* still be used elsewhere, but the perspective-side variant is dead.\n\n**Fix**: Remove `PerspectiveError::DuplicateName` and its `duplicate_name()` helper. Remove the corresponding arm from `From<PerspectiveError> for KanbanError`." #review-finding