---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffa880
title: Test kanban dispatch (execute_operation) — 27.8% coverage
---
File: swissarmyhammer-kanban/src/dispatch.rs (35/126 lines covered, 27.8%)\n\nThe central dispatch function `execute_operation` maps (Verb, Noun) pairs to operation structs. Most verb/noun match arms are untested. Need integration tests that exercise each operation variant through the dispatch layer.\n\nUncovered: most match arms in execute_operation for swimlane, column, activity, and attachment operations.\n\n#coverage-gap #coverage-gap