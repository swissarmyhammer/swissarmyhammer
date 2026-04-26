---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffff80
title: 'clippy: unused import `task_tags` in swissarmyhammer-kanban/src/task/update.rs'
---
cargo clippy --workspace -- -D warnings fails with:\n\nerror: unused import: `task_tags`\n --> swissarmyhammer-kanban/src/task/update.rs:7:48\n  |\n7 | use crate::task_helpers::{task_entity_to_json, task_tags};\n\nFile: /Users/wballard/github/swissarmyhammer-kanban/swissarmyhammer-kanban/src/task/update.rs line 7 #test-failure