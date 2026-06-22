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
    default_scope, Perspective, PerspectiveContext, PerspectiveError, SortDirection, SortEntry,
};

use crate::context::ViewsContext;
use crate::error::ViewsError;
use crate::operations::{
    operations, ClearFilter, ClearGroup, ClearSort, GotoPerspective, LoadPerspective,
    RenamePerspective, SavePerspective, SetFilter, SetGroup, SetSort, SetView, ToggleSort,
};
use crate::types::{ViewDef, ViewKind};

/// The per-board kernels a [`ViewsServer`] needs at tool-call time.
///
/// Perspectives and views are per-board state — each board's
/// [`PerspectiveContext`] / [`ViewsContext`] push their writes onto that
/// board's `StoreContext`. The multi-board kanban app therefore resolves the
/// active board's pair per call rather than capturing one pair at construction.
///
/// `Clone` so the value can be placed into a `tokio::task_local!` (see
/// [`scope_views_board_services`]) and resolved out cheaply per call by the
/// production resolver.
#[derive(Clone)]
pub struct ViewsBoardServices {
    /// The board's shared perspective kernel.
    pub perspectives: Arc<RwLock<PerspectiveContext>>,
    /// The board's shared views kernel.
    pub views: Arc<RwLock<ViewsContext>>,
}

/// Resolves the [`ViewsBoardServices`] to drive for the current task.
///
/// Production deployments back this with a `tokio::task_local!` scope set by
/// the dispatcher (see [`scope_views_board_services`] / [`task_local_resolver`]).
/// Returning `None` means no board is scoped on this task; tool handlers
/// surface that as an `internal_error` rather than panicking.
pub type ViewsBoardResolver = Arc<dyn Fn() -> Option<ViewsBoardServices> + Send + Sync>;

tokio::task_local! {
    /// Per-task active [`ViewsBoardServices`] for production dispatch.
    ///
    /// The kanban app is multi-board: each board's kernel pair is scoped here
    /// by the dispatcher (alongside `swissarmyhammer-kanban`'s `CURRENT_STORE_CTX`
    /// and the entity-mcp `CURRENT_ENTITY_BOARD_SERVICES`), and the production
    /// [`ViewsServer`] resolver — [`task_local_resolver`] — reads back the same
    /// pair inside its per-call `services()` lookup.
    ///
    /// Outside a [`scope_views_board_services`] (e.g. in tests that build the
    /// server with a constant pair via [`ViewsServer::new`]) this task-local is
    /// unset and a resolver built from [`task_local_resolver`] returns `None`.
    pub static CURRENT_VIEWS_BOARD_SERVICES: ViewsBoardServices;
}

/// Scope [`CURRENT_VIEWS_BOARD_SERVICES`] to `services` for the duration of
/// `fut`.
///
/// The production [`ViewsServer`] resolver ([`task_local_resolver`]) reads back
/// the scoped pair inside every tool call, so the in-process `views` MCP
/// surface routes per call to whichever board's kernels the dispatcher scoped.
pub async fn scope_views_board_services<F>(services: ViewsBoardServices, fut: F) -> F::Output
where
    F: std::future::Future,
{
    CURRENT_VIEWS_BOARD_SERVICES.scope(services, fut).await
}

/// Build the production [`ViewsBoardResolver`] that reads
/// [`CURRENT_VIEWS_BOARD_SERVICES`].
///
/// Pair this with [`ViewsServer::with_resolver`]; the app's dispatcher then
/// scopes the per-board pair around its dispatch via
/// [`scope_views_board_services`]. Outside a scope the resolver returns `None`
/// and tool calls fail with a structured error — a dispatcher that forgets to
/// scope degrades gracefully rather than panicking.
pub fn task_local_resolver() -> ViewsBoardResolver {
    Arc::new(|| {
        CURRENT_VIEWS_BOARD_SERVICES
            .try_with(|services| services.clone())
            .ok()
    })
}

/// In-process `rmcp::ServerHandler` for the `views` operation tool.
///
/// Holds a [`ViewsBoardResolver`] — consulted at the top of every verb handler
/// — so a single `ViewsServer` exposed app-wide on a plugin host can route per
/// call to whichever board's kernels are scoped on the current `tokio` task.
/// The previous direct-handle constructor is preserved as a thin wrapper that
/// produces a resolver returning the same pair every call.
#[derive(Clone)]
pub struct ViewsServer {
    /// Resolves the active board's perspective + views kernels per call. See
    /// [`ViewsBoardResolver`].
    resolver: ViewsBoardResolver,
}

impl std::fmt::Debug for ViewsServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewsServer").finish()
    }
}

impl ViewsServer {
    /// Construct a server wired to a single board's perspective and views
    /// kernels.
    ///
    /// Preserved as a constant-pair wrapper around [`ViewsServer::with_resolver`]
    /// so single-board callers (most tests) keep a simple constructor.
    pub fn new(
        perspectives: Arc<RwLock<PerspectiveContext>>,
        views: Arc<RwLock<ViewsContext>>,
    ) -> Self {
        Self::with_resolver(Arc::new(move || {
            Some(ViewsBoardServices {
                perspectives: Arc::clone(&perspectives),
                views: Arc::clone(&views),
            })
        }))
    }

    /// Build a server that resolves the active board's kernels per call.
    ///
    /// Production constructor: pairs with a dispatcher-set `tokio::task_local`.
    /// The resolver is consulted at the top of every verb handler so a single
    /// `ViewsServer` can serve every board on a plugin host. Returning `None`
    /// from the resolver surfaces as a tool-level `internal_error` rather than
    /// panicking.
    pub fn with_resolver(resolver: ViewsBoardResolver) -> Self {
        Self { resolver }
    }

    /// Resolve the active board's kernels, or return a structured
    /// `internal_error` describing the gap.
    fn services(&self) -> Result<ViewsBoardServices, McpError> {
        (self.resolver)().ok_or_else(|| {
            McpError::internal_error(
                "no ViewsBoardServices active on this tokio task; the dispatcher \
                 must scope a board (see `scope_views_board_services`) before \
                 invoking a `views` tool",
                None,
            )
        })
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
        let services = self.services()?;
        let pctx = services.perspectives.read().await;
        let perspective = pctx
            .get_by_name(&req.name)
            .or_else(|| pctx.get_by_id(&req.name))
            .ok_or_else(|| perspective_not_found(&req.name))?;
        Ok(json!({ "ok": true, "perspective": perspective_to_json(perspective)? }))
    }

    /// Handle `save perspective` — build a perspective and write it.
    ///
    /// With `if_absent` set this becomes the idempotent ensure path the
    /// frontend's auto-create-default dispatches (via the
    /// `perspective-commands` plugin): see [`Self::handle_ensure_save`].
    async fn handle_save(&self, req: SavePerspective) -> Result<Value, McpError> {
        let services = self.services()?;
        if req.if_absent {
            return self.handle_ensure_save(req, &services).await;
        }

        let id = req
            .id
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| ulid::Ulid::new().to_string());
        let mut perspective = Perspective::new(id, req.name, req.view);
        perspective.view_id = req.view_id;
        perspective.filter = req.filter;
        perspective.group = req.group;

        let mut pctx = services.perspectives.write().await;
        let entry_id = pctx.write(&perspective).await.map_err(persp_error_to_mcp)?;
        Ok(json!({
            "ok": true,
            "perspective": perspective_to_json(&perspective)?,
            "entry_id": entry_id.map(|e| e.to_string()),
        }))
    }

    /// Handle `save perspective` in `if_absent` (ensure) mode.
    ///
    /// Storage-layer convergence guard for the auto-created "Default"
    /// perspective (live bug 01KTY6T1GPY94VYWANE9X41SKJ — duplicate Defaults
    /// minted per window per boot, pinned to the frontend's `"default"`
    /// placeholder view id):
    ///
    /// 1. A `view_id` the views registry does not know falls back to the
    ///    view-kind scope — otherwise an ensure against a dead view re-mints
    ///    a default the next board-open reconciliation prunes (create/prune
    ///    churn). With an EMPTY registry the filename-safety check is the
    ///    backstop against path separators and overlong components.
    /// 2. An existing perspective for the scope is returned WITHOUT a write
    ///    (no store-changed notification, no refetch loop re-trigger).
    /// 3. A genuine create lands under the deterministic `default-<scope>`
    ///    id so racing creators converge on the same file.
    ///
    /// The scope rules are shared with the kanban board-open reconciliation
    /// via `swissarmyhammer_perspectives::default_scope`.
    async fn handle_ensure_save(
        &self,
        req: SavePerspective,
        services: &ViewsBoardServices,
    ) -> Result<Value, McpError> {
        let mut view_id = req.view_id.filter(|s| !s.is_empty());
        if let Some(vid) = view_id.clone() {
            let views = services.views.read().await;
            let valid = if views.all_views().is_empty() {
                default_scope::is_safe_scope_component(&vid)
            } else {
                views.get_by_id(&vid).is_some()
            };
            if !valid {
                tracing::warn!(
                    view_id = %vid,
                    "ensure save: view_id is unknown or unsafe — falling back to view-kind scope"
                );
                view_id = None;
            }
        }

        let mut pctx = services.perspectives.write().await;
        if let Some(existing) = pctx
            .all()
            .iter()
            .find(|p| default_scope::matches_scope(p, view_id.as_deref(), &req.view))
        {
            return Ok(json!({
                "ok": true,
                "perspective": perspective_to_json(existing)?,
                "entry_id": Value::Null,
            }));
        }

        let scope = view_id.clone().unwrap_or_else(|| req.view.clone());
        let id = default_scope::default_perspective_id(&scope);
        let mut perspective = Perspective::new(id, req.name, req.view);
        perspective.view_id = view_id;
        perspective.filter = req.filter;
        perspective.group = req.group;

        let entry_id = pctx.write(&perspective).await.map_err(persp_error_to_mcp)?;
        Ok(json!({
            "ok": true,
            "perspective": perspective_to_json(&perspective)?,
            "entry_id": entry_id.map(|e| e.to_string()),
        }))
    }

    /// Handle `rename perspective` — change a perspective's name.
    async fn handle_rename(&self, req: RenamePerspective) -> Result<Value, McpError> {
        let services = self.services()?;
        let mut pctx = services.perspectives.write().await;
        let updated = pctx
            .rename(&req.id, req.new_name)
            .await
            .map_err(persp_error_to_mcp)?;
        Ok(json!({ "ok": true, "perspective": perspective_to_json(&updated)? }))
    }

    /// Handle `list perspective` — every loaded perspective.
    async fn handle_list(&self) -> Result<Value, McpError> {
        let services = self.services()?;
        let pctx = services.perspectives.read().await;
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
        let services = self.services()?;
        let mut pctx = services.perspectives.write().await;
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
            let current = p
                .sort
                .iter()
                .find(|e| e.field == field)
                .map(|e| e.direction.clone());
            p.sort.retain(|e| e.field != field);
            match current {
                None => p.sort.push(SortEntry::new(field, SortDirection::Asc)),
                Some(SortDirection::Asc) => p.sort.push(SortEntry::new(field, SortDirection::Desc)),
                // desc -> none: already removed by the retain above.
                Some(SortDirection::Desc) => {}
            }
        })
        .await
    }

    // --- Navigation ------------------------------------------------------
    //
    // Only `goto` (pure resolution) lives here. Perspective ACTIVATION
    // (next / prev / switch) moved to the `entity` tool — it needs the shared
    // `UIState` this server does not hold. See card 01KTYQY0ZB62KHN6BPK3FBMBD7.

    /// Handle `goto perspective` — resolve by id, optionally validating view.
    async fn handle_goto(&self, req: GotoPerspective) -> Result<Value, McpError> {
        let services = self.services()?;
        let pctx = services.perspectives.read().await;
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

    // --- Views -----------------------------------------------------------

    /// Handle `set view` — create or update a view definition.
    ///
    /// On an EXISTING view this is a partial read-modify-write merge: only the
    /// fields present on the wire overwrite the on-disk view, omitted optional
    /// fields preserve their current value (the tri-state optionals on
    /// [`SetView`] distinguish "omitted → preserve" from an explicit empty/null
    /// → clear). `commands` has no wire surface and is always preserved across
    /// updates. On a NON-EXISTENT view (or an omitted id, so a fresh ULID is
    /// minted) the view is created from the supplied fields, with omitted
    /// optionals defaulting to empty/none.
    async fn handle_set_view(&self, mut req: SetView) -> Result<Value, McpError> {
        let id = req
            .id
            .take()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| ulid::Ulid::new().to_string());

        let services = self.services()?;
        let mut vctx = services.views.write().await;

        // Read-modify-write: when the view already exists, omitted fields
        // preserve the existing value; only provided fields are overwritten.
        let existing = vctx.get_by_id(&id).cloned();
        let def = merge_set_view(req, id, existing);

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
fn perspective_belongs_to_view(
    p: &Perspective,
    active_view_id: Option<&str>,
    view_kind: &str,
) -> bool {
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

/// Merge a [`SetView`] request onto the existing view (if any) into a
/// [`ViewDef`] to persist.
///
/// Implements the partial read-modify-write contract for `set view`:
///
/// - **Update** (`existing` is `Some`): every optional field on the request is
///   tri-state — omitted (`None`) preserves the existing value, while an
///   explicit value (including an explicit empty/null) overwrites it. `name`
///   and `kind` likewise preserve the existing value when omitted. `commands`
///   has no wire surface and is carried over verbatim, so a partial update
///   cannot strip it.
/// - **Create** (`existing` is `None`): omitted optionals default to
///   empty/none, omitted `kind` falls through to [`ViewKind::Unknown`], and an
///   omitted `name` defaults to empty — which `ViewDef::validate` rejects, so
///   a create still requires a real name.
fn merge_set_view(req: SetView, id: String, existing: Option<ViewDef>) -> ViewDef {
    match existing {
        Some(existing) => ViewDef {
            id,
            name: req.name.unwrap_or(existing.name),
            kind: req
                .kind
                .map(|k| parse_view_kind(&k))
                .unwrap_or(existing.kind),
            icon: req.icon.unwrap_or(existing.icon),
            entity_type: req.entity_type.unwrap_or(existing.entity_type),
            card_fields: req.card_fields.unwrap_or(existing.card_fields),
            // `commands` is not exposed on the wire; preserve it across updates.
            commands: existing.commands,
        },
        None => ViewDef {
            id,
            name: req.name.unwrap_or_default(),
            kind: req
                .kind
                .map(|k| parse_view_kind(&k))
                .unwrap_or(ViewKind::Unknown),
            icon: req.icon.unwrap_or_default(),
            entity_type: req.entity_type.unwrap_or_default(),
            card_fields: req.card_fields.unwrap_or_default(),
            commands: Vec::new(),
        },
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
        PerspectiveError::NotFound { resource, id } => {
            McpError::invalid_params(message, Some(json!({ "resource": resource, "id": id })))
        }
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
        ViewsError::InvalidViewDef { id, reason } => {
            McpError::invalid_params(message, Some(json!({ "id": id, "reason": reason })))
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
            "rename perspective" => self.handle_rename(deserialize_op(arguments, op)?).await,
            "list perspective" => self.handle_list().await,
            "set filter" => self.handle_set_filter(deserialize_op(arguments, op)?).await,
            "focus filter" => Ok(json!({ "ok": true })),
            "clear filter" => {
                self.handle_clear_filter(deserialize_op(arguments, op)?)
                    .await
            }
            "set group" => self.handle_set_group(deserialize_op(arguments, op)?).await,
            "clear group" => {
                self.handle_clear_group(deserialize_op(arguments, op)?)
                    .await
            }
            "set sort" => self.handle_set_sort(deserialize_op(arguments, op)?).await,
            "clear sort" => self.handle_clear_sort(deserialize_op(arguments, op)?).await,
            "toggle sort" => {
                self.handle_toggle_sort(deserialize_op(arguments, op)?)
                    .await
            }
            "goto perspective" => self.handle_goto(deserialize_op(arguments, op)?).await,
            "set view" => self.handle_set_view(deserialize_op(arguments, op)?).await,
            other => Err(McpError::invalid_params(
                format!("unknown op {other:?} for views tool"),
                None,
            )),
        }
    }
}
