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

### Actor Registration

Your actor is **automatically created** when you connect via MCP — the server registers you using your MCP client name. You do NOT need to call `add actor` yourself. Tasks you create are automatically assigned to you.

### Essential Operations

| Operation | Description | Example |
|-----------|-------------|---------|
| `add task` | Create a new task (auto-assigned to you) | `op: "add task", title: "Fix login bug"` |
| `list tasks` | View all tasks | `op: "list tasks"` or `op: "list tasks", column: "todo"` |
| `next task` | Get next actionable task (not done) | `op: "next task"` or `op: "next task", tag: "bug"` |
| `complete task` | Move task to done | `op: "complete task", id: "<task_id>"` |
| `move task` | Move to different column | `op: "move task", id: "<task_id>", column: "doing"` |

### Workflow Pattern

When the user gives you work:

1. **Plan** - Break down the work into discrete tasks
2. **Create** - Add each task to the board with `add task` (auto-assigned to you)
3. **Execute** - Work through tasks one at a time
4. **Complete** - Mark each task done immediately after finishing

### Task Lifecycle

When a `review` column is in use (the standard workflow in this project):

```
[add task] --> TODO --> [move to doing] --> DOING --> [move to review] --> REVIEW --> [/review passes] --> DONE
```

The bare tool lifecycle (no review gate) is still available for boards that don't use the review workflow:

```
[add task] --> TODO --> [move to doing] --> DOING --> [complete task] --> DONE
```

Skills like `implement`, `review`, `finish`, and `kanban` in this project take the first path — `complete task` is not used because it would skip the review gate.

### Using Dependencies

Tasks can depend on other tasks. A task is only "ready" when all its dependencies are complete:

```
kanban op: "add task", title: "Deploy to production", depends_on: ["<build_task_id>", "<test_task_id>"]
```

The `next task` operation automatically returns only ready tasks (those with no incomplete dependencies) from any non-done column. It supports `tag` and `assignee` filters — use these to focus on specific work (e.g., `op: "next task", tag: "bug"`).

### Columns and Organization

Default columns: **To Do** --> **Doing** --> **Done**. Workflow skills (`implement`, `review`, `finish`, `kanban`) also ensure a **Review** column sits immediately before **Done**.

Use columns to show work state:
- **To Do**: Planned work not yet started
- **Doing**: Work in progress
- **Review**: Implementation complete, waiting on (or in) code review
- **Done**: Reviewed and completed work

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

### Example Session

User: "Add user authentication to the app"

Assistant thinking:
- I need to break this into tasks
- Tasks are auto-assigned to me via MCP

```
kanban op: "add task", title: "Design auth architecture", description: "What: Decide on JWT vs session, storage strategy. Acceptance Criteria: Auth strategy documented in task comments; Token format and expiry policy decided. Tests: No code tests — this is a design task."
kanban op: "add task", title: "Create user model", description: "What: Add User table with email, password_hash, created_at in src/models/user.rs. Acceptance Criteria: User struct with email, password_hash, created_at fields; Migration creates users table. Tests: Unit test in src/models/user.rs for User creation; cargo test --lib models::user passes."
kanban op: "add task", title: "Implement login endpoint", description: "What: POST /api/login with email/password in src/routes/auth.rs. Acceptance Criteria: Returns JWT on valid credentials; Returns 401 on invalid credentials. Tests: Integration test in tests/auth.rs for login success and failure; cargo test auth::login passes."
```

Then work through each task, moving it to `review` when the work is done (the review skill drives it through to `done`):

```
kanban op: "move task", id: "<task1_id>", column: "doing"
... do the work ...
kanban op: "move task", id: "<task1_id>", column: "review"
kanban op: "next task"  -- get next ready task
```
