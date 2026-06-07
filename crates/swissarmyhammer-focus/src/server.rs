//! In-process `rmcp::ServerHandler` for the `focus` operation tool.
//!
//! [`FocusServer`] is the spatial-navigation kernel's MCP face. It holds the
//! shared [`SpatialRegistry`] (layer store + cross-snapshot focus memory) and
//! [`SpatialState`] (per-window focus tracker) and advertises a single
//! `focus` operation tool whose `inputSchema` and `_meta` are derived from
//! the operation structs in [`crate::operations`].
//!
//! # 1:1 port of the spatial-nav Tauri commands
//!
//! Every verb dispatches to exactly the [`SpatialRegistry`] / [`SpatialState`]
//! method the matching `spatial_*` Tauri command drove, with no behavior
//! change. Where the Tauri command emitted a `focus-changed` event on the
//! calling `tauri::Window`, this server returns the `FocusChangedEvent` in the
//! structured response instead — the side-effecting `emit` lived in the
//! adapter layer (`apps/kanban-app/src/commands.rs`), not in the kernel, so
//! the port surfaces the event for the caller to forward.
//!
//! `ui.setFocus` (in the ui-commands plugin) routes to the `set focus` op.
//!
//! # State holding
//!
//! The Tauri `AppState` holds `spatial_registry` and `spatial_state` each
//! behind a `tokio::sync::Mutex`, and serializes every spatial command behind
//! both locks taken in a fixed order (`spatial_registry` then `spatial_state`).
//! [`FocusServer`] mirrors that exactly: it holds an `Arc<Mutex<…>>` per piece
//! of state and acquires them in the same canonical order via
//! [`FocusServer::with_spatial`]. No interior-mutability change to
//! `SpatialRegistry` / `SpatialState` was needed — the lock lives on the
//! holder, exactly as it does in `AppState`.

use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde::de::DeserializeOwned;
use serde_json::Value;
use swissarmyhammer_operations_macros::operation_tool;
use tokio::sync::Mutex;

use crate::observer::{FocusEventSink, NoopSink};
use crate::operations::{
    operations, ClearFocus, DrillIn, DrillOut, Focus, FocusLost, GenerateSneakCodes, Navigate,
    PopLayer, PushLayer, QueryFocus, QueryGeometry, QueryScopeChain,
};
use crate::provider::{NoopProvider, UiGeometryProvider};
use crate::registry::SpatialRegistry;
use crate::snapshot::{IndexedSnapshot, NavSnapshot};
use crate::sneak::generate_sneak_codes;
use crate::state::{FocusChangedEvent, SpatialState};
use crate::types::{Direction, FullyQualifiedMoniker, WindowLabel};

/// In-process `rmcp::ServerHandler` for the `focus` operation tool.
///
/// Holds the shared spatial kernel state behind the same `Arc<Mutex<…>>`
/// shape the Tauri `AppState` uses, so production can wire this server to the
/// very same registry / state the rest of the app reads from by cloning the
/// arcs.
#[derive(Clone)]
pub struct FocusServer {
    /// Layer store + cross-snapshot focus memory. Locked before
    /// `spatial_state` for any verb that holds both (canonical order).
    spatial_registry: Arc<Mutex<SpatialRegistry>>,
    /// Per-window focus tracker. Locked after `spatial_registry`.
    spatial_state: Arc<Mutex<SpatialState>>,
    /// Optional observer that mirrors every produced `FocusChangedEvent`
    /// onto an external transport (e.g. the Tauri app's
    /// `app.emit_to("focus-changed", ...)` path). Defaults to [`NoopSink`]
    /// so unit tests that only consume return-value events stay unaffected.
    sink: Arc<dyn FocusEventSink>,
    /// On-demand pull seam into the webview's live UI geometry. The
    /// host-driven nav ops (`navigate`/`drill_in`/`drill_out` with a
    /// `window` and no inline snapshot) and the `query *` ops ask this
    /// provider for geometry / scope chain / focus. Defaults to
    /// [`NoopProvider`] so callers that only use the inline-snapshot path
    /// stay unaffected.
    provider: Arc<dyn UiGeometryProvider>,
}

impl std::fmt::Debug for FocusServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FocusServer").finish()
    }
}

impl Default for FocusServer {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusServer {
    /// Construct a fresh server over an empty registry and state.
    pub fn new() -> Self {
        Self {
            spatial_registry: Arc::new(Mutex::new(SpatialRegistry::new())),
            spatial_state: Arc::new(Mutex::new(SpatialState::new())),
            sink: Arc::new(NoopSink),
            provider: Arc::new(NoopProvider),
        }
    }

    /// Construct a server over caller-supplied shared state.
    ///
    /// Production bootstrap clones the arcs the `AppState` already holds so
    /// the MCP face and the rest of the app share one source of truth.
    pub fn with_state(
        spatial_registry: Arc<Mutex<SpatialRegistry>>,
        spatial_state: Arc<Mutex<SpatialState>>,
    ) -> Self {
        Self {
            spatial_registry,
            spatial_state,
            sink: Arc::new(NoopSink),
            provider: Arc::new(NoopProvider),
        }
    }

    /// Attach a [`FocusEventSink`] so every produced [`FocusChangedEvent`]
    /// is mirrored onto an external transport.
    ///
    /// Production wiring (the kanban app) passes a sink that calls
    /// `app.emit_to(event.window_label, "focus-changed", event)` so the
    /// React `SpatialFocusProvider` keeps receiving the same Tauri event
    /// it did when the legacy `spatial_*` Tauri commands emitted on the
    /// host side. Tests and unwired callers default to [`NoopSink`].
    ///
    /// Returns `self` so this fits in the builder-style construction the
    /// bootstrap uses.
    pub fn with_sink(mut self, sink: Arc<dyn FocusEventSink>) -> Self {
        self.sink = sink;
        self
    }

    /// Attach a [`UiGeometryProvider`] so the host-driven nav ops and the
    /// `query *` ops can PULL live geometry / scope chain / focus from the
    /// webview on demand.
    ///
    /// Production wiring (the kanban app) passes a provider that answers each
    /// query by issuing a `request_from_ui` host→UI request and awaiting the
    /// webview's reply. Tests and unwired callers default to [`NoopProvider`]
    /// (every pull yields "nothing"), so the inline-snapshot ops are
    /// unaffected. Returns `self` for builder-style construction, mirroring
    /// [`with_sink`](Self::with_sink).
    pub fn with_provider(mut self, provider: Arc<dyn UiGeometryProvider>) -> Self {
        self.provider = provider;
        self
    }

    /// The shared registry arc, exposed for tests / bootstrap.
    pub fn registry(&self) -> Arc<Mutex<SpatialRegistry>> {
        Arc::clone(&self.spatial_registry)
    }

    /// The shared state arc, exposed for tests / bootstrap.
    pub fn state(&self) -> Arc<Mutex<SpatialState>> {
        Arc::clone(&self.spatial_state)
    }

    /// Acquire both spatial locks in canonical order and run `f`.
    ///
    /// Order is `spatial_registry` then `spatial_state` — identical to the
    /// `with_spatial` helper the Tauri adapter used, so the MCP face cannot
    /// lock-invert against the GUI path when both share the same arcs.
    async fn with_spatial<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&mut SpatialRegistry, &mut SpatialState) -> R,
    {
        let mut registry = self.spatial_registry.lock().await;
        let mut state = self.spatial_state.lock().await;
        f(&mut registry, &mut state)
    }

    /// Build the platform-facing `focus` tool definition.
    fn build_tool_definition() -> Tool {
        operation_tool! {
            name: "focus",
            description: "Spatial focus and keyboard-navigation actions over the per-window focus kernel.",
            operations: operations(),
        }
    }

    /// Forward an optional focus-changed event onto the attached sink.
    ///
    /// Centralized so every focus-mutating handler emits via the same code
    /// path — a no-op when the kernel returned `None` (idempotent commit,
    /// already-focused, unknown FQM) or when the sink is the default
    /// [`NoopSink`].
    fn forward_event(&self, event: &Option<FocusChangedEvent>) {
        if let Some(ev) = event.as_ref() {
            self.sink.emit(ev);
        }
    }

    /// Handle a `set focus` call (`ui.setFocus` routing target).
    ///
    /// Ports `spatial_focus`: a `None` snapshot drops the commit silently.
    async fn handle_focus(&self, req: Focus) -> Result<Value, McpError> {
        let Some(snapshot) = req.snapshot else {
            tracing::debug!(
                op = "set focus",
                focused_fq = %req.fq,
                "snapshot=None — dropping focus commit (transient unmount race)"
            );
            return Ok(serde_json::json!({ "ok": true, "event": Value::Null }));
        };
        let event = self
            .with_spatial(|registry, state| state.focus(registry, &snapshot, req.fq.clone(), None))
            .await;
        self.forward_event(&event);
        Ok(serde_json::json!({ "ok": true, "event": event }))
    }

    /// Handle a `clear focus` call. Ports `spatial_clear_focus`.
    async fn handle_clear_focus(&self, req: ClearFocus) -> Result<Value, McpError> {
        let event = self
            .with_spatial(|_registry, state| state.clear_focus(&req.window))
            .await;
        self.forward_event(&event);
        Ok(serde_json::json!({ "ok": true, "event": event }))
    }

    /// Read the focused FQM for `window` from the kernel, dropping the lock
    /// before returning so no spatial lock is held across the geometry pull
    /// that follows.
    ///
    /// The lock is acquired and released entirely within this call — the
    /// returned owned FQM carries no borrow, satisfying the F1 deadlock
    /// discipline (no lock held across the subsequent provider `.await`).
    async fn focused_in_window(&self, window: &WindowLabel) -> Option<FullyQualifiedMoniker> {
        let state = self.spatial_state.lock().await;
        state.focused_in(window).cloned()
    }

    /// Resolve the geometry source for a host-driven nav op without holding
    /// any spatial lock across the pull.
    ///
    /// Returns the `(focused_fq, snapshot)` pair to run the kernel logic
    /// against, or `None` when the op should drop silently:
    ///
    /// - **Inline** (`snapshot` present): pairs the wire `snapshot` with the
    ///   wire `focused_fq`; `None`/missing `focused_fq` drops.
    /// - **Host-driven pull** (`window` present, no inline `snapshot`):
    ///   resolves focus as wire `focused_fq` → PULLED UI focus
    ///   (`provider.focus`, authoritative) → kernel `focus_by_window` slot
    ///   (fallback), then PULLS the live snapshot from the provider. The kernel
    ///   read and the provider awaits never overlap a held lock — each acquires
    ///   and releases internally before the next.
    async fn resolve_nav_source(
        &self,
        focused_fq: Option<FullyQualifiedMoniker>,
        snapshot: Option<NavSnapshot>,
        window: Option<WindowLabel>,
        op: &str,
        direction: Option<Direction>,
    ) -> Option<(FullyQualifiedMoniker, NavSnapshot)> {
        if let Some(snapshot) = snapshot {
            // Inline path: the wire carries both geometry and the source FQM.
            let Some(from) = focused_fq else {
                tracing::debug!(op, "inline snapshot but no focused_fq — dropping");
                return None;
            };
            return Some((from, snapshot));
        }

        let Some(window) = window else {
            tracing::debug!(op, ?direction, "no snapshot and no window — dropping");
            return None;
        };

        // Host-driven pull. The webview owns the AUTHORITATIVE current focus;
        // the kernel's per-window slot is routinely empty in the running app
        // (React drives focus, and the kernel `focus_by_window` commit drops
        // when no snapshot accompanies the set-focus). So resolve focus as:
        // explicit wire `focused_fq` → PULLED UI focus (provider, authoritative)
        // → kernel slot (last-resort fallback). Every await runs with NO spatial
        // lock held (`provider.focus`/`focused_in_window` each acquire+release
        // internally).
        let from = match focused_fq {
            Some(from) => from,
            None => match self
                .provider
                .focus(&window)
                .await
                .or(self.focused_in_window(&window).await)
            {
                Some(from) => from,
                None => {
                    tracing::debug!(op, window = %window, "no UI focus and no kernel slot — dropping");
                    return None;
                }
            },
        };
        let Some(snapshot) = self.provider.snapshot(&window).await else {
            tracing::debug!(op, window = %window, "provider yielded no snapshot — dropping");
            return None;
        };
        Some((from, snapshot))
    }

    /// Handle a `navigate focus` call.
    ///
    /// Inline path ports `spatial_navigate` verbatim; the host-driven pull
    /// path resolves focus from the kernel and pulls geometry from the
    /// provider (Card F2). Either way the kernel mutation runs under the
    /// spatial locks AFTER any provider await — never across it.
    async fn handle_navigate(&self, req: Navigate) -> Result<Value, McpError> {
        let Some((from, snapshot)) = self
            .resolve_nav_source(
                req.focused_fq,
                req.snapshot,
                req.window.clone(),
                "navigate focus",
                Some(req.direction),
            )
            .await
        else {
            return Ok(serde_json::json!({ "ok": true, "event": Value::Null }));
        };
        let event = self
            .with_spatial(|registry, state| {
                state.navigate(registry, &snapshot, from, req.direction, req.window)
            })
            .await;
        self.forward_event(&event);
        Ok(serde_json::json!({ "ok": true, "event": event }))
    }

    /// Handle a `query geometry` call — pull the live snapshot for `window`.
    async fn handle_query_geometry(&self, req: QueryGeometry) -> Result<Value, McpError> {
        let snapshot = self.provider.snapshot(&req.window).await;
        Ok(serde_json::json!({ "ok": true, "snapshot": snapshot }))
    }

    /// Handle a `query scope_chain` call — pull the scope chain for `window`.
    async fn handle_query_scope_chain(&self, req: QueryScopeChain) -> Result<Value, McpError> {
        let scope_chain = self.provider.scope_chain(&req.window).await;
        Ok(serde_json::json!({ "ok": true, "scope_chain": scope_chain }))
    }

    /// Handle a `query focus` call — pull the focused FQM for `window`.
    async fn handle_query_focus(&self, req: QueryFocus) -> Result<Value, McpError> {
        let focus = self.provider.focus(&req.window).await;
        Ok(serde_json::json!({ "ok": true, "focus": focus }))
    }

    /// Handle a `lose focus` call. Ports `spatial_focus_lost`.
    async fn handle_focus_lost(&self, req: FocusLost) -> Result<Value, McpError> {
        let event = self
            .with_spatial(|registry, state| {
                state.focus_lost(
                    registry,
                    &req.snapshot,
                    &req.focused_fq,
                    req.lost_parent_zone.as_ref(),
                    &req.lost_layer_fq,
                    req.lost_rect,
                    None,
                )
            })
            .await;
        self.forward_event(&event);
        Ok(serde_json::json!({ "ok": true, "event": event }))
    }

    /// Handle a `push layer` call. Ports `spatial_push_layer`.
    async fn handle_push_layer(&self, req: PushLayer) -> Result<Value, McpError> {
        self.with_spatial(|registry, _state| {
            registry.push_layer(crate::layer::FocusLayer {
                fq: req.fq,
                segment: req.segment,
                name: req.name,
                parent: req.parent,
                window_label: req.window,
                last_focused: None,
            });
        })
        .await;
        Ok(serde_json::json!({ "ok": true }))
    }

    /// Handle a `pop layer` call. Ports `spatial_pop_layer`.
    async fn handle_pop_layer(&self, req: PopLayer) -> Result<Value, McpError> {
        let next_fq = self
            .with_spatial(|registry, _state| {
                let next_fq = registry.layer(&req.fq).and_then(|l| l.last_focused.clone());
                registry.remove_layer(&req.fq);
                next_fq
            })
            .await;
        Ok(serde_json::json!({ "ok": true, "next_fq": next_fq }))
    }

    /// Resolve the `(focused_fq, snapshot)` source for a drill op, mirroring
    /// [`resolve_nav_source`](Self::resolve_nav_source) but surfacing the
    /// resolved `focused_fq` even when no snapshot is available — drill's
    /// no-op contract echoes the focused FQM rather than dropping silently.
    ///
    /// Returns `(focused_fq, Some(snapshot))` when geometry is available, or
    /// `(focused_fq, None)` when the snapshot could not be obtained but a
    /// source FQM was resolved (the drill echoes it). Returns
    /// `(None, None)` when no focus could be resolved at all.
    async fn resolve_drill_source(
        &self,
        focused_fq: Option<FullyQualifiedMoniker>,
        snapshot: Option<NavSnapshot>,
        window: Option<WindowLabel>,
    ) -> (Option<FullyQualifiedMoniker>, Option<NavSnapshot>) {
        if let Some(snapshot) = snapshot {
            return (focused_fq, Some(snapshot));
        }
        let Some(window) = window else {
            // Inline path with no snapshot: echo the wire focused_fq.
            return (focused_fq, None);
        };
        let resolved = match focused_fq {
            Some(f) => f,
            None => match self
                .provider
                .focus(&window)
                .await
                .or(self.focused_in_window(&window).await)
            {
                Some(f) => f,
                None => return (None, None),
            },
        };
        let snapshot = self.provider.snapshot(&window).await;
        (Some(resolved), snapshot)
    }

    /// Handle a `drill_in layer` call. Inline path ports `spatial_drill_in`;
    /// the host-driven pull path resolves focus from the kernel and pulls
    /// geometry from the provider (Card F2). With no snapshot the drill echoes
    /// the resolved focused FQM (the no-op contract).
    async fn handle_drill_in(&self, req: DrillIn) -> Result<Value, McpError> {
        let window = req.window.clone();
        let (focused_fq, snapshot) = self
            .resolve_drill_source(req.focused_fq, req.snapshot, window.clone())
            .await;
        let Some(focused_fq) = focused_fq else {
            return Ok(serde_json::json!({ "ok": true, "next_fq": Value::Null }));
        };
        let Some(snapshot) = snapshot else {
            return Ok(serde_json::json!({ "ok": true, "next_fq": focused_fq }));
        };
        // Compute the drill-in target (pure snapshot query), then COMMIT focus
        // to it and emit `focus-changed` — exactly as `handle_navigate` does.
        // Previously this only returned `next_fq` without committing or
        // forwarding the event, so the UI never moved focus on drill.
        let echo = focused_fq.clone();
        let event = self
            .with_spatial(|registry, state| {
                let view = IndexedSnapshot::new(&snapshot);
                let target =
                    crate::navigate::drill_in(&view, registry, req.fq.clone(), &focused_fq);
                state.focus_from(registry, &snapshot, focused_fq, target, window)
            })
            .await;
        self.forward_event(&event);
        let next_fq = event.and_then(|e| e.next_fq).unwrap_or(echo);
        Ok(serde_json::json!({ "ok": true, "next_fq": next_fq }))
    }

    /// Handle a `generate sneak_codes` call. Ports `generate_jump_codes`.
    ///
    /// Pure compute over [`crate::generate_sneak_codes`]; no spatial state
    /// is consulted. Surfaces the kernel's [`crate::SneakError`] as an
    /// `invalid_params` error so an over-capacity request from the React
    /// side fails with a structured, recoverable shape.
    async fn handle_generate_sneak_codes(
        &self,
        req: GenerateSneakCodes,
    ) -> Result<Value, McpError> {
        let codes = generate_sneak_codes(req.count)
            .map_err(|err| McpError::invalid_params(err.to_string(), None))?;
        Ok(serde_json::json!({ "ok": true, "codes": codes }))
    }

    /// Handle a `drill_out layer` call. Inline path ports `spatial_drill_out`;
    /// the host-driven pull path resolves focus from the kernel and pulls
    /// geometry from the provider (Card F2). With no snapshot the drill echoes
    /// the resolved focused FQM (the no-op contract).
    async fn handle_drill_out(&self, req: DrillOut) -> Result<Value, McpError> {
        let window = req.window.clone();
        let (focused_fq, snapshot) = self
            .resolve_drill_source(req.focused_fq, req.snapshot, window.clone())
            .await;
        let Some(focused_fq) = focused_fq else {
            return Ok(serde_json::json!({ "ok": true, "next_fq": Value::Null }));
        };
        let Some(snapshot) = snapshot else {
            return Ok(serde_json::json!({ "ok": true, "next_fq": focused_fq }));
        };
        // Compute the drill-out target (pure snapshot query), then COMMIT focus
        // to it and emit `focus-changed` — exactly as `handle_navigate` does.
        // Previously this only returned `next_fq` without committing or
        // forwarding the event, so the UI never moved focus on drill.
        let echo = focused_fq.clone();
        let event = self
            .with_spatial(|registry, state| {
                let view = IndexedSnapshot::new(&snapshot);
                let target = crate::navigate::drill_out(&view, req.fq.clone(), &focused_fq);
                state.focus_from(registry, &snapshot, focused_fq, target, window)
            })
            .await;
        self.forward_event(&event);
        let next_fq = event.and_then(|e| e.next_fq).unwrap_or(echo);
        Ok(serde_json::json!({ "ok": true, "next_fq": next_fq }))
    }
}

/// Map a JSON value into one of the operation structs, returning a readable
/// rmcp error when the shape is wrong.
fn deserialize_op<T: DeserializeOwned>(arguments: Value, op: &str) -> Result<T, McpError> {
    serde_json::from_value(arguments).map_err(|err| {
        McpError::invalid_params(format!("invalid arguments for op {op:?}: {err}"), None)
    })
}

impl ServerHandler for FocusServer {
    /// Advertise the single `focus` operation tool.
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

    /// Route a `tools/call` for the `focus` tool to the matching verb handler.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        if request.name.as_ref() != "focus" {
            return Err(McpError::invalid_request(
                format!("unknown tool {:?}; expected \"focus\"", request.name),
                None,
            ));
        }

        let arguments = Value::Object(request.arguments.unwrap_or_default());
        let op = arguments
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                McpError::invalid_params(
                    "missing required field `op` for focus tool".to_string(),
                    None,
                )
            })?
            .to_string();

        let response = match op.as_str() {
            "set focus" => {
                let req: Focus = deserialize_op(arguments, &op)?;
                self.handle_focus(req).await?
            }
            "clear focus" => {
                let req: ClearFocus = deserialize_op(arguments, &op)?;
                self.handle_clear_focus(req).await?
            }
            "navigate focus" => {
                let req: Navigate = deserialize_op(arguments, &op)?;
                self.handle_navigate(req).await?
            }
            "lose focus" => {
                let req: FocusLost = deserialize_op(arguments, &op)?;
                self.handle_focus_lost(req).await?
            }
            "push layer" => {
                let req: PushLayer = deserialize_op(arguments, &op)?;
                self.handle_push_layer(req).await?
            }
            "pop layer" => {
                let req: PopLayer = deserialize_op(arguments, &op)?;
                self.handle_pop_layer(req).await?
            }
            "drill_in layer" => {
                let req: DrillIn = deserialize_op(arguments, &op)?;
                self.handle_drill_in(req).await?
            }
            "drill_out layer" => {
                let req: DrillOut = deserialize_op(arguments, &op)?;
                self.handle_drill_out(req).await?
            }
            "generate sneak_codes" => {
                let req: GenerateSneakCodes = deserialize_op(arguments, &op)?;
                self.handle_generate_sneak_codes(req).await?
            }
            "query geometry" => {
                let req: QueryGeometry = deserialize_op(arguments, &op)?;
                self.handle_query_geometry(req).await?
            }
            "query scope_chain" => {
                let req: QueryScopeChain = deserialize_op(arguments, &op)?;
                self.handle_query_scope_chain(req).await?
            }
            "query focus" => {
                let req: QueryFocus = deserialize_op(arguments, &op)?;
                self.handle_query_focus(req).await?
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown op {other:?} for focus tool"),
                    None,
                ))
            }
        };

        Ok(CallToolResult::structured(response))
    }
}
