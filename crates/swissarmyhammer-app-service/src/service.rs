//! In-process `rmcp::ServerHandler` for the `app` operation tool.
//!
//! [`AppService`] is the platform-facing surface of the app-shell actions.
//! It holds an `Arc<dyn AppShell>` and advertises a single `app` operation
//! tool whose `inputSchema` and `_meta` are derived from the operation
//! structs in [`crate::operations`].
//!
//! The tool exposes app-shell actions:
//!
//! - **quit** (`quit app`) — terminate the process (ports the original
//!   `quit_app` Tauri command).
//! - **about** (`show about`) — surface the app's name / version.
//! - **help** (`show help`) — route the user to the help / docs.
//!
//! The multi-board management reads (`list open boards` / `get board data`) live
//! on the `window` server, alongside the board-lifecycle writes — not here.
//!
//! Undo / redo are NOT here — those live on the `store` server. UI panel
//! toggles live on the `ui_state` server.
//!
//! The `AppService` is bootstrapped into the plugin platform via
//! `host.expose_rust_module("app", service)`. The integration tests in
//! `crates/swissarmyhammer-app-service/tests/integration/` stand the service
//! up directly against a spy `AppShell`; production bootstrap lives in the
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

use crate::operations::{operations, QuitApp, ShowAbout, ShowHelp};
use crate::shell::AppShell;

/// In-process `rmcp::ServerHandler` for the `app` operation tool.
///
/// Holds an `Arc<dyn AppShell>` so every verb routes through the injectable
/// seam: production wires a `TauriAppShell`, tests wire a recording spy.
#[derive(Clone)]
pub struct AppService {
    /// The injectable shell seam. All app-shell side effects (quit, about,
    /// help) go through this trait object so the dispatch path is testable
    /// without a live GUI.
    shell: Arc<dyn AppShell>,
}

impl std::fmt::Debug for AppService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppService").finish()
    }
}

impl AppService {
    /// Construct a fresh service wired to the given shell seam.
    pub fn new(shell: Arc<dyn AppShell>) -> Self {
        Self { shell }
    }

    /// Build the platform-facing `app` tool definition.
    ///
    /// The `inputSchema` is the flat `op` enum derived from the operation
    /// structs in [`crate::operations`]; the `_meta` tree under
    /// `io.swissarmyhammer/operations` is the discovery surface for the SDK
    /// path sugar. Both come from the same operation slice via the
    /// `operation_tool!` macro, so they cannot drift.
    fn build_tool_definition() -> Tool {
        operation_tool! {
            name: "app",
            description: "App-shell actions: quit, about, and help.",
            operations: operations(),
        }
    }

    /// Handle a `QuitApp` call — terminate the process via the shell.
    fn handle_quit(&self, _req: QuitApp) -> Result<Value, McpError> {
        self.shell.quit();
        Ok(serde_json::json!({ "ok": true }))
    }

    /// Handle a `ShowAbout` call — surface the app's name / version.
    fn handle_show_about(&self, _req: ShowAbout) -> Result<Value, McpError> {
        let info = self.shell.show_about();
        Ok(serde_json::json!({
            "ok": true,
            "name": info.name,
            "version": info.version,
        }))
    }

    /// Handle a `ShowHelp` call — route the user to the help target.
    fn handle_show_help(&self, _req: ShowHelp) -> Result<Value, McpError> {
        let target = self.shell.show_help();
        Ok(serde_json::json!({
            "ok": true,
            "target": target,
        }))
    }
}

/// Map a JSON value into one of the operation structs, returning a readable
/// rmcp error when the shape is wrong.
fn deserialize_op<T: DeserializeOwned>(arguments: Value, op: &str) -> Result<T, McpError> {
    serde_json::from_value(arguments).map_err(|err| {
        McpError::invalid_params(format!("invalid arguments for op {op:?}: {err}"), None)
    })
}

impl ServerHandler for AppService {
    /// Advertise the single `app` operation tool.
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

    /// Route a `tools/call` for the `app` tool to the matching verb handler.
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
        if request.name.as_ref() != "app" {
            return Err(McpError::invalid_request(
                format!("unknown tool {:?}; expected \"app\"", request.name),
                None,
            ));
        }

        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let op = arguments
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                McpError::invalid_params(
                    "missing required field `op` for app tool".to_string(),
                    None,
                )
            })?
            .to_string();

        let response = match op.as_str() {
            "quit app" => {
                let req: QuitApp = deserialize_op(arguments, &op)?;
                self.handle_quit(req)?
            }
            "show about" => {
                let req: ShowAbout = deserialize_op(arguments, &op)?;
                self.handle_show_about(req)?
            }
            "show help" => {
                let req: ShowHelp = deserialize_op(arguments, &op)?;
                self.handle_show_help(req)?
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown op {other:?} for app tool"),
                    None,
                ))
            }
        };

        Ok(CallToolResult::structured(response))
    }
}
