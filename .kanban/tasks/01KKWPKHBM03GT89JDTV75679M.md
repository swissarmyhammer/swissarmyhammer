---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffff280
title: Duplicated flush-and-emit logic between dispatch_command and flush_and_emit_for_handle
---
**kanban-app/src/commands.rs:902-963 vs 1302-1348**\n\nThe entity enrichment + search index update + event emission block in `dispatch_command` (lines 902-963) is copy-pasted almost identically into `flush_and_emit_for_handle` (lines 1302-1348). The original `dispatch_command` should be refactored to call `flush_and_emit_for_handle` instead of inlining the same logic.\n\n**Suggestion:** Replace the inline block in `dispatch_command` (lines 902-963) with a call to `flush_and_emit_for_handle(app, handle).await`. This eliminates ~60 lines of duplication.