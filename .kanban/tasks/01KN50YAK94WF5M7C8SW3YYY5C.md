---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffb880
title: '[nit] Noun::Attachment removed from operation.rs but dispatch.rs still references attachment CRUD in parent branch diff'
---
**File**: `swissarmyhammer-kanban/src/dispatch.rs` (diff context)\n\n**What**: The `kanban-perspective` parent branch still has `Noun::Attachment` and `Noun::Attachments` in `operation.rs` and attachment CRUD dispatch arms in `dispatch.rs`. These are removed on the `kanban` branch. This is correct and expected as part of the refactor -- attachments are now fields, not nouns.\n\n**Why**: This is informational, not a problem. The removal is clean. Verified that `operation.rs` on this branch has no trace of `Attachment` or `Attachments` in the `Noun` enum, and `dispatch.rs` has no attachment-specific dispatch arms." #review-finding