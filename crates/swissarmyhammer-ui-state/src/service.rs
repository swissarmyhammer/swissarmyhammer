//! In-process `rmcp::ServerHandler` for the `ui_state` operation tool.
//!
//! [`UiStateServer`] wraps an [`Arc<UIState>`](crate::state::UIState) and
//! advertises a single `ui_state` operation tool whose `inputSchema` and
//! `_meta` are derived from the operation structs in [`crate::operations`].
//! Every verb routes to the matching mutating method on the wrapped
//! [`UIState`], so behavior is a 1:1 port of the original command layer with
//! no behavior change.
//!
//! The server holds the `UIState` directly (no extra seam) because `UIState`
//! is itself the injectable, file-backed state machine: construct it with
//! [`UiStateServer::new`] over a [`UIState::load(path)`](crate::state::UIState::load)
//! for production persistence, or over a temp-file-backed `UIState` in tests.
//!
//! There is no focus / `set_focus` op — spatial focus lives on the separate
//! `focus` MCP server.

use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde::de::DeserializeOwned;
use serde_json::Value;
use swissarmyhammer_operations_macros::operation_tool;

use crate::operations::{
    operations, Dismiss, DragCancel, DragComplete, DragStart, Inspect, InspectorClose,
    InspectorCloseAll, InspectorSetWidth, PaletteClose, PaletteOpen, SetActiveView, SetKeymapMode,
    SetScopeChain, ShowCommand, ShowPalette, ShowSearch, StartRename,
};
use crate::state::{DragSession, DragSource, UIState};

/// Minimum inspector width enforced by `set_width`.
///
/// Mirrors the `MIN_INSPECTOR_WIDTH` clamp the original `InspectorSetWidthCmd`
/// applied so a direct dispatch with `width: 1` cannot persist a 1 px panel.
const MIN_INSPECTOR_WIDTH: u32 = 320;

/// Absolute upper clamp on inspector width.
///
/// Mirrors the original `MAX_INSPECTOR_WIDTH` constant.
const MAX_INSPECTOR_WIDTH: u32 = 800;

/// The window label used when a scope chain carries no `window:` moniker.
///
/// Matches the `ctx.window_label_from_scope().unwrap_or("main")` fallback the
/// original command layer used.
const DEFAULT_WINDOW_LABEL: &str = "main";

/// Resolve the target window label from a scope chain.
///
/// The window is carried in the scope chain as a `window:<label>` moniker — the
/// scope chain is the single structured parameter every per-window op receives,
/// so the window is read from it rather than from a redundant denormalized
/// `window_label` field. Returns the first `window:` moniker's label, or
/// [`DEFAULT_WINDOW_LABEL`] when the chain carries none.
fn window_from_scope(scope_chain: &[String]) -> &str {
    scope_chain
        .iter()
        .find_map(|m| m.strip_prefix("window:"))
        .unwrap_or(DEFAULT_WINDOW_LABEL)
}

/// In-process `rmcp::ServerHandler` for the `ui_state` operation tool.
///
/// Holds an `Arc<UIState>` so every verb routes through the same shared,
/// file-backed state. Cloning the server shares the underlying `UIState`.
#[derive(Clone)]
pub struct UiStateServer {
    /// The shared UI state every op mutates. File-backed when constructed
    /// over a `UIState::load(path)`; in-memory when constructed over
    /// `UIState::new()`.
    ui_state: Arc<UIState>,
}

impl std::fmt::Debug for UiStateServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiStateServer")
            .field("ui_state", &self.ui_state)
            .finish()
    }
}

impl UiStateServer {
    /// Construct a server over a shared [`UIState`].
    ///
    /// Production wires a `UIState::load(~/.swissarmyhammer/ui-state.json)`;
    /// tests wire a temp-file-backed `UIState` so no real home dir is touched.
    pub fn new(ui_state: Arc<UIState>) -> Self {
        Self { ui_state }
    }

    /// Borrow the wrapped [`UIState`] (used by tests to observe persisted
    /// state after driving an op).
    pub fn ui_state(&self) -> &Arc<UIState> {
        &self.ui_state
    }

    /// Build the platform-facing `ui_state` tool definition.
    ///
    /// The `inputSchema` is the flat `op` enum derived from the operation
    /// structs in [`crate::operations`]; the `_meta` tree under
    /// `io.swissarmyhammer/operations` is the discovery surface. Both come
    /// from the same operation slice via the `operation_tool!` macro, so they
    /// cannot drift.
    fn build_tool_definition() -> Tool {
        operation_tool! {
            name: "ui_state",
            description: "UI-state mutations: inspector, palette, keymap, rename, drag, and app-UI toggles.",
            operations: operations(),
        }
    }

    /// Handle `inspect inspector` — push a moniker onto the window's stack.
    fn handle_inspect(&self, req: Inspect) -> Result<Value, McpError> {
        let window = window_from_scope(&req.scope_chain);
        let change = self.ui_state.inspect(window, &req.moniker);
        Ok(serde_json::json!({ "ok": true, "change": change }))
    }

    /// Handle `close inspector` — pop the topmost entry.
    fn handle_inspector_close(&self, req: InspectorClose) -> Result<Value, McpError> {
        let window = window_from_scope(&req.scope_chain);
        let change = self.ui_state.inspector_close(window);
        Ok(serde_json::json!({ "ok": true, "change": change }))
    }

    /// Handle `close_all inspector` — clear the stack.
    fn handle_inspector_close_all(&self, req: InspectorCloseAll) -> Result<Value, McpError> {
        let window = window_from_scope(&req.scope_chain);
        let change = self.ui_state.inspector_close_all(window);
        Ok(serde_json::json!({ "ok": true, "change": change }))
    }

    /// Handle `set_width inspector` — persist the clamped panel width.
    fn handle_inspector_set_width(&self, req: InspectorSetWidth) -> Result<Value, McpError> {
        let window = window_from_scope(&req.scope_chain);
        let width = req.width.clamp(MIN_INSPECTOR_WIDTH, MAX_INSPECTOR_WIDTH);
        let change = self.ui_state.set_inspector_width(window, width);
        Ok(serde_json::json!({ "ok": true, "change": change }))
    }

    /// Handle `open palette` — open the palette in the requested mode.
    fn handle_palette_open(&self, req: PaletteOpen) -> Result<Value, McpError> {
        let window = window_from_scope(&req.scope_chain);
        let change = self
            .ui_state
            .set_palette_open_with_mode(window, true, &req.mode);
        Ok(serde_json::json!({ "ok": true, "change": change }))
    }

    /// Handle `close palette` — close the palette.
    fn handle_palette_close(&self, req: PaletteClose) -> Result<Value, McpError> {
        let window = window_from_scope(&req.scope_chain);
        let change = self.ui_state.set_palette_open(window, false);
        Ok(serde_json::json!({ "ok": true, "change": change }))
    }

    /// Handle `set keymap` — set the active keymap mode.
    fn handle_set_keymap_mode(&self, req: SetKeymapMode) -> Result<Value, McpError> {
        let change = self.ui_state.set_keymap_mode(&req.mode);
        Ok(serde_json::json!({ "ok": true, "change": change }))
    }

    /// Handle `set scope_chain` — record the focus scope chain (`ui.setFocus`).
    fn handle_set_scope_chain(&self, req: SetScopeChain) -> Result<Value, McpError> {
        let change = self.ui_state.set_scope_chain(req.scope_chain);
        Ok(serde_json::json!({ "ok": true, "change": change }))
    }

    /// Handle `set active_view` — record the window's active view and keep the
    /// recorded focus scope chain's `view:*` monikers pointed at it.
    ///
    /// Ports the original `SetActiveViewCmd`: without the scope-chain rewrite the
    /// palette / context menu keep offering commands for whichever view was last
    /// in scope, so `entity.add:{type}` and friends fan out from the stale view.
    fn handle_set_active_view(&self, req: SetActiveView) -> Result<Value, McpError> {
        let window = window_from_scope(&req.scope_chain);
        let change = self.ui_state.set_active_view(window, &req.view_id);

        let mut chain = self.ui_state.scope_chain();
        let new_moniker = format!("view:{}", req.view_id);
        let mut mutated = false;
        for moniker in &mut chain {
            if moniker.starts_with("view:") && *moniker != new_moniker {
                *moniker = new_moniker.clone();
                mutated = true;
            }
        }
        if mutated {
            self.ui_state.set_scope_chain(chain);
        }

        Ok(serde_json::json!({ "ok": true, "change": change }))
    }

    /// Handle `start rename` — backend no-op (frontend-handled).
    fn handle_start_rename(&self, _req: StartRename) -> Result<Value, McpError> {
        Ok(serde_json::json!({ "ok": true }))
    }

    /// Handle `start drag` — replace any active session with a new one.
    fn handle_drag_start(&self, req: DragStart) -> Result<Value, McpError> {
        // Cancel any existing session before starting a new one (matches the
        // original `DragStartCmd`).
        self.ui_state.cancel_drag();
        let session = DragSession {
            session_id: req.session_id,
            from: DragSource::FocusChain {
                entity_type: req.entity_type,
                entity_id: req.entity_id,
                fields: Value::Null,
                source_board_path: req.source_board_path,
                source_window_label: req.source_window_label,
            },
            copy_mode: req.copy_mode,
            started_at_ms: req.started_at_ms,
        };
        self.ui_state.start_drag(session.clone());
        Ok(serde_json::json!({ "ok": true, "session": session }))
    }

    /// Handle `cancel drag` — clear the active session.
    fn handle_drag_cancel(&self, _req: DragCancel) -> Result<Value, McpError> {
        self.ui_state.cancel_drag();
        Ok(serde_json::json!({ "ok": true }))
    }

    /// Handle `complete drag` — take and return the active session.
    fn handle_drag_complete(&self, _req: DragComplete) -> Result<Value, McpError> {
        let session = self.ui_state.take_drag();
        Ok(serde_json::json!({ "ok": true, "session": session }))
    }

    /// Handle `show command` — open the palette in command mode.
    fn handle_show_command(&self, req: ShowCommand) -> Result<Value, McpError> {
        let window = window_from_scope(&req.scope_chain);
        let change = self
            .ui_state
            .set_palette_open_with_mode(window, true, "command");
        Ok(serde_json::json!({ "ok": true, "change": change }))
    }

    /// Handle `show palette` — open the palette without forcing a mode.
    fn handle_show_palette(&self, req: ShowPalette) -> Result<Value, McpError> {
        let window = window_from_scope(&req.scope_chain);
        let change = self.ui_state.set_palette_open(window, true);
        Ok(serde_json::json!({ "ok": true, "change": change }))
    }

    /// Handle `show search` — open the palette in search mode.
    fn handle_show_search(&self, req: ShowSearch) -> Result<Value, McpError> {
        let window = window_from_scope(&req.scope_chain);
        let change = self
            .ui_state
            .set_palette_open_with_mode(window, true, "search");
        Ok(serde_json::json!({ "ok": true, "change": change }))
    }

    /// Handle `dismiss ui` — layered close: palette first, then inspector.
    fn handle_dismiss(&self, req: Dismiss) -> Result<Value, McpError> {
        let window = window_from_scope(&req.scope_chain);
        // Layer 1: close the palette if open in this window.
        if self.ui_state.palette_open(window) {
            let change = self.ui_state.set_palette_open(window, false);
            return Ok(serde_json::json!({ "ok": true, "change": change }));
        }
        // Layer 2: pop the topmost inspector entry.
        if !self.ui_state.inspector_stack(window).is_empty() {
            let change = self.ui_state.inspector_close(window);
            return Ok(serde_json::json!({ "ok": true, "change": change }));
        }
        // Layer 3: nothing to dismiss.
        Ok(serde_json::json!({ "ok": true, "change": Value::Null }))
    }
}

/// Map a JSON value into one of the operation structs, returning a readable
/// rmcp error when the shape is wrong.
fn deserialize_op<T: DeserializeOwned>(arguments: Value, op: &str) -> Result<T, McpError> {
    serde_json::from_value(arguments).map_err(|err| {
        McpError::invalid_params(format!("invalid arguments for op {op:?}: {err}"), None)
    })
}

impl ServerHandler for UiStateServer {
    /// Advertise the single `ui_state` operation tool.
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: vec![Self::build_tool_definition()],
            next_cursor: None,
            meta: None,
        })
    }

    /// Route a `tools/call` for the `ui_state` tool to the matching verb
    /// handler.
    ///
    /// Reads `arguments["op"]` to pick the verb, deserializes the rest of the
    /// arguments into the matching operation struct, then calls the handler.
    /// The set of verbs accepted here is exactly the set the `inputSchema`'s
    /// `op` enum publishes.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        if request.name.as_ref() != "ui_state" {
            return Err(McpError::invalid_request(
                format!("unknown tool {:?}; expected \"ui_state\"", request.name),
                None,
            ));
        }

        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let op = arguments
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                McpError::invalid_params(
                    "missing required field `op` for ui_state tool".to_string(),
                    None,
                )
            })?
            .to_string();

        let response = match op.as_str() {
            "inspect inspector" => self.handle_inspect(deserialize_op(arguments, &op)?)?,
            "close inspector" => self.handle_inspector_close(deserialize_op(arguments, &op)?)?,
            "close_all inspector" => {
                self.handle_inspector_close_all(deserialize_op(arguments, &op)?)?
            }
            "set_width inspector" => {
                self.handle_inspector_set_width(deserialize_op(arguments, &op)?)?
            }
            "open palette" => self.handle_palette_open(deserialize_op(arguments, &op)?)?,
            "close palette" => self.handle_palette_close(deserialize_op(arguments, &op)?)?,
            "set keymap" => self.handle_set_keymap_mode(deserialize_op(arguments, &op)?)?,
            "set scope_chain" => self.handle_set_scope_chain(deserialize_op(arguments, &op)?)?,
            "set active_view" => self.handle_set_active_view(deserialize_op(arguments, &op)?)?,
            "start rename" => self.handle_start_rename(deserialize_op(arguments, &op)?)?,
            "start drag" => self.handle_drag_start(deserialize_op(arguments, &op)?)?,
            "cancel drag" => self.handle_drag_cancel(deserialize_op(arguments, &op)?)?,
            "complete drag" => self.handle_drag_complete(deserialize_op(arguments, &op)?)?,
            "show command" => self.handle_show_command(deserialize_op(arguments, &op)?)?,
            "show palette" => self.handle_show_palette(deserialize_op(arguments, &op)?)?,
            "show search" => self.handle_show_search(deserialize_op(arguments, &op)?)?,
            "dismiss ui" => self.handle_dismiss(deserialize_op(arguments, &op)?)?,
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown op {other:?} for ui_state tool"),
                    None,
                ))
            }
        };

        Ok(CallToolResult::structured(response))
    }
}
