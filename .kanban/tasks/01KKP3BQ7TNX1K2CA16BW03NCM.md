---
position_column: done
position_ordinal: '9780'
title: No tests for dispatch::execute_operation
---
swissarmyhammer-kanban/src/dispatch.rs\n\nThe dispatch module has no tests. While the MCP tool has integration tests that exercise the same code paths, the dispatch module is now a public API of swissarmyhammer-kanban and should have its own unit tests to prevent regressions.\n\nSuggestion: Add at least smoke tests for init board, add task, and list tasks via dispatch::execute_operation."