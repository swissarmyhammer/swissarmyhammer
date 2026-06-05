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

use swissarmyhammer_app_service::{AppService, AppShell};
use swissarmyhammer_command_service::bootstrap::install_commands_module_with;
use swissarmyhammer_command_service::CommandMetadata;
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_entity_mcp::server::{
    task_local_resolver as entity_task_local_resolver, EntityServer,
};
use swissarmyhammer_focus::{FocusChangedEvent, FocusEventSink, FocusServer};
use swissarmyhammer_kanban::command_seam::{task_local_store_resolver, StoreTransactionSeam};
use swissarmyhammer_plugin::{InProcessServer, McpServer, PluginHost};
use swissarmyhammer_store::StoreServer;
use swissarmyhammer_ui_state::{UIState, UiStateServer};
use swissarmyhammer_views::{task_local_resolver as views_task_local_resolver, ViewsServer};
use swissarmyhammer_window_service::{WindowService, WindowShell};
use tauri::{AppHandle, Emitter};

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
        if let Err(e) = app_handle.emit_to(event.window_label.as_str(), "focus-changed", event) {
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
    app_shell: Option<Arc<dyn AppShell>>,
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

    // views — multi-board via task-local resolver. Perspectives and views are
    // per-board kernels (each pushes onto its board's StoreContext), so the
    // single exposed module resolves the active board's pair per call from
    // `CURRENT_VIEWS_BOARD_SERVICES`, which the dispatcher scopes alongside
    // `store` / `entity`. No Tauri AppHandle is needed, so this is wired in the
    // no-AppHandle path here (unlike `window` / `app`).
    let views_server: Arc<dyn McpServer> = Arc::new(
        InProcessServer::from_arc(Arc::new(ViewsServer::with_resolver(
            views_task_local_resolver(),
        )))
        .await
        .map_err(|e| format!("wrap views as InProcessServer: {e}"))?,
    );
    host.expose_rust_module("views", views_server)
        .await
        .map_err(|e| format!("expose views module: {e}"))?;

    // ui_state — app-wide, captures the shared UIState arc.
    let ui_state_server: Arc<dyn McpServer> = Arc::new(
        InProcessServer::from_arc(Arc::new(UiStateServer::new(ui_state)))
            .await
            .map_err(|e| format!("wrap ui_state as InProcessServer: {e}"))?,
    );
    host.expose_rust_module("ui_state", ui_state_server)
        .await
        .map_err(|e| format!("expose ui_state module: {e}"))?;

    // window + app — both AppShell/WindowShell-backed (Tauri `AppHandle`).
    // Conditional: the kanban app's `AppState::new` calls this helper with
    // `None` for both because the `AppHandle` only exists from the `setup_app`
    // hook; they are wired later from there via [`expose_apphandle_modules`].
    expose_apphandle_modules(host, window_shell, app_shell).await?;

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

/// Expose the `AppHandle`-backed `window` and `app` modules on `host`.
///
/// The `WindowShell` / `AppShell` seams both require a live Tauri `AppHandle`,
/// which does not exist when [`install_app_command_services`] first runs from
/// `AppState::new`. This helper is therefore called twice:
///
/// 1. From inside [`install_app_command_services`] with `None`/`None` (the
///    no-AppHandle bootstrap) — a no-op.
/// 2. From the `setup_app` hook with the constructed shells, BEFORE the global
///    host's deferred plugin discovery, so the `file-commands` / `ui-commands`
///    / `kanban-misc-commands` / `app-shell-commands` builtin plugins find
///    their `window` / `app` backends already exposed at `ensureServices` time.
///
/// Each module is exposed only when its shell is supplied; a `None` shell skips
/// that module so the helper is safe to call in either phase.
///
/// # Errors
///
/// Returns the platform error string when `expose_rust_module` rejects an id
/// (in practice, an id already exposed — e.g. exposing `window`/`app` twice).
pub async fn expose_apphandle_modules(
    host: &PluginHost,
    window_shell: Option<Arc<dyn WindowShell>>,
    app_shell: Option<Arc<dyn AppShell>>,
) -> Result<(), String> {
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

    if let Some(app_shell) = app_shell {
        let app_server: Arc<dyn McpServer> = Arc::new(
            InProcessServer::from_arc(Arc::new(AppService::new(app_shell)))
                .await
                .map_err(|e| format!("wrap app as InProcessServer: {e}"))?,
        );
        host.expose_rust_module("app", app_server)
            .await
            .map_err(|e| format!("expose app module: {e}"))?;
    }

    Ok(())
}

/// Project one [`CommandMetadata`] (the callback-free shape `CommandService`
/// exposes) onto the synchronous [`CommandDef`] the legacy
/// [`CommandsRegistry`] façade stores.
///
/// `CommandService` is the sole source of command metadata after the Stage 4
/// cut-over, but three synchronous callers still read the `CommandsRegistry`
/// snapshot: the dispatch undoable-gate (`lookup_undoable`), scope/keybinding
/// listing (`list_commands_for_scope`), and the native menu builder. This
/// conversion lets [`build_registry_from_metadata`] repopulate that façade
/// from the live service after plugin discovery so those callers see every
/// builtin-plugin command.
///
/// Field shapes differ between the two types and are bridged here:
///
/// - `scope: Option<Vec<String>>` → `Option<String>` (comma-joined; the
///   `CommandsRegistry` scope grammar is comma-separated `entity:type`).
/// - `keys: Option<HashMap<mode, key>>` → [`KeysDef`] (`vim` / `cua` /
///   `emacs` picked out by mode name; unknown modes are dropped).
/// - `menu` / `tab_button`: `Option<Value>` deserialized into their typed
///   counterparts — the plugin-registered JSON already matches the
///   [`MenuPlacement`] / [`TabButtonDef`] shape.
/// - `undoable` / `context_menu` / `visible`: `Option<bool>` unwrapped to the
///   `CommandDef` defaults (`false` / `false` / `true`) when absent.
/// - `params`: re-serialized through serde — both crates' `ParamDef` types
///   are structurally identical and share the same wire shape.
///
/// `description` / `category` carry no `CommandDef` field and are dropped.
///
/// Returns `None` when a sub-payload fails to deserialize (a malformed
/// `menu` / `tab_button` / `params`); the caller logs and skips it rather
/// than poisoning the whole snapshot.
fn command_metadata_to_def(
    meta: &CommandMetadata,
) -> Option<swissarmyhammer_kanban::commands_core::CommandDef> {
    use swissarmyhammer_kanban::commands_core::{
        CommandDef, KeysDef, MenuPlacement, ParamDef, TabButtonDef,
    };

    let keys = meta.keys.as_ref().map(|map| KeysDef {
        vim: map.get("vim").cloned(),
        cua: map.get("cua").cloned(),
        emacs: map.get("emacs").cloned(),
    });

    let scope = meta
        .scope
        .as_ref()
        .filter(|s| !s.is_empty())
        .map(|s| s.join(","));

    let menu: Option<MenuPlacement> = match &meta.menu {
        Some(value) => Some(serde_json::from_value(value.clone()).ok()?),
        None => None,
    };

    let tab_button: Option<TabButtonDef> = match &meta.tab_button {
        Some(value) => Some(serde_json::from_value(value.clone()).ok()?),
        None => None,
    };

    let params: Vec<ParamDef> = match &meta.params {
        Some(list) => serde_json::from_value(serde_json::to_value(list).ok()?).ok()?,
        None => Vec::new(),
    };

    Some(CommandDef {
        id: meta.id.clone(),
        name: meta.name.clone(),
        menu_name: meta.menu_name.clone(),
        scope,
        visible: meta.visible.unwrap_or(true),
        keys,
        params,
        undoable: meta.undoable.unwrap_or(false),
        context_menu: meta.context_menu.unwrap_or(false),
        context_menu_group: meta.context_menu_group,
        context_menu_order: meta.context_menu_order,
        menu,
        view_kinds: meta.view_kinds.clone(),
        tab_button,
    })
}

/// Build a [`CommandsRegistry`] snapshot from a live [`CommandService`]'s
/// command catalogue.
///
/// Each [`CommandMetadata`] is projected onto a [`CommandDef`] via
/// [`command_metadata_to_def`]; entries whose sub-payloads fail to
/// deserialize are logged and skipped so one malformed command never empties
/// the snapshot. The result is the synchronous façade the menu / scope /
/// undoable-gate callers read until they migrate to the MCP path.
pub fn build_registry_from_metadata(
    metadata: &[CommandMetadata],
) -> swissarmyhammer_kanban::commands_core::CommandsRegistry {
    let defs: Vec<swissarmyhammer_kanban::commands_core::CommandDef> = metadata
        .iter()
        .filter_map(|meta| {
            let def = command_metadata_to_def(meta);
            if def.is_none() {
                tracing::warn!(id = %meta.id, "skipping command with un-mappable metadata");
            }
            def
        })
        .collect();
    swissarmyhammer_kanban::commands_core::CommandsRegistry::from_defs(defs)
}

#[cfg(test)]
mod registry_population_tests {
    use super::*;
    use std::collections::HashMap;
    use swissarmyhammer_command_service::CommandMetadata;

    /// A populated [`CommandMetadata`] maps every façade-relevant field onto
    /// the [`CommandDef`] shape the synchronous callers read — the regression
    /// guard for the command-dispatch bug where the registry was left empty
    /// and `lookup_undoable` rejected every plugin-registered command.
    #[test]
    fn metadata_maps_onto_command_def() {
        let mut keys = HashMap::new();
        keys.insert("vim".to_string(), ":".to_string());
        keys.insert("cua".to_string(), "Mod+Shift+P".to_string());

        let meta = CommandMetadata {
            id: "ui.setFocus".to_string(),
            name: "Set Focus".to_string(),
            menu_name: None,
            description: Some("dropped".to_string()),
            category: Some("dropped".to_string()),
            scope: Some(vec!["entity:task".to_string(), "entity:column".to_string()]),
            keys: Some(keys),
            menu: Some(serde_json::json!({ "path": ["Edit"], "group": 0, "order": 2 })),
            context_menu: Some(true),
            context_menu_group: Some(1),
            context_menu_order: Some(3),
            tab_button: None,
            view_kinds: Some(vec!["grid".to_string()]),
            undoable: Some(true),
            visible: Some(true),
            params: None,
        };

        let def = command_metadata_to_def(&meta).expect("metadata should map");
        assert_eq!(def.id, "ui.setFocus");
        assert_eq!(def.name, "Set Focus");
        // scope vec is comma-joined into the CommandsRegistry grammar.
        assert_eq!(def.scope.as_deref(), Some("entity:task,entity:column"));
        // keys are picked out by mode name.
        let k = def.keys.expect("keys present");
        assert_eq!(k.vim.as_deref(), Some(":"));
        assert_eq!(k.cua.as_deref(), Some("Mod+Shift+P"));
        assert!(k.emacs.is_none());
        // menu deserializes into the typed placement.
        let menu = def.menu.expect("menu present");
        assert_eq!(menu.path, vec!["Edit".to_string()]);
        assert_eq!(menu.order, 2);
        assert!(def.undoable);
        assert!(def.context_menu);
        assert_eq!(def.context_menu_group, Some(1));
        assert_eq!(def.context_menu_order, Some(3));
        assert_eq!(def.view_kinds.as_deref(), Some(&["grid".to_string()][..]));
    }

    /// A minimal metadata (only id + name, all options `None`) maps with the
    /// `CommandDef` serde defaults: `visible = true`, `undoable = false`,
    /// `context_menu = false`, empty params, no scope.
    #[test]
    fn minimal_metadata_uses_command_def_defaults() {
        let meta = CommandMetadata {
            id: "app.quit".to_string(),
            name: "Quit".to_string(),
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
        };

        let def = command_metadata_to_def(&meta).expect("metadata should map");
        assert!(def.scope.is_none());
        assert!(def.keys.is_none());
        assert!(def.visible);
        assert!(!def.undoable);
        assert!(!def.context_menu);
        assert!(def.params.is_empty());
    }

    /// `build_registry_from_metadata` produces a registry whose `get` /
    /// `undoable` lookups succeed for every mapped command — the exact path
    /// `lookup_undoable` exercises at dispatch time.
    #[test]
    fn registry_population_makes_commands_resolvable() {
        let meta = vec![
            CommandMetadata {
                id: "perspective.list".to_string(),
                name: "List Perspectives".to_string(),
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
                undoable: Some(false),
                visible: None,
                params: None,
            },
            CommandMetadata {
                id: "task.add".to_string(),
                name: "Add Task".to_string(),
                menu_name: None,
                description: None,
                category: None,
                scope: Some(vec!["entity:column".to_string()]),
                keys: None,
                menu: None,
                context_menu: None,
                context_menu_group: None,
                context_menu_order: None,
                tab_button: None,
                view_kinds: None,
                undoable: Some(true),
                visible: None,
                params: None,
            },
        ];

        let registry = build_registry_from_metadata(&meta);
        // Both commands resolve — the dispatch gate would no longer reject them.
        assert!(registry.get("perspective.list").is_some());
        assert!(!registry.get("perspective.list").unwrap().undoable);
        let task_add = registry.get("task.add").expect("task.add present");
        assert!(task_add.undoable);
        assert_eq!(task_add.scope.as_deref(), Some("entity:column"));
    }
}
