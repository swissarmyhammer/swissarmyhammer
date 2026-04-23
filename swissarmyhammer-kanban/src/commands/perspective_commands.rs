//! Perspective-related command implementations.
//!
//! Commands for loading, saving, deleting perspectives and for updating
//! filter/group settings on an active perspective.

use super::run_op;
use crate::context::KanbanContext;
use crate::perspective::{
    AddPerspective, DeletePerspective, GetPerspective, ListPerspectives, RenamePerspective,
    SortDirection, SortEntry, UpdatePerspective,
};
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{Command, CommandContext, CommandError};
use swissarmyhammer_filter_expr;

/// Validate a filter expression string, returning a `CommandError` if invalid.
///
/// Empty strings are allowed (treated as "no filter"). Non-empty strings must
/// parse as a valid filter DSL expression.
fn validate_filter(filter: &str) -> Result<(), CommandError> {
    if filter.trim().is_empty() {
        return Ok(());
    }
    swissarmyhammer_filter_expr::parse(filter).map_err(|errors| {
        let messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        CommandError::ExecutionFailed(format!(
            "invalid filter expression: {}",
            messages.join("; ")
        ))
    })?;
    Ok(())
}

/// Where a resolved `perspective_id` came from.
///
/// Distinguishes caller-supplied ids (explicit arg or scope-chain moniker)
/// from resolver-chosen ids (UIState active, or first-perspective-for-view
/// fallback). When the resolver picks the id itself, the caller should
/// persist the choice by writing it back to [`UIState::set_active_perspective`]
/// so subsequent palette invocations find a non-empty active id — making the
/// fallback self-healing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedFrom {
    /// The id came from `args.perspective_id`.
    Arg,
    /// The id came from a `perspective:{id}` moniker in the scope chain.
    Scope,
    /// The id came from `UIState.active_perspective_id(window_label)`.
    UiState,
    /// The id came from the first perspective whose `view` matches the
    /// active view kind (last-resort fallback).
    FirstForViewKind,
}

/// Resolve the perspective_id a mutation command should act on.
///
/// Tries (in order):
///
/// 1. explicit `args.perspective_id`
/// 2. a `perspective:{id}` moniker in the scope chain (innermost-first)
/// 3. `UIState.active_perspective_id(window_label)` for the current window
/// 4. the first perspective whose `view` matches the active view kind
///
/// Returns [`CommandError::MissingArg`] only if every fallback fails — i.e.
/// no perspectives are registered for the active view kind. The caller is
/// responsible for surfacing a useful error in that case (though in practice
/// `useAutoCreateDefaultPerspective` on the frontend creates a "Default"
/// perspective when none exist, so this terminal `MissingArg` is rare and
/// transient).
///
/// Returns the resolved id plus a [`ResolvedFrom`] tag indicating which
/// source won. When the tag is [`ResolvedFrom::UiState`] or
/// [`ResolvedFrom::FirstForViewKind`] (i.e. the caller did not supply the
/// id), [`persist_resolved_perspective_id`] writes the choice back to
/// [`UIState`] so subsequent commands find it set.
async fn resolve_perspective_id(
    ctx: &CommandContext,
    kanban: &KanbanContext,
) -> swissarmyhammer_commands::Result<(String, ResolvedFrom)> {
    let resolved = resolve_perspective_id_inner(ctx, kanban).await?;
    tracing::debug!(
        command_id = %ctx.command_id,
        perspective_id = %resolved.0,
        scope_chain = ?ctx.scope_chain,
        branch = ?resolved.1,
        "resolve_perspective_id",
    );
    Ok(resolved)
}

/// Pure resolution body for [`resolve_perspective_id`] — no tracing.
///
/// Walks the four fallbacks (arg → scope → UIState → first-for-view-kind)
/// and returns the first hit. Split out so the wrapper stays short enough
/// to pass the `code-quality:function-length` validator while the single
/// tracing line in the wrapper still records every resolved call.
async fn resolve_perspective_id_inner(
    ctx: &CommandContext,
    kanban: &KanbanContext,
) -> swissarmyhammer_commands::Result<(String, ResolvedFrom)> {
    if let Some(id) = ctx.arg("perspective_id").and_then(|v| v.as_str()) {
        return Ok((id.to_string(), ResolvedFrom::Arg));
    }
    if let Some(id) = ctx.resolve_entity_id("perspective") {
        return Ok((id.to_string(), ResolvedFrom::Scope));
    }
    let window_label = ctx.window_label_from_scope().unwrap_or("main");
    if let Some(ui) = ctx.ui_state.as_ref() {
        let active = ui.active_perspective_id(window_label);
        if !active.is_empty() {
            return Ok((active, ResolvedFrom::UiState));
        }
    }
    let view_kind = resolve_view_kind(ctx, kanban).await;
    let pctx = kanban
        .perspective_context()
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
    let pctx = pctx.read().await;
    pctx.all()
        .iter()
        .find(|p| p.view == view_kind)
        .map(|p| (p.id.clone(), ResolvedFrom::FirstForViewKind))
        .ok_or_else(|| CommandError::MissingArg("perspective_id".into()))
}

/// Persist the resolved perspective id back to [`UIState`] when the resolver
/// chose it (rather than accepting a caller-supplied id).
///
/// This makes the fallback self-healing: after the first palette/keybinding
/// invocation, [`UIState::active_perspective_id`] is non-empty, so subsequent
/// commands hit path 3 instead of walking the full resolver chain.
///
/// [`UIState::set_active_perspective`] is idempotent — it returns `None`
/// (and skips the save) when the value is unchanged — so this is safe to
/// call unconditionally within the fallback branches.
fn persist_resolved_perspective_id(
    ctx: &CommandContext,
    perspective_id: &str,
    resolved_from: ResolvedFrom,
) {
    match resolved_from {
        ResolvedFrom::Arg | ResolvedFrom::Scope => {
            // The caller supplied the id; don't mutate UIState on their behalf.
        }
        ResolvedFrom::UiState | ResolvedFrom::FirstForViewKind => {
            if let Some(ui) = ctx.ui_state.as_ref() {
                let window_label = ctx.window_label_from_scope().unwrap_or("main");
                ui.set_active_perspective(window_label, perspective_id);
            }
        }
    }
}

/// Resolve the perspective id and persist it to [`UIState`] when the resolver
/// chose the id via fallback. Returns the id as a [`String`].
///
/// This is the combined form used by every mutation command that targets
/// "the current perspective". Use the split form
/// ([`resolve_perspective_id`] + [`persist_resolved_perspective_id`]) only
/// when you need to inspect [`ResolvedFrom`] for branching logic.
async fn resolve_and_persist_perspective_id(
    ctx: &CommandContext,
    kanban: &KanbanContext,
) -> swissarmyhammer_commands::Result<String> {
    let (id, resolved_from) = resolve_perspective_id(ctx, kanban).await?;
    persist_resolved_perspective_id(ctx, &id, resolved_from);
    Ok(id)
}

/// Load a perspective by name, returning its full configuration.
///
/// Requires `name` arg (the perspective name or ID).
pub struct LoadPerspectiveCmd;

#[async_trait]
impl Command for LoadPerspectiveCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.arg("name").and_then(|v| v.as_str()).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let name = ctx
            .arg("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("name".into()))?;

        let op = GetPerspective::new(name);
        run_op(&op, &kanban).await
    }
}

/// Creates a new perspective with the given name.
///
/// Multiple perspectives may share the same name.
/// Requires `name` arg. Optional args: `view`, `filter`, `group`.
pub struct SavePerspectiveCmd;

#[async_trait]
impl Command for SavePerspectiveCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.arg("name").and_then(|v| v.as_str()).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let name = ctx
            .arg("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("name".into()))?;

        let view = ctx.arg("view").and_then(|v| v.as_str()).unwrap_or("board");

        let filter = ctx.arg("filter").and_then(|v| v.as_str()).map(String::from);
        let group = ctx.arg("group").and_then(|v| v.as_str()).map(String::from);

        if let Some(ref f) = filter {
            validate_filter(f)?;
        }

        let mut add_op = AddPerspective::new(name, view);
        add_op.filter = filter;
        add_op.group = group;

        run_op(&add_op, &kanban).await
    }
}

/// Delete a perspective by name or scope chain.
///
/// Accepts `name` arg (the perspective name or ID), or resolves the
/// perspective ID from the scope chain moniker `perspective:{id}`.
pub struct DeletePerspectiveCmd;

#[async_trait]
impl Command for DeletePerspectiveCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.arg("name").and_then(|v| v.as_str()).is_some() || ctx.has_in_scope("perspective")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        // Try explicit name arg first, then fall back to scope chain moniker.
        let id = if let Some(name) = ctx.arg("name").and_then(|v| v.as_str()) {
            // Resolve name to ID if necessary
            let pctx = kanban
                .perspective_context()
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
            let pctx = pctx.read().await;
            if let Some(p) = pctx.get_by_name(name) {
                p.id.to_string()
            } else if pctx.get_by_id(name).is_some() {
                name.to_string()
            } else {
                return Err(CommandError::ExecutionFailed(format!(
                    "perspective not found: {name}"
                )));
            }
        } else if let Some(scope_id) = ctx.resolve_entity_id("perspective") {
            scope_id.to_string()
        } else {
            return Err(CommandError::MissingArg("name".into()));
        };

        let op = DeletePerspective::new(id);
        run_op(&op, &kanban).await
    }
}

/// Rename a perspective.
///
/// Required args: `id` (perspective ULID), `new_name` (the new name).
pub struct RenamePerspectiveCmd;

#[async_trait]
impl Command for RenamePerspectiveCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let id = ctx.require_arg_str("id")?;
        let new_name = ctx.require_arg_str("new_name")?;
        let op = RenamePerspective::new(id, new_name);
        run_op(&op, &kanban).await
    }
}

/// Set the filter on an active perspective.
///
/// Always available. The target perspective is resolved at execute time via
/// [`resolve_perspective_id`] (explicit arg → scope-chain moniker → UIState
/// active → first perspective for the active view kind). See
/// [`ClearFilterCmd`] for the same resolution semantics.
pub struct SetFilterCmd;

#[async_trait]
impl Command for SetFilterCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let perspective_id = resolve_and_persist_perspective_id(ctx, &kanban).await?;

        let filter = ctx
            .arg("filter")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("filter".into()))?;

        validate_filter(filter)?;

        let op = UpdatePerspective::new(&perspective_id).with_filter(Some(filter.to_string()));
        run_op(&op, &kanban).await
    }
}

/// Clear the filter on an active perspective.
///
/// Always available. The target perspective is resolved at execute time via
/// [`resolve_perspective_id`] (explicit arg → scope-chain moniker → UIState
/// active → first perspective for the active view kind).
pub struct ClearFilterCmd;

#[async_trait]
impl Command for ClearFilterCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let perspective_id = resolve_and_persist_perspective_id(ctx, &kanban).await?;

        let op = UpdatePerspective::new(&perspective_id).with_filter(None);
        run_op(&op, &kanban).await
    }
}

/// Set the group on an active perspective.
///
/// Always available. The target perspective is resolved at execute time via
/// [`resolve_perspective_id`] (explicit arg → scope-chain moniker → UIState
/// active → first perspective for the active view kind).
pub struct SetGroupCmd;

#[async_trait]
impl Command for SetGroupCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let perspective_id = resolve_and_persist_perspective_id(ctx, &kanban).await?;

        let group = ctx
            .arg("group")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("group".into()))?;

        let op = UpdatePerspective::new(&perspective_id).with_group(Some(group.to_string()));
        run_op(&op, &kanban).await
    }
}

/// Clear the group on an active perspective.
///
/// Always available. The target perspective is resolved at execute time via
/// [`resolve_perspective_id`] (explicit arg → scope-chain moniker → UIState
/// active → first perspective for the active view kind).
pub struct ClearGroupCmd;

#[async_trait]
impl Command for ClearGroupCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let perspective_id = resolve_and_persist_perspective_id(ctx, &kanban).await?;

        let op = UpdatePerspective::new(&perspective_id).with_group(None);
        run_op(&op, &kanban).await
    }
}

/// Set a sort entry on the active perspective.
///
/// Adds or replaces a sort entry for the given field. If the field already
/// appears in the sort list, its direction is updated. Otherwise it is
/// appended.
///
/// Always available. Requires `field` and `direction` ("asc" or "desc") args.
/// The target perspective is resolved at execute time via
/// [`resolve_perspective_id`].
pub struct SetSortCmd;

#[async_trait]
impl Command for SetSortCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let perspective_id = resolve_and_persist_perspective_id(ctx, &kanban).await?;

        let field = ctx
            .arg("field")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("field".into()))?;

        let direction_str = ctx
            .arg("direction")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("direction".into()))?;

        let direction = match direction_str {
            "asc" => SortDirection::Asc,
            "desc" => SortDirection::Desc,
            other => {
                return Err(CommandError::ExecutionFailed(format!(
                    "invalid sort direction: {other} (expected \"asc\" or \"desc\")"
                )))
            }
        };

        // Read existing sort, replace or append
        let existing_sort = {
            let pctx = kanban
                .perspective_context()
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
            let pctx = pctx.read().await;
            pctx.get_by_id(&perspective_id)
                .map(|p| p.sort.clone())
                .unwrap_or_default()
        };

        let mut new_sort: Vec<SortEntry> = existing_sort
            .into_iter()
            .filter(|e| e.field != field)
            .collect();
        new_sort.push(SortEntry::new(field, direction));

        let op = UpdatePerspective::new(&perspective_id).with_sort(new_sort);
        run_op(&op, &kanban).await
    }
}

/// Clear every sort entry on the active perspective.
///
/// Multi-field perspectives are reset to unsorted; perspectives that are
/// already unsorted become a no-op (the command still returns the resolved
/// perspective, so callers can treat it uniformly). The command never takes
/// a `field` arg — per-field removal is covered by [`ToggleSortCmd`]'s
/// asc → desc → none state cycle.
///
/// Always available. The target perspective is resolved at execute time via
/// [`resolve_perspective_id`].
pub struct ClearSortCmd;

#[async_trait]
impl Command for ClearSortCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let perspective_id = resolve_and_persist_perspective_id(ctx, &kanban).await?;
        let op = UpdatePerspective::new(&perspective_id).with_sort(Vec::new());
        run_op(&op, &kanban).await
    }
}

/// Toggle sort direction for a field on the active perspective.
///
/// Cycles through: none → asc → desc → none. If the field is not in the
/// sort list, it is added as ascending. If it is ascending, it becomes
/// descending. If it is descending, it is removed.
///
/// Always available. The target perspective is resolved at execute time via
/// [`resolve_perspective_id`].
pub struct ToggleSortCmd;

#[async_trait]
impl Command for ToggleSortCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let perspective_id = resolve_and_persist_perspective_id(ctx, &kanban).await?;

        let field = ctx
            .arg("field")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("field".into()))?;

        // Read existing sort entries
        let existing_sort = {
            let pctx = kanban
                .perspective_context()
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
            let pctx = pctx.read().await;
            pctx.get_by_id(&perspective_id)
                .map(|p| p.sort.clone())
                .unwrap_or_default()
        };

        let current_direction = existing_sort
            .iter()
            .find(|e| e.field == field)
            .map(|e| e.direction.clone());

        let mut new_sort: Vec<SortEntry> = existing_sort
            .into_iter()
            .filter(|e| e.field != field)
            .collect();

        match current_direction.as_ref() {
            None => {
                // none -> asc
                new_sort.push(SortEntry::new(field, SortDirection::Asc));
            }
            Some(SortDirection::Asc) => {
                // asc -> desc
                new_sort.push(SortEntry::new(field, SortDirection::Desc));
            }
            Some(SortDirection::Desc) => {
                // desc -> none (already filtered out)
            }
        }

        let op = UpdatePerspective::new(&perspective_id).with_sort(new_sort);
        run_op(&op, &kanban).await
    }
}

/// Cycle to the next perspective within the same view kind.
///
/// Always available. Required arg: `view_kind` (e.g. "board", "grid").
/// Filters perspectives to those matching `view_kind`, finds the current
/// active perspective, and advances to the next one (wrapping around).
/// No-op (returns `null`) when fewer than 2 perspectives match.
pub struct NextPerspectiveCmd;

#[async_trait]
impl Command for NextPerspectiveCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        cycle_perspective(ctx, CycleDirection::Next).await
    }
}

/// Cycle to the previous perspective within the same view kind.
///
/// Always available. Required arg: `view_kind` (e.g. "board", "grid").
/// Filters perspectives to those matching `view_kind`, finds the current
/// active perspective, and moves to the previous one (wrapping around).
/// No-op (returns `null`) when fewer than 2 perspectives match.
pub struct PrevPerspectiveCmd;

#[async_trait]
impl Command for PrevPerspectiveCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        cycle_perspective(ctx, CycleDirection::Prev).await
    }
}

/// Resolve the view kind for perspective cycling.
///
/// Resolution order: explicit `view_kind` arg > scope chain `view:{id}` moniker
/// looked up against the views registry > `"board"` default. When invoked via
/// keybinding or command palette no args are passed, so the scope chain fallback
/// is the primary path.
async fn resolve_view_kind(ctx: &CommandContext, kanban: &KanbanContext) -> String {
    if let Some(explicit) = ctx.arg("view_kind").and_then(|v| v.as_str()) {
        return explicit.to_string();
    }

    let view_id = ctx.scope_chain.iter().find_map(|m| m.strip_prefix("view:"));

    if let Some(kind) = resolve_kind_from_view_id(view_id, kanban).await {
        return kind;
    }

    "board".to_string()
}

/// Look up a view ID in the views registry and return its kind as a string.
async fn resolve_kind_from_view_id(
    view_id: Option<&str>,
    kanban: &KanbanContext,
) -> Option<String> {
    let view_id = view_id?;
    let views_lock = kanban.views()?;
    let views = views_lock.read().await;
    let view_def = views.get_by_id(view_id)?;
    serde_json::to_value(&view_def.kind)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
}

/// Direction for perspective cycling.
enum CycleDirection {
    Next,
    Prev,
}

/// Shared logic for next/prev perspective cycling.
///
/// Lists all perspectives, filters to those matching `view_kind`, finds the
/// current active perspective by index, and advances or retreats by one
/// (wrapping). Updates UIState and returns the `UIStateChange`, or `null`
/// if cycling is not possible (fewer than 2 matching perspectives).
async fn cycle_perspective(
    ctx: &CommandContext,
    direction: CycleDirection,
) -> swissarmyhammer_commands::Result<Value> {
    let kanban = ctx.require_extension::<KanbanContext>()?;
    let ui = ctx
        .ui_state
        .as_ref()
        .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

    let view_kind = resolve_view_kind(ctx, &kanban).await;
    let window_label = ctx.window_label_from_scope().unwrap_or("main");
    let current_id = ui.active_perspective_id(window_label);

    // Get perspectives matching the requested view kind
    let matching: Vec<String> = {
        let pctx = kanban
            .perspective_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        let pctx = pctx.read().await;
        pctx.all()
            .iter()
            .filter(|p| p.view == view_kind)
            .map(|p| p.id.clone())
            .collect()
    };

    if matching.len() < 2 {
        return Ok(Value::Null);
    }

    let current_index = matching.iter().position(|id| id == &current_id);
    let len = matching.len();

    let next_index = match (current_index, &direction) {
        (Some(i), CycleDirection::Next) => (i + 1) % len,
        (Some(i), CycleDirection::Prev) => (i + len - 1) % len,
        // Current perspective not found in matching set — start from beginning/end
        (None, CycleDirection::Next) => 0,
        (None, CycleDirection::Prev) => len - 1,
    };

    let new_id = &matching[next_index];
    let change = ui.set_active_perspective(window_label, new_id);
    Ok(serde_json::to_value(change).unwrap_or(Value::Null))
}

/// Switch to a perspective by its ID.
///
/// Always available. Required arg: `id` (perspective ULID).
/// Optional arg: `view_kind` — if provided, validates that the perspective's
/// view matches before switching. Returns an error if the perspective is not
/// found or the view kind does not match.
pub struct GotoPerspectiveCmd;

#[async_trait]
impl Command for GotoPerspectiveCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let id = ctx.require_arg_str("id")?;
        let view_kind = ctx.arg("view_kind").and_then(|v| v.as_str());
        let window_label = ctx.window_label_from_scope().unwrap_or("main");

        // Validate the perspective exists.
        let pctx = kanban
            .perspective_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        let pctx = pctx.read().await;

        let perspective = pctx
            .get_by_id(id)
            .ok_or_else(|| CommandError::ExecutionFailed(format!("perspective not found: {id}")))?;

        // If view_kind is specified, validate it matches.
        if let Some(expected) = view_kind {
            if perspective.view != expected {
                return Err(CommandError::ExecutionFailed(format!(
                    "perspective '{}' has view '{}', expected '{expected}'",
                    perspective.name, perspective.view
                )));
            }
        }

        // Release the lock before mutating UIState.
        drop(pctx);

        let change = ui.set_active_perspective(window_label, id);
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}

/// List all perspectives on the board.
///
/// No arguments required. Returns a JSON object with `perspectives` array
/// and `count`.
pub struct ListPerspectivesCmd;

#[async_trait]
impl Command for ListPerspectivesCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let op = ListPerspectives::new();
        run_op(&op, &kanban).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::context::KanbanContext;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_operations::Execute;
    use tempfile::TempDir;

    /// Create a temp KanbanContext with an initialized board.
    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);
        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        (temp, ctx)
    }

    /// Build a CommandContext with the given args and a KanbanContext extension.
    fn make_ctx(kanban: Arc<KanbanContext>, args: HashMap<String, Value>) -> CommandContext {
        let mut ctx = CommandContext::new("test", vec![], None, args);
        ctx.set_extension(kanban);
        ctx
    }

    /// Build a CommandContext with a scope chain (for commands that need `has_in_scope`).
    fn make_ctx_with_scope(
        kanban: Arc<KanbanContext>,
        args: HashMap<String, Value>,
        scope_chain: Vec<String>,
    ) -> CommandContext {
        let mut ctx = CommandContext::new("test", scope_chain, None, args);
        ctx.set_extension(kanban);
        ctx
    }

    /// Helper: create a perspective with a specific view kind and return its ID.
    async fn create_perspective_with_view(
        kanban: &Arc<KanbanContext>,
        name: &str,
        view: &str,
    ) -> String {
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String(name.into()));
        args.insert("view".into(), Value::String(view.into()));
        let cmd_ctx = make_ctx(Arc::clone(kanban), args);
        let result = SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        result["id"].as_str().unwrap().to_string()
    }

    /// Helper: create a perspective and return its ID.
    async fn create_perspective(kanban: &Arc<KanbanContext>, name: &str) -> String {
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String(name.into()));
        args.insert("view".into(), Value::String("grid".into()));
        let cmd_ctx = make_ctx(Arc::clone(kanban), args);
        let result = SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        result["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn test_list_perspectives_cmd_empty() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(Arc::clone(&kanban), HashMap::new());

        let result = ListPerspectivesCmd.execute(&cmd_ctx).await.unwrap();
        let perspectives = result["perspectives"].as_array().unwrap();
        assert!(perspectives.is_empty());
        assert_eq!(result["count"], 0);
    }

    #[tokio::test]
    async fn test_list_perspectives_cmd_after_save() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        // Save a perspective via the SavePerspectiveCmd
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("My View".into()));
        args.insert("view".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args);
        SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();

        // Now list
        let cmd_ctx = make_ctx(Arc::clone(&kanban), HashMap::new());
        let result = ListPerspectivesCmd.execute(&cmd_ctx).await.unwrap();
        let perspectives = result["perspectives"].as_array().unwrap();
        assert_eq!(perspectives.len(), 1);
        assert_eq!(result["count"], 1);
        assert_eq!(perspectives[0]["name"], "My View");
        assert_eq!(perspectives[0]["view"], "board");
        // Each perspective should have an id
        assert!(perspectives[0]["id"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_set_sort_cmd_adds_sort_entry() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let pid = create_perspective(&kanban, "Sort Test").await;

        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("field".into(), Value::String("title".into()));
        args.insert("direction".into(), Value::String("asc".into()));
        let cmd_ctx = make_ctx_with_scope(
            Arc::clone(&kanban),
            args,
            vec![format!("perspective:{pid}")],
        );
        let result = SetSortCmd.execute(&cmd_ctx).await.unwrap();
        let sort = result["sort"].as_array().unwrap();
        assert_eq!(sort.len(), 1);
        assert_eq!(sort[0]["field"], "title");
        assert_eq!(sort[0]["direction"], "asc");
    }

    #[tokio::test]
    async fn test_set_sort_cmd_replaces_existing_field() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let pid = create_perspective(&kanban, "Sort Test").await;

        // Set asc first
        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("field".into(), Value::String("title".into()));
        args.insert("direction".into(), Value::String("asc".into()));
        let cmd_ctx = make_ctx_with_scope(
            Arc::clone(&kanban),
            args,
            vec![format!("perspective:{pid}")],
        );
        SetSortCmd.execute(&cmd_ctx).await.unwrap();

        // Now set desc — should replace, not append
        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("field".into(), Value::String("title".into()));
        args.insert("direction".into(), Value::String("desc".into()));
        let cmd_ctx = make_ctx_with_scope(
            Arc::clone(&kanban),
            args,
            vec![format!("perspective:{pid}")],
        );
        let result = SetSortCmd.execute(&cmd_ctx).await.unwrap();
        let sort = result["sort"].as_array().unwrap();
        assert_eq!(sort.len(), 1);
        assert_eq!(sort[0]["field"], "title");
        assert_eq!(sort[0]["direction"], "desc");
    }

    #[tokio::test]
    async fn test_clear_sort_cmd_removes_all_entries() {
        // ClearSortCmd drops every sort entry on the resolved perspective.
        // This is the regression guard for the bug where the palette/context-menu
        // invocation failed with MissingArg("field") — the `field` arg was
        // dropped from the command's contract.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let pid = create_perspective(&kanban, "Sort Test").await;

        // Add two sort entries on different fields.
        for (field, direction) in [("title", "asc"), ("priority", "desc")] {
            let mut args = HashMap::new();
            args.insert("perspective_id".into(), Value::String(pid.clone()));
            args.insert("field".into(), Value::String(field.into()));
            args.insert("direction".into(), Value::String(direction.into()));
            let cmd_ctx = make_ctx_with_scope(
                Arc::clone(&kanban),
                args,
                vec![format!("perspective:{pid}")],
            );
            SetSortCmd.execute(&cmd_ctx).await.unwrap();
        }

        // Clear — no `field` arg supplied; the command must drop both entries.
        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        let cmd_ctx = make_ctx_with_scope(
            Arc::clone(&kanban),
            args,
            vec![format!("perspective:{pid}")],
        );
        let result = ClearSortCmd.execute(&cmd_ctx).await.unwrap();
        let sort = result["sort"].as_array();
        assert!(
            sort.is_none() || sort.unwrap().is_empty(),
            "sort should be fully cleared, got: {:?}",
            sort
        );
    }

    #[tokio::test]
    async fn test_clear_sort_cmd_noop_on_empty_sort() {
        // Clearing an already-empty sort list must succeed without error.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let pid = create_perspective(&kanban, "Empty Sort").await;

        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        let cmd_ctx = make_ctx_with_scope(
            Arc::clone(&kanban),
            args,
            vec![format!("perspective:{pid}")],
        );
        let result = ClearSortCmd.execute(&cmd_ctx).await.unwrap();
        let sort = result["sort"].as_array();
        assert!(sort.is_none() || sort.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_clear_sort_cmd_works_from_palette_with_no_perspective_id() {
        // Palette invocation: no args, no scope perspective moniker.
        // The resolver falls back to UIState.active_perspective_id("main"),
        // and the command must clear the sort list on that perspective.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let pid = create_perspective_with_view(&kanban, "Palette Sort", "grid").await;

        // Mark this perspective as UIState-active for the default window.
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());
        ui.set_active_perspective("main", &pid);

        // Seed a sort entry on it.
        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("field".into(), Value::String("title".into()));
        args.insert("direction".into(), Value::String("asc".into()));
        let cmd_ctx = make_ctx_with_scope(
            Arc::clone(&kanban),
            args,
            vec![format!("perspective:{pid}")],
        );
        SetSortCmd.execute(&cmd_ctx).await.unwrap();

        // Palette dispatch: empty args, no scope moniker — UIState must win.
        let cmd_ctx = make_ctx_with_ui(Arc::clone(&kanban), HashMap::new(), Arc::clone(&ui));
        let result = ClearSortCmd.execute(&cmd_ctx).await.unwrap();
        let sort = result["sort"].as_array();
        assert!(
            sort.is_none() || sort.unwrap().is_empty(),
            "palette dispatch should resolve perspective via UIState and clear sort, got: {:?}",
            sort
        );
    }

    #[tokio::test]
    async fn test_toggle_sort_cmd_cycles_none_asc_desc_none() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let pid = create_perspective(&kanban, "Toggle Test").await;

        let scope = vec![format!("perspective:{pid}")];

        // Toggle 1: none → asc
        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("field".into(), Value::String("priority".into()));
        let cmd_ctx = make_ctx_with_scope(Arc::clone(&kanban), args, scope.clone());
        let result = ToggleSortCmd.execute(&cmd_ctx).await.unwrap();
        let sort = result["sort"].as_array().unwrap();
        assert_eq!(sort.len(), 1);
        assert_eq!(sort[0]["direction"], "asc");

        // Toggle 2: asc → desc
        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("field".into(), Value::String("priority".into()));
        let cmd_ctx = make_ctx_with_scope(Arc::clone(&kanban), args, scope.clone());
        let result = ToggleSortCmd.execute(&cmd_ctx).await.unwrap();
        let sort = result["sort"].as_array().unwrap();
        assert_eq!(sort.len(), 1);
        assert_eq!(sort[0]["direction"], "desc");

        // Toggle 3: desc → none
        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("field".into(), Value::String("priority".into()));
        let cmd_ctx = make_ctx_with_scope(Arc::clone(&kanban), args, scope.clone());
        let result = ToggleSortCmd.execute(&cmd_ctx).await.unwrap();
        let sort = result["sort"].as_array();
        assert!(sort.is_none() || sort.unwrap().is_empty());
    }

    // ── Filter validation tests ──────────────────────────────────────

    #[tokio::test]
    async fn test_set_filter_cmd_accepts_valid_dsl() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let pid = create_perspective(&kanban, "Filter Test").await;

        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("filter".into(), Value::String("#bug && @will".into()));
        let cmd_ctx = make_ctx_with_scope(
            Arc::clone(&kanban),
            args,
            vec![format!("perspective:{pid}")],
        );
        let result = SetFilterCmd.execute(&cmd_ctx).await;
        assert!(result.is_ok(), "valid DSL should be accepted");
        assert_eq!(result.unwrap()["filter"], "#bug && @will");
    }

    #[tokio::test]
    async fn test_set_filter_cmd_rejects_invalid_expression() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let pid = create_perspective(&kanban, "Filter Test").await;

        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("filter".into(), Value::String("invalid $$$ garbage".into()));
        let cmd_ctx = make_ctx_with_scope(
            Arc::clone(&kanban),
            args,
            vec![format!("perspective:{pid}")],
        );
        let result = SetFilterCmd.execute(&cmd_ctx).await;
        assert!(result.is_err(), "invalid expression should be rejected");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("invalid filter expression"),
            "error should mention invalid filter: {err}"
        );
    }

    #[tokio::test]
    async fn test_set_filter_cmd_rejects_old_js_expression() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let pid = create_perspective(&kanban, "Filter Test").await;

        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("filter".into(), Value::String("Status !== \"Done\"".into()));
        let cmd_ctx = make_ctx_with_scope(
            Arc::clone(&kanban),
            args,
            vec![format!("perspective:{pid}")],
        );
        let result = SetFilterCmd.execute(&cmd_ctx).await;
        assert!(
            result.is_err(),
            "old JS expressions should be rejected as invalid"
        );
    }

    #[tokio::test]
    async fn test_save_perspective_cmd_validates_filter() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        // Valid DSL filter should work
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("Valid".into()));
        args.insert("view".into(), Value::String("board".into()));
        args.insert("filter".into(), Value::String("#bug || #feature".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args);
        let result = SavePerspectiveCmd.execute(&cmd_ctx).await;
        assert!(
            result.is_ok(),
            "valid DSL filter should be accepted on save"
        );

        // Invalid filter should fail
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("Invalid".into()));
        args.insert("view".into(), Value::String("board".into()));
        args.insert("filter".into(), Value::String("$$garbage".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args);
        let result = SavePerspectiveCmd.execute(&cmd_ctx).await;
        assert!(result.is_err(), "invalid filter should be rejected on save");
    }

    #[tokio::test]
    async fn test_perspective_mutation_cmds_always_available() {
        // Commands that resolve `perspective_id` at execute time are always
        // available from the palette — scope-chain membership is no longer a
        // gate. See `resolve_perspective_id` for the resolution order.
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        assert!(SetFilterCmd.available(&ctx));
        assert!(ClearFilterCmd.available(&ctx));
        assert!(SetGroupCmd.available(&ctx));
        assert!(ClearGroupCmd.available(&ctx));
        assert!(SetSortCmd.available(&ctx));
        assert!(ClearSortCmd.available(&ctx));
        assert!(ToggleSortCmd.available(&ctx));
    }

    #[tokio::test]
    async fn test_perspective_mutation_cmds_available_with_scope() {
        // Still available when a perspective moniker is in scope (context
        // menu / right-click path).
        let ctx = CommandContext::new(
            "test",
            vec!["perspective:01ABC".into()],
            None,
            HashMap::new(),
        );
        assert!(SetFilterCmd.available(&ctx));
        assert!(ClearFilterCmd.available(&ctx));
        assert!(SetGroupCmd.available(&ctx));
        assert!(ClearGroupCmd.available(&ctx));
        assert!(SetSortCmd.available(&ctx));
        assert!(ClearSortCmd.available(&ctx));
        assert!(ToggleSortCmd.available(&ctx));
    }

    // =========================================================================
    // Next / Prev perspective cycling
    // =========================================================================

    /// Build a CommandContext with KanbanContext extension and UIState.
    fn make_ctx_with_ui(
        kanban: Arc<KanbanContext>,
        args: HashMap<String, Value>,
        ui: Arc<swissarmyhammer_commands::UIState>,
    ) -> CommandContext {
        let mut ctx = CommandContext::new("test", vec![], None, args);
        ctx.set_extension(kanban);
        ctx.ui_state = Some(ui);
        ctx
    }

    #[tokio::test]
    async fn test_next_perspective_cycles_forward_with_wrapping() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        let id_a = create_perspective_with_view(&kanban, "A", "board").await;
        let id_b = create_perspective_with_view(&kanban, "B", "board").await;
        let id_c = create_perspective_with_view(&kanban, "C", "board").await;

        // Set active to A
        ui.set_active_perspective("main", &id_a);

        // Next: A -> B
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx_with_ui(Arc::clone(&kanban), args, Arc::clone(&ui));
        let result = NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert!(result != Value::Null);
        assert_eq!(ui.active_perspective_id("main"), id_b);

        // Next: B -> C
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx_with_ui(Arc::clone(&kanban), args, Arc::clone(&ui));
        NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(ui.active_perspective_id("main"), id_c);

        // Next: C -> A (wrap)
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx_with_ui(Arc::clone(&kanban), args, Arc::clone(&ui));
        NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(ui.active_perspective_id("main"), id_a);
    }

    #[tokio::test]
    async fn test_prev_perspective_cycles_backward_with_wrapping() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        let id_a = create_perspective_with_view(&kanban, "A", "grid").await;
        let id_b = create_perspective_with_view(&kanban, "B", "grid").await;
        let id_c = create_perspective_with_view(&kanban, "C", "grid").await;

        // Set active to A
        ui.set_active_perspective("main", &id_a);

        // Prev: A -> C (wrap)
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("grid".into()));
        let cmd_ctx = make_ctx_with_ui(Arc::clone(&kanban), args, Arc::clone(&ui));
        PrevPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(ui.active_perspective_id("main"), id_c);

        // Prev: C -> B
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("grid".into()));
        let cmd_ctx = make_ctx_with_ui(Arc::clone(&kanban), args, Arc::clone(&ui));
        PrevPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(ui.active_perspective_id("main"), id_b);

        // Prev: B -> A
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("grid".into()));
        let cmd_ctx = make_ctx_with_ui(Arc::clone(&kanban), args, Arc::clone(&ui));
        PrevPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(ui.active_perspective_id("main"), id_a);
    }

    #[tokio::test]
    async fn test_cycle_noop_with_zero_matching_perspectives() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        // Create perspectives for "board" but query for "grid"
        create_perspective_with_view(&kanban, "A", "board").await;

        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("grid".into()));
        let cmd_ctx = make_ctx_with_ui(Arc::clone(&kanban), args, Arc::clone(&ui));
        let result = NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result, Value::Null);
    }

    #[tokio::test]
    async fn test_cycle_noop_with_one_matching_perspective() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        let id_a = create_perspective_with_view(&kanban, "A", "board").await;
        ui.set_active_perspective("main", &id_a);

        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx_with_ui(Arc::clone(&kanban), args, Arc::clone(&ui));
        let result = NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result, Value::Null);
    }

    #[tokio::test]
    async fn test_cycle_filters_by_view_kind() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        let id_board = create_perspective_with_view(&kanban, "Board1", "board").await;
        let _id_grid = create_perspective_with_view(&kanban, "Grid1", "grid").await;
        let id_board2 = create_perspective_with_view(&kanban, "Board2", "board").await;

        // Set active to board perspective
        ui.set_active_perspective("main", &id_board);

        // Next should go to Board2, not Grid1
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx_with_ui(Arc::clone(&kanban), args, Arc::clone(&ui));
        NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(ui.active_perspective_id("main"), id_board2);
    }

    #[tokio::test]
    async fn test_next_prev_always_available() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        assert!(NextPerspectiveCmd.available(&ctx));
        assert!(PrevPerspectiveCmd.available(&ctx));
    }

    /// Create a KanbanContext with views initialized (via `open`).
    async fn setup_with_views() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();
        let ctx = KanbanContext::open(&kanban_dir).await.unwrap();
        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        (temp, ctx)
    }

    /// Build a CommandContext with scope chain, UI state, and args.
    fn make_ctx_with_scope_and_ui(
        kanban: Arc<KanbanContext>,
        args: HashMap<String, Value>,
        scope_chain: Vec<String>,
        ui: Arc<swissarmyhammer_commands::UIState>,
    ) -> CommandContext {
        let mut ctx = CommandContext::new("test", scope_chain, None, args);
        ctx.set_extension(kanban);
        ctx.ui_state = Some(ui);
        ctx
    }

    #[tokio::test]
    async fn test_next_perspective_derives_view_kind_from_scope_chain() {
        let (_temp, ctx) = setup_with_views().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        // Create board perspectives
        let id_a = create_perspective_with_view(&kanban, "A", "board").await;
        let id_b = create_perspective_with_view(&kanban, "B", "board").await;

        ui.set_active_perspective("main", &id_a);

        // Invoke without view_kind arg, but with view:01JMVIEW0000000000BOARD0 in scope chain
        let scope = vec!["view:01JMVIEW0000000000BOARD0".to_string()];
        let cmd_ctx =
            make_ctx_with_scope_and_ui(Arc::clone(&kanban), HashMap::new(), scope, Arc::clone(&ui));
        let result = NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();

        assert!(result != Value::Null, "should cycle, not return null");
        assert_eq!(ui.active_perspective_id("main"), id_b);
    }

    #[tokio::test]
    async fn test_next_perspective_explicit_view_kind_overrides_scope() {
        let (_temp, ctx) = setup_with_views().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        // Create board and grid perspectives
        let _id_board_a = create_perspective_with_view(&kanban, "BoardA", "board").await;
        let _id_board_b = create_perspective_with_view(&kanban, "BoardB", "board").await;
        let id_grid_a = create_perspective_with_view(&kanban, "GridA", "grid").await;
        let id_grid_b = create_perspective_with_view(&kanban, "GridB", "grid").await;

        ui.set_active_perspective("main", &id_grid_a);

        // Scope chain says board view, but explicit arg says "grid" — explicit wins
        let scope = vec!["view:01JMVIEW0000000000BOARD0".to_string()];
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("grid".into()));
        let cmd_ctx = make_ctx_with_scope_and_ui(Arc::clone(&kanban), args, scope, Arc::clone(&ui));
        let result = NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();

        assert!(result != Value::Null, "should cycle grid perspectives");
        assert_eq!(
            ui.active_perspective_id("main"),
            id_grid_b,
            "explicit view_kind=grid should override scope chain's board view"
        );
    }

    // =========================================================================
    // perspective.goto — switch to perspective by ID
    // =========================================================================

    #[tokio::test]
    async fn test_goto_perspective_valid_id_sets_active() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        let id = create_perspective_with_view(&kanban, "Target", "board").await;

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(id.clone()));
        let cmd_ctx = make_ctx_with_ui(Arc::clone(&kanban), args, Arc::clone(&ui));
        let result = GotoPerspectiveCmd.execute(&cmd_ctx).await.unwrap();

        assert!(result != Value::Null);
        assert_eq!(ui.active_perspective_id("main"), id);
    }

    #[tokio::test]
    async fn test_goto_perspective_invalid_id_returns_error() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String("nonexistent".into()));
        let cmd_ctx = make_ctx_with_ui(Arc::clone(&kanban), args, Arc::clone(&ui));
        let result = GotoPerspectiveCmd.execute(&cmd_ctx).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_goto_perspective_mismatched_view_kind_returns_error() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        let id = create_perspective_with_view(&kanban, "BoardView", "board").await;

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(id.clone()));
        args.insert("view_kind".into(), Value::String("grid".into()));
        let cmd_ctx = make_ctx_with_ui(Arc::clone(&kanban), args, Arc::clone(&ui));
        let result = GotoPerspectiveCmd.execute(&cmd_ctx).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_goto_perspective_without_view_kind_succeeds() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        let id = create_perspective_with_view(&kanban, "GridView", "grid").await;

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(id.clone()));
        // No view_kind arg — should succeed regardless of the perspective's view
        let cmd_ctx = make_ctx_with_ui(Arc::clone(&kanban), args, Arc::clone(&ui));
        let result = GotoPerspectiveCmd.execute(&cmd_ctx).await.unwrap();

        assert!(result != Value::Null);
        assert_eq!(ui.active_perspective_id("main"), id);
    }

    #[tokio::test]
    async fn test_goto_perspective_always_available() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        assert!(GotoPerspectiveCmd.available(&ctx));
    }

    // =========================================================================
    // Rename perspective
    // =========================================================================

    #[tokio::test]
    async fn test_rename_perspective_cmd() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        // Create a perspective
        let id = create_perspective(&kanban, "Old Name").await;

        // Rename it
        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(id.clone()));
        args.insert("new_name".into(), Value::String("New Name".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args);
        let result = RenamePerspectiveCmd.execute(&cmd_ctx).await.unwrap();

        // Verify the result contains the new name
        assert_eq!(result["name"].as_str().unwrap(), "New Name");
    }

    #[tokio::test]
    async fn test_rename_perspective_cmd_not_found() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String("nonexistent".into()));
        args.insert("new_name".into(), Value::String("Whatever".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args);
        let result = RenamePerspectiveCmd.execute(&cmd_ctx).await;

        assert!(result.is_err());
    }

    // =========================================================================
    // resolve_perspective_id — four-tier fallback
    // =========================================================================

    /// Build a CommandContext with scope chain, args, UIState, and a
    /// KanbanContext extension. Used for resolver tests that need every
    /// input simultaneously.
    fn make_full_ctx(
        kanban: Arc<KanbanContext>,
        args: HashMap<String, Value>,
        scope_chain: Vec<String>,
        ui: Arc<swissarmyhammer_commands::UIState>,
    ) -> CommandContext {
        let mut ctx = CommandContext::new("test", scope_chain, None, args);
        ctx.set_extension(kanban);
        ctx.ui_state = Some(ui);
        ctx
    }

    #[tokio::test]
    async fn resolve_perspective_id_prefers_explicit_arg() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        // Populate UIState so every fallback path has an id available —
        // the explicit arg must still win.
        ui.set_active_perspective("main", "ui-id");

        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String("arg-id".into()));
        let cmd_ctx = make_full_ctx(
            Arc::clone(&kanban),
            args,
            vec!["perspective:scope-id".into(), "window:main".into()],
            Arc::clone(&ui),
        );

        let (id, source) = resolve_perspective_id(&cmd_ctx, &kanban).await.unwrap();
        assert_eq!(id, "arg-id");
        assert_eq!(source, ResolvedFrom::Arg);
    }

    #[tokio::test]
    async fn resolve_perspective_id_falls_back_to_scope_chain() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());
        ui.set_active_perspective("main", "ui-id");

        // No arg — scope chain's perspective moniker should win over UIState.
        let cmd_ctx = make_full_ctx(
            Arc::clone(&kanban),
            HashMap::new(),
            vec!["perspective:scope-id".into(), "window:main".into()],
            Arc::clone(&ui),
        );

        let (id, source) = resolve_perspective_id(&cmd_ctx, &kanban).await.unwrap();
        assert_eq!(id, "scope-id");
        assert_eq!(source, ResolvedFrom::Scope);
    }

    #[tokio::test]
    async fn resolve_perspective_id_falls_back_to_uistate() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());
        ui.set_active_perspective("main", "ui-id");

        // No arg, no scope-chain perspective moniker — UIState wins.
        let cmd_ctx = make_full_ctx(
            Arc::clone(&kanban),
            HashMap::new(),
            vec!["window:main".into()],
            Arc::clone(&ui),
        );

        let (id, source) = resolve_perspective_id(&cmd_ctx, &kanban).await.unwrap();
        assert_eq!(id, "ui-id");
        assert_eq!(source, ResolvedFrom::UiState);
    }

    #[tokio::test]
    async fn resolve_perspective_id_falls_back_to_first_for_view_kind() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        // Two perspectives: one for each view kind.
        let board_pid = create_perspective_with_view(&kanban, "Board", "board").await;
        let _grid_pid = create_perspective_with_view(&kanban, "Grid", "grid").await;

        // No arg, no scope-chain perspective, empty UIState.
        // view_kind arg steers the fallback to the "board" perspective.
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("board".into()));
        let cmd_ctx = make_full_ctx(
            Arc::clone(&kanban),
            args,
            vec!["window:main".into()],
            Arc::clone(&ui),
        );

        let (id, source) = resolve_perspective_id(&cmd_ctx, &kanban).await.unwrap();
        assert_eq!(id, board_pid, "should pick the board-kind perspective");
        assert_eq!(source, ResolvedFrom::FirstForViewKind);
    }

    #[tokio::test]
    async fn resolve_and_persist_writes_uistate_when_fallback_used() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        let board_pid = create_perspective_with_view(&kanban, "Board", "board").await;

        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("board".into()));
        let cmd_ctx = make_full_ctx(
            Arc::clone(&kanban),
            args,
            vec!["window:main".into()],
            Arc::clone(&ui),
        );

        // Confirm UIState starts empty.
        assert_eq!(ui.active_perspective_id("main"), "");

        let resolved = resolve_and_persist_perspective_id(&cmd_ctx, &kanban)
            .await
            .unwrap();

        assert_eq!(resolved, board_pid);
        // Self-healing: the fallback choice should have been written back.
        assert_eq!(
            ui.active_perspective_id("main"),
            board_pid,
            "fallback resolution should persist to UIState so subsequent commands find it"
        );
    }

    #[tokio::test]
    async fn resolve_and_persist_does_not_touch_uistate_when_arg_supplied() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String("arg-id".into()));
        let cmd_ctx = make_full_ctx(
            Arc::clone(&kanban),
            args,
            vec!["window:main".into()],
            Arc::clone(&ui),
        );

        let resolved = resolve_and_persist_perspective_id(&cmd_ctx, &kanban)
            .await
            .unwrap();
        assert_eq!(resolved, "arg-id");
        // Caller-supplied ids must not mutate UIState on the caller's behalf —
        // changing the active perspective is the caller's decision.
        assert_eq!(ui.active_perspective_id("main"), "");
    }

    #[tokio::test]
    async fn resolve_and_persist_does_not_touch_uistate_when_scope_supplied() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        let cmd_ctx = make_full_ctx(
            Arc::clone(&kanban),
            HashMap::new(),
            vec!["perspective:scope-id".into(), "window:main".into()],
            Arc::clone(&ui),
        );

        let resolved = resolve_and_persist_perspective_id(&cmd_ctx, &kanban)
            .await
            .unwrap();
        assert_eq!(resolved, "scope-id");
        // Right-click on a perspective tab shouldn't change the active
        // perspective — it just targets that one perspective for the command.
        assert_eq!(ui.active_perspective_id("main"), "");
    }

    #[tokio::test]
    async fn resolve_perspective_id_errors_when_no_fallback_succeeds() {
        // No arg, no scope-chain perspective, empty UIState, no perspectives
        // registered for the view kind → the resolver must surface the
        // missing-arg error so the caller can report it.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        let cmd_ctx = make_full_ctx(
            Arc::clone(&kanban),
            HashMap::new(),
            vec!["window:main".into()],
            Arc::clone(&ui),
        );

        let err = resolve_perspective_id(&cmd_ctx, &kanban)
            .await
            .expect_err("should error when every fallback fails");
        assert!(matches!(err, CommandError::MissingArg(ref s) if s == "perspective_id"));
    }

    // =========================================================================
    // End-to-end: mutation commands must work from the palette (no args, no
    // scope-chain perspective). These tests reproduce the user-reported bug.
    // =========================================================================

    #[tokio::test]
    async fn clear_filter_works_from_palette_with_no_args() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        // Create a perspective with a filter, then mark it active in UIState.
        let pid = create_perspective_with_view(&kanban, "Active", "board").await;
        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("filter".into(), Value::String("#bug".into()));
        let prep_ctx = make_full_ctx(
            Arc::clone(&kanban),
            args,
            vec!["window:main".into()],
            Arc::clone(&ui),
        );
        SetFilterCmd.execute(&prep_ctx).await.unwrap();

        ui.set_active_perspective("main", &pid);

        // Palette invocation: empty args, no perspective moniker.
        let palette_ctx = make_full_ctx(
            Arc::clone(&kanban),
            HashMap::new(),
            vec!["window:main".into()],
            Arc::clone(&ui),
        );
        let result = ClearFilterCmd.execute(&palette_ctx).await;
        assert!(
            result.is_ok(),
            "ClearFilterCmd must succeed from the palette, got: {result:?}"
        );

        // The filter should now be cleared on the persisted perspective.
        let pctx = kanban.perspective_context().await.unwrap();
        let pctx = pctx.read().await;
        let p = pctx.get_by_id(&pid).expect("perspective still exists");
        assert!(
            p.filter.is_none() || p.filter.as_deref() == Some(""),
            "filter should be cleared, got: {:?}",
            p.filter
        );
    }

    #[tokio::test]
    async fn clear_group_works_from_palette_with_no_args() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        let pid = create_perspective_with_view(&kanban, "Active", "board").await;
        // Seed a group value via SetGroupCmd (explicit arg, full scope).
        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("group".into(), Value::String("status".into()));
        let prep_ctx = make_full_ctx(
            Arc::clone(&kanban),
            args,
            vec!["window:main".into()],
            Arc::clone(&ui),
        );
        SetGroupCmd.execute(&prep_ctx).await.unwrap();

        ui.set_active_perspective("main", &pid);

        // Palette invocation of clearGroup.
        let palette_ctx = make_full_ctx(
            Arc::clone(&kanban),
            HashMap::new(),
            vec!["window:main".into()],
            Arc::clone(&ui),
        );
        ClearGroupCmd.execute(&palette_ctx).await.unwrap();

        let pctx = kanban.perspective_context().await.unwrap();
        let pctx = pctx.read().await;
        let p = pctx.get_by_id(&pid).expect("perspective still exists");
        assert!(
            p.group.is_none() || p.group.as_deref() == Some(""),
            "group should be cleared, got: {:?}",
            p.group
        );
    }

    #[tokio::test]
    async fn toggle_sort_works_from_palette_with_no_perspective_arg() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_commands::UIState::new());

        let pid = create_perspective_with_view(&kanban, "Active", "board").await;
        ui.set_active_perspective("main", &pid);

        // Palette-style: only field arg, no perspective_id, no scope perspective.
        let mut args = HashMap::new();
        args.insert("field".into(), Value::String("priority".into()));
        let palette_ctx = make_full_ctx(
            Arc::clone(&kanban),
            args,
            vec!["window:main".into()],
            Arc::clone(&ui),
        );

        let result = ToggleSortCmd.execute(&palette_ctx).await.unwrap();
        let sort = result["sort"].as_array().unwrap();
        assert_eq!(sort.len(), 1);
        assert_eq!(sort[0]["field"], "priority");
        assert_eq!(sort[0]["direction"], "asc");
    }
}
