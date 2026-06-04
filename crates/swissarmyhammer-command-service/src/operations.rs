//! The six `#[operation]` structs that make up the `command` operation tool.
//!
//! These structs are the source of truth for the tool's verb/noun/description/
//! parameters surface. The `_meta`-tree generator in
//! `swissarmyhammer-operations-macros` reads the `#[operation]` attribute plus
//! each field's doc comment to assemble the wire-level `inputSchema` and the
//! discovery `_meta` tree.

use crate::types::{CallbackMarker, CommandContext, ParamDef};
use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;
use swissarmyhammer_operations::{notification, operation, Notification, Operation};

/// Register a new command (or replace this caller's existing entry for the
/// same id).
///
/// The fields below mirror every field today's command YAML supports, so
/// built-in plugins can register without losing fidelity. Re-registration
/// by the same caller with the same id replaces that caller's entry in
/// place — it never produces a duplicate stack entry.
///
/// This struct IS the registration payload; [`crate::CommandRegistration`]
/// is a re-export of it for callers who refer to "the registration data"
/// outside the operation-tool context.
#[operation(
    verb = "register",
    noun = "command",
    description = "Register a new command (or replace this caller's existing entry for the same id)"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RegisterCommand {
    /// Stable command id, e.g. `"task.move"`. Must be non-empty.
    pub id: String,
    /// Human-readable name (palette / menu label). May contain template
    /// variables that the renderer resolves at display time.
    pub name: String,
    /// Optional display name override for native menus. Falls back to
    /// [`Self::name`] when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu_name: Option<String>,
    /// Optional long-form description (palette detail row, tooltip).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional category for grouping (e.g. `"Cleanup"`, `"Navigation"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Scope expression list (e.g. `["entity:task"]`). Empty / absent
    /// means the command is global.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Vec<String>>,
    /// Keybindings keyed by keymap mode (e.g. `vim`, `cua`, `emacs`).
    /// Kept as a free-form map so new keymap modes can be added without a
    /// schema change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<HashMap<String, String>>,
    /// Native menu-bar placement payload. Free-form `Value` so
    /// downstream surfaces can evolve the placement schema independently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu: Option<Value>,
    /// Whether this command appears in the right-click context menu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_menu: Option<bool>,
    /// Context-menu group bucket (commands with the same group render
    /// contiguously, separator between groups).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_menu_group: Option<u32>,
    /// Sort order within the same context-menu group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_menu_order: Option<u32>,
    /// Tab-button affordance payload. Free-form `Value` because the
    /// supported icon names / placement metadata evolve independently of
    /// the command schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_button: Option<Value>,
    /// View-kind UI-surface filter (e.g. `["grid"]` to restrict emission
    /// to grid views).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_kinds: Option<Vec<String>>,
    /// Whether the command produces an undoable change. Defaults to
    /// `false` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undoable: Option<bool>,
    /// Whether the command appears in palettes / context menus / native
    /// menus. Defaults to `true` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    /// Param definitions. None or empty means the command takes no
    /// dispatch-time arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<ParamDef>>,
    /// Optional `available` callback (returns whether the command can
    /// currently run). Absent means the command is always available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available: Option<CallbackMarker>,
    /// Required `execute` callback (runs the command's effect).
    pub execute: CallbackMarker,
}

impl Default for RegisterCommand {
    /// Default-constructs a [`RegisterCommand`] with empty `id` / `name`
    /// and a sentinel `execute` callback id (`""`).
    ///
    /// Used solely for the `#[operation]` macro's static slice — the
    /// runtime never accepts a [`RegisterCommand`] whose `id`, `name`,
    /// or `execute` callback id are empty. The verb handler validates
    /// each field on incoming payloads.
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            menu_name: None,
            description: None,
            category: None,
            scope: None,
            keys: None,
            menu: None,
            context_menu: None,
            context_menu_group: None,
            context_menu_order: None,
            tab_button: None,
            view_kinds: None,
            undoable: None,
            visible: None,
            params: None,
            available: None,
            execute: CallbackMarker::new(""),
        }
    }
}

/// Unregister this caller's entry for the given command id.
///
/// If the caller has no entry for the id, the call is a no-op success —
/// plugin unload purges may race with explicit `unregister` and the latter
/// should not produce a hard error in that case.
#[operation(
    verb = "unregister",
    noun = "command",
    description = "Unregister this caller's entry for the given command id"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UnregisterCommand {
    /// The command id to unregister.
    #[serde(default)]
    pub id: String,
}

/// Execute a registered command.
///
/// Resolves the active stack entry for the id, rechecks `available` (unless
/// `force` is true), and invokes the `execute` callback in the registering
/// caller's isolate. Returns the callback's result.
#[operation(
    verb = "execute",
    noun = "command",
    description = "Execute a registered command via its execute callback"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExecuteCommand {
    /// The command id to execute.
    #[serde(default)]
    pub id: String,
    /// Execution context (scope chain, target, args bag).
    #[serde(default)]
    pub ctx: CommandContext,
    /// When true, skip the `available` recheck and run regardless.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,
}

/// Ask whether a registered command can currently run.
///
/// Invokes the command's `available` callback (if any) with the given
/// context and returns its boolean result, or a structured
/// `{ ok: false, reason }` payload.
#[operation(
    verb = "available",
    noun = "command",
    description = "Ask whether a registered command can currently run for the given context"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AvailableCommand {
    /// The command id to check.
    #[serde(default)]
    pub id: String,
    /// Execution context (scope chain, target, args bag).
    #[serde(default)]
    pub ctx: CommandContext,
}

/// List active (top-of-stack) commands, optionally filtered.
///
/// All filters intersect — passing `scope` + `category` returns only
/// commands matching both. With no filters, returns every active entry.
#[operation(
    verb = "list",
    noun = "command",
    description = "List active (top-of-stack) commands, optionally filtered by scope, category, or id prefix"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ListCommand {
    /// Filter to commands whose `scope` field is empty (global) or
    /// contains this expression (e.g. `"entity:task"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Exact-match filter on the command's `category`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Filter to commands whose id starts with this prefix
    /// (e.g. `"task."`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id_prefix: Option<String>,
}

/// Return the param schema for one registered command.
///
/// Returns the command's `params` array as registered. Used by surfaces
/// that need to render param-collection UI (popovers, palette argument
/// rows) without round-tripping the full registration.
#[operation(
    verb = "schema",
    noun = "command",
    description = "Return the param schema for one registered command"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SchemaCommand {
    /// The command id whose schema is requested.
    #[serde(default)]
    pub id: String,
}

/// All command operations — the canonical list used for schema generation.
///
/// Holds one instance of each of the six operation structs. Both the
/// wire-schema generator (`generate_mcp_schema`) and the discovery `_meta`
/// generator (`generate_operations_meta`) are driven from this single slice
/// — there is one source of truth for what the `command` tool exposes.
///
/// Matches the `LazyLock<Vec<&'static dyn Operation>>` pattern used by
/// `crates/swissarmyhammer-kanban/src/schema.rs::KANBAN_OPERATIONS` so the
/// "static slice of operation trait objects" convention stays consistent
/// across the workspace.
static COMMAND_OPERATIONS: LazyLock<Vec<&'static dyn Operation>> = LazyLock::new(|| {
    vec![
        Box::leak(Box::<RegisterCommand>::default()) as &dyn Operation,
        Box::leak(Box::<UnregisterCommand>::default()) as &dyn Operation,
        Box::leak(Box::<ExecuteCommand>::default()) as &dyn Operation,
        Box::leak(Box::<AvailableCommand>::default()) as &dyn Operation,
        Box::leak(Box::<ListCommand>::default()) as &dyn Operation,
        Box::leak(Box::<SchemaCommand>::default()) as &dyn Operation,
    ]
});

/// Get the canonical slice of all command operations.
pub fn operations() -> &'static [&'static dyn Operation] {
    &COMMAND_OPERATIONS
}

/// The `notifications/commands/executed` event payload.
///
/// This struct is the single source of truth for the event: it IS the published
/// payload (it serializes to the notification's `params` via
/// [`McpNotification::from_declared`](swissarmyhammer_plugin::McpNotification::from_declared))
/// AND the declaration the SDK reads (its fields drive the
/// `io.swissarmyhammer/notifications` `_meta`). The two cannot drift. Emitted
/// after every successful command execution (see
/// [`build_commands_executed`](crate::txn::build_commands_executed)).
///
/// Provenance (`txn`/`origin`) is universal cross-cutting metadata stamped on
/// every notification at publish time; it is intentionally NOT a field here.
#[notification(
    method = "notifications/commands/executed",
    description = "A command finished executing successfully."
)]
#[derive(Debug, Default, Serialize)]
pub(crate) struct CommandsExecuted {
    /// The id of the command that executed.
    pub id: String,
    /// The execution context the command ran with.
    pub ctx: Value,
    /// The command's return value.
    pub result: Value,
}

/// The canonical slice of notifications the `command` tool emits.
///
/// Mirrors [`operations`]: a leaked `Default` instance per notification, used
/// only for its static metadata. Fed to `operation_tool!`'s `notifications:`
/// field so the tool advertises its events in `_meta`.
static COMMAND_NOTIFICATIONS: LazyLock<Vec<&'static dyn Notification>> =
    LazyLock::new(|| vec![Box::leak(Box::<CommandsExecuted>::default()) as &dyn Notification]);

/// Get the canonical slice of all command notifications.
pub fn command_notifications() -> &'static [&'static dyn Notification] {
    &COMMAND_NOTIFICATIONS
}
