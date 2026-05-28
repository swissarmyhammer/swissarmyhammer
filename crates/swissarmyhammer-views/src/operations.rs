//! The `#[operation]` structs that make up the `views` operation tool.
//!
//! These structs are the source of truth for the tool's verb / noun /
//! description / parameters surface. Both the wire-level `inputSchema`
//! generator and the discovery `_meta` tree generator are driven from the
//! same [`operations`] slice via the `operation_tool!` macro, so the two
//! cannot drift.
//!
//! The `views` tool is the in-process face over two registry kernels — the
//! `PerspectiveContext` (from `swissarmyhammer-perspectives`) and the
//! `ViewsContext` (from `swissarmyhammer-views`). It exposes the
//! perspective/view *state* mutations the `perspective.*` and `view.set`
//! commands depend on, as thin verbs over the existing context methods.
//! Mutations write through each context's `StoreHandle`, so undo/redo via the
//! shared `StoreContext` reverts them for free — this tool implements no undo
//! of its own.
//!
//! The eighteen operations group into six sub-domains:
//!
//! - **lifecycle** (`load`, `save`, `delete`, `rename`, `list`) — CRUD over
//!   perspective definitions.
//! - **filter** (`set`, `clear`, `focus`) — the perspective `filter` field.
//! - **group** (`set`, `clear`) — the perspective `group` field.
//! - **sort** (`set`, `clear`, `toggle`) — the perspective `sort` list.
//! - **nav** (`next`, `prev`, `goto`, `switch`) — resolve a perspective within
//!   a view's ordered list.
//! - **view** (`set`) — write a `ViewDef` through the views kernel.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use swissarmyhammer_operations::{operation, Operation};

// Lifecycle operations ───────────────────────────────────────────────────

/// Load a perspective by name or id, returning its full configuration.
///
/// Resolves the perspective through `PerspectiveContext::get_by_name` first,
/// falling back to `get_by_id`. Returns `{ ok: true, perspective: <object> }`
/// or a not-found error.
#[operation(
    verb = "load",
    noun = "perspective",
    description = "Load a perspective by name or id"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LoadPerspective {
    /// The perspective name or id to load.
    #[serde(default)]
    pub name: String,
}

/// Create or update a perspective.
///
/// Builds a [`Perspective`](swissarmyhammer_perspectives::Perspective) from
/// the supplied fields and writes it through `PerspectiveContext::write`. When
/// `id` is omitted a fresh ULID is minted. The write is undoable (pushed onto
/// the shared `StoreContext`) and emits a `PerspectiveEvent`.
///
/// Returns `{ ok: true, perspective: <object>, entry_id: <string|null> }`.
#[operation(
    verb = "save",
    noun = "perspective",
    description = "Create or update a perspective"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SavePerspective {
    /// Optional explicit id. A fresh ULID is minted when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Human-readable name (e.g. "Active Sprint").
    #[serde(default)]
    pub name: String,
    /// View kind (e.g. "board", "grid").
    #[serde(default)]
    pub view: String,
    /// Optional id of the view instance this perspective is scoped to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_id: Option<String>,
    /// Optional filter DSL expression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    /// Optional group-by field name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// Delete a perspective by id.
///
/// Routes through `PerspectiveContext::delete`, which trashes the YAML file
/// for undo support. The delete is undoable and emits a `PerspectiveEvent`.
///
/// Returns `{ ok: true, entry_id: <string|null> }`.
#[operation(
    verb = "delete",
    noun = "perspective",
    description = "Delete a perspective by id"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeletePerspective {
    /// The perspective id to delete.
    #[serde(default)]
    pub id: String,
}

/// Rename a perspective.
///
/// Routes through `PerspectiveContext::rename`, which changes the name and
/// writes the perspective back atomically. Undoable; emits a
/// `PerspectiveEvent`.
///
/// Returns `{ ok: true, perspective: <object> }`.
#[operation(
    verb = "rename",
    noun = "perspective",
    description = "Rename a perspective by id"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RenamePerspective {
    /// The perspective id to rename.
    #[serde(default)]
    pub id: String,
    /// The new name for the perspective.
    #[serde(default)]
    pub new_name: String,
}

/// List every loaded perspective.
///
/// Reads `PerspectiveContext::all`. Returns
/// `{ ok: true, perspectives: [<object>, ...], count: <n> }`.
#[operation(
    verb = "list",
    noun = "perspective",
    description = "List every loaded perspective"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ListPerspective {}

// Filter operations ───────────────────────────────────────────────────────

/// Set the filter expression on a perspective.
///
/// Reads the perspective by id, replaces its `filter` field, and writes it
/// back through `PerspectiveContext::write`. The filter string is preserved
/// verbatim — the backend never evaluates it here.
///
/// Returns `{ ok: true, perspective: <object>, entry_id: <string|null> }`.
#[operation(
    verb = "set",
    noun = "filter",
    description = "Set the filter expression on a perspective"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetFilter {
    /// The perspective id to mutate.
    #[serde(default)]
    pub perspective_id: String,
    /// The filter DSL expression to store.
    #[serde(default)]
    pub filter: String,
}

/// Focus the filter editor for a perspective — a UI-only marker.
///
/// Mirrors the `perspective.filter.focus` command: the focus claim flows
/// through the frontend's `nav.focus`, so there is no backend state to mutate.
/// Returns `{ ok: true }` so callers can treat it uniformly.
#[operation(
    verb = "focus",
    noun = "filter",
    description = "Focus the filter editor for a perspective (UI-only no-op)"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FocusFilter {
    /// The perspective id whose filter editor to focus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perspective_id: Option<String>,
}

/// Clear the filter expression on a perspective.
///
/// Reads the perspective by id, sets its `filter` field to `None`, and writes
/// it back. Undoable; emits a `PerspectiveEvent`.
///
/// Returns `{ ok: true, perspective: <object>, entry_id: <string|null> }`.
#[operation(
    verb = "clear",
    noun = "filter",
    description = "Clear the filter expression on a perspective"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ClearFilter {
    /// The perspective id to mutate.
    #[serde(default)]
    pub perspective_id: String,
}

// Group operations ─────────────────────────────────────────────────────────

/// Set the group-by field on a perspective.
///
/// Reads the perspective by id, replaces its `group` field, and writes it
/// back. Undoable; emits a `PerspectiveEvent`.
///
/// Returns `{ ok: true, perspective: <object>, entry_id: <string|null> }`.
#[operation(
    verb = "set",
    noun = "group",
    description = "Set the group-by field on a perspective"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetGroup {
    /// The perspective id to mutate.
    #[serde(default)]
    pub perspective_id: String,
    /// The group-by field name to store.
    #[serde(default)]
    pub group: String,
}

/// Clear the group-by field on a perspective.
///
/// Reads the perspective by id, sets its `group` field to `None`, and writes
/// it back. Undoable; emits a `PerspectiveEvent`.
///
/// Returns `{ ok: true, perspective: <object>, entry_id: <string|null> }`.
#[operation(
    verb = "clear",
    noun = "group",
    description = "Clear the group-by field on a perspective"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ClearGroup {
    /// The perspective id to mutate.
    #[serde(default)]
    pub perspective_id: String,
}

// Sort operations ──────────────────────────────────────────────────────────

/// Set (add or replace) a sort entry on a perspective.
///
/// Reads the perspective's existing `sort` list, removes any entry for the
/// same field, appends `{ field, direction }`, and writes the perspective
/// back. `direction` must be `"asc"` or `"desc"`. Undoable; emits a
/// `PerspectiveEvent`.
///
/// Returns `{ ok: true, perspective: <object>, entry_id: <string|null> }`.
#[operation(
    verb = "set",
    noun = "sort",
    description = "Add or replace a sort entry on a perspective"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetSort {
    /// The perspective id to mutate.
    #[serde(default)]
    pub perspective_id: String,
    /// The field to sort by.
    #[serde(default)]
    pub field: String,
    /// The sort direction: `"asc"` or `"desc"`.
    #[serde(default)]
    pub direction: String,
}

/// Clear every sort entry on a perspective.
///
/// Resets the perspective's `sort` list to empty and writes it back. A no-op
/// on an already-unsorted perspective still succeeds. Undoable; emits a
/// `PerspectiveEvent`.
///
/// Returns `{ ok: true, perspective: <object>, entry_id: <string|null> }`.
#[operation(
    verb = "clear",
    noun = "sort",
    description = "Clear every sort entry on a perspective"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ClearSort {
    /// The perspective id to mutate.
    #[serde(default)]
    pub perspective_id: String,
}

/// Toggle the sort direction for a field on a perspective.
///
/// Cycles the field through `none → asc → desc → none`: absent fields become
/// ascending, ascending becomes descending, descending is removed. Writes the
/// updated `sort` list back. Undoable; emits a `PerspectiveEvent`.
///
/// Returns `{ ok: true, perspective: <object>, entry_id: <string|null> }`.
#[operation(
    verb = "toggle",
    noun = "sort",
    description = "Toggle the sort direction for a field on a perspective"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToggleSort {
    /// The perspective id to mutate.
    #[serde(default)]
    pub perspective_id: String,
    /// The field whose sort direction to cycle.
    #[serde(default)]
    pub field: String,
}

// Navigation operations ────────────────────────────────────────────────────

/// Resolve the next perspective in a view's ordered list.
///
/// Filters perspectives belonging to the given view (by `view_id` when
/// present, else by view kind), finds `current` in that ordered slice, and
/// returns the next one (wrapping). A no-op (`{ ok: true, perspective: null }`)
/// when fewer than two perspectives match.
///
/// This server holds no UIState, so navigation only *resolves* the target
/// perspective from the perspective context's ordered list; persisting the
/// active selection is the caller's (window/ui-state server's) concern.
#[operation(
    verb = "next",
    noun = "perspective",
    description = "Resolve the next perspective in a view's ordered list"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NextPerspective {
    /// The view kind to scope cycling to (e.g. "board", "grid").
    #[serde(default)]
    pub view: String,
    /// Optional view instance id to scope cycling to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_id: Option<String>,
    /// The currently-active perspective id to cycle from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
}

/// Resolve the previous perspective in a view's ordered list.
///
/// The reverse of [`NextPerspective`] — moves one position backward (wrapping)
/// within the perspectives matching the view. Same no-op semantics.
#[operation(
    verb = "prev",
    noun = "perspective",
    description = "Resolve the previous perspective in a view's ordered list"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PrevPerspective {
    /// The view kind to scope cycling to (e.g. "board", "grid").
    #[serde(default)]
    pub view: String,
    /// Optional view instance id to scope cycling to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_id: Option<String>,
    /// The currently-active perspective id to cycle from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
}

/// Resolve a perspective by id, optionally validating it belongs to a view.
///
/// Looks the perspective up by id; when `view` is supplied, verifies the
/// perspective belongs to that view (id-scoped perspectives match strictly by
/// id, legacy ones fall back to kind) and errors otherwise. Returns the
/// resolved perspective.
#[operation(
    verb = "goto",
    noun = "perspective",
    description = "Resolve a perspective by id, optionally validating its view"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GotoPerspective {
    /// The perspective id to resolve.
    #[serde(default)]
    pub id: String,
    /// Optional view kind to validate the perspective against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    /// Optional view instance id to validate the perspective against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_id: Option<String>,
}

/// Resolve a perspective by id and surface its filter for evaluation.
///
/// The state-layer half of the `perspective.switch` command: looks the
/// perspective up by id and returns it plus its filter expression. The
/// command layer pairs this with the entity filter evaluator + UIState write;
/// this server, holding neither, returns the filter so the caller can drive
/// evaluation.
#[operation(
    verb = "switch",
    noun = "perspective",
    description = "Resolve a perspective by id and surface its filter"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SwitchPerspective {
    /// The perspective id to switch to.
    #[serde(default)]
    pub perspective_id: String,
}

// View operations ──────────────────────────────────────────────────────────

/// Create or update a view definition.
///
/// Writes the supplied [`ViewDef`](crate::ViewDef) through
/// `ViewsContext::write_view`. When `id` is omitted a fresh ULID is minted.
/// The write is undoable (pushed onto the shared `StoreContext`) and emits a
/// `ViewEvent`.
///
/// Returns `{ ok: true, view: <object>, entry_id: <string|null> }`.
#[operation(verb = "set", noun = "view", description = "Create or update a view definition")]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetView {
    /// Optional explicit id. A fresh ULID is minted when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Human-readable name (e.g. "Board").
    #[serde(default)]
    pub name: String,
    /// View kind token (e.g. "board", "grid", "list").
    #[serde(default)]
    pub kind: String,
    /// Optional icon hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Optional entity type this view renders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
    /// Optional ordered list of card field names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub card_fields: Vec<String>,
}

/// All `views` operations — the canonical list used for schema generation.
///
/// Both the wire-schema generator and the discovery `_meta` generator are
/// driven from this single slice via the `operation_tool!` macro, so there is
/// one source of truth for what the `views` tool exposes.
static VIEWS_OPERATIONS: LazyLock<Vec<&'static dyn Operation>> = LazyLock::new(|| {
    vec![
        // lifecycle
        Box::leak(Box::<LoadPerspective>::default()) as &dyn Operation,
        Box::leak(Box::<SavePerspective>::default()) as &dyn Operation,
        Box::leak(Box::<DeletePerspective>::default()) as &dyn Operation,
        Box::leak(Box::<RenamePerspective>::default()) as &dyn Operation,
        Box::leak(Box::<ListPerspective>::default()) as &dyn Operation,
        // filter
        Box::leak(Box::<SetFilter>::default()) as &dyn Operation,
        Box::leak(Box::<FocusFilter>::default()) as &dyn Operation,
        Box::leak(Box::<ClearFilter>::default()) as &dyn Operation,
        // group
        Box::leak(Box::<SetGroup>::default()) as &dyn Operation,
        Box::leak(Box::<ClearGroup>::default()) as &dyn Operation,
        // sort
        Box::leak(Box::<SetSort>::default()) as &dyn Operation,
        Box::leak(Box::<ClearSort>::default()) as &dyn Operation,
        Box::leak(Box::<ToggleSort>::default()) as &dyn Operation,
        // nav
        Box::leak(Box::<NextPerspective>::default()) as &dyn Operation,
        Box::leak(Box::<PrevPerspective>::default()) as &dyn Operation,
        Box::leak(Box::<GotoPerspective>::default()) as &dyn Operation,
        Box::leak(Box::<SwitchPerspective>::default()) as &dyn Operation,
        // view
        Box::leak(Box::<SetView>::default()) as &dyn Operation,
    ]
});

/// Get the canonical slice of all `views` operations.
pub fn operations() -> &'static [&'static dyn Operation] {
    &VIEWS_OPERATIONS
}
