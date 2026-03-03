---
title: Remove Subtask types and add markdown checklist progress parser
position:
  column: done
  ordinal: b0
---
Remove the Subtask data model and all subtask operations from the Rust kanban kernel. Replace the structured progress calculation with a markdown checklist parser.

**Files to modify:**
- `swissarmyhammer-kanban/src/types/task.rs` — Remove `Subtask` struct, remove `subtasks: Vec<Subtask>` field from `Task` (keep `#[serde(default, skip_serializing)]` for compat), remove `find_subtask`/`find_subtask_mut` methods, rewrite `progress()` to parse `- [ ]` / `- [x]` from description
- `swissarmyhammer-kanban/src/types/ids.rs` — Remove `SubtaskId` type
- `swissarmyhammer-kanban/src/types/operation.rs` — Remove `Noun::Subtask` variant and valid operation entries
- `swissarmyhammer-kanban/src/subtask/` — Delete the entire directory (add.rs, complete.rs, update.rs, delete.rs, mod.rs)
- `swissarmyhammer-kanban/src/lib.rs` — Remove `mod subtask` declaration
- `swissarmyhammer-kanban/src/task/update.rs` — Remove `subtasks` from UpdateTask params
- `swissarmyhammer-kanban/src/schema.rs` — Remove subtask-related MCP schema examples

## Checklist
- [ ] Remove Subtask struct and SubtaskId
- [ ] Remove subtasks field from Task struct (keep serde compat with legacy field)
- [ ] Remove find_subtask methods
- [ ] Rewrite progress() to parse markdown checklists from description
- [ ] Delete src/subtask/ directory
- [ ] Remove Noun::Subtask from operation.rs
- [ ] Remove mod subtask from lib.rs
- [ ] Update UpdateTask to remove subtasks param
- [ ] Update schema examples
- [ ] Run tests