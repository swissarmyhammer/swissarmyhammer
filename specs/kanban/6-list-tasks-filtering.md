# Enhanced List Tasks Filtering

## Status: Partially Implemented

## Problem

`list tasks` currently supports limited filtering:
- `column` - Filter by column ID
- `ready` - Filter by dependency readiness

Missing useful filters that would help agents and users find relevant tasks:
- By assignee
- By tag
- By swimlane
- By multiple criteria combined

## Current State

```rust
#[derive(Debug, Default, Deserialize)]
pub struct ListTasks {
    pub column: Option<ColumnId>,
    pub ready: Option<bool>,
}
```

MCP dispatch:
```rust
(Verb::List, Noun::Tasks) => {
    let mut cmd = ListTasks::new();
    if let Some(column) = op.get_string("column") {
        cmd = cmd.with_column(column);
    }
    if let Some(ready) = op.get_param("ready").and_then(|v| v.as_bool()) {
        cmd = cmd.with_ready(ready);
    }
    cmd.execute(ctx).await
}
```

## Required Enhancements

### 1. Filter by Assignee

Get all tasks assigned to a specific actor.

```rust
pub struct ListTasks {
    pub column: Option<ColumnId>,
    pub ready: Option<bool>,
    pub assignee: Option<ActorId>,  // NEW
}
```

**Usage:**
```json
{
  "op": "list tasks",
  "assignee": "assistant"
}
```

**Returns tasks where** `task.assignees.contains(&assignee)`

### 2. Filter by Tag

Get all tasks with a specific tag.

```rust
pub struct ListTasks {
    pub column: Option<ColumnId>,
    pub ready: Option<bool>,
    pub assignee: Option<ActorId>,
    pub tag: Option<TagId>,  // NEW
}
```

**Usage:**
```json
{
  "op": "list tasks",
  "tag": "bug"
}
```

**Returns tasks where** `task.tags.contains(&tag)`

### 3. Filter by Swimlane

Get all tasks in a specific swimlane.

```rust
pub struct ListTasks {
    pub column: Option<ColumnId>,
    pub ready: Option<bool>,
    pub assignee: Option<ActorId>,
    pub tag: Option<TagId>,
    pub swimlane: Option<SwimlaneId>,  // NEW
}
```

**Usage:**
```json
{
  "op": "list tasks",
  "swimlane": "backend"
}
```

**Returns tasks where** `task.position.swimlane == Some(swimlane)`

### 4. Exclude Completed

Useful to see only active work.

```rust
pub struct ListTasks {
    // ...
    pub exclude_done: Option<bool>,  // NEW
}
```

**Usage:**
```json
{
  "op": "list tasks",
  "exclude_done": true
}
```

**Returns tasks where** `task.position.column != terminal_column_id`

## Combined Filtering

All filters should work together (AND logic):

```json
{
  "op": "list tasks",
  "column": "doing",
  "assignee": "assistant",
  "tag": "bug",
  "swimlane": "backend"
}
```

Returns tasks that match **all** criteria.

## Implementation

```rust
impl Execute<KanbanContext, KanbanError> for ListTasks {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let board = ctx.read_board().await?;
        let all_tasks = ctx.read_all_tasks().await?;

        // Apply filters
        let filtered: Vec<&Task> = all_tasks
            .iter()
            .filter(|task| {
                // Column filter
                if let Some(ref col) = self.column {
                    if &task.position.column != col {
                        return false;
                    }
                }

                // Assignee filter
                if let Some(ref assignee) = self.assignee {
                    if !task.assignees.contains(assignee) {
                        return false;
                    }
                }

                // Tag filter
                if let Some(ref tag) = self.tag {
                    if !task.tags.contains(tag) {
                        return false;
                    }
                }

                // Swimlane filter
                if let Some(ref swimlane) = self.swimlane {
                    if task.position.swimlane.as_ref() != Some(swimlane) {
                        return false;
                    }
                }

                // Ready filter
                if let Some(ready) = self.ready {
                    let terminal = board.terminal_column().unwrap();
                    let is_ready = task.is_ready(&all_tasks, terminal.id.as_str());
                    if ready != is_ready {
                        return false;
                    }
                }

                // Exclude done filter
                if let Some(true) = self.exclude_done {
                    let terminal = board.terminal_column().unwrap();
                    if task.position.column == terminal.id {
                        return false;
                    }
                }

                true
            })
            .collect();

        Ok(serde_json::json!({
            "tasks": filtered,
            "count": filtered.len()
        }))
    }
}
```

## Builder Methods

Add fluent builder methods:

```rust
impl ListTasks {
    pub fn with_assignee(mut self, assignee: impl Into<ActorId>) -> Self {
        self.assignee = Some(assignee.into());
        self
    }

    pub fn with_tag(mut self, tag: impl Into<TagId>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn with_swimlane(mut self, swimlane: impl Into<SwimlaneId>) -> Self {
        self.swimlane = Some(swimlane.into());
        self
    }

    pub fn exclude_done(mut self) -> Self {
        self.exclude_done = Some(true);
        self
    }
}
```

## MCP Dispatch Updates

Update the MCP tool to parse new parameters:

```rust
(Verb::List, Noun::Tasks) => {
    let mut cmd = ListTasks::new();
    if let Some(column) = op.get_string("column") {
        cmd = cmd.with_column(column);
    }
    if let Some(ready) = op.get_param("ready").and_then(|v| v.as_bool()) {
        cmd = cmd.with_ready(ready);
    }
    if let Some(assignee) = op.get_string("assignee") {
        cmd = cmd.with_assignee(assignee);
    }
    if let Some(tag) = op.get_string("tag") {
        cmd = cmd.with_tag(tag);
    }
    if let Some(swimlane) = op.get_string("swimlane") {
        cmd = cmd.with_swimlane(swimlane);
    }
    if let Some(exclude_done) = op.get_param("exclude_done").and_then(|v| v.as_bool()) {
        if exclude_done {
            cmd = cmd.exclude_done();
        }
    }
    cmd.execute(ctx).await
}
```

## Testing Requirements

- Test each filter individually
- Test combined filters (assignee + tag, column + ready + assignee, etc.)
- Test exclude_done filter
- Test empty results when no matches
- Test filter with nonexistent IDs (should return empty, not error)

## Use Cases

**For agents:**
```json
// My assigned work
{ "op": "list tasks", "assignee": "assistant", "exclude_done": true }

// Bugs in progress
{ "op": "list tasks", "column": "doing", "tag": "bug" }

// Backend work that's ready to start
{ "op": "list tasks", "swimlane": "backend", "ready": true, "column": "todo" }
```

**For users:**
```json
// What's Alice working on?
{ "op": "list tasks", "assignee": "alice", "column": "doing" }

// All P0 bugs
{ "op": "list tasks", "tag": "p0", "tag": "bug" }
```

## Priority

**High** - Assignee and tag filtering are essential for multi-agent scenarios and task organization.

## File Changes

1. Update `swissarmyhammer-kanban/src/task/list.rs`:
   - Add new filter fields
   - Add builder methods
   - Update execute logic

2. Update MCP tool dispatch to parse new parameters

3. Add tests for each filter combination
