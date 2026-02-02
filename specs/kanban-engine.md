# Kanban Engine Specification

A Rust-based kanban engine exposed as MCP tools for task management within a repository.

## Overview

The kanban engine provides file-backed task board management. A `.kanban` directory in a repository root **is** the project - one board per repo.

## Design Principles

- **One repo = one board** - The `.kanban` directory lives at the repo root
- **File-per-task** - Tasks are individual JSON files for clean git diffs
- **Git-friendly** - Human-readable JSON, no binary formats
- **Agent-aware** - Tracks which agent/user modified tasks and why

## Storage Structure

```
repo/
└── .kanban/
    ├── board.json         # Board metadata and column definitions
    ├── tasks/
    │   ├── {id}.json      # Current task state
    │   ├── {id}.jsonl     # Per-task operation log
    │   └── ...
    └── activity/
        ├── 000001.jsonl   # Global log (archived)
        └── current.jsonl  # Active global log
```

**Dual logging**: Operations are logged both globally (for board-wide queries) and per-task (for task history). The per-task `.jsonl` contains all operations that affected that task.

**Key principle:** Tasks are the single source of truth for their own state. The board file defines structure (what columns exist), not membership (which tasks are in them). Column contents are computed by reading tasks and filtering by `column_id`.

## Identifiers

Newtype wrappers for each noun to prevent mixing up IDs at compile time. Use a macro to avoid repetition:

```rust
use serde::{Deserialize, Serialize};
use std::fmt;

/// Macro to define ID newtypes with consistent derives and impls
macro_rules! define_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new() -> Self {
                Self(ulid::Ulid::new().to_string())
            }

            pub fn from_str(s: impl Into<String>) -> Self {
                Self(s.into())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

// Define all ID types
define_id!(TaskId, "ULID-based identifier for tasks");
define_id!(ColumnId, "Identifier for columns (slug-style)");
define_id!(SubtaskId, "ULID-based identifier for subtasks");
define_id!(AttachmentId, "ULID-based identifier for attachments");
define_id!(LogEntryId, "ULID-based identifier for log entries");
define_id!(SwimlaneId, "Identifier for swimlanes (slug-style)");
define_id!(ActorId, "Identifier for actors (people or agents)");
define_id!(TagId, "Identifier for tags (slug-style)");
define_id!(CommentId, "ULID-based identifier for comments");
```

### Position

Position is the full location of a task: column + optional swimlane + ordinal within that cell.

```rust
/// Full position of a task on the board
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub column: ColumnId,
    pub swimlane: Option<SwimlaneId>,
    pub ordinal: Ordinal,
}

/// Ordering within a column/swimlane cell. Uses fractional indexing.
#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Ordinal(String);

impl Ordinal {
    /// Ordinal at the start
    pub fn first() -> Self {
        Self("a0".to_string())
    }

    /// Ordinal after all existing ordinals
    pub fn after(last: &Ordinal) -> Self {
        // Increment the last character or append
        // e.g., "a0" → "a1", "a9" → "b0", "az" → "b0"
    }

    /// Ordinal between two existing ordinals
    pub fn between(before: &Ordinal, after: &Ordinal) -> Self {
        // Fractional index: find midpoint string
        // e.g., between("a0", "a2") → "a1"
        // e.g., between("a1", "a2") → "a1V" (midpoint character)
    }
}

impl Ord for Ordinal {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)  // Lexicographic ordering
    }
}
```

**Usage:**

```rust
// Move task to "done" column, no swimlane, at the end
let pos = Position {
    column: ColumnId("done".into()),
    swimlane: None,
    ordinal: Ordinal::after(&last_ordinal_in_done),
};

// Move task to "in_progress" column, "backend" swimlane, between two tasks
let pos = Position {
    column: ColumnId("in_progress".into()),
    swimlane: Some(SwimlaneId("backend".into())),
    ordinal: Ordinal::between(&task_a.ordinal, &task_b.ordinal),
};
```

**Why fractional indexing for ordinal:**
- Insert between two items without updating other positions
- No gaps to maintain, no reordering needed
- Lexicographic sort = display order
- Works well with concurrent edits

This prevents errors like:
```rust
// Compile error - can't pass TaskId where ColumnId expected
fn move_task(task: TaskId, column: TaskId) // wrong!
fn move_task(task: TaskId, column: ColumnId) // correct
```

## Core Data Models

### Board

The board file defines metadata and structure. It does NOT track which tasks are in which columns.

```rust
pub struct Board {
    pub name: String,
    pub description: Option<String>,
    pub columns: Vec<Column>,
    pub swimlanes: Vec<Swimlane>,
    pub actors: Vec<Actor>,
    pub tags: Vec<Tag>,
}
```

**Default columns** when creating a new board:
- To Do
- Doing
- Done

### Column

Columns define structure, not membership. Task membership is determined by each task's `column` field. **The last column (highest `order`) is the terminal/done column** - tasks there are considered complete for dependency purposes.

```rust
pub struct Column {
    pub id: ColumnId,    // Stable identifier (e.g., "todo", "in_progress")
    pub name: String,    // Display name (e.g., "To Do", "In Progress")
    pub order: usize,    // Sort order (highest = terminal/done)
}
```

### Swimlane

Swimlanes are orthogonal to columns - a task is in one column AND optionally one swimlane.

```rust
pub struct Swimlane {
    pub id: SwimlaneId,  // Stable identifier (e.g., "frontend", "backend")
    pub name: String,    // Display name
    pub order: usize,    // Sort order for display
}
```

### Tag

Tags categorize tasks. Stored in the board file alongside columns/swimlanes/actors.

```rust
pub struct Tag {
    pub id: TagId,           // Slug identifier (e.g., "bug", "feature")
    pub name: String,        // Display name (e.g., "Bug", "Feature Request")
    pub description: Option<String>,  // Optional description (max 100 chars)
    pub color: String,       // 6-character hex code without # (e.g., "ff0000")
}
```

Example:
```json
{
  "id": "bug",
  "name": "Bug",
  "description": "Something isn't working",
  "color": "d73a4a"
}
```

### Actor

Actors are people or agents that can be assigned to tasks and perform operations. Uses an enum with associated values rather than a struct with a type field.

```rust
pub enum Actor {
    Human {
        id: ActorId,
        name: String,
    },
    Agent {
        id: ActorId,
        name: String,
        // Could add agent-specific fields later (e.g., model, session_id)
    },
}

impl Actor {
    pub fn id(&self) -> &ActorId {
        match self {
            Actor::Human { id, .. } => id,
            Actor::Agent { id, .. } => id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Actor::Human { name, .. } => name,
            Actor::Agent { name, .. } => name,
        }
    }
}
```

Examples (JSON uses adjacently tagged representation):
- `{ "Human": { "id": "alice", "name": "Alice Smith" } }`
- `{ "Agent": { "id": "claude", "name": "Claude" } }`

### Task

Tasks are the **source of truth**. Each task is stored as an individual JSON file in `.kanban/tasks/`. The task owns its column membership and all its state.

```rust
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub description: String,  // Supports markdown
    pub tags: Vec<TagId>,     // References to Tag objects in board

    // Position = column + swimlane + ordinal
    pub position: Position,

    // Dependencies - creates a DAG
    pub depends_on: Vec<TaskId>,

    // Actors (people or agents)
    pub assignees: Vec<ActorId>,

    // Nested items
    pub comments: Vec<Comment>,
    pub subtasks: Vec<Subtask>,
    pub attachments: Vec<Attachment>,

    // NOTE: No timestamp fields stored. Derive from per-task JSONL log:
    // - created_at: timestamp of first log entry
    // - updated_at: timestamp of last log entry
    // - created_by: actor of first log entry
    // - modified_by: actor of last log entry
}
// No timestamps, no priority, no type - keep it minimal.
// Classification via tags. History in per-task log.
```

### Comment

Comments are the discussion thread on a task. Unlike the task description (what needs to be done), comments capture the ongoing dialogue about the work.

**Use comments for:**
- Asking clarifying questions
- Recording decisions and their rationale
- Providing status updates
- Agent reasoning about changes made

Comments are stored in the task file and timestamps are derived from the per-task operation log.

```rust
pub struct Comment {
    pub id: CommentId,
    pub body: String,           // Supports markdown
    pub author: ActorId,        // Who wrote it
    // NOTE: No created_at/updated_at. Derive from per-task operation log
    // by finding log entries for this comment ID.
}
```

### Subtask

```rust
pub struct Subtask {
    pub id: SubtaskId,
    pub title: String,
    pub completed: bool,
    // NOTE: No completed_at timestamp. Derive from per-task operation log
    // by finding the log entry that set this subtask's completed to true.
}
```

### Attachment

```rust
pub struct Attachment {
    pub id: AttachmentId,
    pub name: String,
    pub path: String,
    pub mime_type: Option<String>,
    pub size: Option<u64>,
    // NOTE: No created_at field. Derive from the per-task operation log
    // by finding the first log entry that added this attachment ID.
}
```

### Activity (Operation Log)

**Operations ARE the log.** Every operation that flows through the system is logged as JSONL. The activity log is the canonical record of all mutations.

```rust
pub struct LogEntry {
    pub id: LogEntryId,
    pub timestamp: DateTime<Utc>,
    pub op: String,              // Canonical op (e.g., "add task")
    pub input: Value,            // The normalized input
    pub output: Value,           // The result (or error)
    pub actor: Option<String>,   // "claude[session_id]" or user ID
    pub duration_ms: u64,
}
```

**Rolling scheme**: Files roll by entry count (not time). Default: 1000 entries per file.

```
.kanban/activity/
├── 000001.jsonl    # Entries 1-1000
├── 000002.jsonl    # Entries 1001-2000
└── current.jsonl   # Active file, renamed when full
```

Each line in the JSONL file is a complete `LogEntry`. This means:
- You can replay the log to reconstruct state
- You can tail `current.jsonl` for real-time events
- Old files can be archived/deleted (they're just history)

## MCP Tool Interface

**Single tool, multiple operations.** The `kanban` tool accepts verb+noun operations to avoid tool pollution.

### Tool: `kanban`

```json
{
  "name": "kanban",
  "description": "Kanban board operations for task management",
  "inputSchema": {
    "type": "object",
    "additionalProperties": true
  }
}
```

### Forgiving Input Parsing

The tool is extremely liberal in what it accepts. All of these are valid:

```json
// Explicit op
{ "op": "add task", "title": "Fix bug" }

// Op as array (batch)
{ "op": ["add task", "add task"], "titles": ["Task 1", "Task 2"] }

// List of operation objects
[
  { "op": "add task", "title": "Task 1" },
  { "op": "move task", "id": "abc", "column": "done" }
]

// Inferred op - if it looks like an op, treat it as one
{ "add": "task", "title": "Fix bug" }
{ "verb": "add", "noun": "task", "title": "Fix bug" }
{ "action": "add task", "title": "Fix bug" }

// Even just the data - infer op from context
{ "title": "Fix bug" }  // → add task (has title, no id)
{ "id": "abc", "column": "done" }  // → move task (has id + column)
```

**Inference rules:**
- Has `title` but no `id` → `add task`
- Has `id` + `column` but no other updates → `move task`
- Has `id` + other fields → `update task`
- Has only `id` → `get task`
- Empty or just `path` → `get board`

**Parameter aliases** - all of these work:
- `id`, `taskId`, `task_id`, `taskID`
- `op`, `operation`, `action`, `verb`
- `description`, `desc`, `body`, `content`
- etc.

The parser normalizes everything before execution.

### Parser Chain

Input goes through a chain of parsers until one succeeds:

```
┌──────────────────┐
│ Explicit Op      │  Has "op" field with "verb noun"
└────────┬─────────┘
         │ fail
         ▼
┌──────────────────┐
│ Split Fields     │  Has "verb"/"action" + "noun"/"target"
└────────┬─────────┘
         │ fail
         ▼
┌──────────────────┐
│ Shorthand Keys   │  Has verb as key: { "add": "task" }
└────────┬─────────┘
         │ fail
         ▼
┌──────────────────┐
│ Infer from Data  │  Guess op from which fields are present
└────────┬─────────┘
         │ fail
         ▼
┌──────────────────┐
│ Error            │  Cannot determine operation
└──────────────────┘
```

### Canonical Form

Regardless of input format, every operation is normalized to canonical form before logging/execution:

```json
{
  "op": "add task",
  "title": "Fix the bug",
  "priority": "high",
  "column": "todo"
}
```

**Canonical op strings** (always lowercase, single space):
- `init board`
- `get board`
- `update board`
- `list tasks`
- `get task`
- `next task`
- `add task`
- `update task`
- `move task`
- `delete task`
- `list activity`

**Field normalization**:
- All keys converted to `snake_case`
- Aliases resolved: `taskId` → `id`, `desc` → `description`
- Empty strings become `null`
- Whitespace trimmed

### Vocabulary

**Nouns:**
| Noun | Description |
|------|-------------|
| `board` | The kanban board itself (metadata) |
| `column` | A workflow stage |
| `columns` | All columns (for listing) |
| `swimlane` | A horizontal grouping |
| `swimlanes` | All swimlanes (for listing) |
| `actor` | A person or agent |
| `actors` | All actors (for listing) |
| `task` | A single task/card |
| `tasks` | Multiple tasks (for listing/filtering) |
| `tag` | A tag on a task |
| `tags` | All tags (for listing) |
| `comment` | A discussion item on a task - threaded conversation for context, questions, updates, and decisions |
| `comments` | All comments on a task (for listing) |
| `activity` | Activity log events |

**Verbs:**
| Verb | Aliases | Description |
|------|---------|-------------|
| `init` | `create`, `new` | Initialize/create something new |
| `get` | `show`, `read`, `fetch` | Retrieve a single item |
| `list` | `ls`, `find`, `search`, `query` | Retrieve multiple items |
| `add` | `create`, `new`, `insert` | Add a new item |
| `update` | `edit`, `modify`, `set`, `patch` | Modify an existing item |
| `move` | `mv` | Change task's column/position |
| `delete` | `remove`, `rm`, `del` | Delete an item |
| `next` | | Get next actionable item |
| `complete` | `done`, `finish`, `close` | Move task to terminal column |
| `tag` | `label` | Apply a tag to a task |
| `untag` | `unlabel` | Remove a tag from a task |

### Verb + Noun Matrix

Which combinations are valid:

| | `board` | `column` | `columns` | `swimlane` | `swimlanes` | `actor` | `actors` | `task` | `tasks` | `tag` | `tags` | `comment` | `comments` | `activity` |
|---------|:-------:|:--------:|:---------:|:----------:|:-----------:|:-------:|:--------:|:------:|:-------:|:-----:|:------:|:---------:|:----------:|:----------:|
| `init` | ✓ | | | | | | | | | | | | | |
| `get` | ✓ | ✓ | | ✓ | | ✓ | | ✓ | | ✓ | | ✓ | | |
| `list` | | | ✓ | | ✓ | | ✓ | | ✓ | | ✓ | | ✓ | ✓ |
| `add` | | ✓ | | ✓ | | ✓ | | ✓ | | ✓ | | ✓ | | |
| `update` | ✓ | ✓ | | ✓ | | ✓ | | ✓ | | ✓ | | ✓ | | |
| `move` | | | | | | | | ✓ | | | | | | |
| `delete` | | ✓ | | ✓ | | ✓ | | ✓ | | ✓ | | ✓ | | |
| `next` | | | | | | | | ✓ | | | | | | |
| `complete` | | | | | | | | ✓ | | | | | | |
| `tag` | | | | | | | | ✓ | | | | | | |
| `untag` | | | | | | | | ✓ | | | | | | |

**Valid operations (39 total):**
- Board: `init board`, `get board`, `update board`
- Column: `get column`, `add column`, `update column`, `delete column`, `list columns`
- Swimlane: `get swimlane`, `add swimlane`, `update swimlane`, `delete swimlane`, `list swimlanes`
- Actor: `get actor`, `add actor`, `update actor`, `delete actor`, `list actors`
- Task: `get task`, `add task`, `update task`, `move task`, `delete task`, `next task`, `complete task`, `tag task`, `untag task`
- Tasks: `list tasks`
- Tag: `get tag`, `add tag`, `update tag`, `delete tag`, `list tags`
- Comment: `get comment`, `add comment`, `update comment`, `delete comment`, `list comments`
- Activity: `list activity`

### Equivalent Input Forms

All of these invoke `add task`:

```json
// Canonical
{ "op": "add task", "title": "Fix bug" }

// Verb + noun separated
{ "verb": "add", "noun": "task", "title": "Fix bug" }
{ "action": "add", "target": "task", "title": "Fix bug" }

// Using aliases
{ "op": "create task", "title": "Fix bug" }
{ "op": "new task", "title": "Fix bug" }
{ "operation": "insert task", "title": "Fix bug" }

// Shorthand object keys
{ "add": "task", "title": "Fix bug" }
{ "create": "task", "title": "Fix bug" }

// Inferred from context (has title, no id)
{ "title": "Fix bug" }
```

All of these invoke `move task`:

```json
{ "op": "move task", "id": "abc", "column": "done" }
{ "op": "mv task", "id": "abc", "column": "done" }
{ "move": "task", "id": "abc", "column": "done" }

// Inferred (has id + column, no other updates)
{ "id": "abc", "column": "done" }
```

All of these invoke `get board`:

```json
{ "op": "get board" }
{ "op": "show board" }
{ "get": "board" }
{ "show": "board" }

// Inferred (empty or just path)
{ }
{ "path": "/some/repo" }
```

### Operation Schemas

#### init board
```json
{
  "op": "init board",
  "name": "string (required)",
  "description": "string (optional)"
}
```

#### get board
```json
{
  "op": "get board"
}
```
Returns: Board with columns, swimlanes, and task counts per column.

#### update board
```json
{
  "op": "update board",
  "name": "string (optional)",
  "description": "string (optional)"
}
```

#### get column
```json
{
  "op": "get column",
  "id": "string (required)"
}
```

#### add column
```json
{
  "op": "add column",
  "id": "string (required) - slug identifier",
  "name": "string (required) - display name",
  "order": "integer (optional, defaults to end)"
}
```

#### update column
```json
{
  "op": "update column",
  "id": "string (required)",
  "name": "string (optional)",
  "order": "integer (optional)"
}
```

#### delete column
```json
{
  "op": "delete column",
  "id": "string (required)"
}
```
Fails if column has tasks. Move tasks first.

#### list columns
```json
{
  "op": "list columns"
}
```

#### get swimlane
```json
{
  "op": "get swimlane",
  "id": "string (required)"
}
```

#### add swimlane
```json
{
  "op": "add swimlane",
  "id": "string (required) - slug identifier",
  "name": "string (required) - display name",
  "order": "integer (optional, defaults to end)"
}
```

#### update swimlane
```json
{
  "op": "update swimlane",
  "id": "string (required)",
  "name": "string (optional)",
  "order": "integer (optional)"
}
```

#### delete swimlane
```json
{
  "op": "delete swimlane",
  "id": "string (required)"
}
```
Tasks in this swimlane will have their swimlane set to null.

#### list swimlanes
```json
{
  "op": "list swimlanes"
}
```

#### get actor
```json
{
  "op": "get actor",
  "id": "string (required)"
}
```

#### add actor
```json
{
  "op": "add actor",
  "id": "string (required) - identifier",
  "name": "string (required) - display name",
  "type": "human | agent (required)"
}
```

#### update actor
```json
{
  "op": "update actor",
  "id": "string (required)",
  "name": "string (optional)"
}
```

#### delete actor
```json
{
  "op": "delete actor",
  "id": "string (required)"
}
```
Removes actor from all task assignee lists.

#### list actors
```json
{
  "op": "list actors",
  "type": "human | agent (optional) - filter by type"
}
```

#### list tasks
```json
{
  "op": "list tasks",
  "column": "string (optional)",
  "swimlane": "string (optional)",
  "tag": "string (optional)",
  "assignee": "string (optional)",
  "ready": "boolean (optional) - only tasks with deps complete"
}
```

#### get task
```json
{
  "op": "get task",
  "id": "string (required)"
}
```

#### next task
```json
{
  "op": "next task",
  "swimlane": "string (optional)",
  "assignee": "string (optional)"
}
```
Returns: Oldest ready task in first column, filtered by swimlane/assignee.

#### add task
```json
{
  "op": "add task",
  "title": "string (required)",
  "description": "string (optional)",
  "position": {
    "column": "string (optional, defaults to first)",
    "swimlane": "string (optional)",
    "ordinal": "string (optional, defaults to end)"
  },
  "tags": "string[] (optional)",
  "assignees": "string[] (optional)",
  "depends_on": "string[] (optional)"
}
```

Shorthand: `column` and `swimlane` can be top-level for convenience:
```json
{ "op": "add task", "title": "Fix bug", "column": "todo", "swimlane": "backend" }
```

#### update task
```json
{
  "op": "update task",
  "id": "string (required)",
  "title": "string (optional)",
  "description": "string (optional)",
  "swimlane": "string (optional)",
  "tags": "string[] (optional)",
  "assignees": "string[] (optional)",
  "depends_on": "string[] (optional)",
  "subtasks": "Subtask[] (optional)",
  "attachments": "Attachment[] (optional)"
}
```
Arrays are replaced wholesale when provided.

#### move task
```json
{
  "op": "move task",
  "id": "string (required)",
  "position": {
    "column": "string (required)",
    "swimlane": "string (optional)",
    "ordinal": "string (optional, defaults to end)"
  }
}
```

Shorthand forms also accepted:
```json
// Just column (swimlane unchanged, append to end)
{ "op": "move task", "id": "abc", "column": "done" }

// Column + swimlane
{ "op": "move task", "id": "abc", "column": "done", "swimlane": "backend" }

// Full position
{ "op": "move task", "id": "abc", "position": { "column": "done", "ordinal": "a5" } }
```

#### delete task
```json
{
  "op": "delete task",
  "id": "string (required)"
}
```

#### complete task
Move a task to the terminal (done) column.
```json
{
  "op": "complete task",
  "id": "string (required)"
}
```

#### add tag
Create a new tag definition in the board.
```json
{
  "op": "add tag",
  "id": "string (required) - slug identifier",
  "name": "string (required) - display name",
  "description": "string (optional) - max 100 chars",
  "color": "string (required) - 6-char hex without #"
}
```

#### get tag
```json
{
  "op": "get tag",
  "id": "string (required)"
}
```

#### update tag
```json
{
  "op": "update tag",
  "id": "string (required)",
  "name": "string (optional)",
  "description": "string (optional)",
  "color": "string (optional)"
}
```

#### delete tag
Remove tag definition from board. Also removes from all tasks.
```json
{
  "op": "delete tag",
  "id": "string (required)"
}
```

#### list tags
```json
{
  "op": "list tags"
}
```
Returns: All tag definitions with usage counts.

#### tag task
Apply a tag to a task.
```json
{
  "op": "tag task",
  "task_id": "string (required)",
  "tag_id": "string (required)"
}
```

#### untag task
Remove a tag from a task.
```json
{
  "op": "untag task",
  "task_id": "string (required)",
  "tag_id": "string (required)"
}
```

#### add comment
Add a comment to a task.
```json
{
  "op": "add comment",
  "task_id": "string (required)",
  "body": "string (required) - supports markdown",
  "author": "string (required) - actor ID"
}
```
Returns: The created comment with generated `id`.

#### get comment
```json
{
  "op": "get comment",
  "task_id": "string (required)",
  "comment_id": "string (required)"
}
```

#### update comment
```json
{
  "op": "update comment",
  "task_id": "string (required)",
  "comment_id": "string (required)",
  "body": "string (required)"
}
```
Note: Only the author can update their own comments.

#### delete comment
```json
{
  "op": "delete comment",
  "task_id": "string (required)",
  "comment_id": "string (required)"
}
```

#### list comments
```json
{
  "op": "list comments",
  "task_id": "string (required)"
}
```
Returns: All comments on the task, ordered by creation time (derived from log).

#### list activity
```json
{
  "op": "list activity",
  "limit": "integer (default: 20)",
  "task": "string (optional) - filter to specific task"
}
```

### Batch Operations

When multiple operations are passed (as array or list), they execute sequentially and return an array of results:

```json
// Input
[
  { "op": "add task", "title": "Task A" },
  { "op": "add task", "title": "Task B", "depends_on": ["$0"] }
]

// Output
[
  { "ok": true, "id": "01ABC...", "title": "Task A" },
  { "ok": true, "id": "01DEF...", "title": "Task B" }
]
```

**Result references**: Use `$0`, `$1`, etc. to reference the ID from a previous operation's result. Useful for creating tasks with dependencies in one call.

## Operations

### Operation

An `Operation` is the canonical, validated representation of a single request. After parsing and normalization, every input becomes an Operation.

```rust
/// A single canonical operation ready for execution
pub struct Operation {
    /// Unique identifier for this operation instance
    pub id: LogEntryId,

    /// The canonical verb (add, get, update, delete, move, list, next, init)
    pub verb: Verb,

    /// The canonical noun (board, task, column, swimlane, actor, tag, activity)
    pub noun: Noun,

    /// Normalized parameters (all aliases resolved, snake_case keys)
    pub params: serde_json::Map<String, Value>,

    /// Who initiated this operation
    pub actor: Option<ActorId>,

    /// Optional note/reasoning (useful for agent operations)
    pub note: Option<String>,
}

pub enum Verb {
    Init,
    Get,
    List,
    Add,
    Update,
    Move,
    Delete,
    Next,
}

pub enum Noun {
    Board,
    Task,
    Tasks,
    Column,
    Columns,
    Swimlane,
    Swimlanes,
    Actor,
    Actors,
    Tag,
    Tags,
    Activity,
}

impl Operation {
    /// Returns the canonical op string (e.g., "add task")
    pub fn op_string(&self) -> String {
        format!("{} {}", self.verb.as_str(), self.noun.as_str())
    }
}
```

### OperationQueue

**All operations flow through a queue.** Even single operations are enqueued. The queue handles:
- Lock acquisition with retry
- Sequential execution with dependency on previous results
- `$0`, `$1`, etc. references to prior operation outputs
- Transactional semantics (all-or-nothing for batch)

```rust
pub struct OperationQueue {
    ops: Vec<Operation>,
    results: Vec<OperationResult>,
    config: QueueConfig,
}

pub struct QueueConfig {
    /// Maximum retry attempts when lock is held
    pub max_retries: usize,          // default: 5
    /// Base delay between retries (exponential backoff)
    pub retry_base_delay_ms: u64,    // default: 100
    /// Maximum delay between retries
    pub retry_max_delay_ms: u64,     // default: 5000
    /// Total timeout for acquiring lock
    pub lock_timeout_ms: u64,        // default: 30000
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            retry_base_delay_ms: 100,
            retry_max_delay_ms: 5000,
            lock_timeout_ms: 30000,
        }
    }
}

pub struct OperationResult {
    pub op_id: LogEntryId,
    pub ok: bool,
    pub data: Value,           // The response payload
    pub error: Option<String>, // Error message if !ok
    pub duration_ms: u64,
    pub retries: usize,        // How many retry attempts
}

impl OperationQueue {
    /// Create a queue from parsed input (single op, array, or batch)
    pub fn from_input(input: Value, actor: Option<ActorId>) -> Result<Self, ParseError>;

    /// Execute all operations in sequence with retry on lock contention
    pub async fn execute(&mut self, ctx: &KanbanContext) -> Vec<OperationResult> {
        for (i, op) in self.ops.iter_mut().enumerate() {
            // Resolve $N references from previous results
            self.resolve_references(op, i);

            let result = self.execute_with_retry(ctx, op.clone()).await;
            self.results.push(result);

            // Stop on first error (transactional)
            if !self.results.last().unwrap().ok {
                break;
            }
        }
        self.results.clone()
    }

    /// Execute single operation with exponential backoff retry
    async fn execute_with_retry(&self, ctx: &KanbanContext, op: Operation) -> OperationResult {
        let mut retries = 0;
        let mut delay = self.config.retry_base_delay_ms;

        loop {
            // Try to acquire lock and execute
            match ctx.lock().await {
                Ok(_lock) => {
                    // Lock acquired - execute the command
                    let cmd = op.clone().into_command();
                    let start = Instant::now();

                    let result = match cmd {
                        Ok(cmd) => cmd.execute(ctx).await,
                        Err(e) => Err(KanbanError::Parse(e)),
                    };

                    return OperationResult {
                        op_id: op.id.clone(),
                        ok: result.is_ok(),
                        data: result.unwrap_or(Value::Null),
                        error: result.err().map(|e| e.to_string()),
                        duration_ms: start.elapsed().as_millis() as u64,
                        retries,
                    };
                }
                Err(KanbanError::LockBusy) if retries < self.config.max_retries => {
                    // Lock held by another process - retry with backoff
                    retries += 1;
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    delay = (delay * 2).min(self.config.retry_max_delay_ms);
                }
                Err(e) => {
                    // Lock timeout or other error - fail
                    return OperationResult {
                        op_id: op.id.clone(),
                        ok: false,
                        data: Value::Null,
                        error: Some(e.to_string()),
                        duration_ms: 0,
                        retries,
                    };
                }
            }
        }
    }

    /// Replace $0, $1 placeholders with actual IDs from prior results
    fn resolve_references(&self, op: &mut Operation, current_index: usize) {
        // Walk params, find "$N" strings, replace with results[N].data["id"]
    }
}
```

### Reference Resolution

Result references allow chaining operations:

```json
[
  { "op": "add task", "title": "Parent task" },
  { "op": "add task", "title": "Child task", "depends_on": ["$0"] }
]
```

Resolution rules:
- `$N` → `results[N].data["id"]` (the ID of the Nth operation's result)
- References to failed operations → entire batch fails
- Forward references (`$1` in op 0) → parse error
- Out-of-bounds references → parse error

## Execution Pipeline

### KanbanContext

The context provides access to storage and utilities. **No business logic methods** - just data access primitives. Commands do all the work.

```rust
/// Context passed to every command - provides access, not logic
pub struct KanbanContext {
    /// Path to the .kanban directory
    root: PathBuf,
}

impl KanbanContext {
    pub fn new(root: PathBuf) -> Self { Self { root } }

    // Path helpers
    pub fn root(&self) -> &Path { &self.root }
    pub fn board_path(&self) -> PathBuf { self.root.join("board.json") }
    pub fn task_path(&self, id: &TaskId) -> PathBuf { self.root.join("tasks").join(format!("{}.json", id)) }
    pub fn task_log_path(&self, id: &TaskId) -> PathBuf { self.root.join("tasks").join(format!("{}.jsonl", id)) }
    pub fn activity_path(&self) -> PathBuf { self.root.join("activity/current.jsonl") }

    // I/O primitives - no business logic
    pub async fn read_board(&self) -> Result<Board, KanbanError>;
    pub async fn write_board(&self, board: &Board) -> Result<(), KanbanError>;
    pub async fn read_task(&self, id: &TaskId) -> Result<Task, KanbanError>;
    pub async fn write_task(&self, task: &Task) -> Result<(), KanbanError>;
    pub async fn delete_task_file(&self, id: &TaskId) -> Result<(), KanbanError>;
    pub async fn list_task_ids(&self) -> Result<Vec<TaskId>, KanbanError>;
    pub async fn append_log(&self, path: &Path, entry: &LogEntry) -> Result<(), KanbanError>;
}
```

**Key distinction**: `KanbanContext` has I/O methods, not business logic methods. There is no `ctx.move_task()` or `ctx.compute_next()` - those live in the Command structs.

### Locking

KanbanContext uses file-based locking to prevent concurrent writes from corrupting state.

```rust
impl KanbanContext {
    /// Try to acquire an exclusive lock (non-blocking)
    pub async fn lock(&self) -> Result<KanbanLock, KanbanError> {
        let lock_path = self.root.join(".lock");
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&lock_path)
            .await?;

        // Non-blocking lock attempt
        match file.try_lock_exclusive() {
            Ok(()) => Ok(KanbanLock { file, path: lock_path }),
            Err(_) => Err(KanbanError::LockBusy), // Caller handles retry
        }
    }
}

/// RAII lock guard - releases on drop
pub struct KanbanLock {
    file: tokio::fs::File,
    path: PathBuf,
}

impl Drop for KanbanLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}
```

**Locking strategy:**
- **Write operations**: Acquire exclusive lock before any mutation
- **Read operations**: No lock needed (reads are atomic at file level)
- **Batch operations**: Single lock for entire batch (transactional)

The Executor acquires the lock:

```rust
impl Executor {
    pub async fn run(&self, op: Operation) -> OperationResult {
        // Acquire lock for write operations
        let _lock = if op.is_mutation() {
            Some(self.ctx.lock().await?)
        } else {
            None
        };

        // ... execute command
    }
}
```

**Multi-process design:**

Multiple processes (agents, CLI tools, IDE plugins) can each have their own `KanbanContext` pointing at the same `.kanban` directory. This is the expected usage pattern.

```
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│  Agent A    │  │  Agent B    │  │    CLI      │
│ KanbanCtx   │  │ KanbanCtx   │  │ KanbanCtx   │
└──────┬──────┘  └──────┬──────┘  └──────┬──────┘
       │                │                │
       └────────────────┼────────────────┘
                        │
                        ▼
                 ┌─────────────┐
                 │  .kanban/   │
                 │  (files)    │
                 └─────────────┘
```

**Coordination rules:**
- Lock is held only for the duration of a single operation (or batch)
- No long-lived locks - acquire, execute, release
- Reads always see consistent file state (atomic file writes)
- Each process re-reads files as needed (no stale caches)

**Why file locking:**
- Works across processes (multiple agents, CLI, etc.)
- No external dependencies (no Redis, no database)
- Automatic cleanup on process death
- Git-compatible (lock file is in .kanban, can be gitignored)

**Queue-based retry:**

```
┌─────────────┐
│ Operation   │
│ submitted   │
└──────┬──────┘
       ▼
┌─────────────┐
│  Try lock   │◄───────────────┐
└──────┬──────┘                │
       │                       │
   ┌───┴───┐                   │
   │ busy? │───yes───► wait ───┘
   └───┬───┘          (backoff)
       │no
       ▼
┌─────────────┐
│  Execute    │
│  command    │
└──────┬──────┘
       ▼
┌─────────────┐
│  Release    │
│  lock       │
└─────────────┘
```

- `LockBusy` → queue retries with exponential backoff
- Max retries exceeded → `KanbanError::LockTimeout`
- No optimistic locking needed - file lock guarantees exclusivity

### Pipeline Stages

Each operation flows through these stages:

```
┌─────────────┐
│   Parse     │  Raw JSON → Operation (normalize aliases, validate)
└──────┬──────┘
       ▼
┌─────────────┐
│  Enqueue    │  Single op or batch → OperationQueue
└──────┬──────┘
       ▼
┌─────────────┐
│  Resolve    │  Replace $N references with prior result IDs
└──────┬──────┘
       ▼
┌─────────────┐
│  Log Start  │  Append to global + per-task logs (op started)
└──────┬──────┘
       ▼
┌─────────────┐
│   Execute   │  Dispatch to handler, mutate state
└──────┬──────┘
       ▼
┌─────────────┐
│ Log Complete│  Append result to logs (op completed)
└──────┬──────┘
       ▼
┌─────────────┐
│   Notify    │  Emit event (async, fire-and-forget)
└──────┬──────┘
       ▼
┌─────────────┐
│   Return    │  OperationResult back to caller
└─────────────┘
```

### Command Pattern

Each operation is implemented as a distinct struct following the **command pattern**. This provides:
- Clear separation of concerns
- Easy testing of individual operations
- Async execution throughout
- Consistent interface for all operations

```rust
/// Trait implemented by all operation handlers
#[async_trait]
pub trait Command: Send + Sync {
    /// Execute the command using the storage layer
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value, KanbanError>;

    /// The canonical op string (e.g., "add task")
    fn op_string(&self) -> &'static str;
}
```

### Command Structs

Each verb+noun combination has its own command struct:

```rust
// ============================================================================
// Board commands
// ============================================================================

pub struct InitBoard {
    pub name: String,
    pub description: Option<String>,
}

#[async_trait]
impl Command for InitBoard {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value, KanbanError> {
        // Each command is self-contained - all logic lives here
        let root = ctx.root();

        // Create directory structure
        tokio::fs::create_dir_all(root.join("tasks")).await?;
        tokio::fs::create_dir_all(root.join("activity")).await?;

        // Build board with default columns
        let board = Board {
            name: self.name.clone(),
            description: self.description.clone(),
            columns: vec![
                Column { id: ColumnId("todo".into()), name: "To Do".into(), order: 0 },
                Column { id: ColumnId("in_progress".into()), name: "In Progress".into(), order: 1 },
                Column { id: ColumnId("review".into()), name: "Review".into(), order: 2 },
                Column { id: ColumnId("done".into()), name: "Done".into(), order: 3 },
            ],
            swimlanes: vec![],
            actors: vec![],
        };

        ctx.write_board(&board).await?;
        Ok(serde_json::to_value(&board)?)
    }

    fn op_string(&self) -> &'static str { "init board" }
}

pub struct GetBoard;

#[async_trait]
impl Command for GetBoard {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value, KanbanError> {
        let board = ctx.read_board().await?;

        // Compute task counts per column
        let task_ids = ctx.list_task_ids().await?;
        let mut counts: HashMap<ColumnId, usize> = HashMap::new();

        for id in task_ids {
            let task = ctx.read_task(&id).await?;
            *counts.entry(task.column.clone()).or_default() += 1;
        }

        // Return board with counts
        let mut result = serde_json::to_value(&board)?;
        result["task_counts"] = serde_json::to_value(&counts)?;
        Ok(result)
    }

    fn op_string(&self) -> &'static str { "get board" }
}

pub struct UpdateBoard {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[async_trait]
impl Command for UpdateBoard {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value, KanbanError> {
        let mut board = ctx.read_board().await?;

        if let Some(name) = &self.name {
            board.name = name.clone();
        }
        if let Some(desc) = &self.description {
            board.description = Some(desc.clone());
        }

        ctx.write_board(&board).await?;
        Ok(serde_json::to_value(&board)?)
    }

    fn op_string(&self) -> &'static str { "update board" }
}

// ============================================================================
// Task commands
// ============================================================================

pub struct AddTask {
    pub title: String,
    pub description: Option<String>,
    pub position: Option<Position>,  // None = first column, no swimlane, end
    pub tags: Vec<String>,
    pub assignees: Vec<ActorId>,
    pub depends_on: Vec<TaskId>,
}

#[async_trait]
impl Command for AddTask {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value, KanbanError> {
        let board = ctx.read_board().await?;

        // Build position - default to first column, no swimlane, end
        let position = match &self.position {
            Some(pos) => pos.clone(),
            None => {
                let column = board.columns.iter()
                    .min_by_key(|c| c.order)
                    .map(|c| c.id.clone())
                    .expect("board must have at least one column");

                // Find last ordinal in that column
                let task_ids = ctx.list_task_ids().await?;
                let mut last_ordinal: Option<Ordinal> = None;
                for id in &task_ids {
                    let t = ctx.read_task(id).await?;
                    if t.position.column == column && t.position.swimlane.is_none() {
                        last_ordinal = Some(match last_ordinal {
                            None => t.position.ordinal.clone(),
                            Some(ref o) if t.position.ordinal > *o => t.position.ordinal.clone(),
                            Some(o) => o,
                        });
                    }
                }

                Position {
                    column,
                    swimlane: None,
                    ordinal: match last_ordinal {
                        Some(last) => Ordinal::after(&last),
                        None => Ordinal::first(),
                    },
                }
            }
        };

        let task = Task {
            id: TaskId::new(),
            title: self.title.clone(),
            description: self.description.clone().unwrap_or_default(),
            tags: self.tags.clone(),
            position,
            depends_on: self.depends_on.clone(),
            assignees: self.assignees.clone(),
            subtasks: vec![],
            attachments: vec![],
        };

        ctx.write_task(&task).await?;
        Ok(serde_json::to_value(&task)?)
    }

    fn op_string(&self) -> &'static str { "add task" }
}

pub struct GetTask {
    pub id: TaskId,
}

pub struct UpdateTask {
    pub id: TaskId,
    pub title: Option<String>,
    pub description: Option<String>,
    pub swimlane: Option<SwimlaneId>,
    pub tags: Option<Vec<String>>,
    pub assignees: Option<Vec<ActorId>>,
    pub depends_on: Option<Vec<TaskId>>,
    pub subtasks: Option<Vec<Subtask>>,
    pub attachments: Option<Vec<Attachment>>,
}

pub struct MoveTask {
    pub id: TaskId,
    pub position: Position,  // Full destination: column + swimlane + ordinal
}

pub struct DeleteTask {
    pub id: TaskId,
}

pub struct NextTask {
    pub swimlane: Option<SwimlaneId>,
    pub assignee: Option<ActorId>,
}

pub struct ListTasks {
    pub column: Option<ColumnId>,
    pub swimlane: Option<SwimlaneId>,
    pub tag: Option<String>,
    pub assignee: Option<ActorId>,
    pub ready: Option<bool>,
}

// ============================================================================
// Board item commands (Column, Swimlane, Actor share similar CRUD patterns)
// ============================================================================

/// Trait for items stored in the board (columns, swimlanes, actors)
pub trait BoardItem: Sized {
    type Id: Clone;
    fn id(&self) -> &Self::Id;
    fn noun() -> &'static str;
}

/// Generic Add command for board items
pub struct Add<T: BoardItem> {
    pub id: T::Id,
    pub name: String,
    pub order: Option<usize>,  // Not used for Actor
    _marker: std::marker::PhantomData<T>,
}

/// Generic Get command for board items
pub struct Get<T: BoardItem> {
    pub id: T::Id,
    _marker: std::marker::PhantomData<T>,
}

/// Generic Update command for board items
pub struct Update<T: BoardItem> {
    pub id: T::Id,
    pub name: Option<String>,
    pub order: Option<usize>,  // Not used for Actor
    _marker: std::marker::PhantomData<T>,
}

/// Generic Delete command for board items
pub struct Delete<T: BoardItem> {
    pub id: T::Id,
    _marker: std::marker::PhantomData<T>,
}

/// Generic List command for board items
pub struct List<T: BoardItem> {
    _marker: std::marker::PhantomData<T>,
}

// Type aliases for clarity
pub type AddColumn = Add<Column>;
pub type GetColumn = Get<Column>;
pub type UpdateColumn = Update<Column>;
pub type DeleteColumn = Delete<Column>;
pub type ListColumns = List<Column>;

pub type AddSwimlane = Add<Swimlane>;
pub type GetSwimlane = Get<Swimlane>;
pub type UpdateSwimlane = Update<Swimlane>;
pub type DeleteSwimlane = Delete<Swimlane>;
pub type ListSwimlanes = List<Swimlane>;

// Actor is special - has `actor_type` instead of `order`, so define explicitly
pub struct AddActor {
    pub id: ActorId,
    pub name: String,
    pub actor_type: ActorType,  // Human or Agent
}

pub struct GetActor {
    pub id: ActorId,
}

pub struct UpdateActor {
    pub id: ActorId,
    pub name: Option<String>,
}

pub struct DeleteActor {
    pub id: ActorId,
}

pub struct ListActors {
    pub actor_type: Option<ActorType>,  // Filter by type
}

// ============================================================================
// Tag commands (board-level tag definitions)
// ============================================================================

/// Create a new tag definition in the board
pub struct AddTag {
    pub id: TagId,
    pub name: String,
    pub description: Option<String>,
    pub color: String,  // 6-char hex without #
}

pub struct GetTag {
    pub id: TagId,
}

pub struct UpdateTag {
    pub id: TagId,
    pub name: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
}

/// Delete tag definition from board (removes from all tasks too)
pub struct DeleteTag {
    pub id: TagId,
}

pub struct ListTags;

// ============================================================================
// Task tagging commands (apply/remove tags on tasks)
// ============================================================================

/// Add a tag to a task
pub struct TagTask {
    pub task_id: TaskId,
    pub tag_id: TagId,
}

/// Remove a tag from a task
pub struct UntagTask {
    pub task_id: TaskId,
    pub tag_id: TagId,
}

// ============================================================================
// Comment commands
// ============================================================================

/// Add a comment to a task
pub struct AddComment {
    pub task_id: TaskId,
    pub body: String,
    pub author: ActorId,
}

pub struct GetComment {
    pub task_id: TaskId,
    pub comment_id: CommentId,
}

pub struct UpdateComment {
    pub task_id: TaskId,
    pub comment_id: CommentId,
    pub body: String,
}

pub struct DeleteComment {
    pub task_id: TaskId,
    pub comment_id: CommentId,
}

pub struct ListComments {
    pub task_id: TaskId,
}

// ============================================================================
// Activity commands
// ============================================================================

pub struct ListActivity {
    pub limit: Option<usize>,
    pub task: Option<TaskId>,
}
```

### Command Dispatch

The `Operation` parses into a boxed `Command` for execution:

```rust
impl Operation {
    /// Parse params into the appropriate Command struct
    pub fn into_command(self) -> Result<Box<dyn Command>, ParseError> {
        match (self.verb, self.noun) {
            (Verb::Init, Noun::Board) => Ok(Box::new(InitBoard::from_params(self.params)?)),
            (Verb::Get, Noun::Board) => Ok(Box::new(GetBoard)),
            (Verb::Update, Noun::Board) => Ok(Box::new(UpdateBoard::from_params(self.params)?)),

            (Verb::Add, Noun::Task) => Ok(Box::new(AddTask::from_params(self.params)?)),
            (Verb::Get, Noun::Task) => Ok(Box::new(GetTask::from_params(self.params)?)),
            (Verb::Update, Noun::Task) => Ok(Box::new(UpdateTask::from_params(self.params)?)),
            (Verb::Move, Noun::Task) => Ok(Box::new(MoveTask::from_params(self.params)?)),
            (Verb::Delete, Noun::Task) => Ok(Box::new(DeleteTask::from_params(self.params)?)),
            (Verb::Next, Noun::Task) => Ok(Box::new(NextTask::from_params(self.params)?)),
            (Verb::List, Noun::Tasks) => Ok(Box::new(ListTasks::from_params(self.params)?)),

            // ... other combinations

            _ => Err(ParseError::InvalidOperation {
                verb: self.verb,
                noun: self.noun,
            }),
        }
    }
}
```

### Executor

The `Executor` is a thin coordinator - it just wires together context, logging, and notification. **No business logic here.**

```rust
pub struct Executor {
    ctx: KanbanContext,
    notifier: Notifier,
}

impl Executor {
    pub fn new(ctx: KanbanContext) -> Self {
        Self {
            ctx,
            notifier: Notifier::new(),
        }
    }

    pub async fn run(&self, op: Operation) -> OperationResult {
        let start = Instant::now();
        let op_id = op.id.clone();
        let op_string = op.op_string();

        // Log start (to global + per-task if applicable)
        self.log_start(&op).await;

        // Parse into command and execute
        let result = match op.into_command() {
            Ok(cmd) => cmd.execute(&self.ctx).await,
            Err(e) => Err(KanbanError::Parse(e)),
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        // Build result
        let op_result = match result {
            Ok(data) => OperationResult {
                op_id,
                ok: true,
                data,
                error: None,
                duration_ms,
            },
            Err(e) => OperationResult {
                op_id,
                ok: false,
                data: Value::Null,
                error: Some(e.to_string()),
                duration_ms,
            },
        };

        // Log complete
        self.log_complete(&op_string, &op_result).await;

        // Notify (fire-and-forget)
        self.notifier.emit(Event::from(&op_result));

        op_result
    }

    async fn log_start(&self, op: &Operation) { /* append to ctx logs */ }
    async fn log_complete(&self, op: &str, result: &OperationResult) { /* append to ctx logs */ }
}
```

The key insight: **each Command struct owns its business logic**. The Executor just orchestrates. KanbanContext just provides I/O primitives.

### Notify

The `Notifier` emits events for external listeners. Non-blocking, fire-and-forget.

```rust
pub struct Notifier {
    /// Callbacks registered for events
    callbacks: Vec<Box<dyn Fn(&Event) + Send + Sync>>,
}

pub struct Event {
    pub op: String,           // e.g., "add task"
    pub task_id: Option<TaskId>,
    pub data: Value,
    pub timestamp: DateTime<Utc>,
}

impl Notifier {
    /// Emit an event to all registered callbacks
    pub fn emit(&self, event: Event) {
        for callback in &self.callbacks {
            // Fire-and-forget, don't block on callbacks
            let cb = callback.clone();
            let evt = event.clone();
            tokio::spawn(async move { cb(&evt) });
        }
    }

    /// Register a callback for events
    pub fn on_event(&mut self, callback: impl Fn(&Event) + Send + Sync + 'static) {
        self.callbacks.push(Box::new(callback));
    }
}
```

Consumers can also watch `current.jsonl` directly for a file-based event stream.

## Business Rules

### Single Source of Truth

- Tasks own their state. Moving a task = updating ONE file (the task).
- Board defines structure (columns), tasks define membership.
- No denormalized "task_ids" lists in columns.

### Dependencies (Task DAG)

Tasks can depend on other tasks via `depends_on`. This creates a directed acyclic graph (DAG).

**Readiness**: A task is "ready" when ALL tasks in its `depends_on` list are in the terminal column (last by order).

```rust
impl Task {
    pub fn is_ready(&self, all_tasks: &HashMap<String, Task>, terminal_column_id: &str) -> bool {
        self.depends_on.iter().all(|dep_id| {
            all_tasks.get(dep_id)
                .map(|t| t.column == terminal_column_id)
                .unwrap_or(true)  // Should not happen due to cascade delete
        })
    }
}
```

The terminal column is determined by: `board.columns.iter().max_by_key(|c| c.order)`

**Cycle Prevention**: When setting `depends_on`, reject if it would create a cycle. A depends on B depends on A = error.

**Cascade on Delete**: When a task is deleted, remove its ID from the `depends_on` list of all other tasks. This keeps the dependency graph clean.

**Computed fields**: The `kanban_task` response should include:
- `blocked_by`: List of incomplete dependencies (not in terminal column)
- `blocks`: List of tasks that depend on this one

### "Next" Task Algorithm

The `next task` operation returns the next actionable task:

1. Filter to tasks in the first column (lowest order)
2. Filter to tasks that are "ready" (all deps done)
3. Sort by position (lowest first)
4. Return the first match

The "oldest" task is determined by position, not timestamps (which are derived from logs if needed).

### Activity Logging

Every mutation operation should log an activity event:
- `actor_id`: Use "mcp" for MCP tool calls, or agent identifier
- Include reasoning when AI agents modify tasks

### Task Movement

Moving a task only modifies the task file:
1. Update task's `column` field
2. Update task's `position` field
3. Log `TaskMoved` activity

### Position Ordering

Tasks have a `position` field for ordering within a column. When inserting:
- If no position specified, append to end (max position + 1)
- Positions don't need to be contiguous - just sortable

### Progress Calculation

```rust
impl Task {
    pub fn progress(&self) -> f64 {
        if self.subtasks.is_empty() {
            return 0.0;
        }
        let completed = self.subtasks.iter().filter(|s| s.completed).count();
        completed as f64 / self.subtasks.len() as f64
    }
}
```

## JSON Serialization

Use `snake_case` for all JSON field names.

### board.json
```json
{
  "name": "My Project",
  "description": "Project kanban board",
  "columns": [
    { "id": "todo", "name": "To Do", "order": 0 },
    { "id": "in_progress", "name": "In Progress", "order": 1 },
    { "id": "review", "name": "Review", "order": 2 },
    { "id": "done", "name": "Done", "order": 3 }
  ],
  "swimlanes": [
    { "id": "frontend", "name": "Frontend", "order": 0 },
    { "id": "backend", "name": "Backend", "order": 1 },
    { "id": "infra", "name": "Infrastructure", "order": 2 }
  ],
  "tags": [
    { "id": "bug", "name": "Bug", "description": "Something isn't working", "color": "d73a4a" },
    { "id": "feature", "name": "Feature", "description": "New functionality", "color": "0075ca" },
    { "id": "docs", "name": "Documentation", "color": "0052cc" }
  ],
  "actors": [
    { "Human": { "id": "alice", "name": "Alice Smith" } },
    { "Agent": { "id": "claude", "name": "Claude" } }
  ]
}
```

### tasks/{id}.json
```json
{
  "id": "01HGW2BBG0000000000000000",
  "title": "Implement authentication",
  "description": "Add OAuth2 login flow",
  "tags": ["feature", "backend"],
  "position": {
    "column": "in_progress",
    "swimlane": "backend",
    "ordinal": "a0"
  },
  "depends_on": ["01HGW2AAA0000000000000000"],
  "assignees": ["alice"],
  "subtasks": [
    { "id": "01HGW2CCC0000000000000000", "title": "Write tests", "completed": false },
    { "id": "01HGW2DDD0000000000000000", "title": "Add OAuth provider", "completed": true }
  ],
  "attachments": []
}
```

### tasks/{id}.jsonl (per-task log)
```jsonl
{"id":"01HGW2EEE...","timestamp":"2025-01-30T10:00:00Z","op":"add task","actor":"bob","input":{...}}
{"id":"01HGW2FFF...","timestamp":"2025-01-30T14:30:00Z","op":"update task","actor":"claude[abc123]","input":{...},"note":"Updated based on code review"}
```

**Derived metadata**: `created_at`, `updated_at`, `created_by`, `modified_by` are all derived from the per-task log by reading first/last entries.

### Computed fields in API responses

When returning a task via `kanban_task`, include computed fields:

```json
{
  "...task fields...",
  "ready": true,
  "blocked_by": [],
  "blocks": ["def456", "ghi012"],
  "progress": 0.5
}
```

## Error Handling

```rust
pub enum KanbanError {
    NotInitialized { path: PathBuf },
    TaskNotFound { id: String },
    ColumnNotFound { id: String },
    Io(std::io::Error),
    Json(serde_json::Error),
}
```

## Implementation Notes

### Rust Crate Structure

Each noun/verb combination is its own module. **No god objects.**

```
swissarmyhammer-kanban/
├── Cargo.toml
└── src/
    ├── lib.rs              # Re-exports, Command trait
    ├── context.rs          # KanbanContext - I/O primitives only
    ├── executor.rs         # Thin orchestration only
    ├── error.rs
    ├── types/
    │   ├── mod.rs
    │   ├── ids.rs          # TaskId, ColumnId, etc.
    │   ├── board.rs        # Board, Column, Swimlane, Actor
    │   ├── task.rs         # Task, Subtask, Attachment
    │   ├── operation.rs    # Operation, Verb, Noun
    │   └── log.rs          # LogEntry, OperationResult
    │
    ├── board/
    │   ├── mod.rs
    │   ├── init.rs         # InitBoard
    │   ├── get.rs          # GetBoard
    │   └── update.rs       # UpdateBoard
    │
    ├── task/
    │   ├── mod.rs
    │   ├── add.rs          # AddTask
    │   ├── get.rs          # GetTask
    │   ├── update.rs       # UpdateTask
    │   ├── move.rs         # MoveTask (note: `move` is keyword, use move_.rs or mv.rs)
    │   ├── delete.rs       # DeleteTask
    │   ├── next.rs         # NextTask
    │   └── list.rs         # ListTasks
    │
    ├── column/
    │   ├── mod.rs
    │   ├── add.rs          # AddColumn
    │   ├── get.rs          # GetColumn
    │   ├── update.rs       # UpdateColumn
    │   ├── delete.rs       # DeleteColumn
    │   └── list.rs         # ListColumns
    │
    ├── swimlane/
    │   ├── mod.rs
    │   ├── add.rs          # AddSwimlane
    │   ├── get.rs          # GetSwimlane
    │   ├── update.rs       # UpdateSwimlane
    │   ├── delete.rs       # DeleteSwimlane
    │   └── list.rs         # ListSwimlanes
    │
    ├── actor/
    │   ├── mod.rs
    │   ├── add.rs          # AddActor
    │   ├── get.rs          # GetActor
    │   ├── update.rs       # UpdateActor
    │   ├── delete.rs       # DeleteActor
    │   └── list.rs         # ListActors
    │
    ├── tag/
    │   ├── mod.rs
    │   ├── add.rs          # AddTag
    │   ├── get.rs          # GetTag
    │   ├── update.rs       # UpdateTag
    │   ├── delete.rs       # DeleteTag
    │   └── list.rs         # ListTags
    │
    ├── comment/
    │   ├── mod.rs
    │   ├── add.rs          # AddComment
    │   ├── get.rs          # GetComment
    │   ├── update.rs       # UpdateComment
    │   ├── delete.rs       # DeleteComment
    │   └── list.rs         # ListComments
    │
    └── activity/
        ├── mod.rs
        └── list.rs         # ListActivity
```

Each `<noun>/<verb>.rs` file contains:
- One struct (e.g., `AddTask`)
- Its `Command` impl
- Its `from_params` constructor
- Any validation logic specific to that operation

Example: `src/task/add.rs`:

```rust
use crate::{Command, KanbanContext, KanbanError};
use crate::types::{Task, TaskId, Position, ActorId};

pub struct AddTask {
    pub title: String,
    pub description: Option<String>,
    pub position: Option<Position>,  // None = first column, no swimlane, end
    pub tags: Vec<String>,
    pub assignees: Vec<ActorId>,
    pub depends_on: Vec<TaskId>,
}

impl AddTask {
    pub fn from_params(params: serde_json::Map<String, Value>) -> Result<Self, ParseError> {
        Ok(Self {
            title: params.get("title")
                .and_then(|v| v.as_str())
                .ok_or(ParseError::MissingField("title"))?
                .to_string(),
            description: params.get("description").and_then(|v| v.as_str()).map(String::from),
            column: params.get("column").and_then(|v| v.as_str()).map(|s| ColumnId(s.into())),
            // ... etc
        })
    }
}

#[async_trait]
impl Command for AddTask {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value, KanbanError> {
        // All business logic for "add task" lives here
        // ...
    }

    fn op_string(&self) -> &'static str { "add task" }
}
```

### Dependencies

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
ulid = "1"
thiserror = "1"
notify = "6"           # Filesystem watching for events
```
