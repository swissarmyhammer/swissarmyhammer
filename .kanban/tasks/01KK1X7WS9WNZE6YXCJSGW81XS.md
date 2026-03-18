---
position_column: done
position_ordinal: ffffaa80
title: Extract shared helper for KanbanOperationProcessor boilerplate
---
**Files:** `swissarmyhammer-kanban/src/commands/task_commands.rs`, `entity_commands.rs`, `column_commands.rs`\n\n**What:** Every command `execute()` repeats the same 3-line pattern: create `KanbanOperationProcessor::new()`, call `process(&op, &kanban).await`, `map_err(|e| CommandError::ExecutionFailed(e.to_string()))`. This appears 9+ times.\n\n**Why:** DRY violation. If the processor creation pattern changes (e.g., adding a cache argument), all 9 sites must be updated.\n\n**Fix:** Extract a helper function:\n```rust\nasync fn run_op<O: Operation>(op: &O, kanban: &KanbanContext) -> Result<Value> {\n    KanbanOperationProcessor::new()\n        .process(op, kanban)\n        .await\n        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))\n}\n```\n\n- [ ] Add `run_op` helper to `commands/mod.rs`\n- [ ] Replace all 9 repetitions\n- [ ] Verify tests pass"