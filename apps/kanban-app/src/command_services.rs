//! App-wide bootstrap of the in-process MCP servers + command service.
//!
//! Called once during `AppState` construction (after `ui_state` and the
//! window shell are built but before plugin discovery), this helper
//! exposes the 5 MCP modules (`store`, `entity`, `ui_state`, `window`,
//! `focus`) the builtin command plugins will activate, and installs the
//! `commands` module with the production store-backed transaction seam.
//! The returned `Arc<CommandService>` is held by `AppState` so the
//! Tauri `dispatch_command` handler can route through it directly.
//!
//! ## Multi-board routing
//!
//! The `store` and `entity` servers use task-local resolvers
//! ([`task_local_store_resolver`] /
//! [`swissarmyhammer_entity_mcp::server::task_local_resolver`]) so a
//! single exposed module routes per-call to whichever board is scoped
//! on the current `tokio` task — the dispatcher's
//! `scope_store_context` + `scope_entity_board_services` set those
//! task-locals around the `service.dispatch(...)` call. The
//! `ui_state` / `window` / `focus` modules are app-wide and take their
//! single shared context at construction.

use std::sync::{Arc, OnceLock};

use swissarmyhammer_command_service::bootstrap::install_commands_module_with;
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_entity_mcp::server::{
    task_local_resolver as entity_task_local_resolver, EntityServer,
};
use swissarmyhammer_focus::{FocusChangedEvent, FocusEventSink, FocusServer};
use tauri::{AppHandle, Emitter};
use swissarmyhammer_kanban::command_seam::{task_local_store_resolver, StoreTransactionSeam};
use swissarmyhammer_plugin::{InProcessServer, McpServer, PluginHost};
use swissarmyhammer_store::StoreServer;
use swissarmyhammer_ui_state::{UIState, UiStateServer};
use swissarmyhammer_window_service::{WindowService, WindowShell};

/// Deferred [`AppHandle`] cell used by the focus event sink.
///
/// The `FocusServer` is wired during [`PluginPlatform::wire_command_services`]
/// — which runs from `AppState::new`, before the Tauri `AppHandle` exists.
/// The sink is installed at the same time so it shares the server's
/// lifetime; it reads its `AppHandle` out of this cell, which is filled
/// later from the `setup_app` Tauri hook via [`install_focus_event_app_handle`].
///
/// Events that arrive before the cell is filled (the brief window
/// between platform wiring and `setup_app`) are dropped, matching the
/// legacy behavior — the spatial Tauri commands could not emit events
/// either before a window existed.
static FOCUS_EVENT_APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Install the [`AppHandle`] the focus event sink uses to emit
/// `focus-changed` Tauri events.
///
/// Idempotent — only the first call wins. Call from `setup_app` once the
/// AppHandle is available.
pub fn install_focus_event_app_handle(app_handle: AppHandle) {
    let _ = FOCUS_EVENT_APP_HANDLE.set(app_handle);
}

/// [`FocusEventSink`] that mirrors every [`FocusChangedEvent`] onto the
/// Tauri `focus-changed` event, targeting the originating window.
///
/// Ports the side-effecting `emit` the legacy `spatial_*` Tauri commands
/// did (`apps/kanban-app/src/commands.rs::emit_focus_changed`). The
/// kernel directs each event to a single window via `emit_to` —
/// load-bearing because FQMs are not unique across windows (every
/// window's root layer is `/window`), so a broadcast would light up the
/// same scope in every window showing the same board.
struct TauriFocusEventSink;

impl FocusEventSink for TauriFocusEventSink {
    fn emit(&self, event: &FocusChangedEvent) {
        // The cell may not be filled yet during the brief platform-wiring
        // → setup_app window. Drop the event in that case — matches the
        // legacy behavior (Tauri commands couldn't emit before a window
        // existed either).
        let Some(app_handle) = FOCUS_EVENT_APP_HANDLE.get() else {
            return;
        };
        if let Err(e) = app_handle.emit_to(
            event.window_label.as_str(),
            "focus-changed",
            event,
        ) {
            tracing::warn!(
                window = %event.window_label,
                error = %e,
                "TauriFocusEventSink: failed to emit focus-changed"
            );
        }
    }
}

/// Install every in-process MCP module the builtin command plugins
/// activate, then install the `commands` module with the production
/// store-backed transaction seam.
///
/// Returns the `Arc<CommandService>` so `AppState` can hold it and the
/// Tauri `dispatch_command` handler can call `service.dispatch(...)`
/// directly without going through the rmcp `call_tool` plumbing.
///
/// Module ids exposed (in order — must complete before
/// `discover_and_load_all` is called so plugins find their modules at
/// activation time):
///
/// - `"store"` — multi-board, reads `CURRENT_STORE_CTX`.
/// - `"entity"` — multi-board, reads `CURRENT_ENTITY_BOARD_SERVICES`.
/// - `"ui_state"` — app-wide, captures `ui_state` at construction.
/// - `"window"` — app-wide, captures `window_shell` at construction.
///   Conditional: only exposed when `window_shell` is supplied —
///   deferred to the Tauri setup hook in the current bootstrap.
/// - `"focus"` — app-wide, no captured state in the no-arg form.
/// - `"commands"` — production seam = `StoreTransactionSeam::task_local()`.
///
/// # Errors
///
/// Returns the platform error message string when any `expose_rust_module`
/// or the `install_commands_module_with` call rejects an id (in practice,
/// an id already exposed — e.g. this helper called twice against the
/// same host).
pub async fn install_app_command_services(
    host: &PluginHost,
    ui_state: Arc<UIState>,
    window_shell: Option<Arc<dyn WindowShell>>,
) -> Result<Arc<CommandService>, String> {
    // store — multi-board via task-local resolver.
    let store_server: Arc<dyn McpServer> = Arc::new(
        InProcessServer::from_arc(Arc::new(StoreServer::with_resolver(
            task_local_store_resolver(),
        )))
        .await
        .map_err(|e| format!("wrap store as InProcessServer: {e}"))?,
    );
    host.expose_rust_module("store", store_server)
        .await
        .map_err(|e| format!("expose store module: {e}"))?;

    // entity — multi-board via task-local resolver.
    let entity_server: Arc<dyn McpServer> = Arc::new(
        InProcessServer::from_arc(Arc::new(EntityServer::with_resolver(
            entity_task_local_resolver(),
        )))
        .await
        .map_err(|e| format!("wrap entity as InProcessServer: {e}"))?,
    );
    host.expose_rust_module("entity", entity_server)
        .await
        .map_err(|e| format!("expose entity module: {e}"))?;

    // ui_state — app-wide, captures the shared UIState arc.
    let ui_state_server: Arc<dyn McpServer> = Arc::new(
        InProcessServer::from_arc(Arc::new(UiStateServer::new(ui_state)))
            .await
            .map_err(|e| format!("wrap ui_state as InProcessServer: {e}"))?,
    );
    host.expose_rust_module("ui_state", ui_state_server)
        .await
        .map_err(|e| format!("expose ui_state module: {e}"))?;

    // window — app-wide, captures the shared WindowShell arc. Conditional:
    // the kanban app's `AppState::new` calls this helper with `None` because
    // the Tauri `AppHandle` (needed to build the `WindowShell`) only exists
    // from the `setup_app` hook; the window module is wired later from there.
    if let Some(ws) = window_shell {
        let window_server: Arc<dyn McpServer> = Arc::new(
            InProcessServer::from_arc(Arc::new(WindowService::new(ws)))
                .await
                .map_err(|e| format!("wrap window as InProcessServer: {e}"))?,
        );
        host.expose_rust_module("window", window_server)
            .await
            .map_err(|e| format!("expose window module: {e}"))?;
    }

    // focus — app-wide. Attach a Tauri-event sink so every produced
    // `FocusChangedEvent` is mirrored onto the `focus-changed` Tauri
    // event the React `SpatialFocusProvider` listens on — restoring the
    // side-effecting `emit` the legacy `spatial_*` Tauri commands did.
    // The sink reads its AppHandle from a deferred cell that
    // `setup_app` fills via [`install_focus_event_app_handle`].
    let focus_server: Arc<dyn McpServer> = Arc::new(
        InProcessServer::from_arc(Arc::new(
            FocusServer::new().with_sink(Arc::new(TauriFocusEventSink)),
        ))
        .await
        .map_err(|e| format!("wrap focus as InProcessServer: {e}"))?,
    );
    host.expose_rust_module("focus", focus_server)
        .await
        .map_err(|e| format!("expose focus module: {e}"))?;

    // commands — production store-backed transaction seam.
    let seam = Arc::new(StoreTransactionSeam::task_local());
    install_commands_module_with(host, Some(seam))
        .await
        .map_err(|e| format!("install commands module: {e}"))
}
