---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffcc80
title: 'Race condition: cross_board_transfer not atomic across two boards'
---
**kanban-app/src/commands.rs:1290-1300**\n\nIn `cross_board_transfer`, the write-to-target and delete-from-source are two separate async operations with a `flush_and_emit_for_handle` between them. If the process crashes or the source delete fails after the target write succeeds, the task is duplicated (exists on both boards) with no undo path.\n\nThe error from `source_ectx.delete()` is propagated, but the target task is already written and events already emitted — the frontend shows the task on the target board. The caller (`complete_drag_session`) reports success=false via the event, but the target write is not rolled back.\n\n**Suggestion:** Document this as a known limitation (eventual consistency), or implement a two-phase approach: write target, delete source, then flush both together. At minimum, the target flush should happen after both operations succeed, not between them.