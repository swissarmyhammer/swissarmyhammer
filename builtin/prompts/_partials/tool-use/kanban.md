---
title: Kanban Board Management
description: How to use the kanban tool for task management
partial: true
---

## Kanban Board Management

The kanban tool provides a powerful task management system. You MUST use it to track your work and provide visibility to the user.

### Core Principles

1. **Always track your work** - Every significant task should be on the board
2. **Break down complex work** - Split large tasks into smaller, actionable items
3. **Mark complete immediately** - Don't batch completions; mark tasks done as you finish them
4. **Be an active participant** - Register yourself as an actor and assign tasks to yourself

### Getting Started - Agent Self-Registration

Before working on tasks, you MUST register yourself as an actor. Use the `ensure: true` parameter for idempotent registration (safe to call multiple times):

```
kanban op: "add actor", id: "assistant", name: "Assistant", type: "agent", ensure: true
```

The `ensure` flag makes this operation safe to repeat:
- **First call**: Creates the actor, returns `{"actor": {...}, "created": true}`
- **Subsequent calls**: Returns existing actor, `{"actor": {...}, "created": false}`

This is the recommended way for agents to establish themselves since it handles both first-time setup and reconnection scenarios gracefully.

### Essential Operations

| Operation | Description | Example |
|-----------|-------------|---------|
| `add actor` | Register yourself (use ensure: true) | `op: "add actor", id: "assistant", name: "Assistant", type: "agent", ensure: true` |
| `add task` | Create a new task | `op: "add task", title: "Fix login bug"` |
| `list tasks` | View all tasks | `op: "list tasks"` or `op: "list tasks", column: "todo"` |
| `next task` | Get next actionable task | `op: "next task"` |
| `complete task` | Move task to done | `op: "complete task", id: "<task_id>"` |
| `assign task` | Assign task to an actor | `op: "assign task", id: "<task_id>", assignee: "assistant"` |
| `move task` | Move to different column | `op: "move task", id: "<task_id>", column: "doing"` |

### Workflow Pattern

When the user gives you work:

1. **Register** - Ensure you're registered as an actor (with `ensure: true`)
2. **Plan** - Break down the work into discrete tasks
3. **Create** - Add each task to the board with `add task`
4. **Assign** - Assign tasks to yourself with `assign task`
5. **Execute** - Work through tasks one at a time
6. **Complete** - Mark each task done immediately after finishing

### Task Lifecycle

```
[add task] --> TODO --> [move to doing] --> DOING --> [complete task] --> DONE
```

### Using Dependencies

Tasks can depend on other tasks. A task is only "ready" when all its dependencies are complete:

```
kanban op: "add task", title: "Deploy to production", depends_on: ["<build_task_id>", "<test_task_id>"]
```

The `next task` operation automatically returns only ready tasks (those with no incomplete dependencies).

### Columns and Organization

Default columns: **To Do** --> **Doing** --> **Done**

Use columns to show work state:
- **To Do**: Planned work not yet started
- **Doing**: Work in progress
- **Done**: Completed work

### Tags for Categorization

Create and use tags to categorize tasks:

```
kanban op: "add tag", id: "bug", name: "Bug", color: "ff0000"
kanban op: "tag task", id: "<task_id>", tag: "bug"
```

Filter tasks by tag:
```
kanban op: "list tasks", tag: "bug"
```

### Comments for Context

Add comments to tasks for notes and updates:

```
kanban op: "add comment", task_id: "<task_id>", body: "Found root cause - null pointer in auth module", author: "assistant"
```

### Best Practices

1. **Granular tasks** - Each task should be completable in one focused effort
2. **Clear titles** - Task titles should describe the outcome, not the process
3. **Use descriptions** - Add details in the description field for complex tasks
4. **Track blockers** - Use dependencies to model task relationships
5. **Regular updates** - Move tasks through columns as status changes
6. **Always use ensure** - When registering as an actor, always use `ensure: true`

### Example Session

User: "Add user authentication to the app"

Assistant thinking:
- I need to register myself first (with ensure: true for safety)
- I need to break this into tasks
- Then create and assign the tasks

```
kanban op: "add actor", id: "assistant", name: "Assistant", type: "agent", ensure: true

kanban op: "add task", title: "Design auth architecture", description: "Decide on JWT vs session, storage strategy"
kanban op: "add task", title: "Create user model", description: "Add User table with email, password hash, created_at"
kanban op: "add task", title: "Implement login endpoint", description: "POST /api/login with email/password"
kanban op: "add task", title: "Implement logout endpoint", description: "POST /api/logout to invalidate session"
kanban op: "add task", title: "Add auth middleware", description: "Protect routes that require authentication"
kanban op: "add task", title: "Write auth tests", description: "Unit and integration tests for auth flow"

kanban op: "assign task", id: "<task1_id>", assignee: "assistant"
```

Then work through each task, marking complete as you go:

```
kanban op: "move task", id: "<task1_id>", column: "doing"
... do the work ...
kanban op: "complete task", id: "<task1_id>"
kanban op: "next task"  -- get next ready task
```
