# kanban

Kanban board operations for task management. This is the best way to keep a TODO list for a project.

## Overview

The kanban tool provides file-backed task board management. A `.kanban` directory in a repository root **is** the project - one board per repo.

## Operations

The tool accepts `op` as a "verb noun" string (e.g., "add task", "move task").

### Board Operations

- `init board` - Initialize a new board
  - Required: `name`
  - Optional: `description`

- `get board` - Get board metadata

- `update board` - Update board name or description
  - Optional: `name`, `description`

### Column Operations

- `add column` - Add a workflow stage
  - Required: `id`, `name`
  - Optional: `order`

- `get column` - Get column by ID
  - Required: `id`

- `update column` - Update column name or order
  - Required: `id`
  - Optional: `name`, `order`

- `delete column` - Delete a column (fails if has tasks)
  - Required: `id`

- `list columns` - List all columns

### Swimlane Operations

- `add swimlane` - Add horizontal grouping
  - Required: `id`, `name`
  - Optional: `order`

- `get swimlane` - Get swimlane by ID
  - Required: `id`

- `update swimlane` - Update swimlane name or order
  - Required: `id`
  - Optional: `name`, `order`

- `delete swimlane` - Delete a swimlane
  - Required: `id`

- `list swimlanes` - List all swimlanes

### Actor Operations

- `add actor` - Register a person or agent
  - Required: `id`, `name`, `type` (human|agent)
  - Optional: `ensure` (boolean, default false)
  - When `ensure: true`, returns existing actor instead of error if ID exists

- `get actor` - Get actor by ID
  - Required: `id`

- `update actor` - Update actor name
  - Required: `id`
  - Optional: `name`

- `delete actor` - Delete actor and remove from all task assignments
  - Required: `id`

- `list actors` - List all actors
  - Optional: `type` (filter by human|agent)

### Task Operations

- `add task` - Create a new task
  - Required: `title`
  - Optional: `description`, `position`, `assignees`, `tags`, `depends_on`

- `get task` - Get task by ID
  - Required: `id`

- `update task` - Update task properties
  - Required: `id`
  - Optional: `title`, `description`, `assignees`, `tags`, `depends_on`, `subtasks`, `attachments`

- `move task` - Move task to a different column
  - Required: `id`, `column`
  - Optional: `swimlane`, `ordinal`

- `delete task` - Delete a task (removes from dependencies)
  - Required: `id`

- `complete task` - Move task to the done column
  - Required: `id`

- `assign task` - Assign an actor to a task
  - Required: `id`, `assignee`

- `unassign task` - Remove an actor from a task
  - Required: `id`, `assignee`

- `tag task` - Add a tag to a task
  - Required: `id` (task), `tag` (tag ID)

- `untag task` - Remove a tag from a task
  - Required: `id` (task), `tag` (tag ID)

- `next task` - Get next actionable task (no incomplete dependencies)

- `list tasks` - List tasks with optional filters
  - Optional: `column`, `swimlane`, `tag`, `assignee`, `ready`

### Tag Operations

- `add tag` - Create a tag for categorizing tasks
  - Required: `id`, `name`, `color` (6-char hex without #)
  - Optional: `description`

- `get tag` - Get tag by ID
  - Required: `id`

- `update tag` - Update tag properties
  - Required: `id`
  - Optional: `name`, `description`, `color`

- `delete tag` - Delete a tag (removes from all tasks)
  - Required: `id`

- `list tags` - List all tags

### Comment Operations

- `add comment` - Add a comment to a task
  - Required: `task_id`, `body`, `author`

- `get comment` - Get a specific comment
  - Required: `task_id`, `id`

- `update comment` - Update comment body
  - Required: `task_id`, `id`
  - Optional: `body`

- `delete comment` - Delete a comment
  - Required: `task_id`, `id`

- `list comments` - List all comments on a task
  - Required: `task_id`

### Subtask Operations

- `add subtask` - Add a checklist item to a task
  - Required: `task_id`, `title`

- `update subtask` - Update subtask properties
  - Required: `task_id`, `id`
  - Optional: `title`, `completed`

- `complete subtask` - Mark a subtask as complete
  - Required: `task_id`, `id`

- `delete subtask` - Delete a subtask
  - Required: `task_id`, `id`

### Attachment Operations

- `add attachment` - Add a file reference to a task
  - Required: `task_id`, `name`, `path`
  - Optional: `mime_type`, `size` (auto-detected if not provided)

- `get attachment` - Get attachment by ID
  - Required: `task_id`, `id`

- `update attachment` - Update attachment properties
  - Required: `task_id`, `id`
  - Optional: `name`, `path`

- `delete attachment` - Delete an attachment
  - Required: `task_id`, `id`

- `list attachments` - List all attachments on a task
  - Required: `task_id`

### Activity Operations

- `list activity` - List recent operations (most recent first)
  - Optional: `limit` (number of entries)

## Examples

### Initialize a board

```json
{
  "op": "init board",
  "name": "My Project",
  "description": "Sprint planning board"
}
```

### Add workflow columns

```json
{
  "op": "add column",
  "id": "review",
  "name": "In Review",
  "order": 2
}
```

### Add swimlanes for organization

```json
{
  "op": "add swimlane",
  "id": "feature",
  "name": "Feature Work"
}
```

### Register actors

```json
{
  "op": "add actor",
  "id": "alice",
  "name": "Alice Smith",
  "type": "human"
}
```

Agent self-registration (idempotent):
```json
{
  "op": "add actor",
  "id": "assistant",
  "name": "AI Assistant",
  "type": "agent",
  "ensure": true
}
```

### Create and manage tasks

Add a task:
```json
{
  "op": "add task",
  "title": "Implement feature X",
  "description": "Details about the feature",
  "assignees": ["alice"],
  "tags": ["feature"]
}
```

Assign a task:
```json
{
  "op": "assign task",
  "id": "01ABC123...",
  "assignee": "alice"
}
```

Move a task:
```json
{
  "op": "move task",
  "id": "01ABC123...",
  "column": "doing",
  "swimlane": "feature"
}
```

Complete a task:
```json
{
  "op": "complete task",
  "id": "01ABC123..."
}
```

### Add tags for categorization

```json
{
  "op": "add tag",
  "id": "bug",
  "name": "Bug",
  "color": "ff0000",
  "description": "Bug fixes"
}
```

Tag a task:
```json
{
  "op": "tag task",
  "id": "01ABC123...",
  "tag": "bug"
}
```

### Add subtasks for checklists

```json
{
  "op": "add subtask",
  "task_id": "01ABC123...",
  "title": "Write tests"
}
```

Complete a subtask:
```json
{
  "op": "complete subtask",
  "task_id": "01ABC123...",
  "id": "01DEF456..."
}
```

### Add comments

```json
{
  "op": "add comment",
  "task_id": "01ABC123...",
  "body": "This needs review",
  "author": "alice"
}
```

### Add attachments

```json
{
  "op": "add attachment",
  "task_id": "01ABC123...",
  "name": "screenshot.png",
  "path": "./docs/screenshot.png"
}
```

### Query tasks

List ready tasks:
```json
{
  "op": "list tasks",
  "ready": true
}
```

List tasks by assignee:
```json
{
  "op": "list tasks",
  "assignee": "alice"
}
```

Get next actionable task:
```json
{
  "op": "next task"
}
```

### Using dependencies

Create a blocker task:
```json
{
  "op": "add task",
  "title": "Design API schema"
}
```

Create a task that depends on the blocker (use the task ID from above):
```json
{
  "op": "add task",
  "title": "Implement API endpoints",
  "depends_on": ["01ABC123..."]
}
```

List only ready tasks (no incomplete dependencies):
```json
{
  "op": "list tasks",
  "ready": true
}
```

The blocked task won't appear in ready tasks until the blocker is completed:
```json
{
  "op": "complete task",
  "id": "01ABC123..."
}
```

### View activity log

```json
{
  "op": "list activity",
  "limit": 10
}
```

## Complete Workflow Example

```json
// 1. Initialize board
{"op": "init board", "name": "Sprint 1"}

// 2. Register yourself
{"op": "add actor", "id": "assistant", "name": "AI Assistant", "type": "agent", "ensure": true}

// 3. Add a tag
{"op": "add tag", "id": "feature", "name": "Feature", "color": "00ff00"}

// 4. Create a task
{"op": "add task", "title": "Implement login", "tags": ["feature"]}

// 5. Assign to yourself
{"op": "assign task", "id": "<task-id>", "assignee": "assistant"}

// 6. Add a subtask
{"op": "add subtask", "task_id": "<task-id>", "title": "Write tests"}

// 7. Move to doing
{"op": "move task", "id": "<task-id>", "column": "doing"}

// 8. Complete subtask
{"op": "complete subtask", "task_id": "<task-id>", "id": "<subtask-id>"}

// 9. Add a comment
{"op": "add comment", "task_id": "<task-id>", "body": "Implementation complete", "author": "assistant"}

// 10. Complete the task
{"op": "complete task", "id": "<task-id>"}
```

## Forgiving Input

The tool accepts multiple input formats:

```json
// Explicit op
{ "op": "add task", "title": "Fix bug" }

// Shorthand
{ "add": "task", "title": "Fix bug" }

// Inferred (has title but no id)
{ "title": "Fix bug" }
```

### Parameter Aliases

Many parameters accept aliases for convenience:
- `id` can be `task_id`, `column_id`, `tag_id`, etc.
- `assignees` can be `assignee` (single value)
- `tags` can be `tag` (single value)

### Operation Inference

If `op` is not provided, the tool attempts to infer the operation from:
1. Presence of specific fields (e.g., `title` without `id` → "add task")
2. Shorthand verb/noun fields (e.g., `{"add": "task"}`)

### Batch Operations

Multiple operations can be batched by providing an array of operation objects.
