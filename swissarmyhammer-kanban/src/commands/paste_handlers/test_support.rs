//! Shared fixtures for `paste_handlers/*` colocated tests.
//!
//! Each paste handler's `#[cfg(test)]` module independently grew its own
//! `setup()`, `make_ctx()`, snapshot helpers, and per-handler local
//! [`PasteMatrix`] builder. Across the seven handlers that adds up to
//! several hundred lines of near-identical scaffolding. This module
//! consolidates the patterns so each handler test file only contains the
//! handler-specific assertions.
//!
//! All helpers are thin wrappers around the production APIs they exercise
//! (`KanbanContext`, `EntityContext`, `serialize_to_clipboard`); no test
//! double or stub is introduced. The module is gated behind `#[cfg(test)]`
//! so it ships nothing into release builds.
//!
//! ## Conventions
//!
//! - `setup()` returns `(TempDir, Arc<KanbanContext>)` with the default
//!   board (`todo` / `doing` / `done`) seeded. Tests that need a different
//!   column shape can install one via [`install_columns`] after `setup()`.
//!   The [`TempDir`] must be held for the test's lifetime — dropping it
//!   wipes the on-disk board.
//! - `make_ctx*` returns a [`CommandContext`] whose extension map carries
//!   the supplied [`KanbanContext`]. The `entity.paste` command id and an
//!   empty arg map are used throughout, mirroring how `PasteEntityCmd`
//!   invokes a handler — the moniker under test is passed as the `target`
//!   argument to `PasteHandler::execute`, not embedded in the context.
//! - `*_clipboard*` constructors build [`ClipboardPayload`] values with
//!   the same on-wire shape `entity.copy` produces. Tests that need the
//!   round-trip through `serialize_to_clipboard` use [`snapshot_*_for_paste`].

#![cfg(test)]

use crate::board::InitBoard;
use crate::clipboard::{
    deserialize_from_clipboard, serialize_to_clipboard, ClipboardData, ClipboardPayload,
    ClipboardProviderExt, InMemoryClipboard,
};
use crate::commands::paste_handlers::{PasteHandler, PasteMatrix};
use crate::context::KanbanContext;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_commands::{CommandContext, UIState};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::Execute;
use tempfile::TempDir;

// =============================================================================
// Board / context setup
// =============================================================================

/// Build a fresh `KanbanContext` on a tempdir with the default board
/// (`todo` / `doing` / `done`) seeded by [`InitBoard`].
///
/// Returns the [`TempDir`] alongside the context — callers must hold the
/// `TempDir` for the duration of the test or the on-disk board is dropped.
pub async fn setup() -> (TempDir, Arc<KanbanContext>) {
    let temp = TempDir::new().unwrap();
    let kanban = Arc::new(KanbanContext::new(temp.path().join(".kanban")));
    InitBoard::new("Test")
        .execute(kanban.as_ref())
        .await
        .into_result()
        .unwrap();
    (temp, kanban)
}

/// Like [`setup`], but does *not* call [`InitBoard`] — used by tests that
/// need to opt into a custom column shape via [`install_columns`].
pub async fn setup_uninitialized() -> (TempDir, Arc<KanbanContext>) {
    let temp = TempDir::new().unwrap();
    let kanban = Arc::new(KanbanContext::new(temp.path().join(".kanban")));
    (temp, kanban)
}

/// Replace whatever columns exist on the board with `columns`, expressed
/// as `(id, order)` pairs. Used by tests that need non-default column
/// orderings (e.g. positions 0/100/200 to make "leftmost" unambiguous) or
/// need to drop every column off the board to exercise the "no columns"
/// path.
pub async fn install_columns(kanban: &Arc<KanbanContext>, columns: &[(&str, u64)]) {
    let ectx = kanban.entity_context().await.unwrap();
    // Drop any pre-existing columns first so the test starts from a known
    // state regardless of whether `setup` or `setup_uninitialized` was used.
    for col in ectx.list("column").await.unwrap() {
        ectx.delete("column", col.id.as_str()).await.unwrap();
    }
    for (id, order) in columns {
        let mut entity = Entity::new("column", *id);
        entity.set("name", json!(id));
        entity.set("order", json!(*order));
        ectx.write(&entity).await.unwrap();
    }
}

// =============================================================================
// CommandContext builders
// =============================================================================

/// Build a [`CommandContext`] with the supplied [`KanbanContext`] extension.
///
/// The scope chain is empty — `target` is passed directly to
/// [`PasteHandler::execute`] in tests, mirroring how `PasteEntityCmd`
/// invokes a handler.
pub fn make_ctx(kanban: &Arc<KanbanContext>) -> CommandContext {
    let mut ctx = CommandContext::new("entity.paste", vec![], None, HashMap::new());
    ctx.set_extension(Arc::clone(kanban));
    ctx
}

/// Like [`make_ctx`] but also attaches a fresh [`UIState`] extension.
///
/// Some handlers (e.g. `actor_onto_task`, `attachment_onto_task`) read
/// `ctx.ui_state` indirectly via the dispatcher contract; this variant
/// keeps those tests faithful to the real wiring.
pub fn make_ctx_with_ui(target: &str, kanban: &Arc<KanbanContext>) -> CommandContext {
    let mut ctx = CommandContext::new(
        "entity.paste",
        Vec::new(),
        Some(target.to_string()),
        HashMap::new(),
    );
    ctx.set_extension(Arc::clone(kanban));
    ctx.ui_state = Some(Arc::new(UIState::new()));
    ctx
}

/// Build a [`CommandContext`] with a scope chain, kanban context, in-memory
/// clipboard provider, and UI state — the full extension set used by the
/// `*_into_board` handlers.
pub fn make_ctx_with_clipboard(
    scope: &[&str],
    kanban: &Arc<KanbanContext>,
    clipboard: &Arc<ClipboardProviderExt>,
    ui: &Arc<UIState>,
) -> CommandContext {
    let mut ctx = CommandContext::new(
        "entity.paste",
        scope.iter().map(|s| s.to_string()).collect(),
        None,
        HashMap::new(),
    );
    ctx.set_extension(Arc::clone(kanban));
    ctx.set_extension(Arc::clone(clipboard));
    ctx.ui_state = Some(Arc::clone(ui));
    ctx
}

/// Build the in-memory clipboard provider extension wrapper used by the
/// `*_into_board` handler tests.
pub fn in_memory_clipboard_ext() -> Arc<ClipboardProviderExt> {
    Arc::new(ClipboardProviderExt(Arc::new(InMemoryClipboard::new())))
}

// =============================================================================
// Matrix builders
// =============================================================================

/// Register a single handler on a fresh [`PasteMatrix`].
///
/// Mirrors how the orchestrator wires production matrices, but keeps each
/// handler's tests independent of `register_paste_handlers()` — the
/// hygiene tests in `paste_handlers/mod.rs` cover the production matrix.
pub fn matrix_with<H: PasteHandler + 'static>(handler: H) -> PasteMatrix {
    let mut m = PasteMatrix::default();
    m.register(handler);
    m
}

// =============================================================================
// Clipboard payload constructors
// =============================================================================

/// Build a [`ClipboardPayload`] directly (no serialize round-trip) with
/// the supplied entity type, id, mode, and field snapshot.
///
/// Use this when the handler under test does not depend on any field the
/// real `serialize_to_clipboard` injects beyond what the test already
/// supplies — typically the simple "association" handlers
/// (`tag`, `actor`, `attachment`) that consult only `entity_id`.
pub fn clipboard_payload(
    entity_type: &str,
    entity_id: &str,
    mode: &str,
    fields: Value,
) -> ClipboardPayload {
    ClipboardPayload {
        swissarmyhammer_clipboard: ClipboardData {
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            mode: mode.into(),
            fields,
        },
    }
}

/// Build a [`ClipboardPayload`] for a tag — common shape used by the
/// `tag_onto_task` tests.
pub fn tag_clipboard(tag_id: &str, tag_name: &str, mode: &str) -> ClipboardPayload {
    clipboard_payload("tag", tag_id, mode, json!({ "tag_name": tag_name }))
}

/// Build a [`ClipboardPayload`] for an actor — common shape used by the
/// `actor_onto_task` tests.
pub fn actor_clipboard(actor_id: &str, mode: &str) -> ClipboardPayload {
    clipboard_payload("actor", actor_id, mode, json!({}))
}

/// Build a [`ClipboardPayload`] for an attachment, populating only the
/// fields the copy path emits.
///
/// `mime_type` and `size` are optional — `None` exercises the handler's
/// extension-based MIME / `stat`-based size fallbacks.
pub fn attachment_clipboard(
    path: &str,
    name: &str,
    mime_type: Option<&str>,
    size: Option<u64>,
    mode: &str,
) -> ClipboardPayload {
    let mut fields = serde_json::Map::new();
    fields.insert("name".into(), json!(name));
    if let Some(mime) = mime_type {
        fields.insert("mime_type".into(), json!(mime));
    }
    if let Some(s) = size {
        fields.insert("size".into(), json!(s));
    }
    clipboard_payload("attachment", path, mode, Value::Object(fields))
}

/// Build a synthetic task [`ClipboardPayload`] by pumping the supplied
/// fields through `serialize_to_clipboard` / `deserialize_from_clipboard`
/// — the same wire-format round-trip the real `entity.copy` performs.
///
/// Use this when the test wants to assert what the handler does given a
/// hand-crafted snapshot (e.g. stale position fields) rather than a
/// snapshot of a real on-board entity.
pub fn task_clipboard_from_fields(task_id: &str, fields: Value, mode: &str) -> ClipboardPayload {
    let json = serialize_to_clipboard("task", task_id, mode, fields);
    deserialize_from_clipboard(&json).expect("payload roundtrip must succeed")
}

// =============================================================================
// Snapshot helpers — read an existing entity into a clipboard payload
// =============================================================================

/// Snapshot an existing task's fields into a [`ClipboardPayload`] by
/// reading the entity, serialising via `serialize_to_clipboard`, and
/// deserialising back into the typed payload — mirroring what `entity.copy`
/// writes to the system clipboard.
pub async fn snapshot_task(kanban: &KanbanContext, task_id: &str, mode: &str) -> ClipboardPayload {
    snapshot_entity(kanban, "task", task_id, mode).await
}

/// Snapshot an existing column's fields into a [`ClipboardPayload`].
pub async fn snapshot_column(
    kanban: &KanbanContext,
    column_id: &str,
    mode: &str,
) -> ClipboardPayload {
    snapshot_entity(kanban, "column", column_id, mode).await
}

/// Generic snapshot helper — reads `entity_type:entity_id` and round-trips
/// it through the clipboard wire format. Specialised wrappers
/// ([`snapshot_task`], [`snapshot_column`]) are provided for the common
/// cases.
pub async fn snapshot_entity(
    kanban: &KanbanContext,
    entity_type: &str,
    entity_id: &str,
    mode: &str,
) -> ClipboardPayload {
    let ectx = kanban.entity_context().await.unwrap();
    let entity = ectx.read(entity_type, entity_id).await.unwrap();
    let fields = serde_json::to_value(&entity.fields).unwrap();
    let json = serialize_to_clipboard(entity_type, entity_id, mode, fields);
    deserialize_from_clipboard(&json).expect("snapshot must round-trip")
}

// =============================================================================
// Read helpers
// =============================================================================

/// List every task currently on the board.
pub async fn list_tasks(kanban: &KanbanContext) -> Vec<Entity> {
    kanban
        .entity_context()
        .await
        .unwrap()
        .list("task")
        .await
        .unwrap()
}

/// List every column currently on the board.
pub async fn list_columns(kanban: &KanbanContext) -> Vec<Entity> {
    kanban
        .entity_context()
        .await
        .unwrap()
        .list("column")
        .await
        .unwrap()
}
