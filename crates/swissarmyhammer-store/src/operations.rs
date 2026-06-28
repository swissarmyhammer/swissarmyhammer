//! The `#[operation]` structs that make up the `store` operation tool.
//!
//! These structs are the source of truth for the tool's verb / noun /
//! description / parameters surface. Both the wire-level `inputSchema`
//! generator and the discovery `_meta` tree generator are driven from the
//! same `STORE_OPERATIONS` slice via the `operation_tool!` macro, so the
//! two cannot drift.
//!
//! Operations divide into three groups:
//!
//! - **stack-wide** (`undo`, `redo`, `can_undo`, `can_redo`, `undo_depth`)
//!   — no `store` parameter; they operate on the one unified undo stack
//!   that spans every store in the substrate.
//! - **transaction grouping** (`begin_transaction`, `end_transaction`) —
//!   public lifecycle for non-command callers; sets / clears the ambient
//!   per-task txn id that every store's `push` honors.
//! - **store-scoped** (`history`, `get_item`, `list_stores`) — take a
//!   `store` parameter naming one of the registered stores in the
//!   substrate (e.g. `"task"`, `"column"`).

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use swissarmyhammer_operations::{notification, operation, Notification, Operation};
use swissarmyhammer_plugin::notify::FieldChange;

// Stack-wide operations ─────────────────────────────────────────────────

/// Undo the most recent operation on the unified undo stack.
///
/// Reverts every write in the most recent undo group as one step,
/// dispatching the per-entry reversal to whichever store(s) the group
/// touched. Returns the list of `(store, item)` pairs that were
/// resynced so callers can reconcile any caches mirroring on-disk state.
#[operation(
    verb = "undo",
    noun = "stack",
    description = "Undo the most recent operation on the unified undo stack"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Undo {}

/// Redo the most recently undone operation on the unified undo stack.
///
/// Reapplies every write in the most recently undone group as one step.
/// Symmetric to [`Undo`]; returns the same `(store, item)` shape.
#[operation(
    verb = "redo",
    noun = "stack",
    description = "Redo the most recently undone operation on the unified undo stack"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Redo {}

/// Whether an undo would currently succeed.
///
/// Cheap read-only probe over the unified stack. Returns
/// `{ ok: true, can_undo: <bool> }`.
#[operation(
    verb = "can_undo",
    noun = "stack",
    description = "Whether an undo would currently succeed on the unified stack"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CanUndo {}

/// Whether a redo would currently succeed.
///
/// Cheap read-only probe over the unified stack. Returns
/// `{ ok: true, can_redo: <bool> }`.
#[operation(
    verb = "can_redo",
    noun = "stack",
    description = "Whether a redo would currently succeed on the unified stack"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CanRedo {}

/// Number of entries currently available to undo.
///
/// Equivalent to the count of consecutive `undo` calls that would
/// succeed from the current pointer position. Returns
/// `{ ok: true, depth: <usize> }`.
#[operation(
    verb = "depth",
    noun = "stack",
    description = "Number of entries currently available to undo on the unified stack"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UndoDepth {}

// Transaction-grouping operations ──────────────────────────────────────

/// Begin a transaction on the calling task.
///
/// Allocates a fresh `UndoEntryId` and installs it as the ambient
/// transaction id for the current tokio task. Every subsequent `push`
/// from this task — until the matching `EndTransaction` — is stamped
/// with this id and undone / redone as a single group.
///
/// Different tokio tasks each have their own ambient slot, so two
/// transactions opened concurrently from different tasks do not
/// interfere.
///
/// Returns `{ ok: true, id: "<ulid>" }`.
#[operation(
    verb = "begin",
    noun = "transaction",
    description = "Begin a transaction on the calling task and return its id"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BeginTransaction {}

/// End the transaction with the given id on the calling task.
///
/// Clears the ambient slot when (and only when) the id matches the
/// task's current slot. A stale or mismatched id is a no-op. Returns
/// `{ ok: true }`.
#[operation(
    verb = "end",
    noun = "transaction",
    description = "End the transaction with the given id on the calling task"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EndTransaction {
    /// The transaction id returned by `BeginTransaction`.
    #[serde(default)]
    pub id: String,
}

// Store-scoped operations ──────────────────────────────────────────────

/// Read every changelog entry for an item in the named store.
///
/// Returns the full per-item mutation history, oldest first. Returns
/// an empty list when the item has never been written. Errors when the
/// store name does not match any registered store.
///
/// Returns `{ ok: true, entries: [<ChangelogEntry>, ...] }`.
#[operation(
    verb = "history",
    noun = "item",
    description = "Read every changelog entry for an item in the named store"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct History {
    /// The store name (e.g. `"task"`, `"column"`).
    #[serde(default)]
    pub store: String,
    /// The item id within that store.
    #[serde(default)]
    pub item_id: String,
}

/// Read the current serialized bytes for an item in the named store.
///
/// Returns the on-disk text. Returns `{ ok: true, bytes: null }` when
/// the item does not exist (never written, or trashed / archived).
/// Errors when the store name does not match any registered store.
///
/// Returns `{ ok: true, bytes: <string|null> }`.
#[operation(
    verb = "get",
    noun = "item",
    description = "Read the current serialized bytes for an item in the named store"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GetItem {
    /// The store name (e.g. `"task"`, `"column"`).
    #[serde(default)]
    pub store: String,
    /// The item id within that store.
    #[serde(default)]
    pub item_id: String,
}

/// List every registered store by its human-readable name.
///
/// Order matches the order of registration with the underlying
/// `StoreContext`. Returns `{ ok: true, stores: ["task", "column", …] }`.
#[operation(
    verb = "list",
    noun = "stores",
    description = "List every registered store by its human-readable name"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ListStores {}

/// All store operations — the canonical list used for schema generation.
///
/// Both the wire-schema generator (`generate_mcp_schema`) and the
/// discovery `_meta` generator (`generate_operations_meta`) are driven
/// from this single slice via the `operation_tool!` macro, so there is
/// one source of truth for what the `store` tool exposes.
static STORE_OPERATIONS: LazyLock<Vec<&'static dyn Operation>> = LazyLock::new(|| {
    vec![
        Box::leak(Box::<Undo>::default()) as &dyn Operation,
        Box::leak(Box::<Redo>::default()) as &dyn Operation,
        Box::leak(Box::<CanUndo>::default()) as &dyn Operation,
        Box::leak(Box::<CanRedo>::default()) as &dyn Operation,
        Box::leak(Box::<UndoDepth>::default()) as &dyn Operation,
        Box::leak(Box::<BeginTransaction>::default()) as &dyn Operation,
        Box::leak(Box::<EndTransaction>::default()) as &dyn Operation,
        Box::leak(Box::<History>::default()) as &dyn Operation,
        Box::leak(Box::<GetItem>::default()) as &dyn Operation,
        Box::leak(Box::<ListStores>::default()) as &dyn Operation,
    ]
});

/// Get the canonical slice of all store operations.
pub fn operations() -> &'static [&'static dyn Operation] {
    &STORE_OPERATIONS
}

// Notifications ─────────────────────────────────────────────────────────

/// The `notifications/store/changed` event payload.
///
/// The one generic data-change schema for entities, views, and perspectives.
/// This struct is the single source of truth for the event: it IS the
/// published payload (it serializes to the notification's `params` via
/// [`McpNotification::from_declared`](swissarmyhammer_plugin::McpNotification::from_declared))
/// AND the declaration the SDK reads (its fields drive the
/// `io.swissarmyhammer/notifications` `_meta`). The two cannot drift.
///
/// `store` names the store (`"task"`, `"tag"`, `"view"`, `"perspective"`, …);
/// `item` is the item id; `op` is the change kind (`"created"` / `"updated"` /
/// `"removed"`). `changes` carries the field-level diff for entity stores and
/// is omitted for views/perspectives (reload-item semantics).
///
/// Published by the kanban notification fan-in (`swissarmyhammer-kanban`'s
/// `notify_fanin`), which translates the in-process entity / view / perspective
/// buses into this shape. Provenance (`txn`/`origin`) is universal
/// cross-cutting metadata stamped on every notification at publish time; it is
/// intentionally NOT a field here.
#[notification(
    method = "notifications/store/changed",
    description = "An entity, view, or perspective item was created, updated, or removed."
)]
#[derive(Debug, Default, Serialize)]
pub struct StoreChanged {
    /// The store the item belongs to (`"task"`, `"view"`, `"perspective"`, …).
    pub store: String,
    /// The item id within that store.
    pub item: String,
    /// The change kind: `"created"`, `"updated"`, or `"removed"`.
    pub op: String,
    /// Field-level diff for entity stores; omitted for views/perspectives.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changes: Option<Vec<FieldChange>>,
}

/// The `notifications/store/undo_changed` event payload.
///
/// Reports whether undo/redo are currently possible and the labels of the
/// entries at the top of each side of the unified undo stack. Like
/// [`StoreChanged`], this struct IS the published payload and the declaration,
/// so the `_meta` schema and the wire payload share one source.
///
/// Published by the kanban notification fan-in from the store's stack-state
/// broadcast. Ephemeral undo-stack state is not itself a stored, undoable
/// thing, so this notification carries no provenance.
#[notification(
    method = "notifications/store/undo_changed",
    description = "The unified undo stack's undo/redo availability or labels changed."
)]
#[derive(Debug, Default, Serialize)]
pub struct StoreUndoChanged {
    /// Whether an undo would currently succeed.
    pub can_undo: bool,
    /// Whether a redo would currently succeed.
    pub can_redo: bool,
    /// Label of the entry an undo would revert, if any.
    pub undo_label: Option<String>,
    /// Label of the entry a redo would reapply, if any.
    pub redo_label: Option<String>,
}

/// The canonical slice of notifications the `store` tool declares.
///
/// Mirrors [`operations`]: a leaked `Default` instance per notification, used
/// only for its static metadata. Fed to `operation_tool!`'s `notifications:`
/// field so the tool advertises its events in `_meta`.
///
/// Owner/publisher split: these events are DECLARED here (on the `store` tool)
/// but PUBLISHED by the kanban fan-in. The namespace-based coverage guard keeps
/// the two in lockstep — every `notifications/store/*` the fan-in publishes must
/// appear here, and vice versa.
static STORE_NOTIFICATIONS: LazyLock<Vec<&'static dyn Notification>> = LazyLock::new(|| {
    vec![
        Box::leak(Box::<StoreChanged>::default()) as &dyn Notification,
        Box::leak(Box::<StoreUndoChanged>::default()) as &dyn Notification,
    ]
});

/// Get the canonical slice of all store notifications.
pub fn store_notifications() -> &'static [&'static dyn Notification] {
    &STORE_NOTIFICATIONS
}
