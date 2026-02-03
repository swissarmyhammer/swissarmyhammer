# Attachment Operations

## Status: Not Implemented

## Problem

Tasks have `attachments: Vec<Attachment>` field but no operations to manage them. Attachments allow associating files (screenshots, logs, design docs) with tasks.

## Current State

```rust
pub struct Task {
    // ...
    pub attachments: Vec<Attachment>,
}

pub struct Attachment {
    pub id: AttachmentId,
    pub name: String,
    pub path: String,
    pub mime_type: Option<String>,
    pub size: Option<u64>,
}
```

## Required Operations

### 1. Add Attachment

Attach a file reference to a task.

```rust
#[operation(verb = "add", noun = "attachment")]
pub struct AddAttachment {
    pub task_id: TaskId,
    pub name: String,
    pub path: String,
    pub mime_type: Option<String>,
    pub size: Option<u64>,
}
```

**Usage:**
```json
{
  "op": "add attachment",
  "task_id": "01ABC...",
  "name": "screenshot.png",
  "path": "./docs/screenshots/login-error.png",
  "mime_type": "image/png",
  "size": 45123
}
```

**Returns:**
```json
{
  "attachment": {
    "id": "01DEF...",
    "name": "screenshot.png",
    "path": "./docs/screenshots/login-error.png",
    "mime_type": "image/png",
    "size": 45123
  },
  "task_id": "01ABC..."
}
```

### 2. Update Attachment

Update attachment metadata (name, mime type).

```rust
#[operation(verb = "update", noun = "attachment")]
pub struct UpdateAttachment {
    pub task_id: TaskId,
    pub id: AttachmentId,
    pub name: Option<String>,
    pub mime_type: Option<String>,
    pub size: Option<u64>,
}
```

**Note**: Path is intentionally NOT updatable. If the file changes, delete and add a new attachment.

### 3. Delete Attachment

Remove an attachment from a task.

```rust
#[operation(verb = "delete", noun = "attachment")]
pub struct DeleteAttachment {
    pub task_id: TaskId,
    pub id: AttachmentId,
}
```

**Usage:**
```json
{
  "op": "delete attachment",
  "task_id": "01ABC...",
  "id": "01DEF..."
}
```

**Returns:**
```json
{
  "deleted": true,
  "attachment_id": "01DEF...",
  "task_id": "01ABC..."
}
```

### 4. Get Attachment

Retrieve a specific attachment.

```rust
#[operation(verb = "get", noun = "attachment")]
pub struct GetAttachment {
    pub task_id: TaskId,
    pub id: AttachmentId,
}
```

### 5. List Attachments

List all attachments for a task.

```rust
#[operation(verb = "list", noun = "attachments")]
pub struct ListAttachments {
    pub task_id: TaskId,
}
```

**Returns:**
```json
{
  "attachments": [
    {
      "id": "01DEF...",
      "name": "screenshot.png",
      "path": "./docs/screenshots/login-error.png",
      "mime_type": "image/png",
      "size": 45123
    }
  ],
  "count": 1,
  "task_id": "01ABC..."
}
```

## Verb+Noun Matrix Update

Add new valid operations:
- `(Verb::Add, Noun::Attachment)`
- `(Verb::Get, Noun::Attachment)`
- `(Verb::Update, Noun::Attachment)`
- `(Verb::Delete, Noun::Attachment)`
- `(Verb::List, Noun::Attachments)`

Add `Attachment` and `Attachments` to the `Noun` enum.

## Total Operations

Current: 40 operations
After attachments: 45 operations

## File Structure

Create in `swissarmyhammer-kanban/src/attachment/`:
- `mod.rs` - Module declaration
- `add.rs` - AddAttachment command
- `get.rs` - GetAttachment command
- `update.rs` - UpdateAttachment command
- `delete.rs` - DeleteAttachment command
- `list.rs` - ListAttachments command

## Design Considerations

### Path Storage

Attachments store **path references**, not actual files. This keeps the kanban engine lightweight:
- The path can be relative to repo root
- The actual file lives in the repo (e.g., `docs/`, `screenshots/`)
- Good for git commits (kanban + attachments committed together)

Example:
```
repo/
├── .kanban/
│   └── tasks/
│       └── 01ABC.json  (references "./docs/design.pdf")
└── docs/
    └── design.pdf  (actual file)
```

### MIME Type Detection

Should the tool auto-detect MIME type from file extension?

**Recommendation**: Make it optional. Agent can provide it, or tool can detect it:

```rust
impl AddAttachment {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mime_type = match &self.mime_type {
            Some(m) => Some(m.clone()),
            None => detect_mime_type(&self.path), // Auto-detect from extension
        };
        // ...
    }
}
```

### File Size

Similarly, auto-stat the file to get size if not provided:

```rust
let size = match self.size {
    Some(s) => Some(s),
    None => get_file_size(&self.path).ok(),
};
```

## Testing Requirements

- Test adding attachments to tasks
- Test updating attachment metadata
- Test deleting attachments
- Test listing attachments
- Test error cases (nonexistent task, nonexistent attachment)
- Test attachment operations trigger task-level plan notifications
- Test auto-detection of mime type and size (if implemented)

## MCP Integration

All attachment operations should trigger plan notifications since they modify task state.

## Use Cases

**For agents:**
- Attach error logs when reporting bugs
- Attach generated diagrams to design tasks
- Reference test output files
- Link to related documentation

**For users:**
- Attach screenshots of UI issues
- Reference design mockups
- Link to external documents
- Attach test data files
