# Subtask Operations

## Status: Not Implemented

## Problem

Tasks have `subtasks: Vec<Subtask>` field for checklist items, but there are no operations to manage them. Currently the only way to modify subtasks is through `update task` which replaces the entire subtask array - no way to add/complete/delete individual items.

## Current State

```rust
pub struct Task {
    // ...
    pub subtasks: Vec<Subtask>,
}

pub struct Subtask {
    pub id: SubtaskId,
    pub title: String,
    pub completed: bool,
}
```

Tasks can be created with subtasks, but cannot be modified granularly.

## Required Operations

### 1. Add Subtask

Add a checklist item to an existing task.

```rust
#[operation(verb = "add", noun = "subtask")]
pub struct AddSubtask {
    pub task_id: TaskId,
    pub title: String,
}
```

**Usage:**
```json
{
  "op": "add subtask",
  "task_id": "01ABC...",
  "title": "Write unit tests"
}
```

**Returns:**
```json
{
  "subtask": {
    "id": "01DEF...",
    "title": "Write unit tests",
    "completed": false
  },
  "task_id": "01ABC..."
}
```

### 2. Update Subtask

Update subtask title or completion status.

```rust
#[operation(verb = "update", noun = "subtask")]
pub struct UpdateSubtask {
    pub task_id: TaskId,
    pub id: SubtaskId,
    pub title: Option<String>,
    pub completed: Option<bool>,
}
```

**Usage:**
```json
{
  "op": "update subtask",
  "task_id": "01ABC...",
  "id": "01DEF...",
  "title": "Write comprehensive unit tests"
}
```

### 3. Complete Subtask

Convenience operation to mark a subtask complete (instead of update).

```rust
#[operation(verb = "complete", noun = "subtask")]
pub struct CompleteSubtask {
    pub task_id: TaskId,
    pub id: SubtaskId,
}
```

**Usage:**
```json
{
  "op": "complete subtask",
  "task_id": "01ABC...",
  "id": "01DEF..."
}
```

**Returns:**
```json
{
  "completed": true,
  "subtask_id": "01DEF...",
  "task_id": "01ABC...",
  "task_progress": 0.5
}
```

### 4. Delete Subtask

Remove a subtask from a task.

```rust
#[operation(verb = "delete", noun = "subtask")]
pub struct DeleteSubtask {
    pub task_id: TaskId,
    pub id: SubtaskId,
}
```

**Usage:**
```json
{
  "op": "delete subtask",
  "task_id": "01ABC...",
  "id": "01DEF..."
}
```

## Verb+Noun Matrix Update

Add new valid operations:
- `(Verb::Add, Noun::Subtask)`
- `(Verb::Update, Noun::Subtask)`
- `(Verb::Complete, Noun::Subtask)`
- `(Verb::Delete, Noun::Subtask)`

Add `Subtask` to the `Noun` enum.

## Total Operations

Current: 40 operations
After subtasks: 44 operations

## File Structure

Create in `swissarmyhammer-kanban/src/subtask/`:
- `mod.rs` - Module declaration
- `add.rs` - AddSubtask command
- `update.rs` - UpdateSubtask command
- `complete.rs` - CompleteSubtask command
- `delete.rs` - DeleteSubtask command

## Testing Requirements

- Test adding subtasks to a task
- Test completing subtasks updates task progress
- Test deleting subtasks
- Test updating subtask title
- Test error cases (nonexistent task, nonexistent subtask)
- Test that subtask operations trigger task-level plan notifications

## Task Progress Calculation

The `Task::progress()` method already exists and calculates progress based on completed subtasks:

```rust
pub fn progress(&self) -> f64 {
    if self.subtasks.is_empty() {
        return 0.0;
    }
    let completed = self.subtasks.iter().filter(|s| s.completed).count();
    completed as f64 / self.subtasks.len() as f64
}
```

Subtask operations should include updated progress in responses.

## MCP Integration

All subtask operations should trigger plan notifications since they modify task state (they change task progress).

## Implementation Notes

- Subtasks are stored inline in the task JSON (not separate files)
- All subtask operations require loading task, modifying subtasks array, and writing task back
- Consider idempotent complete (like CompleteTask)
- Should respect the same patterns as other operations (operation macro, Execute trait, tests)
