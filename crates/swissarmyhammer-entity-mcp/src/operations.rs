//! The `#[operation]` structs that make up the `entity` operation tool.
//!
//! These structs are the source of truth for the tool's verb / noun /
//! description / parameters surface. Both the wire-level `inputSchema`
//! generator and the discovery `_meta` tree generator are driven from the
//! same `ENTITY_OPERATIONS` slice via the `operation_tool!` macro, so the
//! two cannot drift.
//!
//! The `entity` tool is the generic, type-agnostic face over the entity
//! **kernel** (`EntityContext`). Every operation takes an `entity_type`
//! naming the kind of entity to act on (`"task"`, `"tag"`, …); the kernel
//! resolves the type's `EntityDef` and routes the call through the one CRUD
//! implementation shared with the domain `kanban` face. Operations divide
//! into three groups:
//!
//! - **read** (`get`, `list`) — fetch one entity or every entity of a type.
//! - **write** (`add`, `update`, `delete`) — create / mutate / trash entities.
//!   Writes go through `EntityContext`, which pushes onto the shared
//!   `StoreContext` (undoable) and broadcasts `EntityEvent`s.
//! - **archive** (`archive`, `unarchive`) — move entities to / from the
//!   `.archive/` directory without trashing them.
//! - **search** (`search`) — free-text query over the entities, backed by
//!   `EntitySearchIndex`, optionally narrowed to a single type.
//! - **clipboard** (`copy`, `cut`, `paste`) — copy/cut snapshot an entity to
//!   the clipboard; paste dispatches through the shared `kanban` `PasteMatrix`
//!   onto a target moniker. These reuse the exact command structs that back
//!   the domain face, so there is no duplicate clipboard logic. Cut and paste
//!   write through the kernel, so they are undoable and emit events.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::LazyLock;
use swissarmyhammer_operations::{operation, Operation};

// Read operations ───────────────────────────────────────────────────────

/// Read a single entity by type and id.
///
/// Resolves the entity through the kernel's `EntityContext::read`, which
/// hits the attached cache when present and falls through to disk on a
/// miss. Returns the entity as a JSON object (the `Entity::to_json` shape,
/// with `id`, `entity_type`, and `moniker` injected).
///
/// Returns `{ ok: true, entity: <object> }`.
#[operation(
    verb = "get",
    noun = "entity",
    description = "Read a single entity of the given type by id"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GetEntity {
    /// The entity type (e.g. `"task"`, `"tag"`).
    #[serde(default, rename = "type")]
    pub entity_type: String,
    /// The entity id within that type.
    #[serde(default)]
    pub id: String,
}

/// List every live entity of a type.
///
/// Reads through `EntityContext::list`, which surfaces the cache when
/// attached. Archived and trashed entities are excluded. The optional
/// `filter` is reserved for a later filter-DSL pass; today it is accepted
/// and ignored so the wire shape is stable when filtering lands.
///
/// Returns `{ ok: true, entities: [<object>, ...] }`.
#[operation(
    verb = "list",
    noun = "entities",
    description = "List every live entity of the given type"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ListEntities {
    /// The entity type (e.g. `"task"`, `"tag"`).
    #[serde(default, rename = "type")]
    pub entity_type: String,
    /// Optional filter expression. Reserved for future use; ignored today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
}

// Write operations ──────────────────────────────────────────────────────

/// Create or overwrite an entity of a type from a bag of fields.
///
/// Builds an `Entity` from `type` + `id` + `fields` and writes it through
/// `EntityContext::write`. The write is undoable (pushed onto the shared
/// `StoreContext`) and emits an `EntityEvent`. When `id` is omitted a fresh
/// ULID is minted.
///
/// Returns `{ ok: true, id: <string>, entry_id: <string|null> }` where
/// `entry_id` is the undo-stack entry id (`null` when the write was
/// idempotent or no store handle is registered).
#[operation(
    verb = "add",
    noun = "entity",
    description = "Create or overwrite an entity of the given type from a field map"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AddEntity {
    /// The entity type (e.g. `"task"`, `"tag"`).
    #[serde(default, rename = "type")]
    pub entity_type: String,
    /// Optional explicit id. A fresh ULID is minted when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The field name -> value map for the new entity.
    #[serde(default)]
    pub fields: serde_json::Map<String, Value>,
}

/// Set a single field on an existing entity.
///
/// Reads the current entity through the kernel, replaces `field` with
/// `value`, and writes it back via `EntityContext::write` — so the update
/// is undoable and emits an `EntityEvent` exactly like any other write.
///
/// Returns `{ ok: true, id: <string>, entry_id: <string|null> }`.
#[operation(
    verb = "update",
    noun = "field",
    description = "Set a single field on an existing entity of the given type"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateField {
    /// The entity type (e.g. `"task"`, `"tag"`).
    #[serde(default, rename = "type")]
    pub entity_type: String,
    /// The entity id within that type.
    #[serde(default)]
    pub id: String,
    /// The field name to set.
    #[serde(default)]
    pub field: String,
    /// The new value for the field.
    #[serde(default)]
    pub value: Value,
}

/// Delete (trash) an entity by type and id.
///
/// Routes through `EntityContext::delete`, which moves the entity's files
/// to the type's `.trash/` directory. The delete is undoable and emits an
/// `EntityEvent`.
///
/// Returns `{ ok: true, entry_id: <string|null> }`.
#[operation(
    verb = "delete",
    noun = "entity",
    description = "Delete (trash) an entity of the given type by id"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeleteEntity {
    /// The entity type (e.g. `"task"`, `"tag"`).
    #[serde(default, rename = "type")]
    pub entity_type: String,
    /// The entity id within that type.
    #[serde(default)]
    pub id: String,
}

// Archive operations ────────────────────────────────────────────────────

/// Archive an entity by type and id.
///
/// Routes through `EntityContext::archive`, moving the entity to the type's
/// `.archive/` directory. Archived entities are excluded from `list` but
/// remain restorable via `unarchive`. The archive is undoable and emits an
/// `EntityEvent`.
///
/// Returns `{ ok: true, entry_id: <string|null> }`.
#[operation(
    verb = "archive",
    noun = "entity",
    description = "Archive an entity of the given type by id"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ArchiveEntity {
    /// The entity type (e.g. `"task"`, `"tag"`).
    #[serde(default, rename = "type")]
    pub entity_type: String,
    /// The entity id within that type.
    #[serde(default)]
    pub id: String,
}

/// Restore an archived entity back to live storage.
///
/// Routes through `EntityContext::unarchive`, the inverse of `archive`.
/// The unarchive is undoable and emits an `EntityEvent`.
///
/// Returns `{ ok: true, entry_id: <string|null> }`.
#[operation(
    verb = "unarchive",
    noun = "entity",
    description = "Restore an archived entity of the given type back to live storage"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UnarchiveEntity {
    /// The entity type (e.g. `"task"`, `"tag"`).
    #[serde(default, rename = "type")]
    pub entity_type: String,
    /// The entity id within that type.
    #[serde(default)]
    pub id: String,
}

// Search operations ─────────────────────────────────────────────────────

/// Search entities by free-text query, optionally narrowed to one type.
///
/// Backed by `swissarmyhammer_entity_search::EntitySearchIndex`, which the
/// server builds from the live entities of the searchable types. The query
/// runs fuzzy matching over entity fields; when `type` is supplied, results
/// are filtered down to that single entity type.
///
/// Returns `{ ok: true, results: [{ id, type, score, entity }, ...] }`,
/// ordered best-match first.
#[operation(
    verb = "search",
    noun = "entities",
    description = "Search entities by free-text query, optionally filtered to one type"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Search {
    /// The free-text query to match against entity fields.
    #[serde(default)]
    pub query: String,
    /// Optional entity type filter (e.g. `"task"`). When omitted, every
    /// searchable type is considered.
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
}

// Clipboard operations ──────────────────────────────────────────────────
//
// Copy / Cut / Paste reuse the *exact* clipboard machinery that backs the
// domain `kanban` face — `CopyEntityCmd`, `CutEntityCmd`, `PasteEntityCmd`
// from `swissarmyhammer_kanban::commands::clipboard_commands`, dispatched
// over the same `PasteMatrix`. The server constructs a `CommandContext`
// over the shared kernel and runs those command structs, so there is one
// copy/cut/paste implementation in the codebase, not two.
//
// External drag-in and clipboard paste both create through the paste
// matrix (this is the paste path). Internal drag — repositioning an entity
// already on the board — is a property mutation handled by the drag
// commands elsewhere, NOT here.

/// Copy an entity of the given type to the clipboard.
///
/// Snapshots the `type:id` entity's field set into a clipboard payload via
/// the shared `CopyEntityCmd`, which reads through the kernel and writes
/// the JSON to the injected clipboard provider. Copy is non-destructive —
/// the source entity is untouched.
///
/// Returns the underlying command's result, e.g.
/// `{ copied: true, id, entity_type, clipboard_json }`.
#[operation(
    verb = "copy",
    noun = "entity",
    description = "Copy an entity of the given type to the clipboard"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Copy {
    /// The entity type (e.g. `"task"`, `"tag"`).
    #[serde(default, rename = "type")]
    pub entity_type: String,
    /// The entity id within that type.
    #[serde(default)]
    pub id: String,
    /// Optional scope chain (innermost-first monikers like
    /// `["task:01T", "column:todo"]`). Required only when copying an
    /// `attachment`, which the shared command resolves against its parent
    /// `task:` moniker; ignored for self-contained types.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
}

/// Cut an entity of the given type: copy it, then run its destructive op.
///
/// Delegates to the shared `CutEntityCmd`, which snapshots the entity to
/// the clipboard and then deletes / detaches the source (task delete, tag
/// untag, attachment removal). The destructive step flows through the
/// kernel's `StoreContext`, so the cut is undoable and emits entity events.
/// Types without a defined destructive operation are rejected.
///
/// Returns the underlying command's result (shape depends on entity type;
/// always carries `clipboard_json` and `cut: true`).
#[operation(
    verb = "cut",
    noun = "entity",
    description = "Cut an entity of the given type (copy to clipboard, then delete/detach the source)"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Cut {
    /// The entity type (e.g. `"task"`, `"tag"`).
    #[serde(default, rename = "type")]
    pub entity_type: String,
    /// The entity id within that type.
    #[serde(default)]
    pub id: String,
    /// Optional scope chain (innermost-first monikers). Required when
    /// cutting a `tag` or `attachment`, whose destructive op needs the
    /// owning `task:` moniker in scope; ignored for self-contained types
    /// like `task`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
}

/// Paste the clipboard onto a target entity, via the shared `PasteMatrix`.
///
/// Delegates to the shared `PasteEntityCmd`, which deserializes the
/// clipboard payload, looks up the `(clipboard_type, target_type)` handler
/// in the `PasteMatrix`, and runs it. The handler's writes flow through the
/// kernel's `StoreContext`, so the paste is undoable and emits entity
/// events. This is the external/clipboard paste path; internal drag
/// repositioning is handled elsewhere.
///
/// Returns the paste handler's result (shape depends on the matched pair,
/// e.g. `{ id, ... }` for a new task, `{ tagged: true, ... }` for a tag).
#[operation(
    verb = "paste",
    noun = "entity",
    description = "Paste the clipboard onto the target moniker via the shared PasteMatrix"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Paste {
    /// The destination moniker (e.g. `"column:doing"`, `"task:01T"`). The
    /// `(clipboard_type, target_type)` pair selects the paste handler.
    #[serde(default)]
    pub target: String,
    /// Optional scope chain (innermost-first monikers). Some paste handlers
    /// (e.g. `attachment_onto_attachment`) resolve a parent entity from the
    /// scope chain; supply it when pasting onto association-shaped targets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
}

/// All entity operations — the canonical list used for schema generation.
///
/// Both the wire-schema generator (`generate_mcp_schema`) and the
/// discovery `_meta` generator (`generate_operations_meta`) are driven
/// from this single slice via the `operation_tool!` macro, so there is
/// one source of truth for what the `entity` tool exposes.
static ENTITY_OPERATIONS: LazyLock<Vec<&'static dyn Operation>> = LazyLock::new(|| {
    vec![
        Box::leak(Box::<GetEntity>::default()) as &dyn Operation,
        Box::leak(Box::<ListEntities>::default()) as &dyn Operation,
        Box::leak(Box::<AddEntity>::default()) as &dyn Operation,
        Box::leak(Box::<UpdateField>::default()) as &dyn Operation,
        Box::leak(Box::<DeleteEntity>::default()) as &dyn Operation,
        Box::leak(Box::<ArchiveEntity>::default()) as &dyn Operation,
        Box::leak(Box::<UnarchiveEntity>::default()) as &dyn Operation,
        Box::leak(Box::<Search>::default()) as &dyn Operation,
        Box::leak(Box::<Copy>::default()) as &dyn Operation,
        Box::leak(Box::<Cut>::default()) as &dyn Operation,
        Box::leak(Box::<Paste>::default()) as &dyn Operation,
    ]
});

/// Get the canonical slice of all entity operations.
pub fn operations() -> &'static [&'static dyn Operation] {
    &ENTITY_OPERATIONS
}
