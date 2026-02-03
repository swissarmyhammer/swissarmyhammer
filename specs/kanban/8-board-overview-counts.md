# Board Overview with Task Counts

## Status: Not Implemented

## Problem

`get board` currently returns just the board metadata (name, description, columns, swimlanes, tags). It doesn't provide an overview of the current state:
- How many tasks are in each column?
- How many tasks are ready vs blocked?
- Total task count
- Number of actors

This means agents/users need to call `list tasks` for each column separately to get a quick overview.

## Current State

`get board` returns:
```json
{
  "name": "My Project",
  "description": "Project board",
  "columns": [
    {"id": "todo", "name": "To Do", "order": 0},
    {"id": "doing", "name": "Doing", "order": 1},
    {"id": "done", "name": "Done", "order": 2}
  ],
  "swimlanes": [],
  "tags": []
}
```

No task counts, no actors listed.

## Required Enhancement

### Enhanced Board Response

Add summary statistics to `get board`:

```json
{
  "name": "My Project",
  "description": "Project board",
  "columns": [
    {
      "id": "todo",
      "name": "To Do",
      "order": 0,
      "task_count": 12,
      "ready_count": 8
    },
    {
      "id": "doing",
      "name": "Doing",
      "order": 1,
      "task_count": 5,
      "ready_count": 5
    },
    {
      "id": "done",
      "name": "Done",
      "order": 2,
      "task_count": 47,
      "ready_count": 47
    }
  ],
  "swimlanes": [
    {
      "id": "backend",
      "name": "Backend",
      "order": 0,
      "task_count": 15
    }
  ],
  "tags": [
    {
      "id": "bug",
      "name": "Bug",
      "description": "Something isn't working",
      "color": "d73a4a",
      "task_count": 8
    }
  ],
  "summary": {
    "total_tasks": 64,
    "total_actors": 3,
    "ready_tasks": 60,
    "blocked_tasks": 4
  }
}
```

## Implementation

Update `GetBoard` command:

```rust
#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetBoard {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let board = ctx.read_board().await?;
        let all_tasks = ctx.read_all_tasks().await?;
        let terminal = board.terminal_column().unwrap();

        // Count tasks by column
        let mut column_counts: HashMap<&ColumnId, usize> = HashMap::new();
        let mut column_ready_counts: HashMap<&ColumnId, usize> = HashMap::new();

        for task in &all_tasks {
            *column_counts.entry(&task.position.column).or_insert(0) += 1;

            if task.is_ready(&all_tasks, terminal.id.as_str()) {
                *column_ready_counts.entry(&task.position.column).or_insert(0) += 1;
            }
        }

        // Count tasks by swimlane
        let mut swimlane_counts: HashMap<&SwimlaneId, usize> = HashMap::new();
        for task in &all_tasks {
            if let Some(ref swimlane) = task.position.swimlane {
                *swimlane_counts.entry(swimlane).or_insert(0) += 1;
            }
        }

        // Count tasks by tag
        let mut tag_counts: HashMap<&TagId, usize> = HashMap::new();
        for task in &all_tasks {
            for tag in &task.tags {
                *tag_counts.entry(tag).or_insert(0) += 1;
            }
        }

        // Enhance columns with counts
        let columns: Vec<Value> = board.columns.iter().map(|col| {
            let count = column_counts.get(&col.id).copied().unwrap_or(0);
            let ready = column_ready_counts.get(&col.id).copied().unwrap_or(0);

            json!({
                "id": col.id,
                "name": col.name,
                "order": col.order,
                "task_count": count,
                "ready_count": ready
            })
        }).collect();

        // Enhance swimlanes with counts
        let swimlanes: Vec<Value> = board.swimlanes.iter().map(|sl| {
            let count = swimlane_counts.get(&sl.id).copied().unwrap_or(0);

            json!({
                "id": sl.id,
                "name": sl.name,
                "order": sl.order,
                "task_count": count
            })
        }).collect();

        // Enhance tags with counts
        let tags: Vec<Value> = board.tags.iter().map(|tag| {
            let count = tag_counts.get(&tag.id).copied().unwrap_or(0);

            json!({
                "id": tag.id,
                "name": tag.name,
                "description": tag.description,
                "color": tag.color,
                "task_count": count
            })
        }).collect();

        // Calculate summary
        let total_tasks = all_tasks.len();
        let ready_tasks = all_tasks.iter()
            .filter(|t| t.is_ready(&all_tasks, terminal.id.as_str()))
            .count();
        let blocked_tasks = total_tasks - ready_tasks;
        let total_actors = ctx.list_actor_ids().await?.len();

        Ok(json!({
            "name": board.name,
            "description": board.description,
            "columns": columns,
            "swimlanes": swimlanes,
            "tags": tags,
            "summary": {
                "total_tasks": total_tasks,
                "total_actors": total_actors,
                "ready_tasks": ready_tasks,
                "blocked_tasks": blocked_tasks
            }
        }))
    }
}
```

## Optional Parameter

Add `include_counts` flag to make this opt-in:

```rust
pub struct GetBoard {
    pub include_counts: Option<bool>,
}
```

**Usage:**
```json
{
  "op": "get board",
  "include_counts": true
}
```

**Default**: `true` (always include counts since it's useful)

## Testing Requirements

- Test board overview with tasks in different columns
- Test ready vs blocked counts
- Test swimlane counts
- Test tag counts
- Test summary calculations
- Test empty board (all counts zero)

## Performance Consideration

This requires reading all tasks, which could be slow for boards with thousands of tasks. Consider:
1. Caching task counts
2. Making counts optional (default true)
3. Building an index file

For MVP, reading all tasks is acceptable (boards typically have <1000 tasks).

## Use Cases

**Dashboard view:**
```json
{
  "op": "get board"
}

// Returns:
// "To Do: 12 tasks (8 ready, 4 blocked)"
// "Doing: 5 tasks"
// "Done: 47 tasks"
```

**Agent planning:**
Agent can see at a glance:
- How much work is left
- How much is blocked
- Team size (actor count)

## File Changes

Update `swissarmyhammer-kanban/src/board/get.rs` to calculate and include counts.
