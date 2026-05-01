//! Headless assembly of [`DynamicSources`] for command dispatch.
//!
//! [`build_dynamic_sources`] is the tier-0 replacement for the former
//! `build_dynamic_sources` helper in `kanban-app/src/commands.rs`. The
//! inputs are plain references — [`UIState`], one or more
//! [`KanbanContext`]s, an active window label, and a caller-supplied list
//! of live [`WindowInfo`] — so every piece except the live window data
//! can be constructed in a Rust integration test without standing up any
//! GUI scaffolding.
//!
//! # Why windows are caller-supplied
//!
//! Live window state (title, visibility, focus) is owned by the GUI
//! runtime (Tauri's `AppHandle`) and cannot be derived from [`UIState`]
//! alone: [`UIState`] persists per-window geometry and board assignment,
//! but not the currently-displayed title or focus flag. The GUI crate
//! snapshots those via `app.webview_windows()` and passes them in.
//! Headless tests fabricate the list (often empty) and exercise every
//! other code path verbatim.
//!
//! # Relationship to `scope_commands`
//!
//! `scope_commands::DynamicSources` is the consumer shape — this module
//! is the producer. The two are intentionally separate so a future
//! command-emission change can evolve the consumer without perturbing
//! the headless assembly, and vice versa.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use swissarmyhammer_commands::UIState;

use crate::context::KanbanContext;
use crate::scope_commands::{BoardInfo, DynamicSources, PerspectiveInfo, ViewInfo, WindowInfo};

/// Raw inputs [`build_dynamic_sources`] consumes. See the module docs for
/// how each field is produced in the live app vs. a headless test.
///
/// Borrow semantics: everything is borrowed for the duration of the call,
/// so the caller retains ownership. `windows` is owned because the GUI
/// path constructs it fresh from `AppHandle::webview_windows()` on every
/// `list_commands_for_scope` invocation; there is no cheaper shape.
pub struct DynamicSourcesInputs<'a> {
    /// UI state — provides `open_boards`, `active_view_id`, etc.
    pub ui_state: &'a UIState,
    /// Context for the currently active board, if any.
    ///
    /// `None` when no board is focused — the live app passes `None` while
    /// the splash/welcome screen is showing. When `None`, `views` and
    /// `perspectives` come back empty because both are read from the
    /// active board's registries.
    pub active_ctx: Option<&'a KanbanContext>,
    /// Map of canonical board path → open board context. Supplies the
    /// entity display name and context-name fields on each
    /// [`BoardInfo`]. Paths in `ui_state.open_boards()` with no matching
    /// entry fall back to the parent directory basename (matching the
    /// pre-refactor GUI behavior exactly).
    pub open_board_ctxs: &'a HashMap<PathBuf, Arc<KanbanContext>>,
    /// Window label to query `ui_state.active_view_id` against (e.g.
    /// `Some("main")` in the kanban-app). `None` means "no window is
    /// focused" — [`resolve_active_view_kind`] short-circuits to `None`,
    /// which matches the splash/welcome path. Different consumers
    /// (multi-window CLIs, tests) can address a different active window.
    pub active_window_label: Option<&'a str>,
    /// Pre-gathered live windows — caller-supplied because live window
    /// state only exists in the GUI runtime (see module docs).
    pub windows: Vec<WindowInfo>,
}

/// Hand-rolled `Debug` impl because [`UIState`] is not `Debug` (it owns
/// an `RwLock` with interior mutable state). The impl deliberately elides
/// anything that would require locking — it prints only counts and
/// trivially-copyable flags so tracing can log an `inputs=…` line without
/// risking a deadlock on a contended lock.
impl fmt::Debug for DynamicSourcesInputs<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynamicSourcesInputs")
            .field("active_window_label", &self.active_window_label)
            .field("windows.len", &self.windows.len())
            .field("open_board_ctxs.len", &self.open_board_ctxs.len())
            .field("active_ctx.is_some", &self.active_ctx.is_some())
            .finish()
    }
}

/// Assemble a [`DynamicSources`] from the given inputs.
///
/// Mirrors the semantics of the former
/// `kanban-app::commands::build_dynamic_sources` one-for-one:
///
/// 1. **Views** come from `active_ctx.views()`; empty when no active
///    context.
/// 2. **Boards** come from `ui_state.open_boards()`, with display names
///    read from the matching context's entity `board/board` (or the
///    parent directory basename on miss).
/// 3. **Windows** pass through from the caller.
/// 4. **Perspectives** come from `active_ctx.perspective_context()`,
///    filtered to the active view kind when one can be resolved.
///
/// Every "cannot lock / read" branch in the old helper is preserved so
/// any downstream test comparing the live path to the headless path
/// sees identical output.
pub async fn build_dynamic_sources(inputs: DynamicSourcesInputs<'_>) -> DynamicSources {
    let views = gather_views(inputs.active_ctx);
    let boards = gather_boards(inputs.ui_state, inputs.open_board_ctxs).await;
    let view_kind = resolve_active_view_kind(
        inputs.active_ctx,
        inputs.ui_state,
        inputs.active_window_label,
    );
    let perspectives = gather_perspectives(inputs.active_ctx, view_kind.as_deref()).await;
    DynamicSources {
        views,
        boards,
        windows: inputs.windows,
        perspectives,
    }
}

/// Gather view info from an optional kanban context.
///
/// Returns an empty vec when `ctx` is `None`, when the context has no
/// views sub-context attached, or when the views lock cannot be acquired
/// synchronously. The sync `try_read` preserves the pre-refactor
/// semantics — production never awaits here because the lock is only
/// contended during view editing, which is rare relative to
/// `list_commands_for_scope` invocations.
fn gather_views(ctx: Option<&KanbanContext>) -> Vec<ViewInfo> {
    let Some(ctx) = ctx else { return Vec::new() };
    let Some(views_lock) = ctx.views() else {
        return Vec::new();
    };
    let Ok(vc) = views_lock.try_read() else {
        return Vec::new();
    };
    vc.all_views()
        .iter()
        .map(|v| ViewInfo {
            id: v.id.clone(),
            name: v.name.clone(),
            entity_type: v.entity_type.clone(),
        })
        .collect()
}

/// Gather open-board info from UIState + the caller-supplied context map.
///
/// The path list is authoritative: every entry in `ui_state.open_boards()`
/// produces exactly one [`BoardInfo`]. `open_board_ctxs` is a best-effort
/// accompaniment — paths without a matching context fall back to the
/// parent directory basename for both `entity_name` and `context_name`,
/// matching the pre-refactor GUI behavior.
async fn gather_boards(
    ui_state: &UIState,
    open_board_ctxs: &HashMap<PathBuf, Arc<KanbanContext>>,
) -> Vec<BoardInfo> {
    let open_paths = ui_state.open_boards();
    let mut result = Vec::with_capacity(open_paths.len());
    for path in &open_paths {
        let p = Path::new(path);
        let dir_name = p
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Board")
            .to_string();
        let ctx_opt = open_board_ctxs.get(p);
        let entity_name = match ctx_opt {
            Some(ctx) => board_display_name(ctx)
                .await
                .unwrap_or_else(|| dir_name.clone()),
            None => dir_name.clone(),
        };
        let context_name = ctx_opt
            .map(|c| c.name().to_string())
            .unwrap_or_else(|| dir_name.clone());
        result.push(BoardInfo {
            path: path.clone(),
            name: dir_name,
            entity_name,
            context_name,
        });
    }
    result
}

/// Resolve the active view kind (e.g. `"board"`, `"grid"`) from UIState
/// + the active board's views registry.
///
/// Returns `None` when there is no active context, no active window
/// focused, no active view id for that window, or the views lock is
/// contended. This is consumed by [`gather_perspectives`] to filter
/// perspectives down to the kind currently rendered.
fn resolve_active_view_kind(
    ctx: Option<&KanbanContext>,
    ui_state: &UIState,
    active_window_label: Option<&str>,
) -> Option<String> {
    let ctx = ctx?;
    let label = active_window_label?;
    let active_id = ui_state.active_view_id(label);
    if active_id.is_empty() {
        return None;
    }
    let views_lock = ctx.views()?;
    let vc = views_lock.try_read().ok()?;
    let view = vc.all_views().iter().find(|v| v.id == active_id)?;
    Some(serde_json::to_value(&view.kind).ok()?.as_str()?.to_string())
}

/// Gather perspective info from the active board's perspective registry.
///
/// When `view_kind` is `Some`, only perspectives whose `view` field
/// matches are returned. This prevents the same "Default" perspective
/// from emitting once per view kind.
async fn gather_perspectives(
    ctx: Option<&KanbanContext>,
    view_kind: Option<&str>,
) -> Vec<PerspectiveInfo> {
    let Some(ctx) = ctx else { return Vec::new() };
    let Ok(pctx) = ctx.perspective_context().await else {
        return Vec::new();
    };
    let Ok(pc) = pctx.try_read() else {
        return Vec::new();
    };
    pc.all()
        .iter()
        .filter(|p| view_kind.is_none_or(|vk| p.view == vk))
        .map(|p| PerspectiveInfo {
            id: p.id.clone(),
            name: p.name.clone(),
            view: p.view.clone(),
        })
        .collect()
}

/// Read the board entity's `name` field from the entity store.
///
/// Mirrors the pre-refactor `board_display_name` in `kanban-app`: returns
/// `None` if the entity store isn't reachable, the `board/board` entity
/// doesn't exist yet, or the entity has no non-empty `name` field.
async fn board_display_name(ctx: &KanbanContext) -> Option<String> {
    let ectx = ctx.entity_context().await.ok()?;
    let entity = ectx.read("board", "board").await.ok()?;
    entity
        .fields
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}
