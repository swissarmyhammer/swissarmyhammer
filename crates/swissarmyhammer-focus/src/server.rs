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

use crate::operations::{
    operations, ClearFocus, DrillIn, DrillOut, Focus, FocusLost, Navigate, PopLayer, PushLayer,
};
use crate::registry::SpatialRegistry;
use crate::snapshot::IndexedSnapshot;
use crate::state::SpatialState;

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
        }
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
            .with_spatial(|registry, state| state.focus(registry, &snapshot, req.fq.clone()))
            .await;
        Ok(serde_json::json!({ "ok": true, "event": event }))
    }

    /// Handle a `clear focus` call. Ports `spatial_clear_focus`.
    async fn handle_clear_focus(&self, req: ClearFocus) -> Result<Value, McpError> {
        let event = self
            .with_spatial(|_registry, state| state.clear_focus(&req.window))
            .await;
        Ok(serde_json::json!({ "ok": true, "event": event }))
    }

    /// Handle a `navigate focus` call. Ports `spatial_navigate`.
    async fn handle_navigate(&self, req: Navigate) -> Result<Value, McpError> {
        let Some(snapshot) = req.snapshot else {
            tracing::debug!(
                op = "navigate focus",
                focused_fq = %req.focused_fq,
                direction = ?req.direction,
                "snapshot=None — dropping navigation (transient unmount race)"
            );
            return Ok(serde_json::json!({ "ok": true, "event": Value::Null }));
        };
        let event = self
            .with_spatial(|registry, state| {
                state.navigate(registry, &snapshot, req.focused_fq.clone(), req.direction)
            })
            .await;
        Ok(serde_json::json!({ "ok": true, "event": event }))
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
                )
            })
            .await;
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

    /// Handle a `drill_in layer` call. Ports `spatial_drill_in`.
    async fn handle_drill_in(&self, req: DrillIn) -> Result<Value, McpError> {
        let Some(snapshot) = req.snapshot else {
            return Ok(serde_json::json!({ "ok": true, "next_fq": req.focused_fq }));
        };
        let next_fq = self
            .with_spatial(|registry, _state| {
                let view = IndexedSnapshot::new(&snapshot);
                crate::navigate::drill_in(&view, registry, req.fq.clone(), &req.focused_fq)
            })
            .await;
        Ok(serde_json::json!({ "ok": true, "next_fq": next_fq }))
    }

    /// Handle a `drill_out layer` call. Ports `spatial_drill_out`.
    async fn handle_drill_out(&self, req: DrillOut) -> Result<Value, McpError> {
        let Some(snapshot) = req.snapshot else {
            return Ok(serde_json::json!({ "ok": true, "next_fq": req.focused_fq }));
        };
        // drill_out is a pure snapshot query; it does not touch the registry,
        // but we still take the locks for parity with the Tauri adapter so the
        // canonical lock order is honored uniformly.
        let next_fq = self
            .with_spatial(|_registry, _state| {
                let view = IndexedSnapshot::new(&snapshot);
                crate::navigate::drill_out(&view, req.fq.clone(), &req.focused_fq)
            })
            .await;
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
