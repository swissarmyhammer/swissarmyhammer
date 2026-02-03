# Unassign Task Operation

## Status: Not Implemented

## Problem

We have `assign task` to add an assignee, but no way to remove an assignee. This creates an asymmetry - you can only add assignees, not remove them (except by using `update task` to replace the entire assignees array).

## Current State

- `assign task` - Adds an assignee to a task (idempotent, won't duplicate)
- No unassign operation

The only way to remove an assignee is:
```json
{
  "op": "update task",
  "id": "01ABC...",
  "assignees": ["remaining_assignee_1", "remaining_assignee_2"]
}
```

This is cumbersome and error-prone (need to know all other assignees).

## Required Operation

### Unassign Task

Remove an actor from a task's assignee list.

```rust
#[operation(verb = "unassign", noun = "task")]
pub struct UnassignTask {
    pub id: TaskId,
    pub assignee: ActorId,
}
```

**Implementation:**

```rust
#[async_trait]
impl Execute<KanbanContext, KanbanError> for UnassignTask {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut task = ctx.read_task(&self.id).await?;

        // Remove assignee (idempotent - no error if not assigned)
        let was_assigned = task.assignees.contains(&self.assignee);
        task.assignees.retain(|a| a != &self.assignee);

        ctx.write_task(&task).await?;

        // Return confirmation
        Ok(serde_json::json!({
            "unassigned": was_assigned,
            "task_id": self.id,
            "assignee": self.assignee,
            "all_assignees": task.assignees,
        }))
    }
}
```

**Usage:**
```json
{
  "op": "unassign task",
  "id": "01ABC...",
  "assignee": "assistant"
}
```

**Returns:**
```json
{
  "unassigned": true,
  "task_id": "01ABC...",
  "assignee": "assistant",
  "all_assignees": ["alice"]
}
```

If the assignee wasn't assigned, still succeeds but returns `"unassigned": false` (idempotent).

## Verb Update

Add `Unassign` to the `Verb` enum:

```rust
pub enum Verb {
    // ...existing verbs...
    Assign,
    Unassign,
}
```

Add alias parsing:
```rust
"assign" => Some(Self::Assign),
"unassign" | "remove_assignee" => Some(Self::Unassign),
```

## Valid Operations Update

Add to `is_valid_operation`:
```rust
(Verb::Assign, Noun::Task) |
(Verb::Unassign, Noun::Task) |
```

## Total Operations

Current: 40 operations
After unassign: 41 operations

## File Structure

Create `swissarmyhammer-kanban/src/task/unassign.rs`

Export from `task/mod.rs`:
```rust
pub use unassign::UnassignTask;
```

## Testing Requirements

- Test unassigning an assigned actor
- Test unassigning when not assigned (idempotent)
- Test unassigning from task with multiple assignees
- Test unassigning nonexistent actor (should succeed with unassigned: false)
- Test unassigning from nonexistent task (should error)

## MCP Integration

Add to MCP tool dispatch:
```rust
(Verb::Unassign, Noun::Task) => {
    let id = op.get_string("id")?;
    let assignee = op.get_string("assignee")?;
    UnassignTask::new(id, assignee).execute(ctx).await
}
```

Add to KANBAN_OPERATIONS static:
```rust
static UNASSIGN_TASK: Lazy<UnassignTask> = Lazy::new(|| UnassignTask::new("", ""));
// ...
&*UNASSIGN_TASK as &dyn Operation,
```

Add to `is_task_modifying_operation` (for plan notifications):
```rust
| (Verb::Assign, Noun::Task)
| (Verb::Unassign, Noun::Task)
```

## Design Considerations

### Should it error if not assigned?

**Recommendation**: No, make it idempotent like `assign task`.

Benefits:
- Simpler error handling
- Works well with batch operations
- Aligns with assign behavior
- Can call unassign without checking first

### Should it validate the actor exists?

**Recommendation**: No validation needed.

Rationale:
- You can unassign a deleted actor ID
- Useful for cleanup scenarios
- Simpler implementation
- assign validates, unassign doesn't need to

## Use Cases

- Remove yourself from a task when done
- Reassign work (unassign self, assign someone else)
- Remove deleted actors from tasks (though DeleteActor does this automatically)
- Clean up incorrect assignments
