//! The `#[operation]` structs that make up the `app` operation tool.
//!
//! These structs are the source of truth for the tool's verb / noun /
//! description / parameters surface. Both the wire-level `inputSchema`
//! generator and the discovery `_meta` tree generator are driven from the
//! same `APP_OPERATIONS` slice via the `operation_tool!` macro, so the two
//! cannot drift.
//!
//! The `app` server exposes only genuine app-shell actions — actions that
//! belong to the window manager / OS chrome rather than to any document or
//! UI panel:
//!
//! - **quit** (`quit app`) — terminate the process.
//! - **about** (`show about`) — surface the app's name / version.
//! - **help** (`show help`) — route the user to the help / docs.
//!
//! Undo / redo are deliberately absent — those are store-layer concerns and
//! live on the `store` MCP server. UI panel toggles (command palette, search)
//! live on the `ui_state` server.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use swissarmyhammer_operations::{operation, Operation};

/// Quit the application.
///
/// Ports the existing `quit_app` Tauri command (`apps/kanban-app/src/
/// commands.rs`), which calls `AppHandle::exit(0)`. Routing it through the
/// `AppShell` seam keeps the exit-with-code-0 behavior while making the
/// action testable without a live GUI.
///
/// Returns `{ ok: true }`.
#[operation(
    verb = "quit",
    noun = "app",
    description = "Quit the application (terminates the process with exit code 0)"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct QuitApp {}

/// Show information about the application.
///
/// Returns the app's display name and version, read from the running
/// process's package metadata. The frontend renders this as an about dialog;
/// returning structured data avoids forcing a native dialog (which is heavy
/// and hard to test).
///
/// Returns `{ ok: true, name: <string>, version: <string> }`.
#[operation(
    verb = "show",
    noun = "about",
    description = "Show information about the application (name and version)"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ShowAbout {}

/// Open the application's help / documentation.
///
/// Signals the shell to route the user to the help target. The shell decides
/// how (e.g. by emitting an event the frontend handles, or opening a docs
/// URL). The op returns the help target so callers can render or follow it.
///
/// Returns `{ ok: true, target: <string> }`.
#[operation(
    verb = "show",
    noun = "help",
    description = "Open the application's help / documentation"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ShowHelp {}

/// All app-shell operations — the canonical list used for schema generation.
///
/// Both the wire-schema generator (`generate_mcp_schema`) and the discovery
/// `_meta` generator (`generate_operations_meta`) are driven from this single
/// slice via the `operation_tool!` macro, so there is one source of truth for
/// what the `app` tool exposes.
static APP_OPERATIONS: LazyLock<Vec<&'static dyn Operation>> = LazyLock::new(|| {
    vec![
        Box::leak(Box::<QuitApp>::default()) as &dyn Operation,
        Box::leak(Box::<ShowAbout>::default()) as &dyn Operation,
        Box::leak(Box::<ShowHelp>::default()) as &dyn Operation,
    ]
});

/// Get the canonical slice of all app-shell operations.
pub fn operations() -> &'static [&'static dyn Operation] {
    &APP_OPERATIONS
}
