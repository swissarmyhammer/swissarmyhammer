---
position_column: done
position_ordinal: ffffbc80
title: No integration test for undo/redo via dispatch_command
---
swissarmyhammer-kanban/tests/command_dispatch_integration.rs\n\nThe 13 integration tests cover add, move, untag, update_field, availability, UI state, and a full session. However, there is no test for the undo/redo path via `app.undo` / `app.redo` commands. These commands require a transaction ID arg, and there are no tests verifying:\n1. That the operation_id returned from a mutation can be passed to app.undo\n2. That app.undo actually reverses the mutation\n3. That app.redo re-applies it\n\nThis is the most complex data flow in the dispatch system and is currently untested at the integration level.\n\nSuggestion: Add an integration test that does task.add -> captures operation_id -> dispatches app.undo with that id -> verifies the task is gone -> dispatches app.redo -> verifies the task is back. #review-finding #warning