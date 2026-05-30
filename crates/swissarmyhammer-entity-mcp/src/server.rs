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

use std::collections::HashMap;
use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde::de::DeserializeOwned;
use serde_json::Value;
use swissarmyhammer_operations_macros::operation_tool;

use swissarmyhammer_commands::{Command, CommandContext};
use swissarmyhammer_entity::{Entity, EntityContext, EntityError, EntityId};
use swissarmyhammer_entity_search::EntitySearchIndex;
use swissarmyhammer_kanban::clipboard::{ClipboardProvider, ClipboardProviderExt};
use swissarmyhammer_kanban::commands::clipboard_commands::{
    CopyEntityCmd, CutEntityCmd, PasteEntityCmd,
};
use swissarmyhammer_kanban::KanbanContext;
use swissarmyhammer_ui_state::UIState;

use crate::operations::{
    operations, AddEntity, ArchiveEntity, Copy, Cut, DeleteEntity, GetEntity, ListEntities, Paste,
    Search, UnarchiveEntity, UpdateField,
};

/// The per-board services an [`EntityServer`] needs at tool-call time.
///
/// `entity_ctx` is always required; the `clipboard`-related fields are only
/// populated when the server was constructed via
/// [`EntityServer::with_resolver`] / [`EntityServer::with_clipboard`] — without
/// them, the clipboard ops surface as a structured error rather than panic.
///
/// `Clone` so the value can be placed into a `tokio::task_local!` (see
/// [`scope_entity_board_services`]) and resolved out cheaply per-call by
/// the production resolver.
#[derive(Clone)]
pub struct EntityBoardServices {
    /// The board's entity context (required for every entity op).
    pub entity_ctx: Arc<EntityContext>,
    /// The board's kanban context — needed for clipboard ops only.
    pub kanban: Option<Arc<KanbanContext>>,
    /// The clipboard provider — needed for clipboard ops only.
    pub clipboard: Option<Arc<dyn ClipboardProvider>>,
    /// The UI-state used to track palette/paste affordances — clipboard
    /// ops only.
    pub ui_state: Option<Arc<UIState>>,
}

/// Resolves the [`EntityBoardServices`] to drive for the current task.
///
/// Production deployments back this with a `tokio::task_local!` scope set
/// by the dispatcher (see [`scope_entity_board_services`] /
/// [`task_local_resolver`]). Returning `None` means no board is scoped on
/// this task; tool handlers surface that as an `internal_error` rather
/// than panicking.
pub type EntityBoardResolver = Arc<dyn Fn() -> Option<EntityBoardServices> + Send + Sync>;

tokio::task_local! {
    /// Per-task active [`EntityBoardServices`] for production dispatch.
    ///
    /// The kanban app is multi-board: each board's services bundle is
    /// scoped here by the dispatcher (alongside `swissarmyhammer-kanban`'s
    /// `CURRENT_STORE_CTX`), and the production [`EntityServer`] resolver
    /// — [`task_local_resolver`] — reads back the same bundle inside its
    /// per-call `services()` lookup.
    ///
    /// Outside a [`scope_entity_board_services`] (e.g. in tests that
    /// instantiate `EntityServer` with `new(ctx)` directly), this
    /// task-local is unset and a resolver built from
    /// [`task_local_resolver`] returns `None` — the entity tool handlers
    /// then surface a structured "no board scoped" error rather than
    /// panicking.
    pub static CURRENT_ENTITY_BOARD_SERVICES: EntityBoardServices;
}

/// Scope [`CURRENT_ENTITY_BOARD_SERVICES`] to `services` for the duration
/// of `fut`.
///
/// The production [`EntityServer`] resolver ([`task_local_resolver`])
/// reads back the scoped bundle inside every tool call, so the in-process
/// `entity` MCP surface routes per-call to whichever board's services the
/// dispatcher scoped.
pub async fn scope_entity_board_services<F>(services: EntityBoardServices, fut: F) -> F::Output
where
    F: std::future::Future,
{
    CURRENT_ENTITY_BOARD_SERVICES.scope(services, fut).await
}

/// Build the production [`EntityBoardResolver`] that reads
/// [`CURRENT_ENTITY_BOARD_SERVICES`].
///
/// Pair this with [`EntityServer::with_resolver`]; the app's dispatcher
/// then scopes the per-board bundle around its call to
/// [`crate::EntityServer`]'s tool handlers via
/// [`scope_entity_board_services`]. Outside a scope the resolver returns
/// `None` and tool calls fail with a structured error — a dispatcher that
/// forgets to scope degrades gracefully rather than panicking.
pub fn task_local_resolver() -> EntityBoardResolver {
    Arc::new(|| {
        CURRENT_ENTITY_BOARD_SERVICES
            .try_with(|services| services.clone())
            .ok()
    })
}

/// In-process `rmcp::ServerHandler` for the `entity` operation tool.
///
/// Holds an [`EntityBoardResolver`] — consulted at the top of every tool
/// handler — so a single `EntityServer` exposed app-wide on a plugin host
/// can route per-call to whichever board's services are scoped on the
/// current `tokio` task. The previous single-context constructors are
/// preserved as thin wrappers that produce a resolver returning the same
/// services every call.
#[derive(Clone)]
pub struct EntityServer {
    /// Resolves the active board's services per call. See
    /// [`EntityBoardResolver`].
    resolver: EntityBoardResolver,
}

impl std::fmt::Debug for EntityServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityServer").finish()
    }
}

impl EntityServer {
    /// Construct a fresh server wired to the given shared entity kernel.
    ///
    /// The CRUD / archive / search verbs are fully functional; the
    /// clipboard verbs (`copy` / `cut` / `paste`) are not — they require
    /// the board substrate supplied by [`EntityServer::with_clipboard`].
    /// Single-context callers (most tests) use this constructor.
    pub fn new(ctx: Arc<EntityContext>) -> Self {
        let ctx = Arc::clone(&ctx);
        Self::with_resolver(Arc::new(move || {
            Some(EntityBoardServices {
                entity_ctx: Arc::clone(&ctx),
                kanban: None,
                clipboard: None,
                ui_state: None,
            })
        }))
    }

    /// Construct a server with the full clipboard wiring for a single board.
    ///
    /// `kanban` is the board context the paste handlers dispatch against;
    /// its `entity_context()` is taken as the kernel the server holds, so
    /// the generic CRUD face and the clipboard ops resolve through the one
    /// shared `EntityContext` (and, transitively, the one shared
    /// `StoreContext` — making pastes undoable through the same stack the
    /// rest of the app drives). `clipboard_provider` is the injectable
    /// clipboard seam: tests pass an `InMemoryClipboard`, production passes
    /// the OS-backed provider.
    ///
    /// Preserved as a constant-context wrapper around
    /// [`EntityServer::with_resolver`].
    ///
    /// # Errors
    ///
    /// Surfaces any error from `kanban.entity_context()` (store setup /
    /// cache preload) as an [`McpError::internal_error`].
    pub async fn with_clipboard(
        kanban: Arc<KanbanContext>,
        clipboard_provider: Arc<dyn ClipboardProvider>,
        ui_state: Arc<UIState>,
    ) -> Result<Self, McpError> {
        // Resolve the entity context up-front — the previous body did the
        // same and we preserve that side-effect (store setup / cache
        // preload) here, before building the resolver.
        let entity_ctx = kanban.entity_context().await.map_err(|e| {
            McpError::internal_error(
                format!("entity_context unavailable for clipboard wiring: {e}"),
                None,
            )
        })?;
        let kanban = Arc::clone(&kanban);
        let clipboard_provider = Arc::clone(&clipboard_provider);
        let ui_state = Arc::clone(&ui_state);
        let entity_ctx = Arc::clone(&entity_ctx);
        Ok(Self::with_resolver(Arc::new(move || {
            Some(EntityBoardServices {
                entity_ctx: Arc::clone(&entity_ctx),
                kanban: Some(Arc::clone(&kanban)),
                clipboard: Some(Arc::clone(&clipboard_provider)),
                ui_state: Some(Arc::clone(&ui_state)),
            })
        })))
    }

    /// Build a server that resolves the active board's services per call.
    ///
    /// Production constructor: pairs with a dispatcher-set
    /// `tokio::task_local`. The resolver is consulted at the top of every
    /// tool handler so a single `EntityServer` can serve every board on a
    /// plugin host. Returning `None` from the resolver surfaces as a
    /// tool-level `internal_error` rather than panicking.
    pub fn with_resolver(resolver: EntityBoardResolver) -> Self {
        Self { resolver }
    }

    /// Resolve the active board's services, or return a structured
    /// `internal_error` describing the gap.
    fn services(&self) -> Result<EntityBoardServices, McpError> {
        (self.resolver)().ok_or_else(|| {
            McpError::internal_error(
                "no EntityBoardServices active on this tokio task; \
                 the dispatcher must scope a board (see `scope_store_context` / \
                 the entity-mcp equivalent) before invoking an `entity` tool",
                None,
            )
        })
    }

    /// Return the active board's `Arc<EntityContext>`, when one is scoped.
    ///
    /// Exposed for tests that need to verify the server resolves the same
    /// kernel the rest of the app reads from (`Arc::ptr_eq`). Returns
    /// `None` when no board is scoped on the current task.
    pub fn context(&self) -> Option<Arc<EntityContext>> {
        (self.resolver)().map(|s| s.entity_ctx)
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
            description: "Generic, type-agnostic CRUD + archive + clipboard over the shared EntityContext kernel.",
            operations: operations(),
        }
    }

    /// Handle a `GetEntity` call — read one entity as JSON.
    async fn handle_get(&self, req: GetEntity) -> Result<Value, McpError> {
        let services = self.services()?;
        let entity = services
            .entity_ctx
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
        let services = self.services()?;
        let entities = services
            .entity_ctx
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
        let services = self.services()?;
        let id = req
            .id
            .filter(|s| !s.is_empty())
            .map(EntityId::from)
            .unwrap_or_else(EntityId::new);
        let mut entity = Entity::new(req.entity_type.as_str(), id.clone());
        for (field, value) in req.fields {
            entity.set(field, value);
        }
        let entry_id = services
            .entity_ctx
            .write(&entity)
            .await
            .map_err(entity_error_to_mcp)?;
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
        let services = self.services()?;
        let mut entity = services
            .entity_ctx
            .read(&req.entity_type, &req.id)
            .await
            .map_err(entity_error_to_mcp)?;
        entity.set(req.field, req.value);
        let entry_id = services
            .entity_ctx
            .write(&entity)
            .await
            .map_err(entity_error_to_mcp)?;
        Ok(serde_json::json!({
            "ok": true,
            "id": req.id,
            "entry_id": entry_id.map(|e| e.to_string()),
        }))
    }

    /// Handle a `DeleteEntity` call — trash an entity.
    async fn handle_delete(&self, req: DeleteEntity) -> Result<Value, McpError> {
        let services = self.services()?;
        let entry_id = services
            .entity_ctx
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
        let services = self.services()?;
        let entry_id = services
            .entity_ctx
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
        let services = self.services()?;
        let entry_id = services
            .entity_ctx
            .unarchive(&req.entity_type, &req.id)
            .await
            .map_err(entity_error_to_mcp)?;
        Ok(serde_json::json!({
            "ok": true,
            "entry_id": entry_id.map(|e| e.to_string()),
        }))
    }

    /// Handle a `Search` call — free-text query over the live entities.
    ///
    /// The index is built fresh from the kernel's current entities on every
    /// call so results never go stale after a write made through this same
    /// server. When `type` is supplied only that one type is loaded (which
    /// both narrows the result set and avoids scanning the other types);
    /// otherwise every entity type the kernel knows about is loaded. The
    /// query runs `EntitySearchIndex::search` (fuzzy over entity fields), and
    /// each hit is resolved back to its full entity for the response.
    async fn handle_search(&self, req: Search) -> Result<Value, McpError> {
        let services = self.services()?;
        let entities =
            Self::collect_searchable(&services.entity_ctx, req.entity_type.as_deref()).await?;
        let index = EntitySearchIndex::from_entities(entities);
        let hits = index.search(&req.query, SEARCH_LIMIT);

        let results: Vec<Value> = hits
            .into_iter()
            .filter_map(|hit| {
                index.get(&hit.entity_id).map(|entity| {
                    serde_json::json!({
                        "id": hit.entity_id,
                        "type": entity.entity_type,
                        "score": hit.score,
                        "entity": entity.to_json(),
                    })
                })
            })
            .collect();

        Ok(serde_json::json!({
            "ok": true,
            "results": results,
        }))
    }

    /// Collect the entities to search: a single type when `entity_type` is
    /// given, otherwise every entity type the kernel's schema declares.
    ///
    /// An unknown explicit type surfaces the kernel's structured error;
    /// types that simply have no entities yet contribute nothing.
    async fn collect_searchable(
        entity_ctx: &Arc<EntityContext>,
        entity_type: Option<&str>,
    ) -> Result<Vec<Entity>, McpError> {
        match entity_type {
            Some(ty) => entity_ctx.list(ty).await.map_err(entity_error_to_mcp),
            None => {
                let types: Vec<String> = entity_ctx
                    .fields()
                    .all_entities()
                    .iter()
                    .map(|def| def.name.as_str().to_string())
                    .collect();
                let mut all = Vec::new();
                for ty in types {
                    // A type with no live entities lists empty; skip read
                    // errors for types that aren't backed by a store so one
                    // unbacked type can't sink the whole search.
                    if let Ok(entities) = entity_ctx.list(&ty).await {
                        all.extend(entities);
                    }
                }
                Ok(all)
            }
        }
    }

    /// Build a `CommandContext` carrying the clipboard wiring's extensions.
    ///
    /// The shared clipboard commands resolve their services from the
    /// context's extension map: the [`KanbanContext`] (paste handlers read
    /// `entity_context()` and run sub-ops against it) and the
    /// [`ClipboardProviderExt`] (the clipboard seam), plus the [`UIState`]
    /// that copy/cut flag with the clipboard entity type. `scope` is the
    /// innermost-first moniker chain; `target` is the entity / destination
    /// moniker the command operates on.
    ///
    /// Returns a structured `internal_error` when the active board's
    /// services lack the clipboard wiring (e.g. server constructed via
    /// [`EntityServer::new`]).
    fn build_clipboard_command_context(
        services: &EntityBoardServices,
        command_id: &str,
        scope: Vec<String>,
        target: Option<String>,
    ) -> Result<CommandContext, McpError> {
        let kanban = services.kanban.as_ref().ok_or_else(|| {
            McpError::internal_error(
                "this entity server was constructed without clipboard wiring \
                 (EntityServer::new); use with_clipboard / with_resolver",
                None,
            )
        })?;
        let clipboard = services.clipboard.as_ref().ok_or_else(|| {
            McpError::internal_error(
                "this entity server was constructed without clipboard wiring \
                 (EntityServer::new); use with_clipboard / with_resolver",
                None,
            )
        })?;
        let ui_state = services.ui_state.as_ref().ok_or_else(|| {
            McpError::internal_error(
                "this entity server was constructed without clipboard wiring \
                 (EntityServer::new); use with_clipboard / with_resolver",
                None,
            )
        })?;
        let mut ctx = CommandContext::new(command_id, scope, target, HashMap::new());
        ctx.set_extension(Arc::clone(kanban));
        ctx.set_extension(Arc::new(ClipboardProviderExt(Arc::clone(clipboard))));
        ctx.ui_state = Some(Arc::clone(ui_state));
        Ok(ctx)
    }

    /// Handle a `Copy` call — snapshot the `type:id` entity to the
    /// clipboard via the shared [`CopyEntityCmd`].
    async fn handle_copy(&self, req: Copy) -> Result<Value, McpError> {
        let services = self.services()?;
        let target = format!("{}:{}", req.entity_type, req.id);
        let ctx = Self::build_clipboard_command_context(
            &services,
            "entity.copy",
            req.scope,
            Some(target),
        )?;
        CopyEntityCmd
            .execute(&ctx)
            .await
            .map_err(command_error_to_mcp)
    }

    /// Handle a `Cut` call — copy then run the destructive op via the
    /// shared [`CutEntityCmd`]. The destructive write flows through the
    /// kernel's `StoreContext`, so it is undoable and emits an event.
    async fn handle_cut(&self, req: Cut) -> Result<Value, McpError> {
        let services = self.services()?;
        let target = format!("{}:{}", req.entity_type, req.id);
        let ctx = Self::build_clipboard_command_context(
            &services,
            "entity.cut",
            req.scope,
            Some(target),
        )?;
        CutEntityCmd.execute(&ctx).await.map_err(command_error_to_mcp)
    }

    /// Handle a `Paste` call — dispatch the clipboard payload onto the
    /// target moniker through the shared [`PasteEntityCmd`]'s `PasteMatrix`.
    /// The matched handler writes through the kernel, so the paste is
    /// undoable and emits entity events.
    async fn handle_paste(&self, req: Paste) -> Result<Value, McpError> {
        let services = self.services()?;
        let ctx = Self::build_clipboard_command_context(
            &services,
            "entity.paste",
            req.scope,
            Some(req.target),
        )?;
        PasteEntityCmd::new()
            .execute(&ctx)
            .await
            .map_err(command_error_to_mcp)
    }
}

/// Maximum number of search hits returned by a `Search` call.
const SEARCH_LIMIT: usize = 50;

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

/// Map a [`CommandError`] from a shared clipboard command onto a
/// structured [`McpError`].
///
/// Caller-recoverable shape failures (missing scope/arg, bad moniker,
/// missing source / invalid destination) map to `invalid_params` so a
/// client can branch on them; the catch-all `ExecutionFailed` and the I/O
/// error variants map to `internal_error`.
fn command_error_to_mcp(err: swissarmyhammer_commands::CommandError) -> McpError {
    use swissarmyhammer_commands::CommandError as CE;
    let message = err.to_string();
    match err {
        CE::NotFound(_)
        | CE::NotAvailable(_)
        | CE::MissingScope(_)
        | CE::MissingArg(_)
        | CE::InvalidMoniker(_)
        | CE::SourceEntityMissing(_)
        | CE::DestinationInvalid(_) => McpError::invalid_params(message, None),
        CE::ExecutionFailed(_) | CE::Yaml(_) | CE::Json(_) => {
            McpError::internal_error(message, None)
        }
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
            "search entities" => {
                let req: Search = deserialize_op(arguments, &op)?;
                self.handle_search(req).await?
            }
            "copy entity" => {
                let req: Copy = deserialize_op(arguments, &op)?;
                self.handle_copy(req).await?
            }
            "cut entity" => {
                let req: Cut = deserialize_op(arguments, &op)?;
                self.handle_cut(req).await?
            }
            "paste entity" => {
                let req: Paste = deserialize_op(arguments, &op)?;
                self.handle_paste(req).await?
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Outside a [`scope_entity_board_services`], the production resolver
    /// returns `None` — every tool handler will surface that as a
    /// structured "no board scoped" error rather than panicking. Pins
    /// the graceful-degradation contract the dispatcher relies on.
    #[tokio::test]
    async fn task_local_resolver_returns_none_outside_scope() {
        let resolver = task_local_resolver();
        assert!(
            resolver().is_none(),
            "no CURRENT_ENTITY_BOARD_SERVICES is scoped on this task",
        );
    }
}
