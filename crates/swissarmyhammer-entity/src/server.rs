//! In-process `rmcp::ServerHandler` for the `entity` operation tool.
//!
//! [`EntityServer`] is the generic, type-agnostic MCP face over the entity
//! **kernel** ([`EntityContext`]). It holds an `Arc<EntityContext>` and
//! advertises a single `entity` operation tool whose `inputSchema` and
//! `_meta` are derived from the operation structs in [`crate::operations`].
//!
//! Every verb takes an `entity_type` and routes through the matching
//! `EntityContext` method — there is no duplicate CRUD here. Because the
//! kernel pushes every write onto the shared `StoreContext` and broadcasts
//! `EntityEvent`s, undo / redo and the notification surface work for free:
//! the server is a thin translation layer between the wire protocol and the
//! kernel.
//!
//! The kernel is shared by `Arc::clone` with the domain `kanban` face and
//! the `store` / `views` faces — they all resolve through the same one
//! `EntityContext`, so a write made through `entity` is visible to `kanban`
//! and vice versa. This server adds operations purely additively; it does
//! not touch the `kanban` tool's operation surface.

use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde::de::DeserializeOwned;
use serde_json::Value;
use swissarmyhammer_operations_macros::operation_tool;

use crate::context::EntityContext;
use crate::entity::Entity;
use crate::error::EntityError;
use crate::id_types::EntityId;
use crate::operations::{
    operations, AddEntity, ArchiveEntity, DeleteEntity, GetEntity, ListEntities, UnarchiveEntity,
    UpdateField,
};

/// In-process `rmcp::ServerHandler` for the `entity` operation tool.
///
/// Holds an `Arc<EntityContext>` to the shared entity kernel so every verb
/// dispatches against the same CRUD implementation, the same cache, and the
/// same shared `StoreContext` the rest of the app reads from.
#[derive(Clone)]
pub struct EntityServer {
    /// The shared entity kernel. Held behind an `Arc` because the kernel
    /// invariant requires that exactly one `EntityContext` exists per app
    /// and is shared by `Arc::clone` — this server is just another holder
    /// of the same arc.
    ctx: Arc<EntityContext>,
}

impl std::fmt::Debug for EntityServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityServer").finish()
    }
}

impl EntityServer {
    /// Construct a fresh server wired to the given shared entity kernel.
    pub fn new(ctx: Arc<EntityContext>) -> Self {
        Self { ctx }
    }

    /// Return the shared `Arc<EntityContext>` the server dispatches to.
    ///
    /// Exposed for tests that need to verify the server holds the same
    /// kernel the rest of the app reads from (`Arc::ptr_eq`).
    pub fn context(&self) -> Arc<EntityContext> {
        Arc::clone(&self.ctx)
    }

    /// Build the platform-facing `entity` tool definition.
    ///
    /// The `inputSchema` is the flat `op` enum derived from the operation
    /// structs in [`crate::operations`]; the `_meta` tree under
    /// `io.swissarmyhammer/operations` is the discovery surface for the SDK
    /// path sugar. Both come from the same operation slice via the
    /// `operation_tool!` macro, so they cannot drift.
    fn build_tool_definition() -> Tool {
        operation_tool! {
            name: "entity",
            description: "Generic, type-agnostic CRUD + archive over the shared EntityContext kernel.",
            operations: operations(),
        }
    }

    /// Handle a `GetEntity` call — read one entity as JSON.
    async fn handle_get(&self, req: GetEntity) -> Result<Value, McpError> {
        let entity = self
            .ctx
            .read(&req.entity_type, &req.id)
            .await
            .map_err(entity_error_to_mcp)?;
        Ok(serde_json::json!({
            "ok": true,
            "entity": entity.to_json(),
        }))
    }

    /// Handle a `ListEntities` call — every live entity of a type as JSON.
    async fn handle_list(&self, req: ListEntities) -> Result<Value, McpError> {
        let entities = self
            .ctx
            .list(&req.entity_type)
            .await
            .map_err(entity_error_to_mcp)?;
        let json: Vec<Value> = entities.iter().map(Entity::to_json).collect();
        Ok(serde_json::json!({
            "ok": true,
            "entities": json,
        }))
    }

    /// Handle an `AddEntity` call — create / overwrite an entity from a
    /// field map. Mints a ULID id when none is supplied.
    async fn handle_add(&self, req: AddEntity) -> Result<Value, McpError> {
        let id = req
            .id
            .filter(|s| !s.is_empty())
            .map(EntityId::from)
            .unwrap_or_else(EntityId::new);
        let mut entity = Entity::new(req.entity_type.as_str(), id.clone());
        for (field, value) in req.fields {
            entity.set(field, value);
        }
        let entry_id = self.ctx.write(&entity).await.map_err(entity_error_to_mcp)?;
        Ok(serde_json::json!({
            "ok": true,
            "id": id.to_string(),
            "entry_id": entry_id.map(|e| e.to_string()),
        }))
    }

    /// Handle an `UpdateField` call — set one field on an existing entity.
    ///
    /// Reads the current entity through the kernel, replaces the field, and
    /// writes it back so the mutation is undoable and emits an event.
    async fn handle_update_field(&self, req: UpdateField) -> Result<Value, McpError> {
        let mut entity = self
            .ctx
            .read(&req.entity_type, &req.id)
            .await
            .map_err(entity_error_to_mcp)?;
        entity.set(req.field, req.value);
        let entry_id = self.ctx.write(&entity).await.map_err(entity_error_to_mcp)?;
        Ok(serde_json::json!({
            "ok": true,
            "id": req.id,
            "entry_id": entry_id.map(|e| e.to_string()),
        }))
    }

    /// Handle a `DeleteEntity` call — trash an entity.
    async fn handle_delete(&self, req: DeleteEntity) -> Result<Value, McpError> {
        let entry_id = self
            .ctx
            .delete(&req.entity_type, &req.id)
            .await
            .map_err(entity_error_to_mcp)?;
        Ok(serde_json::json!({
            "ok": true,
            "entry_id": entry_id.map(|e| e.to_string()),
        }))
    }

    /// Handle an `ArchiveEntity` call — move an entity to `.archive/`.
    async fn handle_archive(&self, req: ArchiveEntity) -> Result<Value, McpError> {
        let entry_id = self
            .ctx
            .archive(&req.entity_type, &req.id)
            .await
            .map_err(entity_error_to_mcp)?;
        Ok(serde_json::json!({
            "ok": true,
            "entry_id": entry_id.map(|e| e.to_string()),
        }))
    }

    /// Handle an `UnarchiveEntity` call — restore an archived entity.
    async fn handle_unarchive(&self, req: UnarchiveEntity) -> Result<Value, McpError> {
        let entry_id = self
            .ctx
            .unarchive(&req.entity_type, &req.id)
            .await
            .map_err(entity_error_to_mcp)?;
        Ok(serde_json::json!({
            "ok": true,
            "entry_id": entry_id.map(|e| e.to_string()),
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

/// Map an [`EntityError`] onto a structured [`McpError`].
///
/// Client-recoverable shape failures (`NotFound`, `UnknownEntityType`,
/// `ValidationFailed`) map to `invalid_params`; everything else maps to
/// `internal_error`. The `entity_type` / `id` context, when present, is
/// surfaced in the structured data field so callers can branch without
/// parsing the message.
fn entity_error_to_mcp(err: EntityError) -> McpError {
    let message = err.to_string();
    match &err {
        EntityError::NotFound { entity_type, id } => McpError::invalid_params(
            message,
            Some(serde_json::json!({ "type": entity_type, "id": id })),
        ),
        EntityError::UnknownEntityType { entity_type } => McpError::invalid_params(
            message,
            Some(serde_json::json!({ "type": entity_type })),
        ),
        EntityError::ValidationFailed { field, .. } => {
            McpError::invalid_params(message, Some(serde_json::json!({ "field": field })))
        }
        _ => McpError::internal_error(message, None),
    }
}

impl ServerHandler for EntityServer {
    /// Advertise the single `entity` operation tool.
    ///
    /// Rebuilt on every call so the server has no hidden state to keep in
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

    /// Route a `tools/call` for the `entity` tool to the matching verb
    /// handler.
    ///
    /// Reads `arguments["op"]` to pick the verb, deserializes the rest of
    /// the arguments into the matching operation struct, then calls the
    /// handler. The set of verbs accepted here is exactly the set the
    /// `inputSchema`'s `op` enum publishes.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        if request.name.as_ref() != "entity" {
            return Err(McpError::invalid_request(
                format!("unknown tool {:?}; expected \"entity\"", request.name),
                None,
            ));
        }

        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let op = arguments
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                McpError::invalid_params(
                    "missing required field `op` for entity tool".to_string(),
                    None,
                )
            })?
            .to_string();

        let response = match op.as_str() {
            "get entity" => {
                let req: GetEntity = deserialize_op(arguments, &op)?;
                self.handle_get(req).await?
            }
            "list entities" => {
                let req: ListEntities = deserialize_op(arguments, &op)?;
                self.handle_list(req).await?
            }
            "add entity" => {
                let req: AddEntity = deserialize_op(arguments, &op)?;
                self.handle_add(req).await?
            }
            "update field" => {
                let req: UpdateField = deserialize_op(arguments, &op)?;
                self.handle_update_field(req).await?
            }
            "delete entity" => {
                let req: DeleteEntity = deserialize_op(arguments, &op)?;
                self.handle_delete(req).await?
            }
            "archive entity" => {
                let req: ArchiveEntity = deserialize_op(arguments, &op)?;
                self.handle_archive(req).await?
            }
            "unarchive entity" => {
                let req: UnarchiveEntity = deserialize_op(arguments, &op)?;
                self.handle_unarchive(req).await?
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown op {other:?} for entity tool"),
                    None,
                ))
            }
        };

        Ok(CallToolResult::structured(response))
    }
}
