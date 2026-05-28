//! In-process `rmcp::ServerHandler` for the `views` operation tool.
//!
//! [`ViewsServer`] is the MCP face over two registry kernels: the
//! `PerspectiveContext` (owned by `swissarmyhammer-perspectives`) and the
//! `ViewsContext` (owned by this crate). It holds an `Arc<RwLock<…>>` to each
//! and advertises a single `views` operation tool whose `inputSchema` and
//! `_meta` are derived from the operation structs in [`crate::operations`].
//!
//! Every verb routes through an existing context method — there is no
//! duplicated perspective/view state here. Because each context pushes its
//! writes onto the shared `StoreContext`, undo / redo work for free: this
//! server is a thin translation layer between the wire protocol and the two
//! kernels, and implements no undo of its own.

use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use swissarmyhammer_operations_macros::operation_tool;
use tokio::sync::RwLock;

use swissarmyhammer_perspectives::{
    Perspective, PerspectiveContext, PerspectiveError, SortDirection, SortEntry,
};

use crate::context::ViewsContext;
use crate::error::ViewsError;
use crate::operations::{
    operations, ClearFilter, ClearGroup, ClearSort, DeletePerspective, GotoPerspective,
    LoadPerspective, NextPerspective, PrevPerspective, RenamePerspective, SavePerspective,
    SetFilter, SetGroup, SetSort, SetView, SwitchPerspective, ToggleSort,
};
use crate::types::{ViewDef, ViewKind};

/// In-process `rmcp::ServerHandler` for the `views` operation tool.
///
/// Holds shared handles to the perspective and views kernels so every verb
/// dispatches against the same context the rest of the app reads from.
#[derive(Clone)]
pub struct ViewsServer {
    /// The shared perspective kernel (from `swissarmyhammer-perspectives`).
    perspectives: Arc<RwLock<PerspectiveContext>>,
    /// The shared views kernel (from this crate).
    views: Arc<RwLock<ViewsContext>>,
}

impl std::fmt::Debug for ViewsServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewsServer").finish()
    }
}

impl ViewsServer {
    /// Construct a server wired to the shared perspective and views kernels.
    pub fn new(
        perspectives: Arc<RwLock<PerspectiveContext>>,
        views: Arc<RwLock<ViewsContext>>,
    ) -> Self {
        Self {
            perspectives,
            views,
        }
    }

    /// Return the shared perspective kernel handle.
    ///
    /// Exposed for tests that verify the server shares the same kernel the
    /// rest of the app reads from (`Arc::ptr_eq`).
    pub fn perspective_context(&self) -> Arc<RwLock<PerspectiveContext>> {
        Arc::clone(&self.perspectives)
    }

    /// Return the shared views kernel handle.
    pub fn views_context(&self) -> Arc<RwLock<ViewsContext>> {
        Arc::clone(&self.views)
    }

    /// Build the platform-facing `views` tool definition.
    ///
    /// The `inputSchema` (flat `op` enum) and the `_meta` discovery tree both
    /// derive from the same operation slice via `operation_tool!`, so they
    /// cannot drift.
    fn build_tool_definition() -> Tool {
        operation_tool! {
            name: "views",
            description: "Perspective + view state mutations over the shared PerspectiveContext and ViewsContext kernels.",
            operations: operations(),
        }
    }

    // --- Perspective lifecycle ------------------------------------------

    /// Handle `load perspective` — resolve by name, then by id.
    async fn handle_load(&self, req: LoadPerspective) -> Result<Value, McpError> {
        let pctx = self.perspectives.read().await;
        let perspective = pctx
            .get_by_name(&req.name)
            .or_else(|| pctx.get_by_id(&req.name))
            .ok_or_else(|| perspective_not_found(&req.name))?;
        Ok(json!({ "ok": true, "perspective": perspective_to_json(perspective)? }))
    }

    /// Handle `save perspective` — build a perspective and write it.
    async fn handle_save(&self, req: SavePerspective) -> Result<Value, McpError> {
        let id = req
            .id
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| ulid::Ulid::new().to_string());
        let mut perspective = Perspective::new(id, req.name, req.view);
        perspective.view_id = req.view_id;
        perspective.filter = req.filter;
        perspective.group = req.group;

        let mut pctx = self.perspectives.write().await;
        let entry_id = pctx.write(&perspective).await.map_err(persp_error_to_mcp)?;
        Ok(json!({
            "ok": true,
            "perspective": perspective_to_json(&perspective)?,
            "entry_id": entry_id.map(|e| e.to_string()),
        }))
    }

    /// Handle `delete perspective` — trash a perspective by id.
    async fn handle_delete(&self, req: DeletePerspective) -> Result<Value, McpError> {
        let mut pctx = self.perspectives.write().await;
        let (_deleted, entry_id) = pctx.delete(&req.id).await.map_err(persp_error_to_mcp)?;
        Ok(json!({ "ok": true, "entry_id": entry_id.map(|e| e.to_string()) }))
    }

    /// Handle `rename perspective` — change a perspective's name.
    async fn handle_rename(&self, req: RenamePerspective) -> Result<Value, McpError> {
        let mut pctx = self.perspectives.write().await;
        let updated = pctx
            .rename(&req.id, req.new_name)
            .await
            .map_err(persp_error_to_mcp)?;
        Ok(json!({ "ok": true, "perspective": perspective_to_json(&updated)? }))
    }

    /// Handle `list perspective` — every loaded perspective.
    async fn handle_list(&self) -> Result<Value, McpError> {
        let pctx = self.perspectives.read().await;
        let all = pctx.all();
        let perspectives: Result<Vec<Value>, McpError> =
            all.iter().map(perspective_to_json).collect();
        let perspectives = perspectives?;
        let count = perspectives.len();
        Ok(json!({ "ok": true, "perspectives": perspectives, "count": count }))
    }

    // --- Filter / group / sort ------------------------------------------

    /// Read a perspective by id, apply `mutate`, write it back, and return the
    /// updated perspective plus the undo entry id.
    ///
    /// This is the shared body for every "edit one field then write" verb
    /// (filter, group, sort). Holds the write lock across the read-modify-write
    /// so the perspective cannot be mutated concurrently between snapshot and
    /// write.
    async fn mutate_perspective(
        &self,
        perspective_id: &str,
        mutate: impl FnOnce(&mut Perspective),
    ) -> Result<Value, McpError> {
        let mut pctx = self.perspectives.write().await;
        let mut perspective = pctx
            .get_by_id(perspective_id)
            .ok_or_else(|| perspective_not_found(perspective_id))?
            .clone();
        mutate(&mut perspective);
        let entry_id = pctx.write(&perspective).await.map_err(persp_error_to_mcp)?;
        Ok(json!({
            "ok": true,
            "perspective": perspective_to_json(&perspective)?,
            "entry_id": entry_id.map(|e| e.to_string()),
        }))
    }

    /// Handle `set filter` — store a filter expression verbatim.
    async fn handle_set_filter(&self, req: SetFilter) -> Result<Value, McpError> {
        self.mutate_perspective(&req.perspective_id, |p| p.filter = Some(req.filter))
            .await
    }

    /// Handle `clear filter` — drop the filter expression.
    async fn handle_clear_filter(&self, req: ClearFilter) -> Result<Value, McpError> {
        self.mutate_perspective(&req.perspective_id, |p| p.filter = None)
            .await
    }

    /// Handle `set group` — store a group-by field.
    async fn handle_set_group(&self, req: SetGroup) -> Result<Value, McpError> {
        self.mutate_perspective(&req.perspective_id, |p| p.group = Some(req.group))
            .await
    }

    /// Handle `clear group` — drop the group-by field.
    async fn handle_clear_group(&self, req: ClearGroup) -> Result<Value, McpError> {
        self.mutate_perspective(&req.perspective_id, |p| p.group = None)
            .await
    }

    /// Handle `set sort` — add or replace a sort entry for a field.
    ///
    /// Mirrors `SetSortCmd`: removes any existing entry for the field, then
    /// appends `{ field, direction }`.
    async fn handle_set_sort(&self, req: SetSort) -> Result<Value, McpError> {
        let direction = parse_direction(&req.direction)?;
        let field = req.field;
        self.mutate_perspective(&req.perspective_id, move |p| {
            p.sort.retain(|e| e.field != field);
            p.sort.push(SortEntry::new(field, direction));
        })
        .await
    }

    /// Handle `clear sort` — drop every sort entry.
    async fn handle_clear_sort(&self, req: ClearSort) -> Result<Value, McpError> {
        self.mutate_perspective(&req.perspective_id, |p| p.sort.clear())
            .await
    }

    /// Handle `toggle sort` — cycle a field through none → asc → desc → none.
    ///
    /// Mirrors `ToggleSortCmd`'s state machine.
    async fn handle_toggle_sort(&self, req: ToggleSort) -> Result<Value, McpError> {
        let field = req.field;
        self.mutate_perspective(&req.perspective_id, move |p| {
            let current = p.sort.iter().find(|e| e.field == field).map(|e| e.direction.clone());
            p.sort.retain(|e| e.field != field);
            match current {
                None => p.sort.push(SortEntry::new(field, SortDirection::Asc)),
                Some(SortDirection::Asc) => {
                    p.sort.push(SortEntry::new(field, SortDirection::Desc))
                }
                // desc -> none: already removed by the retain above.
                Some(SortDirection::Desc) => {}
            }
        })
        .await
    }

    // --- Navigation ------------------------------------------------------

    /// Handle `next perspective` — resolve the next perspective in a view.
    async fn handle_next(&self, req: NextPerspective) -> Result<Value, McpError> {
        self.cycle(&req.view, req.view_id.as_deref(), req.current.as_deref(), 1)
            .await
    }

    /// Handle `prev perspective` — resolve the previous perspective in a view.
    async fn handle_prev(&self, req: PrevPerspective) -> Result<Value, McpError> {
        self.cycle(&req.view, req.view_id.as_deref(), req.current.as_deref(), -1)
            .await
    }

    /// Shared cycling body for next/prev navigation.
    ///
    /// Builds the ordered list of perspectives belonging to the view, finds
    /// `current` in it, and steps by `step` (wrapping). Returns
    /// `{ ok: true, perspective: null }` when fewer than two match — there is
    /// nothing to cycle to.
    async fn cycle(
        &self,
        view: &str,
        view_id: Option<&str>,
        current: Option<&str>,
        step: isize,
    ) -> Result<Value, McpError> {
        let pctx = self.perspectives.read().await;
        let matching: Vec<&Perspective> = pctx
            .all()
            .iter()
            .filter(|p| perspective_belongs_to_view(p, view_id, view))
            .collect();

        if matching.len() < 2 {
            return Ok(json!({ "ok": true, "perspective": Value::Null }));
        }

        let len = matching.len() as isize;
        let current_index = current.and_then(|id| matching.iter().position(|p| p.id == id));
        let next_index = match current_index {
            Some(i) => (((i as isize) + step).rem_euclid(len)) as usize,
            // No current match: forward starts at the front, backward at the end.
            None if step > 0 => 0,
            None => (len - 1) as usize,
        };

        Ok(json!({ "ok": true, "perspective": perspective_to_json(matching[next_index])? }))
    }

    /// Handle `goto perspective` — resolve by id, optionally validating view.
    async fn handle_goto(&self, req: GotoPerspective) -> Result<Value, McpError> {
        let pctx = self.perspectives.read().await;
        let perspective = pctx
            .get_by_id(&req.id)
            .ok_or_else(|| perspective_not_found(&req.id))?;

        if let Some(expected_kind) = req.view.as_deref() {
            if !perspective_belongs_to_view(perspective, req.view_id.as_deref(), expected_kind) {
                return Err(McpError::invalid_params(
                    format!(
                        "perspective {:?} does not belong to view (kind={expected_kind:?}, id={:?})",
                        req.id, req.view_id
                    ),
                    None,
                ));
            }
        }

        Ok(json!({ "ok": true, "perspective": perspective_to_json(perspective)? }))
    }

    /// Handle `switch perspective` — resolve by id and surface its filter.
    async fn handle_switch(&self, req: SwitchPerspective) -> Result<Value, McpError> {
        let pctx = self.perspectives.read().await;
        let perspective = pctx
            .get_by_id(&req.perspective_id)
            .ok_or_else(|| perspective_not_found(&req.perspective_id))?;
        Ok(json!({
            "ok": true,
            "perspective": perspective_to_json(perspective)?,
            "filter": perspective.filter.clone().unwrap_or_default(),
        }))
    }

    // --- Views -----------------------------------------------------------

    /// Handle `set view` — create or update a view definition.
    async fn handle_set_view(&self, req: SetView) -> Result<Value, McpError> {
        let id = req
            .id
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| ulid::Ulid::new().to_string());
        let def = ViewDef {
            id,
            name: req.name,
            icon: req.icon,
            kind: parse_view_kind(&req.kind),
            entity_type: req.entity_type,
            card_fields: req.card_fields,
            commands: Vec::new(),
        };

        let mut vctx = self.views.write().await;
        let entry_id = vctx.write_view(&def).await.map_err(views_error_to_mcp)?;
        Ok(json!({
            "ok": true,
            "view": view_to_json(&def)?,
            "entry_id": entry_id.map(|e| e.to_string()),
        }))
    }
}

/// Decide whether a perspective belongs to the given view.
///
/// Ported verbatim from `perspective_commands::perspective_belongs_to_active_view`:
/// id-scoped perspectives (`view_id: Some`) match strictly by id when an
/// active id is known; legacy (`view_id: None`) perspectives match by view
/// kind; scoped perspectives with no known active id do not leak.
fn perspective_belongs_to_view(p: &Perspective, active_view_id: Option<&str>, view_kind: &str) -> bool {
    match (&p.view_id, active_view_id) {
        (Some(pid), Some(active)) => pid == active,
        (None, _) => p.view == view_kind,
        (Some(_), None) => false,
    }
}

/// Parse a sort direction token, erroring on anything but `"asc"`/`"desc"`.
fn parse_direction(direction: &str) -> Result<SortDirection, McpError> {
    match direction {
        "asc" => Ok(SortDirection::Asc),
        "desc" => Ok(SortDirection::Desc),
        other => Err(McpError::invalid_params(
            format!("invalid sort direction {other:?} (expected \"asc\" or \"desc\")"),
            None,
        )),
    }
}

/// Parse a view-kind token into a [`ViewKind`].
///
/// Unknown tokens map to [`ViewKind::Unknown`], matching the enum's
/// `#[serde(other)]` fallthrough, so new kinds need no Rust change.
fn parse_view_kind(kind: &str) -> ViewKind {
    match kind {
        "board" => ViewKind::Board,
        "grid" => ViewKind::Grid,
        "list" => ViewKind::List,
        "calendar" => ViewKind::Calendar,
        "timeline" => ViewKind::Timeline,
        _ => ViewKind::Unknown,
    }
}

/// Serialize a perspective to JSON, mapping serde failures to an rmcp error.
fn perspective_to_json(p: &Perspective) -> Result<Value, McpError> {
    serde_json::to_value(p)
        .map_err(|e| McpError::internal_error(format!("serialize perspective: {e}"), None))
}

/// Serialize a view definition to JSON, mapping serde failures to an rmcp error.
fn view_to_json(def: &ViewDef) -> Result<Value, McpError> {
    serde_json::to_value(def)
        .map_err(|e| McpError::internal_error(format!("serialize view: {e}"), None))
}

/// Build a structured `invalid_params` error for a missing perspective.
fn perspective_not_found(id: &str) -> McpError {
    McpError::invalid_params(
        format!("perspective not found: {id}"),
        Some(json!({ "id": id })),
    )
}

/// Map a JSON value into one of the operation structs.
fn deserialize_op<T: DeserializeOwned>(arguments: Value, op: &str) -> Result<T, McpError> {
    serde_json::from_value(arguments).map_err(|err| {
        McpError::invalid_params(format!("invalid arguments for op {op:?}: {err}"), None)
    })
}

/// Map a [`PerspectiveError`] onto a structured [`McpError`].
///
/// `NotFound` maps to `invalid_params` (client-recoverable); everything else
/// maps to `internal_error`.
fn persp_error_to_mcp(err: PerspectiveError) -> McpError {
    let message = err.to_string();
    match &err {
        PerspectiveError::NotFound { resource, id } => McpError::invalid_params(
            message,
            Some(json!({ "resource": resource, "id": id })),
        ),
        _ => McpError::internal_error(message, None),
    }
}

/// Map a [`ViewsError`] onto a structured [`McpError`].
fn views_error_to_mcp(err: ViewsError) -> McpError {
    let message = err.to_string();
    match &err {
        ViewsError::ViewNotFound { id } => {
            McpError::invalid_params(message, Some(json!({ "id": id })))
        }
        ViewsError::ViewNotFoundByName { name } => {
            McpError::invalid_params(message, Some(json!({ "name": name })))
        }
        _ => McpError::internal_error(message, None),
    }
}

impl ServerHandler for ViewsServer {
    /// Advertise the single `views` operation tool.
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

    /// Route a `tools/call` for the `views` tool to the matching verb handler.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        if request.name.as_ref() != "views" {
            return Err(McpError::invalid_request(
                format!("unknown tool {:?}; expected \"views\"", request.name),
                None,
            ));
        }

        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let op = arguments
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                McpError::invalid_params(
                    "missing required field `op` for views tool".to_string(),
                    None,
                )
            })?
            .to_string();

        let response = self.dispatch(&op, arguments).await?;
        Ok(CallToolResult::structured(response))
    }
}

impl ViewsServer {
    /// Dispatch one `op` string to its verb handler.
    ///
    /// Split out of `call_tool` so the match stays readable and the
    /// `ServerHandler` impl stays short. The set of verbs accepted here is
    /// exactly the set the `inputSchema`'s `op` enum publishes.
    async fn dispatch(&self, op: &str, arguments: Value) -> Result<Value, McpError> {
        match op {
            "load perspective" => self.handle_load(deserialize_op(arguments, op)?).await,
            "save perspective" => self.handle_save(deserialize_op(arguments, op)?).await,
            "delete perspective" => self.handle_delete(deserialize_op(arguments, op)?).await,
            "rename perspective" => self.handle_rename(deserialize_op(arguments, op)?).await,
            "list perspective" => self.handle_list().await,
            "set filter" => self.handle_set_filter(deserialize_op(arguments, op)?).await,
            "focus filter" => Ok(json!({ "ok": true })),
            "clear filter" => self.handle_clear_filter(deserialize_op(arguments, op)?).await,
            "set group" => self.handle_set_group(deserialize_op(arguments, op)?).await,
            "clear group" => self.handle_clear_group(deserialize_op(arguments, op)?).await,
            "set sort" => self.handle_set_sort(deserialize_op(arguments, op)?).await,
            "clear sort" => self.handle_clear_sort(deserialize_op(arguments, op)?).await,
            "toggle sort" => self.handle_toggle_sort(deserialize_op(arguments, op)?).await,
            "next perspective" => self.handle_next(deserialize_op(arguments, op)?).await,
            "prev perspective" => self.handle_prev(deserialize_op(arguments, op)?).await,
            "goto perspective" => self.handle_goto(deserialize_op(arguments, op)?).await,
            "switch perspective" => self.handle_switch(deserialize_op(arguments, op)?).await,
            "set view" => self.handle_set_view(deserialize_op(arguments, op)?).await,
            other => Err(McpError::invalid_params(
                format!("unknown op {other:?} for views tool"),
                None,
            )),
        }
    }
}
