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

use swissarmyhammer_commands::{UIState, WindowInfo};

use crate::commands::perspective_commands::perspective_belongs_to_active_view;
use crate::context::KanbanContext;
use crate::scope_commands::{
    BoardInfo, DynamicSources, PerspectiveFieldInfo, PerspectiveInfo, ViewInfo,
};

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
    /// focused" — [`resolve_active_view`] short-circuits to
    /// `(None, None)`, which matches the splash/welcome path. Different
    /// consumers (multi-window CLIs, tests) can address a different
    /// active window.
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
///    filtered to the active view when one can be resolved. Id-scoped
///    perspectives match strictly by id; legacy `view_id`-less
///    perspectives fall back to kind-equality.
///
/// Every "cannot lock / read" branch in the old helper is preserved so
/// any downstream test comparing the live path to the headless path
/// sees identical output.
pub async fn build_dynamic_sources(inputs: DynamicSourcesInputs<'_>) -> DynamicSources {
    let views = gather_views(inputs.active_ctx);
    let boards = gather_boards(inputs.ui_state, inputs.open_board_ctxs).await;
    let (view_kind, view_id) = resolve_active_view(
        inputs.active_ctx,
        inputs.ui_state,
        inputs.active_window_label,
    );
    let perspectives =
        gather_perspectives(inputs.active_ctx, view_id.as_deref(), view_kind.as_deref()).await;
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
            kind: v.kind.as_kebab_str().to_string(),
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

/// Resolve the active view id + kind from UIState + the active board's
/// views registry.
///
/// Returns `(None, None)` when there is no active context, no active window
/// focused, no active view id for that window, the active view id does not
/// match any registered view, or the views lock is contended.
///
/// Both fields come from the same lookup pass: the id is read straight from
/// `ui_state.active_view_id(label)`, and the kind is the matching
/// `ViewDef.kind` serialized as a lower-case string. The pair drives
/// per-perspective scoping in [`gather_perspectives`] — perspectives with
/// `view_id == Some(active_id)` match strictly; legacy perspectives with
/// `view_id == None` fall back to kind-equality.
fn resolve_active_view(
    ctx: Option<&KanbanContext>,
    ui_state: &UIState,
    active_window_label: Option<&str>,
) -> (Option<String>, Option<String>) {
    let Some(ctx) = ctx else {
        return (None, None);
    };
    let Some(label) = active_window_label else {
        return (None, None);
    };
    let active_id = ui_state.active_view_id(label);
    if active_id.is_empty() {
        return (None, None);
    }
    let Some(views_lock) = ctx.views() else {
        return (None, None);
    };
    let Ok(vc) = views_lock.try_read() else {
        return (None, None);
    };
    let Some(view) = vc.all_views().iter().find(|v| v.id == active_id) else {
        return (None, None);
    };
    let kind = Some(view.kind.as_kebab_str().to_string());
    (kind, Some(active_id))
}

/// Gather perspective info from the active board's perspective registry.
///
/// Filtering rules (delegated to [`perspective_belongs_to_active_view`]):
///
/// - When `active_view_kind` is `None` the active view could not be
///   resolved at all — return every perspective (the splash/welcome path).
/// - When `active_view_kind` is `Some`, each perspective is kept only if it
///   "belongs" to the active view: id-scoped perspectives match strictly
///   against `active_view_id`; legacy `view_id == None` perspectives fall
///   back to kind-equality.
///
/// This prevents the same "Default" perspective from emitting once per
/// view kind, and prevents id-scoped perspectives from leaking across
/// sibling views that share a kind.
async fn gather_perspectives(
    ctx: Option<&KanbanContext>,
    active_view_id: Option<&str>,
    active_view_kind: Option<&str>,
) -> Vec<PerspectiveInfo> {
    let Some(ctx) = ctx else { return Vec::new() };
    let Ok(pctx) = ctx.perspective_context().await else {
        return Vec::new();
    };
    let Ok(pc) = pctx.try_read() else {
        return Vec::new();
    };

    // One-time discovery log for legacy view-id-less perspectives. The
    // helper guards against repeated emissions, so calling it on every
    // `list_commands_for_scope` invocation is cheap. See
    // `perspective::migrate` for the placement rationale.
    if let Some(views_lock) = ctx.views() {
        if let Ok(views) = views_lock.try_read() {
            crate::perspective::migrate::log_legacy_perspectives_once(pc.all(), &views);
        }
    }

    let fields_ctx = ctx.fields();
    pc.all()
        .iter()
        .filter(|p| match active_view_kind {
            None => true,
            Some(kind) => perspective_belongs_to_active_view(p, active_view_id, kind),
        })
        .map(|p| PerspectiveInfo {
            id: p.id.clone(),
            name: p.name.clone(),
            view: p.view.clone(),
            fields: denormalize_perspective_fields(p, fields_ctx),
        })
        .collect()
}

/// Denormalise a perspective's field-id list into [`PerspectiveFieldInfo`]
/// records carrying the resolved display name.
///
/// For each [`swissarmyhammer_perspectives::PerspectiveFieldEntry`] we
/// prefer the per-entry `caption` override (matches the column header the
/// user sees in the grid). When no caption is set, we fall back to the
/// field definition's `name` from [`FieldsContext`]. Entries whose field
/// id is not present in the registry are dropped (the picker should never
/// surface a ghost field).
fn denormalize_perspective_fields(
    perspective: &swissarmyhammer_perspectives::Perspective,
    fields_ctx: Option<&swissarmyhammer_fields::FieldsContext>,
) -> Vec<PerspectiveFieldInfo> {
    perspective
        .fields
        .iter()
        .filter_map(|entry| {
            let display_name = match &entry.caption {
                Some(caption) => caption.clone(),
                None => fields_ctx
                    .and_then(|fc| fc.get_field_by_id(&entry.field))
                    .map(|fd| fd.name.as_str().to_string())?,
            };
            Some(PerspectiveFieldInfo {
                id: entry.field.clone(),
                display_name,
            })
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
