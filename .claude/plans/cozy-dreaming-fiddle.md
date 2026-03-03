# Convert Entity Storage to YAML/Markdown + Add Per-Entity JSONL Logs

## Context

Three changes building on the previous `#[serde(skip)]` + tag rename work:

1. **Fix test_tag_counts test properly**: Capture actual tag ULIDs from `AddTag` results instead of matching on `"bug"`.

2. **Convert primary storage formats**:
   - **Tasks → `.md`** with YAML frontmatter (description = markdown body, metadata = frontmatter)
   - **All other entities → `.yaml`** (tags, columns, swimlanes, actors, board)
   - JSONL log files stay as-is

3. **Add per-entity JSONL logs for ALL entity types**: Currently only tasks have per-entity `.jsonl` logs.

---

## Part 1: Fix test_tag_counts

**File**: `swissarmyhammer-kanban/src/board/get.rs` (test at ~line 424)

Capture tag IDs from `AddTag` results and use them for assertions:

```rust
let bug_result = AddTag::new("bug")
    .with_color("d73a4a")
    .with_description("Something isn't working")
    .execute(&ctx).await.into_result().unwrap();
let bug_id = bug_result["id"].as_str().unwrap().to_string();

let feature_result = AddTag::new("feature")
    .with_color("a2eeef")
    .execute(&ctx).await.into_result().unwrap();
let feature_id = feature_result["id"].as_str().unwrap().to_string();

// ... later ...
let bug_tag = tags.iter().find(|t| t["id"].as_str() == Some(&*bug_id)).unwrap();
let feature_tag = tags.iter().find(|t| t["id"].as_str() == Some(&*feature_id)).unwrap();
```

Also fix `InitBoard::execute` — it serializes `Board::default_columns()` via `serde_json::to_value` which now omits `id` due to `#[serde(skip)]`. Inject column IDs into the init response the same way other operations do.

---

## Part 2: Convert Primary Storage Formats

### Dependency

**File**: `swissarmyhammer-kanban/Cargo.toml`

Add `serde_yaml = { workspace = true }` (already available in workspace at version 0.9).

### Task Storage: `.json` → `.md` with YAML frontmatter

Tasks get a special format: **YAML frontmatter + markdown body**. The `description` field becomes the markdown body (everything after the closing `---`). All other metadata goes in the YAML frontmatter.

Example task file `tasks/{ulid}.md`:
```markdown
---
title: Fix the login bug
position:
  column: doing
  ordinal: a0
depends_on:
  - 01HWXYZ123
assignees:
  - alice
comments:
  - id: 01HWX456
    body: "This needs review"
    author: alice
attachments: []
---
The login page has a bug where the session token expires too quickly.

## Checklist
- [ ] Investigate token lifetime
- [x] Add refresh token logic

#bug #high-priority
```

#### Implementation in context.rs

**Path method**:
```rust
pub fn task_path(&self, id: &TaskId) -> PathBuf {
    self.root.join("tasks").join(format!("{}.md", id))
}
```

**Helper struct** — a `TaskMeta` that has all Task fields except `description` and `id`. Used only for frontmatter serialization:

```rust
/// Helper for YAML frontmatter serialization (everything except description and id)
#[derive(Serialize, Deserialize)]
struct TaskMeta {
    pub title: String,
    #[serde(default, skip_serializing)]
    _legacy_tags: Vec<String>,
    pub position: Position,
    #[serde(default)]
    pub depends_on: Vec<TaskId>,
    #[serde(default)]
    pub assignees: Vec<ActorId>,
    #[serde(default)]
    pub comments: Vec<Comment>,
    #[serde(default, skip_serializing, rename = "subtasks")]
    _legacy_subtasks: Vec<serde_json::Value>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}
```

**write_task**:
```rust
pub async fn write_task(&self, task: &Task) -> Result<()> {
    let path = self.task_path(&task.id);
    let meta = TaskMeta::from(task);  // copies all fields except description/id
    let frontmatter = serde_yaml::to_string(&meta)?;
    let content = format!("---\n{}---\n{}", frontmatter, task.description);
    atomic_write(&path, content.as_bytes()).await
}
```

**read_task**:
```rust
pub async fn read_task(&self, id: &TaskId) -> Result<Task> {
    let md_path = self.task_path(id);  // .md
    let path = if md_path.exists() {
        md_path
    } else {
        // Fall back to legacy .json
        let json_path = self.root.join("tasks").join(format!("{}.json", id));
        if !json_path.exists() {
            return Err(KanbanError::TaskNotFound { id: id.to_string() });
        }
        json_path
    };

    let content = fs::read_to_string(&path).await?;

    let mut task = if path.extension().and_then(|s| s.to_str()) == Some("md") {
        parse_task_markdown(&content)?
    } else {
        // Legacy JSON
        serde_json::from_str(&content)?
    };

    task.id = id.clone();

    // Migrate legacy subtasks
    if task.migrate_legacy_subtasks() {
        self.write_task(&task).await?;
    }

    Ok(task)
}
```

**parse_task_markdown helper**:
```rust
fn parse_task_markdown(content: &str) -> Result<Task> {
    // Split on "---" delimiters
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    // parts[0] = "" (before first ---), parts[1] = frontmatter, parts[2] = body
    if parts.len() < 3 {
        return Err(KanbanError::ParseError { ... });
    }
    let frontmatter = parts[1].trim();
    let body = parts[2].strip_prefix('\n').unwrap_or(parts[2]);

    let meta: TaskMeta = serde_yaml::from_str(frontmatter)?;
    Ok(Task {
        id: TaskId::default(),  // set by caller
        title: meta.title,
        description: body.to_string(),
        _legacy_tags: vec![],
        position: meta.position,
        depends_on: meta.depends_on,
        assignees: meta.assignees,
        comments: meta.comments,
        _legacy_subtasks: vec![],
        attachments: meta.attachments,
    })
}
```

**list_task_ids** — look for `.md` files (with `.json` fallback):
```rust
// Accept both .md and .json extensions
let ext = path.extension().and_then(|s| s.to_str());
if ext == Some("md") || ext == Some("json") { ... }
```

### Other Entities: `.json` → `.yaml`

All non-task entities use plain YAML.

**Path methods** — change extension:

| Method | Current | New |
|--------|---------|-----|
| `board_path()` | `board.json` | `board.yaml` |
| `actor_path()` | `{id}.json` | `{id}.yaml` |
| `tag_path()` | `{id}.json` | `{id}.yaml` |
| `column_path()` | `{id}.json` | `{id}.yaml` |
| `swimlane_path()` | `{id}.json` | `{id}.yaml` |

**Serialization** — replace `serde_json` with `serde_yaml` for entity I/O:

| Operation | Current | New |
|-----------|---------|-----|
| Read entity | `serde_json::from_str(&content)?` | `serde_yaml::from_str(&content)?` |
| Write entity | `serde_json::to_string_pretty(entity)?` | `serde_yaml::to_string(entity)?` |

Applies to: `read_board`/`write_board`, `read_actor`/`write_actor`, `read_tag`/`write_tag`, `read_column`/`write_column`, `read_swimlane`/`write_swimlane`.

**Directory listing** — change `Some("json")` to `Some("yaml")` in `list_actor_ids`, `list_tag_ids`, `list_column_ids`, `list_swimlane_ids` (4 methods). Accept both `.yaml` and `.json` for backward compat.

### Backward Compatibility

In each `read_*` method, if the new file doesn't exist, fall back to trying `.json`. `serde_yaml` can parse JSON, so the same deserializer works for both formats.

```rust
// Example pattern for non-task entities
let yaml_path = self.tag_path(id);  // .yaml
let path = if yaml_path.exists() {
    yaml_path
} else {
    let json_path = yaml_path.with_extension("json");
    if !json_path.exists() {
        return Err(KanbanError::TagNotFound { id: id.to_string() });
    }
    json_path
};
```

### Other Files

| File | Change |
|------|--------|
| `context.rs:is_initialized()` | Check for `board.yaml` OR `board.json` |
| `board/init.rs` | Write `board.yaml` |
| `lib.rs` | Update storage structure docs |
| `tests/integration_tag_storage.rs` | `format!("{}.json", tag_id)` → `format!("{}.yaml", tag_id)` |
| `context.rs` tests | Update path assertions `.json` → `.yaml` / `.md` |

### What Does NOT Change

- JSONL log files (`.jsonl`) — format and extension stay
- `serde_json` usage in operation execute methods (JSON API responses)
- CLI/MCP output format (JSON)

---

## Part 3: Add Per-Entity JSONL Logs

Currently only tasks have per-entity `.jsonl` logs. All other entity types only log to the global activity log.

### context.rs — New Path Methods

```rust
pub fn tag_log_path(&self, id: &TagId) -> PathBuf {
    self.root.join("tags").join(format!("{}.jsonl", id))
}

pub fn actor_log_path(&self, id: &ActorId) -> PathBuf {
    self.root.join("actors").join(format!("{}.jsonl", id))
}

pub fn board_log_path(&self) -> PathBuf {
    self.root.join("board.jsonl")
}
```

Column and swimlane log paths already exist (`column_log_path`, `swimlane_log_path`).

### context.rs — New Append Methods

```rust
pub async fn append_tag_log(&self, id: &TagId, entry: &LogEntry) -> Result<()> {
    self.append_log(&self.tag_log_path(id), entry).await
}

pub async fn append_actor_log(&self, id: &ActorId, entry: &LogEntry) -> Result<()> {
    self.append_log(&self.actor_log_path(id), entry).await
}

pub async fn append_column_log(&self, id: &ColumnId, entry: &LogEntry) -> Result<()> {
    self.append_log(&self.column_log_path(id), entry).await
}

pub async fn append_swimlane_log(&self, id: &SwimlaneId, entry: &LogEntry) -> Result<()> {
    self.append_log(&self.swimlane_log_path(id), entry).await
}

pub async fn append_board_log(&self, entry: &LogEntry) -> Result<()> {
    self.append_log(&self.board_log_path(), entry).await
}
```

### processor.rs — Extended write_log

Parse the noun from the operation string and write to the appropriate per-entity log:

```rust
// Per-entity logs based on noun
let noun = op_string.split_whitespace().nth(1).unwrap_or("");
let entity_id = result.get("id").and_then(|v| v.as_str());

match noun {
    "tag" => if let Some(id) = entity_id {
        ctx.append_tag_log(&TagId::from_string(id), log_entry).await?;
    },
    "column" => if let Some(id) = entity_id {
        ctx.append_column_log(&ColumnId::from_string(id), log_entry).await?;
    },
    "swimlane" => if let Some(id) = entity_id {
        ctx.append_swimlane_log(&SwimlaneId::from_string(id), log_entry).await?;
    },
    "actor" => {
        let id = entity_id.or_else(|| result.get("actor")
            .and_then(|a| a.get("id")).and_then(|v| v.as_str()));
        if let Some(id) = id {
            ctx.append_actor_log(&ActorId::from_string(id), log_entry).await?;
        }
    },
    "board" => ctx.append_board_log(log_entry).await?,
    _ => {}
}
```

### Delete methods — Clean up log files

Update `delete_tag_file` and `delete_actor_file` to also remove the corresponding `.jsonl` log file (column/swimlane already do this).

### lib.rs Documentation

```text
repo/
└── .kanban/
    ├── board.yaml          # Board metadata (YAML)
    ├── board.jsonl          # Board operation log
    ├── tasks/
    │   ├── {id}.md          # Task (YAML frontmatter + markdown body)
    │   ├── {id}.jsonl       # Per-task operation log
    ├── tags/
    │   ├── {id}.yaml        # Tag state
    │   ├── {id}.jsonl       # Per-tag operation log
    ├── columns/
    │   ├── {id}.yaml        # Column state
    │   ├── {id}.jsonl       # Per-column operation log
    ├── swimlanes/
    │   ├── {id}.yaml        # Swimlane state
    │   ├── {id}.jsonl       # Per-swimlane operation log
    ├── actors/
    │   ├── {id}.yaml        # Actor state
    │   ├── {id}.jsonl       # Per-actor operation log
    └── activity/
        └── current.jsonl    # Global operation log
```

---

## Verification

1. `cargo nextest run -p swissarmyhammer-kanban` — all 177+ tests pass
2. `cargo check -p swissarmyhammer-kanban-app` — Tauri app compiles
3. `npm run build` in `ui/` — TypeScript clean
4. Check `.kanban/tasks/*.md` — frontmatter + markdown body, description is body
5. Check `.kanban/tags/*.yaml` — YAML format, no `id` field
6. Check `.kanban/tags/*.jsonl` — log entries present after tag operations
7. Check `.kanban/board.jsonl` — log entries present after board operations
8. Existing `.json` files still readable (backward compat fallback)
