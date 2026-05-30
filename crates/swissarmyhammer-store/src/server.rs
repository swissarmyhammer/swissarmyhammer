//! In-process `rmcp::ServerHandler` for the `store` operation tool.
//!
//! [`StoreServer`] is the platform-facing surface of the substrate's
//! cross-cutting MCP face. It holds a shared `Arc<StoreContext>` and
//! advertises a single `store` operation tool whose `inputSchema` and
//! `_meta` are derived from the operation structs in
//! [`crate::operations`].
//!
//! The tool exposes three kinds of verbs:
//!
//! - **stack-wide** (`undo stack` / `redo stack` / `can_undo stack` /
//!   `can_redo stack` / `depth stack`) — operate on the one unified
//!   undo stack that spans every registered store.
//! - **transaction grouping** (`begin transaction` / `end transaction`)
//!   — public lifecycle for the ambient-per-task transaction id every
//!   store's `push` honors.
//! - **store-scoped** (`history item` / `get item` / `list stores`) —
//!   take a `store` parameter naming one of the registered stores.
//!
//! The `StoreServer` is bootstrapped into the plugin platform via
//! `host.expose_rust_module("store", server)`. The integration tests in
//! `crates/swissarmyhammer-store/tests/integration/` stand the server
//! up directly against a freshly built `StoreContext`; production
//! bootstrap lives in the app-shell cut-over project.

use std::str::FromStr;
use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde::de::DeserializeOwned;
use serde_json::Value;
use swissarmyhammer_operations_macros::operation_tool;

use crate::context::StoreContext;
use crate::error::StoreError;
use crate::id::{StoredItemId, UndoEntryId};
use crate::operations::{
    operations, BeginTransaction, CanRedo, CanUndo, EndTransaction, GetItem, History, ListStores,
    Redo, Undo, UndoDepth,
};

/// Resolves the [`StoreContext`] to use for the current `tokio` task.
///
/// Production deployments back this with a [`tokio::task_local!`] scope
/// (see `swissarmyhammer-kanban`'s `command_seam` module). Returning
/// `None` means no context is active on this task — every tool handler
/// surfaces this as an `internal_error` rather than panicking.
pub type StoreContextResolver = Arc<dyn Fn() -> Option<Arc<StoreContext>> + Send + Sync>;

/// In-process `rmcp::ServerHandler` for the `store` operation tool.
///
/// Holds an `Arc<StoreContext>` to the shared substrate so every verb
/// dispatches against the same undo stack and the same set of
/// registered stores the rest of the app reads from.
#[derive(Clone)]
pub struct StoreServer {
    /// Resolves the active [`StoreContext`] per call. See
    /// [`StoreContextResolver`].
    resolver: StoreContextResolver,
}

impl std::fmt::Debug for StoreServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreServer").finish()
    }
}

impl StoreServer {
    /// Build a server that always serves `ctx` — single-context callers
    /// (most tests, single-board hosts) use this constructor.
    pub fn new(ctx: Arc<StoreContext>) -> Self {
        let ctx = Arc::clone(&ctx);
        Self::with_resolver(Arc::new(move || Some(Arc::clone(&ctx))))
    }

    /// Build a server that resolves the active context per call.
    ///
    /// The resolver is consulted at the top of every tool handler, so a
    /// single `StoreServer` exposed app-wide on a plugin host can route
    /// per-call to whichever board's [`StoreContext`] is scoped on the
    /// current `tokio` task. Returning `None` from the resolver surfaces
    /// as a tool-level `internal_error` (the tool call fails with a
    /// descriptive message rather than panicking).
    pub fn with_resolver(resolver: StoreContextResolver) -> Self {
        Self { resolver }
    }

    /// Resolve the [`StoreContext`] for the current task, or return an
    /// `internal_error` McpError describing the gap.
    fn ctx(&self) -> Result<Arc<StoreContext>, McpError> {
        (self.resolver)().ok_or_else(|| {
            McpError::internal_error(
                "no StoreContext is active on this tokio task; \
                 the dispatcher must scope one (see `scope_store_context`) \
                 before invoking a `store` tool",
                None,
            )
        })
    }

    /// The active [`StoreContext`] — returns `None` when no context is
    /// scoped. Public for callers that need a context-or-nothing read
    /// (parity with the original `context()` accessor's intent without
    /// the panic on missing).
    pub fn context(&self) -> Option<Arc<StoreContext>> {
        (self.resolver)()
    }

    /// Build the platform-facing `store` tool definition.
    ///
    /// The `inputSchema` is the flat `op` enum derived from the
    /// operation structs in [`crate::operations`]; the `_meta` tree
    /// under `io.swissarmyhammer/operations` is the discovery surface
    /// for the SDK path sugar. Both come from the same operation slice
    /// via the `operation_tool!` macro, so they cannot drift.
    fn build_tool_definition() -> Tool {
        operation_tool! {
            name: "store",
            description: "Undo, redo, transaction grouping, and per-item history over the shared StoreContext.",
            operations: operations(),
        }
    }

    /// Handle a stack-wide `undo` call.
    async fn handle_undo(&self, _req: Undo) -> Result<Value, McpError> {
        let ctx = self.ctx()?;
        let outcome = ctx.undo().await.map_err(store_error_to_mcp)?;
        Ok(serde_json::json!({
            "ok": true,
            "store_name": outcome.store_name,
            "item_id": outcome.item_id.as_str(),
            "items": outcome
                .items
                .iter()
                .map(|(s, i)| serde_json::json!({ "store": s, "item_id": i.as_str() }))
                .collect::<Vec<_>>(),
        }))
    }

    /// Handle a stack-wide `redo` call.
    async fn handle_redo(&self, _req: Redo) -> Result<Value, McpError> {
        let ctx = self.ctx()?;
        let outcome = ctx.redo().await.map_err(store_error_to_mcp)?;
        Ok(serde_json::json!({
            "ok": true,
            "store_name": outcome.store_name,
            "item_id": outcome.item_id.as_str(),
            "items": outcome
                .items
                .iter()
                .map(|(s, i)| serde_json::json!({ "store": s, "item_id": i.as_str() }))
                .collect::<Vec<_>>(),
        }))
    }

    /// Handle a stack-wide `can_undo` probe.
    async fn handle_can_undo(&self, _req: CanUndo) -> Result<Value, McpError> {
        let ctx = self.ctx()?;
        Ok(serde_json::json!({
            "ok": true,
            "can_undo": ctx.can_undo().await,
        }))
    }

    /// Handle a stack-wide `can_redo` probe.
    async fn handle_can_redo(&self, _req: CanRedo) -> Result<Value, McpError> {
        let ctx = self.ctx()?;
        Ok(serde_json::json!({
            "ok": true,
            "can_redo": ctx.can_redo().await,
        }))
    }

    /// Handle a stack-wide `depth` probe.
    async fn handle_depth(&self, _req: UndoDepth) -> Result<Value, McpError> {
        let ctx = self.ctx()?;
        Ok(serde_json::json!({
            "ok": true,
            "depth": ctx.undo_depth().await,
        }))
    }

    /// Handle a `BeginTransaction` call — allocate or return the
    /// existing ambient transaction id for the current task.
    fn handle_begin_transaction(&self, _req: BeginTransaction) -> Result<Value, McpError> {
        let ctx = self.ctx()?;
        let id = ctx.begin_transaction();
        Ok(serde_json::json!({
            "ok": true,
            "id": id.to_string(),
        }))
    }

    /// Handle an `EndTransaction` call — clear the current task's
    /// ambient slot iff the supplied id matches.
    fn handle_end_transaction(&self, req: EndTransaction) -> Result<Value, McpError> {
        let ctx = self.ctx()?;
        let id = UndoEntryId::from_str(&req.id).map_err(|e| {
            McpError::invalid_params(format!("invalid transaction id {:?}: {e}", req.id), None)
        })?;
        ctx.end_transaction(id);
        Ok(serde_json::json!({ "ok": true }))
    }

    /// Handle a `History` call — per-item changelog over one store.
    async fn handle_history(&self, req: History) -> Result<Value, McpError> {
        let ctx = self.ctx()?;
        let item_id = StoredItemId::from(req.item_id.clone());
        let entries = ctx
            .read_changelog(&req.store, &item_id)
            .await
            .map_err(|e| store_error_to_mcp_with_store(e, &req.store))?;
        Ok(serde_json::json!({
            "ok": true,
            "entries": entries,
        }))
    }

    /// Handle a `GetItem` call — current bytes for one item.
    async fn handle_get_item(&self, req: GetItem) -> Result<Value, McpError> {
        let ctx = self.ctx()?;
        let item_id = StoredItemId::from(req.item_id.clone());
        let bytes = ctx
            .get_item_bytes(&req.store, &item_id)
            .await
            .map_err(|e| store_error_to_mcp_with_store(e, &req.store))?;
        Ok(serde_json::json!({
            "ok": true,
            "bytes": bytes,
        }))
    }

    /// Handle a `ListStores` call — registered store names.
    async fn handle_list_stores(&self, _req: ListStores) -> Result<Value, McpError> {
        let ctx = self.ctx()?;
        let names = ctx.store_names().await;
        Ok(serde_json::json!({
            "ok": true,
            "stores": names,
        }))
    }
}

/// Map a JSON value into one of the operation structs, returning a
/// readable rmcp error when the shape is wrong.
fn deserialize_op<T: DeserializeOwned>(arguments: Value, op: &str) -> Result<T, McpError> {
    serde_json::from_value(arguments).map_err(|err| {
        McpError::invalid_params(format!("invalid arguments for op {op:?}: {err}"), None)
    })
}

/// Map a [`StoreError`] onto a structured [`McpError`].
///
/// `NotFound`, `EntryNotFound` map to `invalid_params` (client-recoverable
/// shape failures); everything else maps to `internal_error`.
fn store_error_to_mcp(err: StoreError) -> McpError {
    let message = err.to_string();
    match err {
        StoreError::NotFound(_) | StoreError::EntryNotFound(_) => {
            McpError::invalid_params(message, None)
        }
        _ => McpError::internal_error(message, None),
    }
}

/// Map a [`StoreError`] onto an [`McpError`], surfacing the
/// `store` name in the structured data field so callers can branch on
/// "unknown store" without parsing the message.
fn store_error_to_mcp_with_store(err: StoreError, store: &str) -> McpError {
    let message = err.to_string();
    let data = serde_json::json!({ "store": store });
    match err {
        StoreError::NotFound(_) | StoreError::EntryNotFound(_) => {
            McpError::invalid_params(message, Some(data))
        }
        _ => McpError::internal_error(message, Some(data)),
    }
}

impl ServerHandler for StoreServer {
    /// Advertise the single `store` operation tool.
    ///
    /// Rebuilt on every call so the server has no hidden state to keep
    /// in sync; the `operation_tool!` macro expansion is cheap (it
    /// walks a fixed-size operation slice).
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

    /// Route a `tools/call` for the `store` tool to the matching verb
    /// handler.
    ///
    /// Reads `arguments["op"]` to pick the verb, deserializes the rest
    /// of the arguments into the matching operation struct, then calls
    /// the handler. The set of verbs accepted here is exactly the set
    /// the `inputSchema`'s `op` enum publishes.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        if request.name.as_ref() != "store" {
            return Err(McpError::invalid_request(
                format!("unknown tool {:?}; expected \"store\"", request.name),
                None,
            ));
        }

        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let op = arguments
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                McpError::invalid_params(
                    "missing required field `op` for store tool".to_string(),
                    None,
                )
            })?
            .to_string();

        let response = match op.as_str() {
            "undo stack" => {
                let req: Undo = deserialize_op(arguments, &op)?;
                self.handle_undo(req).await?
            }
            "redo stack" => {
                let req: Redo = deserialize_op(arguments, &op)?;
                self.handle_redo(req).await?
            }
            "can_undo stack" => {
                let req: CanUndo = deserialize_op(arguments, &op)?;
                self.handle_can_undo(req).await?
            }
            "can_redo stack" => {
                let req: CanRedo = deserialize_op(arguments, &op)?;
                self.handle_can_redo(req).await?
            }
            "depth stack" => {
                let req: UndoDepth = deserialize_op(arguments, &op)?;
                self.handle_depth(req).await?
            }
            "begin transaction" => {
                let req: BeginTransaction = deserialize_op(arguments, &op)?;
                self.handle_begin_transaction(req)?
            }
            "end transaction" => {
                let req: EndTransaction = deserialize_op(arguments, &op)?;
                self.handle_end_transaction(req)?
            }
            "history item" => {
                let req: History = deserialize_op(arguments, &op)?;
                self.handle_history(req).await?
            }
            "get item" => {
                let req: GetItem = deserialize_op(arguments, &op)?;
                self.handle_get_item(req).await?
            }
            "list stores" => {
                let req: ListStores = deserialize_op(arguments, &op)?;
                self.handle_list_stores(req).await?
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown op {other:?} for store tool"),
                    None,
                ))
            }
        };

        Ok(CallToolResult::structured(response))
    }
}
