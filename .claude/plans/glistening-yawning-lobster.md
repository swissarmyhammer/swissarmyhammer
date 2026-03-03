# Dynamic Field-Driven Entities

## Context

The kanban crate has hardcoded Rust structs (Task, Tag, Actor, Column, Swimlane, Board) with typed fields. The `swissarmyhammer-fields` crate was built to provide a dynamic field registry, but the entity types don't actually use it — they're still static structs. Built-in field definitions are generated from Rust code (`kanban_defaults()`) instead of YAML files.

This plan converts to truly dynamic, field-driven entities:
- Built-in field and entity definitions become YAML files loaded via VirtualFileSystem stacking (builtin → user → local)
- A new `swissarmyhammer-entity` crate provides the generic Entity type and I/O — testable end-to-end without kanban
- Entity mutations produce reversible, field-level change logs with text diffs for string values
- Kanban becomes a thin layer: maps entity types to directories, delegates to the entity crate
- Comments removed, Attachments become their own entity type
- Position becomes three separate fields
- Board becomes a dynamic entity
- UI inspector becomes field-driven: separate Presenter/Editor components per field type, all using CodeMirror for text editing

## Execution Process

Cards are executed **one at a time**. After completing each card:
1. Run **ALL** tests — no excuses, no skipping, no "pre-existing failures." If tests fail, fix them before stopping.
   - `cargo test --workspace` — all Rust tests across all crates (including kanban-app backend)
   - `cd swissarmyhammer-kanban-app/ui && npm test` — all frontend Vitest tests (React components + utilities)
2. **STOP and wait for manual user review** before starting the next card
3. Do NOT proceed to the next card until the user explicitly approves

This is a hard gate — no card begins until ALL tests (Rust + frontend) are green and the user has reviewed and approved.

---

## Card 1: Built-in field and entity YAML files

Create YAML files for all built-in definitions. These get embedded as builtins in the VFS.

**Files to create:**
- `swissarmyhammer-kanban/builtin/fields/definitions/title.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/tags.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/assignees.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/due.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/depends_on.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/body.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/tag_name.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/color.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/description.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/usage.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/last_used.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/name.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/order.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/actor_type.yaml`
- `swissarmyhammer-kanban/builtin/fields/definitions/position_column.yaml` (NEW — reference to column)
- `swissarmyhammer-kanban/builtin/fields/definitions/position_swimlane.yaml` (NEW — reference to swimlane)
- `swissarmyhammer-kanban/builtin/fields/definitions/position_ordinal.yaml` (NEW — text, fractional index)
- `swissarmyhammer-kanban/builtin/fields/definitions/attachment_name.yaml` (NEW)
- `swissarmyhammer-kanban/builtin/fields/definitions/attachment_path.yaml` (NEW)
- `swissarmyhammer-kanban/builtin/fields/definitions/attachment_mime_type.yaml` (NEW)
- `swissarmyhammer-kanban/builtin/fields/definitions/attachment_size.yaml` (NEW)
- `swissarmyhammer-kanban/builtin/fields/definitions/attachment_task.yaml` (NEW — reference to task)
- `swissarmyhammer-kanban/builtin/fields/definitions/attachments.yaml` (NEW — reference to attachment, multiple)
- `swissarmyhammer-kanban/builtin/fields/definitions/progress.yaml` (NEW — computed, derives from GFM task lists in body)
- `swissarmyhammer-kanban/builtin/fields/entities/task.yaml`
- `swissarmyhammer-kanban/builtin/fields/entities/tag.yaml`
- `swissarmyhammer-kanban/builtin/fields/entities/actor.yaml`
- `swissarmyhammer-kanban/builtin/fields/entities/column.yaml`
- `swissarmyhammer-kanban/builtin/fields/entities/swimlane.yaml`
- `swissarmyhammer-kanban/builtin/fields/entities/board.yaml` (NEW)
- `swissarmyhammer-kanban/builtin/fields/entities/attachment.yaml` (NEW)

Subtasks:
- [ ] Create all field definition YAML files (one per field, matching FieldDef schema)
- [ ] Create all entity definition YAML files (one per entity type, matching EntityDef schema)
- [ ] Update task entity: add position_column, position_swimlane, position_ordinal, attachments, progress fields; remove comments
- [ ] Add board entity: fields = [name, description]
- [ ] Add attachment entity: fields = [attachment_name, attachment_path, attachment_mime_type, attachment_size, attachment_task]
- [ ] Verify YAML files parse correctly with existing FieldDef/EntityDef serde
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 2**

## Card 2: VFS-based field loading in FieldsContext

Replace the Rust-code `with_defaults()` pattern with VirtualFileSystem stacking.

**Files to modify:**
- `swissarmyhammer-fields/Cargo.toml` — add swissarmyhammer-directory dep
- `swissarmyhammer-fields/src/context.rs` — new `open_with_vfs()` or rework `open()`
- `swissarmyhammer-kanban/src/defaults.rs` — replace `kanban_defaults()` with builtin YAML loading
- `swissarmyhammer-kanban/src/context.rs` — update `open()` to use VFS

Stacking layers:
1. **Builtin**: YAML files from `builtin/fields/` embedded via `vfs.add_builtin()`
2. **User**: `~/.kanban/fields/` (VFS user layer via `add_search_path`)
3. **Local**: `.kanban/fields/` (VFS local layer via `add_search_path`)

Subtasks:
- [ ] Add swissarmyhammer-directory dependency to swissarmyhammer-fields
- [ ] Create `FieldsContext::open_vfs()` that accepts a VFS with pre-loaded definitions and entities
- [ ] In swissarmyhammer-kanban: load builtin YAML files into VFS, add `.kanban/fields/` as local search path
- [ ] Parse VFS file entries into FieldDef/EntityDef, build in-memory indexes
- [ ] Override matching: same filename = local overrides builtin (VFS handles this automatically)
- [ ] Delete `kanban_defaults()` Rust code and `FieldDefaults` builder
- [ ] Tests: builtin loads, local override replaces builtin, user layer works
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 3**

## Card 3: Computed field derivation engine with native Rust function registry

The `derive` property on computed fields (`kind: computed, derive: "parse-body-tags"`) needs a runtime engine. Derivation functions are native Rust functions registered by name — not JS. The consumer (kanban) registers its derivations at startup.

**Files to modify/create:**
- `swissarmyhammer-fields/src/compute.rs` — new module: derivation engine
- `swissarmyhammer-fields/src/lib.rs` — add module, exports

```rust
/// A native derivation function.
/// Receives the entity's fields and an EntityLookup, returns the derived value.
type DeriveFn = Box<dyn Fn(&HashMap<String, serde_json::Value>, &dyn EntityLookup) -> Pin<Box<dyn Future<Output = serde_json::Value> + Send>> + Send + Sync>;

pub struct ComputeEngine {
    derivations: HashMap<String, DeriveFn>,
    lookup: Option<Box<dyn EntityLookup>>,
}
```

Key methods:
- `register(&mut self, name: &str, f: DeriveFn)` — register a native derivation by name
- `derive(&self, field: &FieldDef, entity_fields: &HashMap<String, Value>) -> Result<Value>` — run derivation if field is computed, error if derive name not registered
- `derive_all(&self, entity_fields: &mut HashMap<String, Value>, field_defs: &[FieldDef])` — compute all computed fields on an entity

Kanban registers its derivations:
- `parse-body-tags` — extracts `#tag` patterns from the body field (the current `tag_parser::parse_tags` logic)
- `parse-body-progress` — parses GFM task lists (`- [ ]` / `- [x]`) from body, returns `{ total, completed, percent }`. This replaces the frontend-only progress computation currently in `SubtaskProgress`.
- `tag-usage-count` — counts how many entities reference a tag
- `tag-last-used` — finds most recent entity referencing a tag

Tag mutation (append_tag, remove_tag, rename_tag) stays as command-level operations on the body field — they're not part of the compute system.

Subtasks:
- [ ] Create compute.rs with ComputeEngine struct and DeriveFn type
- [ ] Implement register() and derive() methods
- [ ] Implement derive_all() for computing all computed fields on an entity
- [ ] Move tag_parser::parse_tags into a derivation function registered as "parse-body-tags"
- [ ] Implement "parse-body-progress" derivation — parses `- [ ]` / `- [x]` from body, returns `{ total, completed, percent }`
- [ ] Register tag-usage-count and tag-last-used as stub derivations (actual implementation in kanban)
- [ ] Error when a computed field references an unregistered derive name
- [ ] Tests: register derivation, derive value, unregistered derive errors
- [ ] tag_parser.rs stays in kanban for append/remove/rename (body mutation), but parse_tags moves to a registered derivation
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 4**

## Card 4: swissarmyhammer-entity crate

Create a new standalone crate `swissarmyhammer-entity` for the dynamic Entity type. Like `swissarmyhammer-fields`, this is consumer-agnostic — it knows nothing about kanban, tasks, or tags.

**Files to create:**
- `swissarmyhammer-entity/Cargo.toml`
- `swissarmyhammer-entity/src/lib.rs`
- `swissarmyhammer-entity/src/entity.rs` — Entity type
- `swissarmyhammer-entity/src/io.rs` — generic read/write (frontmatter+body vs plain YAML)
- `swissarmyhammer-entity/src/error.rs`
- `Cargo.toml` (workspace) — add member

Dependencies: serde, serde_json, serde_yaml, tokio, swissarmyhammer-fields (for EntityDef/FieldsContext)

```rust
pub struct Entity {
    pub entity_type: String,         // "task", "tag", "actor", etc.
    pub id: String,                  // ULID or slug
    pub fields: HashMap<String, serde_json::Value>,  // field_name → value
}
```

Key methods:
- `get(&self, field: &str) -> Option<&Value>` — field accessor
- `get_str(&self, field: &str) -> Option<&str>` — string convenience
- `get_string_list(&self, field: &str) -> Vec<String>` — for reference arrays
- `set(&mut self, field: &str, value: Value)` — field setter
- `remove(&mut self, field: &str)` — remove field
- `to_json(&self) -> Value` — serialize with id and entity_type injected

Generic I/O (in `io.rs`):
- `read_entity(path, entity_def) -> Result<Entity>` — parses frontmatter+body or plain YAML based on EntityDef.body_field
- `write_entity(path, entity, entity_def) -> Result<()>` — writes in correct format, atomic via temp+rename
- `read_entity_dir(dir, entity_type, entity_def) -> Result<Vec<Entity>>` — scans directory
- `delete_entity_files(path) -> Result<()>` — removes data file + optional log file

The I/O module takes paths and EntityDefs — it doesn't know about `.kanban/` or any specific directory layout. The consumer (kanban) maps entity types to directories.

Subtasks:
- [ ] Scaffold crate: Cargo.toml, lib.rs, error.rs
- [ ] Define Entity struct with entity_type, id, fields HashMap
- [ ] Implement field accessor methods (get, get_str, get_i64, get_f64, get_bool, get_string_list)
- [ ] Implement set/remove mutators
- [ ] Implement to_json() serialization (includes id and entity_type)
- [ ] Implement read_entity() — handles frontmatter+body vs plain YAML
- [ ] Implement write_entity() — handles both formats, atomic write
- [ ] Implement read_entity_dir() — scans for .md/.yaml files
- [ ] Implement delete_entity_files()
- [ ] Tests for all accessors, serialization, and I/O round-trips
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 5**

## Card 5: Entity change logging with field-level diffs

Add reversible, field-level change tracking to `swissarmyhammer-entity`. Every mutation to an entity produces a `ChangeEntry` recording exactly which fields changed and how. String fields record a text diff (via `similar` crate) instead of storing full old/new values. Changes are reversible — each change kind has a natural inverse.

**Files to create/modify:**
- `swissarmyhammer-entity/Cargo.toml` — add `similar` dependency
- `swissarmyhammer-entity/src/changelog.rs` — new module: ChangeEntry, FieldChange, diff/apply/reverse
- `swissarmyhammer-entity/src/lib.rs` — add module, exports

```rust
/// What happened to a single field
pub enum FieldChange {
    /// Field was added (entity creation or new field)
    Set { value: Value },
    /// Field was removed
    Removed { old_value: Value },
    /// Non-string field changed — record old and new values
    Changed { old_value: Value, new_value: Value },
    /// String field changed — record a unified text diff
    TextDiff { diff: String },
}

/// A single change event for an entity
pub struct ChangeEntry {
    pub id: String,                        // ULID
    pub timestamp: DateTime<Utc>,
    pub op: String,                        // "create", "update", "delete"
    pub actor: Option<String>,
    pub changes: Vec<(String, FieldChange)>,  // (field_name, change)
}
```

**Reversibility rules:**
- `Set { value }` → reverse is `Removed { old_value: value }`
- `Removed { old_value }` → reverse is `Set { value: old_value }`
- `Changed { old, new }` → reverse is `Changed { old: new, new: old }`
- `TextDiff { diff }` → reverse by inverting the unified diff (swap +/- lines)

**Key functions:**
- `diff_entities(old: &Entity, new: &Entity) -> Vec<(String, FieldChange)>` — compare two entity snapshots, produce field-level changes. String values get text diffs via `similar`, others get Changed with old/new.
- `reverse_changes(changes: &[(String, FieldChange)]) -> Vec<(String, FieldChange)>` — invert each change for undo
- `apply_changes(entity: &mut Entity, changes: &[(String, FieldChange)]) -> Result<()>` — apply changes forward (or reversed changes for undo)
- `append_changelog(path: &Path, entry: &ChangeEntry) -> Result<()>` — append JSONL line to entity's log file
- `read_changelog(path: &Path) -> Result<Vec<ChangeEntry>>` — read all entries from log

**Integration with Entity I/O:**
- `write_entity()` gains an optional `old: Option<&Entity>` parameter. When provided, it computes the diff and appends to the changelog automatically.
- On entity creation: all fields are `Set` changes
- On entity deletion: all fields are `Removed` changes
- The changelog file lives alongside the entity file: `{id}.jsonl` next to `{id}.md` or `{id}.yaml`

**Diff format for strings:**
Uses `similar::TextDiff` to produce unified diff format. The diff is stored as a string in the JSONL entry. For reversal, the +/- lines are swapped. This keeps log entries compact for large text fields (body, description) — only the changed lines are stored, not the full before/after.

Subtasks:
- [ ] Add `similar` and `chrono` dependencies to swissarmyhammer-entity
- [ ] Define FieldChange enum and ChangeEntry struct with serde serialization
- [ ] Implement diff_entities() — compare two entities, produce field-level changes with text diffs for strings
- [ ] Implement reverse_changes() — invert each FieldChange
- [ ] Implement apply_changes() — apply forward or reversed changes to an entity
- [ ] Implement append_changelog() and read_changelog() for JSONL I/O
- [ ] Integrate into write_entity() — optionally diff against previous state and log
- [ ] Tests: diff two entities, reverse the diff, apply reversed diff gets back to original
- [ ] Tests: string field produces text diff not full old/new, round-trip through JSONL
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 6**

## Card 6: Wire entity I/O into KanbanContext

KanbanContext becomes a thin layer that maps entity types to directories and delegates to `swissarmyhammer-entity` for I/O.

**Files to modify:**
- `swissarmyhammer-kanban/Cargo.toml` — add swissarmyhammer-entity dep
- `swissarmyhammer-kanban/src/context.rs` — add generic methods that delegate to entity crate, keep typed methods temporarily

New generic methods on KanbanContext:
- `read_entity(&self, entity_type: &str, id: &str) -> Result<Entity>` — resolves dir, delegates to entity crate
- `write_entity(&self, entity: &Entity) -> Result<()>` — resolves dir, delegates
- `delete_entity(&self, entity_type: &str, id: &str) -> Result<()>` — resolves dir, delegates
- `list_entities(&self, entity_type: &str) -> Result<Vec<Entity>>` — resolves dir, delegates
- `entity_dir(&self, entity_type: &str) -> PathBuf` — maps type → `.kanban/{type}s/` (tasks/, tags/, actors/, columns/, swimlanes/, attachments/)
- `entity_path(&self, entity_type: &str, id: &str) -> PathBuf` — includes extension (.md or .yaml based on EntityDef)

These methods look up the EntityDef from FieldsContext, resolve the storage path, and call the generic `swissarmyhammer_entity::io` functions. Change logging is automatic — `write_entity()` reads the previous state, diffs, and appends to the entity's changelog. This replaces the old per-type `append_task_log`, `append_tag_log`, etc.

Subtasks:
- [ ] Add swissarmyhammer-entity dependency
- [ ] Implement `entity_dir()` — maps entity_type to directory under .kanban/
- [ ] Implement `read_entity()`, `write_entity()`, `delete_entity()`, `list_entities()` delegating to entity crate
- [ ] `write_entity()` passes previous state for automatic field-level changelog
- [ ] Replace per-type `append_*_log()` methods with entity changelog (old LogEntry-based logs become the global activity log only)
- [ ] Handle legacy .json fallback during migration period
- [ ] Handle board as single-instance (id = "board", stored at `.kanban/board.yaml`)
- [ ] Tests: round-trip for task (body_field), round-trip for tag (plain YAML), list entities
- [ ] Tests: verify changelog entries written on create/update/delete
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 7**

## Card 7: Migrate Task to dynamic entity

Convert task commands from `Task` struct to `Entity`. Position becomes field values. Comments removed. Attachments become reference field. This is the largest migration — tasks have the most commands.

**Files to modify:**
- `swissarmyhammer-kanban/src/task/add.rs`
- `swissarmyhammer-kanban/src/task/get.rs`
- `swissarmyhammer-kanban/src/task/update.rs`
- `swissarmyhammer-kanban/src/task/delete.rs`
- `swissarmyhammer-kanban/src/task/list.rs`
- `swissarmyhammer-kanban/src/task/mv.rs`
- `swissarmyhammer-kanban/src/task/next.rs`
- `swissarmyhammer-kanban/src/task/complete.rs`
- `swissarmyhammer-kanban/src/task/assign.rs`
- `swissarmyhammer-kanban/src/task/unassign.rs`
- `swissarmyhammer-kanban/src/task/tag.rs`
- `swissarmyhammer-kanban/src/task/untag.rs`
- `swissarmyhammer-kanban/src/comment/` — DELETE entire module
- `swissarmyhammer-kanban/src/types/task.rs` — remove Task, Comment, Attachment structs

Key changes:
- `AddTask` creates Entity with fields: title, position_column, position_swimlane, position_ordinal, body
- `GetTask` returns Entity serialized to JSON (tags computed from body)
- `MoveTask` updates position_column, position_swimlane, position_ordinal fields
- `CompleteTask` moves to terminal column (updates position_column)
- `AssignTask/UnassignTask` modifies assignees reference field
- `TagTask/UntagTask` modifies body text (tags are computed from #patterns)
- `NextTask` reads list, checks depends_on, finds first ready

Subtasks:
- [ ] Update AddTask to create Entity with field values
- [ ] Update GetTask to read Entity, compute tags from body, return JSON
- [ ] Update UpdateTask to modify Entity fields
- [ ] Update DeleteTask to use delete_entity()
- [ ] Update ListTask to use list_entities()
- [ ] Update MoveTask to set position_column/swimlane/ordinal fields
- [ ] Update CompleteTask to set position_column to terminal column
- [ ] Update NextTask to filter by position_column and depends_on
- [ ] Update AssignTask/UnassignTask to modify assignees field
- [ ] Update TagTask/UntagTask (body text modification stays the same)
- [ ] Delete comment module entirely
- [ ] Remove Task, Comment, Attachment structs from types
- [ ] Update kanban-app/src/commands.rs — all task-related Tauri IPC handlers must work with Entity (not Task struct)
- [ ] Keep tag_parser.rs (still needed for body mutation)
- [ ] Tests for each command
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 8**

## Card 8: Migrate Column to dynamic entity

Convert column commands from `Column` struct to `Entity`.

**Files to modify:**
- `swissarmyhammer-kanban/src/column/add.rs`
- `swissarmyhammer-kanban/src/column/get.rs`
- `swissarmyhammer-kanban/src/column/update.rs`
- `swissarmyhammer-kanban/src/column/delete.rs`
- `swissarmyhammer-kanban/src/column/list.rs`

Subtasks:
- [ ] Migrate add, get, update, delete, list commands to use Entity
- [ ] Remove Column struct from types
- [ ] Update kanban-app/src/commands.rs — column-related Tauri IPC handlers must work with Entity
- [ ] Update any task commands that reference Column type directly
- [ ] Tests for each command
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 9**

## Card 9: Migrate Swimlane to dynamic entity

Convert swimlane commands from `Swimlane` struct to `Entity`.

**Files to modify:**
- `swissarmyhammer-kanban/src/swimlane/add.rs`
- `swissarmyhammer-kanban/src/swimlane/get.rs`
- `swissarmyhammer-kanban/src/swimlane/update.rs`
- `swissarmyhammer-kanban/src/swimlane/delete.rs`
- `swissarmyhammer-kanban/src/swimlane/list.rs`

Subtasks:
- [ ] Migrate add, get, update, delete, list commands to use Entity
- [ ] Remove Swimlane struct from types
- [ ] Update kanban-app/src/commands.rs — swimlane-related Tauri IPC handlers must work with Entity
- [ ] Tests for each command
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 10**

## Card 10: Migrate Tag to dynamic entity

Convert tag commands from `Tag` struct to `Entity`.

**Files to modify:**
- `swissarmyhammer-kanban/src/tag/add.rs`
- `swissarmyhammer-kanban/src/tag/get.rs`
- `swissarmyhammer-kanban/src/tag/update.rs`
- `swissarmyhammer-kanban/src/tag/delete.rs`
- `swissarmyhammer-kanban/src/tag/list.rs`

Subtasks:
- [ ] Migrate add, get, update, delete, list commands to use Entity
- [ ] Remove Tag struct from types
- [ ] Update kanban-app/src/commands.rs — tag-related Tauri IPC handlers must work with Entity
- [ ] Tests for each command
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 11**

## Card 11: Migrate Actor to dynamic entity

Convert actor commands from `Actor` struct to `Entity`. `actor_type` becomes a field value instead of an enum variant.

**Files to modify:**
- `swissarmyhammer-kanban/src/actor/add.rs`
- `swissarmyhammer-kanban/src/actor/get.rs`
- `swissarmyhammer-kanban/src/actor/update.rs`
- `swissarmyhammer-kanban/src/actor/delete.rs`
- `swissarmyhammer-kanban/src/actor/list.rs`

Subtasks:
- [ ] Migrate add, get, update, delete, list commands to use Entity
- [ ] actor_type is a field value (string), not a Rust enum
- [ ] Remove Actor struct from types
- [ ] Update kanban-app/src/commands.rs — actor-related Tauri IPC handlers must work with Entity
- [ ] Tests for each command
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 12**

## Card 12: Migrate Board to dynamic entity

Convert board commands to use Entity. Board becomes a single-instance entity (id = "board") stored at `.kanban/board.yaml`.

**Files to modify:**
- `swissarmyhammer-kanban/src/board/init.rs`
- `swissarmyhammer-kanban/src/board/get.rs`
- `swissarmyhammer-kanban/src/board/update.rs`

Subtasks:
- [ ] InitBoard creates a board Entity with fields: name, description
- [ ] GetBoard reads the board Entity but still assembles composite view (name, description + columns, swimlanes, tags from entity queries) — frontend API unchanged
- [ ] UpdateBoard modifies board Entity fields
- [ ] Remove Board struct from types
- [ ] Update kanban-app/src/commands.rs — board-related Tauri IPC handlers must work with Entity
- [ ] Tests for each command
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 13**

## Card 13: Migrate Attachment to standalone entity

Rewrite attachment commands as entity CRUD. Attachments become their own entity type stored in `.kanban/attachments/{id}.yaml`, referenced by task via attachments field.

**Files to modify:**
- `swissarmyhammer-kanban/src/attachment/*.rs` — rewrite as entity commands

Subtasks:
- [ ] Rewrite add, get, delete, list as entity CRUD operations
- [ ] Remove old Attachment struct from types
- [ ] Update task commands that reference attachments to use entity ID references
- [ ] Tests for each command
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 14**

---

## Card 14: Entity Inspector infrastructure

Backend IPC + TypeScript types + EntityInspector shell component. This card builds the framework; individual field type presenters/editors are separate cards.

**Design principle:** Every field type has separate **Presenter** (read-only display) and **Editor** (interactive edit) components. No combined read/write mode components. The existing `EditableMarkdown` gets split apart and its parts redistributed into the field type components.

**Backend (Rust):**
- `swissarmyhammer-kanban-app/src/commands.rs` — add `get_entity_schema` IPC command (returns FieldDefs + EntityDef for an entity type), add `update_entity_field` generic command (entity_type, id, field_name, value)

**Frontend (TypeScript/React):**
- `ui/src/types/kanban.ts` — add `FieldDef`, `EntityDef` TypeScript types matching Rust definitions; add generic `Entity` shape
- `ui/src/components/entity-inspector.tsx` — NEW: iterates field definitions, renders Presenter or Editor for each based on editing state. Manages which field is being edited (one at a time). Calls `update_entity_field` on commit.
- `ui/src/components/fields/` — NEW directory for per-type presenter/editor components (created in subsequent cards)

Subtasks:
- [ ] Add `get_entity_schema` Tauri IPC command
- [ ] Add `update_entity_field` Tauri IPC command
- [ ] Add FieldDef, EntityDef, Entity TypeScript types
- [ ] Create EntityInspector shell — iterates fields, dispatches to Presenter/Editor by kind
- [ ] Tests for EntityInspector shell (renders field list, dispatches correctly)
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 15**

## Card 15: TextField presenter/editor

For `kind: text` fields (title, name, tag_name, etc.). Single-line text.

**Components:**
- `ui/src/components/fields/text-presenter.tsx` — read-only display of text value, click to edit
- `ui/src/components/fields/text-editor.tsx` — CodeMirror single-line editor with keymap support (vim/emacs/CUA). Enter commits, Escape cancels. Reuses the keymap infrastructure from the existing `EditableMarkdown`.

CodeMirror is required for ALL text editing so keymappings are consistent across the app.

Subtasks:
- [ ] Create TextPresenter — renders text value, emits onEdit event on click
- [ ] Create TextEditor — CodeMirror single-line, vim/emacs/CUA keymaps, Enter commits, Escape cancels
- [ ] Extract shared CodeMirror keymap setup from EditableMarkdown into reusable hook/utility
- [ ] Tests for presenter rendering and editor commit/cancel behavior
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 16**

## Card 16: MarkdownField presenter/editor

For `kind: markdown` fields (body, description). Multiline markdown with rich rendering.

**Components:**
- `ui/src/components/fields/markdown-presenter.tsx` — ReactMarkdown display with GFM, tag pills, interactive checkboxes. Extracted from current `EditableMarkdown` display mode.
- `ui/src/components/fields/markdown-editor.tsx` — CodeMirror multiline with markdown language mode, tag decorations, tag autocomplete, tag tooltips, vim/emacs/CUA keymaps. Extracted from current `EditableMarkdown` edit mode.

This card splits the existing `EditableMarkdown` into its two halves. The old component can be deleted or reduced to a thin wrapper if needed during transition.

Subtasks:
- [ ] Create MarkdownPresenter — ReactMarkdown with GFM, tag pills, checkbox interactivity
- [ ] Create MarkdownEditor — CodeMirror multiline with markdown language, tag decorations/autocomplete/tooltips, keymaps
- [ ] Reuse shared keymap hook from Card 15
- [ ] Tests for presenter (renders markdown, checkboxes work) and editor (commit on blur/Escape)
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 17**

## Card 17: SelectField presenter/editor

For `kind: select` fields (actor_type, status, priority). Dropdown with options from FieldDef.

**Components:**
- `ui/src/components/fields/select-presenter.tsx` — read-only display of selected value (as pill/badge)
- `ui/src/components/fields/select-editor.tsx` — dropdown/combobox showing options from the field definition. Commits on selection.

Subtasks:
- [ ] Create SelectPresenter — renders current value as a styled pill/badge
- [ ] Create SelectEditor — dropdown populated from FieldDef.options, commits on selection
- [ ] Tests for presenter display and editor selection behavior
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 18**

## Card 18: ReferenceField presenter/editor

For `kind: reference` fields (assignees, depends_on, position_column, position_swimlane, attachment_task). Links to other entities.

**Underlying data model:** The canonical storage is a JSON array of entity identifiers (IDs or slugs) in the Entity fields HashMap. The CodeMirror editor converts to/from whitespace-separated text for editing — join on load, split on commit.

**Components:**
- `ui/src/components/fields/reference-presenter.tsx` — read-only display of referenced entities as pills/badges. Prefers showing entity **name** (fetched from the target entity type), falls back to raw **ID** if name unavailable. Handles single and multiple references.
- `ui/src/components/fields/reference-editor.tsx` — **CodeMirror** editor (for keymap consistency) editing the whitespace-separated identifier list, with:
  - **Decorations:** Replace raw IDs with styled pills showing the entity's name (like tag decorations in the markdown editor). Falls back to showing the raw ID if the entity can't be resolved.
  - **Autocomplete:** CodeMirror autocomplete extension, triggered on typing, suggesting available entities of the target type. Shows entity name in the completion list, inserts the ID.
  - **Validation on commit:** When the editor commits, parse the whitespace-separated tokens, validate each against known entities of the target type. Remove any tokens that don't resolve to a valid entity (dangling reference cleanup). Warn or silently clean up partial deletions.
  - **Smart whitespace handling:** Tokens are space-separated. Typing a space after an autocomplete selection starts a new token.
- `ui/src/lib/cm-reference-decorations.ts` — CodeMirror extension: ViewPlugin that replaces recognized entity IDs with name pills (similar pattern to `cm-tag-decorations.ts`)
- `ui/src/lib/cm-reference-autocomplete.ts` — CodeMirror autocomplete source for entity references

Subtasks:
- [ ] Create ReferencePresenter — renders entity references as name pills (fetch names from entity list), fall back to ID
- [ ] Create ReferenceEditor — CodeMirror editing whitespace-separated ID list
- [ ] Implement cm-reference-decorations — ViewPlugin that replaces ID tokens with entity name pills (reuse patterns from cm-tag-decorations)
- [ ] Implement cm-reference-autocomplete — autocomplete source from target entity type list, show name, insert ID
- [ ] Implement validation on commit — parse tokens, strip invalid/dangling references
- [ ] Handle both single-value and array references based on FieldDef
- [ ] Tests for presenter display, editor decorations, autocomplete, and dangling reference cleanup
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 19**

## Card 19: ComputedField presenter

For `kind: computed` fields (tags, progress, usage, last_used). Read-only — no editor needed.

**Components:**
- `ui/src/components/fields/computed-presenter.tsx` — read-only display that varies by derive function:
  - `parse-body-tags` → tag pills
  - `parse-body-progress` → progress bar with percentage
  - Default → formatted JSON value

Subtasks:
- [ ] Create ComputedPresenter — renders computed values as read-only display
- [ ] Handle tags as pills, progress as progress bar, others as formatted text
- [ ] Tests for each computed display variant
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 20**

## Card 20: DateField presenter/editor

For `kind: date` fields (due, last_used).

**Components:**
- `ui/src/components/fields/date-presenter.tsx` — read-only display of formatted date
- `ui/src/components/fields/date-editor.tsx` — date picker input

Subtasks:
- [ ] Create DatePresenter — renders formatted date string
- [ ] Create DateEditor — date picker, commits on selection
- [ ] Tests for presenter formatting and editor selection
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 21**

## Card 21: NumberField presenter/editor

For `kind: number` fields (order, attachment_size).

**Components:**
- `ui/src/components/fields/number-presenter.tsx` — read-only display of formatted number
- `ui/src/components/fields/number-editor.tsx` — CodeMirror single-line (for keymap consistency) or numeric input, commits on Enter/blur

Subtasks:
- [ ] Create NumberPresenter — renders formatted number
- [ ] Create NumberEditor — numeric input with validation, commits on Enter/blur
- [ ] Tests for presenter display and editor validation/commit
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 22**

## Card 22: Wire EntityInspector into the app

Replace hardcoded panels with the dynamic EntityInspector using the field type components from Cards 15-21.

**Files to modify:**
- `ui/src/components/task-detail-panel.tsx` — replace hardcoded fields with EntityInspector for task entity type
- `ui/src/components/tag-inspector.tsx` — replace with EntityInspector for tag entity type
- `ui/src/components/editable-markdown.tsx` — delete or deprecate (functionality now in MarkdownPresenter/MarkdownEditor)
- `ui/src/App.tsx` — update to fetch entity schema and pass to inspector

Subtasks:
- [ ] Replace TaskDetailPanel internals with EntityInspector
- [ ] Replace TagInspector internals with EntityInspector
- [ ] Remove or deprecate old EditableMarkdown component
- [ ] Verify body_field gets MarkdownEditor, other text fields get TextEditor
- [ ] Verify computed fields (tags, progress) render correctly as read-only
- [ ] End-to-end: open a task, see all fields, edit title/body, see computed tags and progress update
- [ ] Tests for integrated inspector behavior
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 23**

## Card 23: Board view refactor — Entity-driven columns, cards, and drag-and-drop

The main board view, task cards, column views, drag-and-drop, and frontend state management all currently use typed `Task`/`Column`/`Board` TypeScript interfaces. This card converts them to work with dynamic Entity data.

**Files to modify:**
- `ui/src/App.tsx` — state management: replace `Board | null` + `Task[]` with Entity-based state. Update fetch cycle (`get_board` → entity queries), `refresh()`, `board-changed` listener.
- `ui/src/components/board-view.tsx` — column layout maps: group tasks by `position_column` field instead of `task.position.column`. Update DnD handlers to set `position_column`/`position_ordinal` fields via `update_entity_field`.
- `ui/src/components/column-view.tsx` — read column name/order from Entity fields. Replace `EditableMarkdown` for column name with `TextEditor`/`TextPresenter` from Card 15. Update `useDroppable` and task sorting.
- `ui/src/components/task-card.tsx` — read title, tags, progress, assignees from Entity fields instead of typed properties. Replace inline `EditableMarkdown` with `TextEditor`/`TextPresenter` for title.
- `ui/src/components/sortable-task-card.tsx` — update props from `Task` to `Entity`
- `ui/src/components/sortable-column.tsx` — update props from `Column` to `Entity`
- `ui/src/types/kanban.ts` — remove old typed interfaces (`Task`, `Column`, `Board`, `Position`), replace with Entity shape
- `ui/src/lib/column-reorder.ts` — update to work with Entity fields for column order

**Key changes:**
- Tasks grouped by `entity.fields.position_column` instead of `task.position.column`
- Task card reads `entity.fields.title`, computed `tags`, computed `progress` from entity fields
- DnD `move_task` becomes `update_entity_field` calls for position_column + position_ordinal
- Column reorder becomes entity field updates for column order
- `blockedIds` computation reads `depends_on` field from entities
- Ordinal computation (`computeOrdinal`) unchanged — still lexicographic fractional indexing

Subtasks:
- [ ] Update App.tsx state to use Entity-based types, update fetch/refresh cycle
- [ ] Update BoardView to group tasks by position_column field, update DnD handlers
- [ ] Update ColumnView to read from Entity fields, use TextPresenter/TextEditor for name
- [ ] Update TaskCard to read title/tags/progress/assignees from Entity fields
- [ ] Update SortableTaskCard and SortableColumn props
- [ ] Update column-reorder.ts for Entity field access
- [ ] Update blockedIds computation for depends_on field
- [ ] Remove old typed TypeScript interfaces (Task, Column, Board, Position)
- [ ] Tests for board view rendering, DnD, task card display
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 24**

## Card 24: Cleanup — remove remaining typed structs and update processor

Remove any remaining typed structs, update processor logging to use entity_type strings, clean up exports.

**Files to modify/delete:**
- `swissarmyhammer-kanban/src/types/board.rs` — remove any remaining structs
- `swissarmyhammer-kanban/src/types/position.rs` — keep Ordinal (fractional indexing logic), remove Position struct
- `swissarmyhammer-kanban/src/types/ids.rs` — simplify or keep as thin string wrappers
- `swissarmyhammer-kanban/src/types/mod.rs` — update re-exports
- `swissarmyhammer-kanban/src/lib.rs` — update public API exports
- `swissarmyhammer-kanban/src/context.rs` — remove per-type I/O methods (read_task, write_tag, etc.)
- `swissarmyhammer-kanban/src/defaults.rs` — remove KanbanLookup (replaced by generic entity I/O)
- `swissarmyhammer-kanban/src/processor.rs` — update logging dispatch to use entity_type string
- `swissarmyhammer-kanban/tests/` — update integration tests

Subtasks:
- [ ] Remove any remaining typed entity structs
- [ ] Remove per-type I/O methods from context.rs
- [ ] Keep Ordinal type for fractional indexing (used by position_ordinal field)
- [ ] Keep ID newtypes if useful, or simplify to plain strings
- [ ] Update lib.rs exports — Entity replaces typed structs
- [ ] Update KanbanLookup to use generic entity I/O
- [ ] Update processor.rs logging dispatch to use entity_type string
- [ ] Update integration tests
- [ ] Run full test suite, fix any remaining issues
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 25**

## Card 25: Cleanup — schema module

Evaluate and likely delete `swissarmyhammer-kanban/src/schema/`. With dynamic entities, board structure queries go through entity I/O.

**Files:**
- `swissarmyhammer-kanban/src/schema/` — likely DELETE entire module
- `swissarmyhammer-kanban/src/lib.rs` — remove `pub mod schema;`

Subtasks:
- [ ] Evaluate what schema module does — determine if any functionality needs to survive
- [ ] Delete module or migrate any needed logic into entity queries
- [ ] Update lib.rs exports
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 26**

## Card 26: Cleanup — activity module

Evaluate and likely delete `swissarmyhammer-kanban/src/activity/`. With entity-level changelogs, the global activity log role changes.

**Files:**
- `swissarmyhammer-kanban/src/activity/` — likely DELETE or simplify
- `swissarmyhammer-kanban/src/lib.rs` — update exports

Subtasks:
- [ ] Evaluate what activity module does — determine if global activity log is still needed alongside per-entity changelogs
- [ ] Delete module or simplify to thin wrapper over entity changelog queries
- [ ] Update lib.rs exports
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Wait for user review before proceeding to Card 27**

## Card 27: Cleanup — parse module

Evaluate and likely delete `swissarmyhammer-kanban/src/parse/`. With dynamic entities, command parsing may be handled differently.

**Files:**
- `swissarmyhammer-kanban/src/parse/` — likely DELETE or simplify
- `swissarmyhammer-kanban/src/lib.rs` — update exports

Subtasks:
- [ ] Evaluate what parse module does — determine if command parsing logic is still needed
- [ ] Delete module or migrate needed logic
- [ ] Update lib.rs exports
- [ ] Run `cargo test --workspace` AND `cd swissarmyhammer-kanban-app/ui && npm test` — all tests must pass, no exceptions
- [ ] **STOP: Final user review — all cards complete**

## Verification

After each card — **no exceptions, no excuses**:
- `cargo test --workspace` passes (ALL Rust tests, ALL crates including kanban-app backend)
- `cd swissarmyhammer-kanban-app/ui && npm test` passes (ALL frontend Vitest tests)
- If any test fails, fix it before presenting for review

After all cards:
- All entity types can be created, read, updated, deleted via commands
- YAML builtin definitions load and can be overridden by ~/.kanban/fields/ (user) and .kanban/fields/ (local)
- Tags still computed from body text via #tag patterns
- Position works with three separate fields
- Attachments are standalone entities referenced by tasks
- Board is a dynamic entity
- Every entity mutation produces a field-level changelog entry (JSONL) that can be reversed
- String field changes are stored as text diffs, not full before/after values
- UI inspector renders fields dynamically from field definitions (not hardcoded)
- Adding a new field to an entity YAML definition makes it appear in the inspector automatically
- Board view, task cards, columns, and drag-and-drop all work with dynamic Entity data
- Legacy .json files still auto-migrate
