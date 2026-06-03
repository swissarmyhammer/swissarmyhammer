//! In-process `rmcp::ServerHandler` for the `window` operation tool.
//!
//! [`WindowService`] is the platform-facing surface of the window-manager and
//! OS-file actions. It holds an `Arc<dyn WindowShell>` and advertises a single
//! `window` operation tool whose `inputSchema` and `_meta` are derived from the
//! operation structs in [`crate::operations`].
//!
//! The tool exposes three op groups:
//!
//! - **window** — `new window` (ports `create_window`), `activate window`,
//!   `set position`, `get position`, `get monitors`, `close window`.
//! - **OS file actions** — `open path` (backs `attachment.open`), `reveal path`
//!   (backs `attachment.reveal`).
//! - **board lifecycle** — `switch board` / `close board` (wrap
//!   `AppState::open_board` / `close_board`), `new board` / `open board` (port
//!   the `new_board_dialog` / `open_board_dialog` folder-picker paths).
//!
//! The `WindowService` is bootstrapped into the plugin platform via
//! `host.expose_rust_module("window", service)`. The integration tests in
//! `crates/swissarmyhammer-window-service/tests/integration/` stand the service
//! up directly against a spy `WindowShell`; production bootstrap lives in the
//! app-shell cut-over project.

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
    operations, ActivateWindow, CloseBoard, CloseWindow, GetMonitors, GetWindowPosition, NewBoard,
    OpenBoard, OpenNewWindow, OpenPath, RevealPath, SetWindowPosition, ShowContextMenu,
    SwitchBoard,
};
use crate::shell::{WindowPosition, WindowShell};

/// In-process `rmcp::ServerHandler` for the `window` operation tool.
///
/// Holds an `Arc<dyn WindowShell>` so every verb routes through the injectable
/// seam: production wires a `TauriWindowShell`, tests wire a recording spy.
#[derive(Clone)]
pub struct WindowService {
    /// The injectable shell seam. All window-manager and OS-file side effects
    /// go through this trait object so the dispatch path is testable without a
    /// live GUI or a real file manager.
    shell: Arc<dyn WindowShell>,
}

impl std::fmt::Debug for WindowService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowService").finish()
    }
}

impl WindowService {
    /// Construct a fresh service wired to the given shell seam.
    pub fn new(shell: Arc<dyn WindowShell>) -> Self {
        Self { shell }
    }

    /// Build the platform-facing `window` tool definition.
    ///
    /// The `inputSchema` is the flat `op` enum derived from the operation
    /// structs in [`crate::operations`]; the `_meta` tree under
    /// `io.swissarmyhammer/operations` is the discovery surface for the SDK
    /// path sugar. Both come from the same operation slice via the
    /// `operation_tool!` macro, so they cannot drift.
    fn build_tool_definition() -> Tool {
        operation_tool! {
            name: "window",
            description: "Window-manager actions (open, activate, position, monitors, close) and OS file actions (open, reveal).",
            operations: operations(),
        }
    }

    /// Handle an `OpenNewWindow` call — open a new window via the shell.
    fn handle_open_new_window(&self, req: OpenNewWindow) -> Result<Value, McpError> {
        let new_window = self
            .shell
            .open_new_window(req.board_path)
            .map_err(shell_error)?;
        Ok(serde_json::json!({
            "ok": true,
            "label": new_window.label,
            "board_path": new_window.board_path,
        }))
    }

    /// Handle an `ActivateWindow` call — focus the labeled window.
    fn handle_activate_window(&self, req: ActivateWindow) -> Result<Value, McpError> {
        self.shell
            .activate_window(&req.label)
            .map_err(shell_error)?;
        Ok(serde_json::json!({ "ok": true, "label": req.label }))
    }

    /// Handle a `SetWindowPosition` call — move the labeled window.
    fn handle_set_window_position(&self, req: SetWindowPosition) -> Result<Value, McpError> {
        self.shell
            .set_window_position(&req.label, WindowPosition { x: req.x, y: req.y })
            .map_err(shell_error)?;
        Ok(serde_json::json!({
            "ok": true,
            "label": req.label,
            "x": req.x,
            "y": req.y,
        }))
    }

    /// Handle a `GetWindowPosition` call — read the labeled window's position.
    fn handle_get_window_position(&self, req: GetWindowPosition) -> Result<Value, McpError> {
        let pos = self
            .shell
            .get_window_position(&req.label)
            .map_err(shell_error)?;
        Ok(serde_json::json!({
            "ok": true,
            "label": req.label,
            "x": pos.x,
            "y": pos.y,
        }))
    }

    /// Handle a `GetMonitors` call — enumerate the connected monitors.
    fn handle_get_monitors(&self, _req: GetMonitors) -> Result<Value, McpError> {
        let monitors = self.shell.get_monitors().map_err(shell_error)?;
        Ok(serde_json::json!({ "ok": true, "monitors": monitors }))
    }

    /// Handle a `CloseWindow` call — close the labeled window.
    fn handle_close_window(&self, req: CloseWindow) -> Result<Value, McpError> {
        self.shell.close_window(&req.label).map_err(shell_error)?;
        Ok(serde_json::json!({ "ok": true, "label": req.label }))
    }

    /// Handle an `OpenPath` call — open a file in the OS default app.
    fn handle_open_path(&self, req: OpenPath) -> Result<Value, McpError> {
        self.shell.open_path(&req.path).map_err(shell_error)?;
        Ok(serde_json::json!({ "ok": true, "opened": req.path }))
    }

    /// Handle a `RevealPath` call — reveal a file in the OS file manager.
    fn handle_reveal_path(&self, req: RevealPath) -> Result<Value, McpError> {
        self.shell.reveal_path(&req.path).map_err(shell_error)?;
        Ok(serde_json::json!({ "ok": true, "revealed": req.path }))
    }

    /// Handle a `SwitchBoard` call — switch the active board via the shell.
    fn handle_switch_board(&self, req: SwitchBoard) -> Result<Value, McpError> {
        self.shell.switch_board(&req.path).map_err(shell_error)?;
        Ok(serde_json::json!({ "ok": true, "path": req.path }))
    }

    /// Handle a `CloseBoard` call — close the board at the path via the shell.
    fn handle_close_board(&self, req: CloseBoard) -> Result<Value, McpError> {
        self.shell.close_board(&req.path).map_err(shell_error)?;
        Ok(serde_json::json!({ "ok": true, "path": req.path }))
    }

    /// Handle a `NewBoard` call — create a board via the picker / dialog path.
    fn handle_new_board(&self, _req: NewBoard) -> Result<Value, McpError> {
        let board = self.shell.new_board().map_err(shell_error)?;
        Ok(serde_json::json!({
            "ok": true,
            "path": board.path,
            "name": board.name,
        }))
    }

    /// Handle an `OpenBoard` call — open a board via the file-open dialog.
    ///
    /// A cancelled dialog is a success with `opened: false` and a null `path`,
    /// not an error — the user simply declined to open anything.
    fn handle_open_board(&self, _req: OpenBoard) -> Result<Value, McpError> {
        let opened = self.shell.open_board().map_err(shell_error)?;
        Ok(serde_json::json!({
            "ok": true,
            "opened": opened.is_some(),
            "path": opened.map(|b| b.path),
        }))
    }

    /// Handle a `ShowContextMenu` call — mount a native context menu via the
    /// shell.
    ///
    /// Selection delivery happens out-of-band (the app's menu-event handler
    /// emits `context-menu-command`), so the response only confirms the menu
    /// was handed to the shell and echoes how many items it carried.
    fn handle_show_context_menu(&self, req: ShowContextMenu) -> Result<Value, McpError> {
        let count = req.items.len();
        self.shell
            .show_context_menu(req.items, req.window_label)
            .map_err(shell_error)?;
        Ok(serde_json::json!({ "ok": true, "count": count }))
    }
}

/// Map a shell error string onto an rmcp `internal_error`.
///
/// Shell failures (unknown window label, OS opener errors) are operational
/// faults on the server side, not malformed requests, so they surface as
/// `internal_error` carrying the shell's human-readable message.
fn shell_error(msg: String) -> McpError {
    McpError::internal_error(msg, None)
}

/// Map a JSON value into one of the operation structs, returning a readable
/// rmcp error when the shape is wrong.
fn deserialize_op<T: DeserializeOwned>(arguments: Value, op: &str) -> Result<T, McpError> {
    serde_json::from_value(arguments).map_err(|err| {
        McpError::invalid_params(format!("invalid arguments for op {op:?}: {err}"), None)
    })
}

impl ServerHandler for WindowService {
    /// Advertise the single `window` operation tool.
    ///
    /// Rebuilt on every call so the service has no hidden state to keep in
    /// sync; the `operation_tool!` macro expansion is cheap (it walks a
    /// fixed-size operation slice).
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

    /// Route a `tools/call` for the `window` tool to the matching verb handler.
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
        if request.name.as_ref() != "window" {
            return Err(McpError::invalid_request(
                format!("unknown tool {:?}; expected \"window\"", request.name),
                None,
            ));
        }

        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let op = arguments
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                McpError::invalid_params(
                    "missing required field `op` for window tool".to_string(),
                    None,
                )
            })?
            .to_string();

        let response = match op.as_str() {
            "new window" => {
                let req: OpenNewWindow = deserialize_op(arguments, &op)?;
                self.handle_open_new_window(req)?
            }
            "activate window" => {
                let req: ActivateWindow = deserialize_op(arguments, &op)?;
                self.handle_activate_window(req)?
            }
            "set position" => {
                let req: SetWindowPosition = deserialize_op(arguments, &op)?;
                self.handle_set_window_position(req)?
            }
            "get position" => {
                let req: GetWindowPosition = deserialize_op(arguments, &op)?;
                self.handle_get_window_position(req)?
            }
            "get monitors" => {
                let req: GetMonitors = deserialize_op(arguments, &op)?;
                self.handle_get_monitors(req)?
            }
            "close window" => {
                let req: CloseWindow = deserialize_op(arguments, &op)?;
                self.handle_close_window(req)?
            }
            "open path" => {
                let req: OpenPath = deserialize_op(arguments, &op)?;
                self.handle_open_path(req)?
            }
            "reveal path" => {
                let req: RevealPath = deserialize_op(arguments, &op)?;
                self.handle_reveal_path(req)?
            }
            "switch board" => {
                let req: SwitchBoard = deserialize_op(arguments, &op)?;
                self.handle_switch_board(req)?
            }
            "close board" => {
                let req: CloseBoard = deserialize_op(arguments, &op)?;
                self.handle_close_board(req)?
            }
            "new board" => {
                let req: NewBoard = deserialize_op(arguments, &op)?;
                self.handle_new_board(req)?
            }
            "open board" => {
                let req: OpenBoard = deserialize_op(arguments, &op)?;
                self.handle_open_board(req)?
            }
            "show context menu" => {
                let req: ShowContextMenu = deserialize_op(arguments, &op)?;
                self.handle_show_context_menu(req)?
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown op {other:?} for window tool"),
                    None,
                ))
            }
        };

        Ok(CallToolResult::structured(response))
    }
}
