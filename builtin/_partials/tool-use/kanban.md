---
title: Kanban Board Management
description: How to use the kanban tool for task management
partial: true
---

## Kanban Board Management

Use the kanban tool to track work and give the user visibility. Every significant task goes on the board.

### Core Principles

1. **Track everything significant.**
2. **Break down complex work** into actionable items.
3. **Mark complete immediately** — never batch completions.

### Actor Registration

Your actor is **auto-created** by the MCP server using your client name. Don't call `add actor`. Tasks you create are auto-assigned to you.

### Essential Operations

| Operation | Description | Example |
|-----------|-------------|---------|
| `add task` | Create a task (auto-assigned) | `op: "add task", title: "Fix login bug"` |
| `list tasks` | View tasks | `op: "list tasks"` (optional `column`) |
| `next task` | Next actionable task | `op: "next task"` (optional `tag`, `assignee`) |
| `complete task` | Move to done | `op: "complete task", id: "<id>"` |
| `move task` | Change column | `op: "move task", id: "<id>", column: "doing"` |

### Workflow

1. **Plan** — break the work down
2. **Create** — `add task` for each piece
3. **Execute** — one task at a time
4. **Complete** — mark done immediately

### Lifecycle

With a `review` column (standard in this project):

```
[add task] → TODO → [move doing] → DOING → [move review] → REVIEW → [/review passes] → DONE
```

Without a review gate:

```
[add task] → TODO → [move doing] → DOING → [complete task] → DONE
```

The `implement`, `review`, `finish`, and `kanban` skills use the first path — `complete task` would skip the review gate.

### Dependencies

A task is "ready" only when its dependencies are complete:

```
kanban op: "add task", title: "Deploy", depends_on: ["<build_id>", "<test_id>"]
```

`next task` returns only ready tasks. It supports `tag` and `assignee` filters.

### Columns

Defaults: **To Do** → **Doing** → **Done**. Workflow skills also ensure a **Review** column before **Done**:
- **To Do**: planned, not started
- **Doing**: in progress
- **Review**: implementation complete, in code review
- **Done**: reviewed and complete

### Tags

```
kanban op: "add tag", id: "bug", name: "Bug", color: "ff0000"
kanban op: "tag task", id: "<id>", tag: "bug"
kanban op: "list tasks", tag: "bug"
```

### Comments

```
kanban op: "add comment", task_id: "<id>", body: "Root cause: null pointer in auth", author: "assistant"
```

### Best Practices

- **Granular tasks** — each task is one focused effort
- **Outcome-focused titles** — describe the result, not the process
- **Use descriptions** for complex tasks
- **Use dependencies** to model blockers
- **Move tasks through columns** as state changes

### Example

User: "Add user authentication."

```
kanban op: "add task", title: "Design auth architecture", description: "What: Decide on JWT vs session, storage strategy. AC: Auth strategy documented; token format and expiry decided. Tests: design task."
kanban op: "add task", title: "Create user model", description: "What: Add User table (email, password_hash, created_at) in src/models/user.rs. AC: User struct + migration. Tests: cargo test --lib models::user."
kanban op: "add task", title: "Implement login endpoint", description: "What: POST /api/login in src/routes/auth.rs. AC: JWT on success, 401 on bad creds. Tests: cargo test auth::login."
```

Then work each through `doing → review`, letting the review skill drive to done:

```
kanban op: "move task", id: "<id>", column: "doing"
... do the work ...
kanban op: "move task", id: "<id>", column: "review"
kanban op: "next task"
```
