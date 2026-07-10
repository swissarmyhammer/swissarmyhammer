//! Perspective-related command implementations.
//!
//! Commands for loading, saving, deleting perspectives and for updating
//! filter/group settings on an active perspective.

use super::run_op;
use crate::commands_core::{Command, CommandContext, CommandError};
use crate::context::KanbanContext;
use crate::perspective::{
    AddPerspective, DeletePerspective, GetPerspective, ListPerspectives, Perspective,
    RenamePerspective, SortDirection, SortEntry, UpdatePerspective,
};
use crate::task_helpers::{enrich_all_task_entities, filter_task_ids, EntitySlugRegistry};
use crate::virtual_tags::default_virtual_tag_registry;
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_filter_expr;

/// Decide whether a perspective belongs to the currently active view.
///
/// New, view-id-scoped perspectives match strictly by id; legacy perspectives
/// (those without `view_id`) keep their pre-existing "shared-by-kind" behavior
/// so existing YAML files do not lose visibility when this scoping rule lands.
///
/// The three cases:
///
/// 1. Perspective has `view_id: Some(pid)` and caller knows the active view id
///    (`active_view_id: Some(active)`) — match strictly: `pid == active`.
/// 2. Perspective has `view_id: None` — legacy shared-by-kind: match by
///    `p.view == active_view_kind`. Holds regardless of whether the caller
///    supplied `active_view_id`.
/// 3. Perspective has `view_id: Some(_)` but the caller cannot resolve an
///    active view id (`active_view_id: None`) — scoped perspectives must not
///    leak out of their view, so this returns `false`.
pub(crate) fn perspective_belongs_to_active_view(
    p: &Perspective,
    active_view_id: Option<&str>,
    active_view_kind: &str,
) -> bool {
    match (&p.view_id, active_view_id) {
        (Some(pid), Some(active)) => pid == active,
        (None, _) => p.view == active_view_kind,
        (Some(_), None) => false,
    }
}

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
/// from resolver-chosen ids (UiState active, or first-perspective-for-view
/// fallback). When the resolver picks the id itself, the caller should
/// persist the choice by writing it back to [`UiState::set_active_perspective`]
/// so subsequent palette invocations find a non-empty active id — making the
/// fallback self-healing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedFrom {
    /// The id came from `args.perspective_id`.
    Arg,
    /// The id came from a `perspective:{id}` moniker in the scope chain.
    Scope,
    /// The id came from `UiState.active_perspective_id(window_label)`.
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
/// 3. `UiState.active_perspective_id(window_label)` for the current window
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
/// [`UiState`] so subsequent commands find it set.
async fn resolve_perspective_id(
    ctx: &CommandContext,
    kanban: &KanbanContext,
) -> crate::commands_core::Result<(String, ResolvedFrom)> {
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
/// Walks the four fallbacks (arg → scope → UiState → first-for-view-kind)
/// and returns the first hit. Split out so the wrapper stays short enough
/// to pass the `code-quality:function-length` validator while the single
/// tracing line in the wrapper still records every resolved call.
async fn resolve_perspective_id_inner(
    ctx: &CommandContext,
    kanban: &KanbanContext,
) -> crate::commands_core::Result<(String, ResolvedFrom)> {
    if let Some(id) = ctx.arg("perspective_id").and_then(|v| v.as_str()) {
        return Ok((id.to_string(), ResolvedFrom::Arg));
    }
    if let Some(id) = ctx.resolve_entity_id("perspective") {
        return Ok((id.to_string(), ResolvedFrom::Scope));
    }
    let window_label = ctx.window_label_required()?;
    if let Some(ui) = ctx.ui_state.as_ref() {
        let active = ui.active_perspective_id(window_label);
        if !active.is_empty() {
            return Ok((active, ResolvedFrom::UiState));
        }
    }
    let (view_kind, view_id) = resolve_active_view(ctx, kanban).await;
    let pctx = kanban
        .perspective_context()
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
    let pctx = pctx.read().await;
    pctx.all()
        .iter()
        .find(|p| perspective_belongs_to_active_view(p, view_id.as_deref(), &view_kind))
        .map(|p| (p.id.clone(), ResolvedFrom::FirstForViewKind))
        .ok_or_else(|| CommandError::MissingArg("perspective_id".into()))
}

/// Persist the resolved perspective id back to [`UiState`] when the resolver
/// chose it (rather than accepting a caller-supplied id).
///
/// This makes the fallback self-healing: after the first palette/keybinding
/// invocation, [`UiState::active_perspective_id`] is non-empty, so subsequent
/// commands hit path 3 instead of walking the full resolver chain.
///
/// [`UiState::set_active_perspective`] is idempotent — it returns `None`
/// (and skips the save) when the value is unchanged — so this is safe to
/// call unconditionally within the fallback branches.
fn persist_resolved_perspective_id(
    ctx: &CommandContext,
    perspective_id: &str,
    resolved_from: ResolvedFrom,
) {
    match resolved_from {
        ResolvedFrom::Arg | ResolvedFrom::Scope => {
            // The caller supplied the id; don't mutate UiState on their behalf.
        }
        ResolvedFrom::UiState | ResolvedFrom::FirstForViewKind => {
            // Persist only when a real window is in scope. No `window:` moniker
            // means we cannot tell which window's selection to record — skip the
            // self-healing persist rather than write it against a hardcoded
            // "main" that could bleed into the wrong window.
            if let (Some(ui), Some(window_label)) =
                (ctx.ui_state.as_ref(), ctx.window_label_from_scope())
            {
                ui.set_active_perspective(window_label, perspective_id);
            }
        }
    }
}

/// Resolve the perspective id and persist it to [`UiState`] when the resolver
/// chose the id via fallback. Returns the id as a [`String`].
///
/// This is the combined form used by every mutation command that targets
/// "the current perspective". Use the split form
/// ([`resolve_perspective_id`] + [`persist_resolved_perspective_id`]) only
/// when you need to inspect [`ResolvedFrom`] for branching logic.
async fn resolve_and_persist_perspective_id(
    ctx: &CommandContext,
    kanban: &KanbanContext,
) -> crate::commands_core::Result<String> {
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

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
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
/// Optional args: `name` (falls back to `"Untitled"` when missing or empty),
/// `view` (falls back to the active view's kind, or `"board"`), `view_id`
/// (falls back to the scope-chain `view:` moniker), `filter`, `group`.
///
/// `view_id` and `view` are both auto-resolved from the scope chain when
/// the args bag does not supply them — mirroring the `from: scope_chain`
/// YAML annotation on the `view_id` param. The registry-rendered
/// `<CommandButton>` popover only collects `name`; the scope chain that
/// `<BarRegistryTabButtons>` builds (`view:<id>`, `board:<id>`, …) carries
/// the active view-instance id, and the views registry maps that id to
/// the view's kind. Without this fallback the dispatcher would lose the
/// per-view-id scoping the prior epic introduced (card
/// `01KRE21GJMPP289N1HSTMJG5HE` review finding).
///
/// `available()` is always `true` so the registry-rendered tab-button
/// (`tab_button: { icon: plus }` on the YAML entry) emits regardless of
/// whether `name` is pre-supplied. The dispatcher's empty-name fallback
/// mirrors the frontend's `generateUntitledName`
/// (`apps/kanban-app/ui/src/components/perspective-tab-bar.tsx`, the `+`
/// immediate-create flow) — see [`first_free_untitled_name`] for the
/// shared convention and its drift pin.
pub struct SavePerspectiveCmd;

#[async_trait]
impl Command for SavePerspectiveCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        // Empty / missing `name` falls back to the first free "Untitled" /
        // "Untitled N" slot so every entry point (palette, keybind, tab
        // button, etc.) gets the same defaulting behavior when no name is
        // supplied.
        let supplied_name = ctx
            .arg("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());

        // Resolve `view_id` and `view` (kind) from args first, then fall
        // back to the scope chain's `view:` moniker — looked up against
        // the views registry for the kind. The YAML declares the
        // `view_id` param with `from: scope_chain, entity_type: view`,
        // but the dispatcher has no automatic scope-chain-to-args
        // injection pass, so the fallback lives here. See
        // `resolve_active_view` for the shared resolver (same pattern
        // used by `cycle_perspective`).
        let explicit_view_arg = ctx.arg("view").and_then(|v| v.as_str()).map(str::to_string);
        let (resolved_view_kind, resolved_view_id) = resolve_active_view(ctx, &kanban).await;
        let view_id = ctx
            .arg("view_id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or(resolved_view_id);
        let view = explicit_view_arg.unwrap_or(resolved_view_kind);

        let filter = ctx.arg("filter").and_then(|v| v.as_str()).map(String::from);
        let group = ctx.arg("group").and_then(|v| v.as_str()).map(String::from);

        if let Some(ref f) = filter {
            validate_filter(f)?;
        }

        // Idempotent "ensure" path used by the frontend auto-create of the
        // "Default" perspective. The frontend list it guards on can be
        // transiently empty on a hot reload / boot race (the provider remounts
        // with an empty list and its per-kind ref reset, then fires before the
        // refetch lands), which previously created a fresh duplicate "Default"
        // YAML on every reload. `if_absent` maps onto `AddPerspective::ensure`,
        // the STORAGE-layer guard: an existing perspective for this view scope
        // is returned without a write (so no store-changed notification, no
        // refetch loop re-trigger), and a genuine create lands under the
        // deterministic `default-<scope>` id so stale-cache races in other
        // windows or sibling processes converge on one file instead of
        // accumulating duplicates.
        let ensure = ctx
            .arg("if_absent")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let name = match supplied_name {
            Some(n) => n.to_string(),
            None => generate_untitled_name(&kanban, &view, view_id.as_deref()).await?,
        };

        let mut add_op = AddPerspective::new(name, &view);
        add_op.view_id = view_id;
        add_op.filter = filter;
        add_op.group = group;
        add_op.ensure = ensure;

        run_op(&add_op, &kanban).await
    }
}

/// Generate a unique "Untitled" / "Untitled N" name for a perspective
/// missing an explicit `name` arg.
///
/// Scopes the taken-name set to the perspectives sharing this view (matched
/// by `view_id` when present, else by view kind — the same
/// view_id-first / kind-fallback rule on `PerspectiveDef`), then picks the
/// first free slot via [`first_free_untitled_name`].
///
/// Reads the perspective list once under the perspective context's read
/// lock; the caller drops the lock before the eventual write through
/// `AddPerspective` so the two operations don't deadlock.
async fn generate_untitled_name(
    kanban: &KanbanContext,
    view: &str,
    view_id: Option<&str>,
) -> crate::commands_core::Result<String> {
    let pctx = kanban
        .perspective_context()
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
    let pctx = pctx.read().await;
    Ok(first_free_untitled_name(
        pctx.all()
            .iter()
            .filter(|p| match (view_id, p.view_id.as_deref()) {
                (Some(vid), Some(pvid)) => vid == pvid,
                // Either side without a view_id falls back to view-kind
                // match — same rule as the frontend's perspective filter.
                _ => p.view == view,
            })
            .map(|p| p.name.as_str()),
    ))
}

/// Pick the first free "Untitled" / "Untitled N" slot given the names
/// already taken in the view scope.
///
/// Cross-language mirror of `generateUntitledName` in
/// `apps/kanban-app/ui/src/components/perspective-tab-bar.tsx` — ONE
/// convention, pinned on both sides by lockstep drift-guard tests
/// (`first_free_untitled_name_matches_the_frontend_generator` here,
/// `generates_the_first_free_untitled_slot` there).
///
/// Scanning for the first free slot by EXACT name match (rather than
/// counting `Untitled`-prefixed names) guarantees the generated name never
/// collides with an existing perspective — a count re-mints an EXISTING
/// name when the sequence has gaps (e.g. "Untitled" deleted, "Untitled 2"
/// surviving), and a prefix count also miscounts user names like
/// "Untitled tasks".
fn first_free_untitled_name<'a>(taken: impl Iterator<Item = &'a str>) -> String {
    let taken: std::collections::HashSet<&str> = taken.collect();
    if !taken.contains("Untitled") {
        return "Untitled".to_string();
    }
    let mut n = 2u64;
    loop {
        let candidate = format!("Untitled {n}");
        if !taken.contains(candidate.as_str()) {
            return candidate;
        }
        n += 1;
    }
}

/// Delete a perspective by name or scope chain.
///
/// Accepts `name` arg (the perspective name or ID), or resolves the
/// perspective ID from the scope chain moniker `perspective:{id}`.
///
/// When the dispatch carries a [`UiState`](swissarmyhammer_ui_state::UiState)
/// (the board-bundle `entity` server's wiring) and the deleted perspective was
/// the dispatching window's ACTIVE selection, selection falls back to a
/// surviving perspective so the tab bar is never left pointing at a
/// just-deleted id (the "empty bar" the never-zero invariant forbids). The
/// fallback is the first perspective still belonging to the active view; when
/// none survive the active id is cleared. The command-layer dispatch (no
/// UiState) skips the reselect — there the delete is purely a storage mutation.
pub struct DeletePerspectiveCmd;

#[async_trait]
impl Command for DeletePerspectiveCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.arg("name").and_then(|v| v.as_str()).is_some() || ctx.has_in_scope("perspective")
    }

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let id = Self::resolve_delete_target(ctx, &kanban).await?;

        let op = DeletePerspective::new(id.clone());
        let mut result = run_op(&op, &kanban).await?;

        // If the deleted perspective was the dispatching window's active
        // selection, re-select a survivor so the tab bar never points at a
        // dangling id. Only the UiState-bearing dispatch (the `entity` server)
        // can do this; the command-layer dispatch has no UiState and skips it
        // (the reselect returns `Null`). FORWARD the reselect's
        // `PerspectiveSwitch` change on the result so the UiState-bearing
        // caller can surface it where the host's `ui-state-changed` emit
        // unwraps the selection — the backend write would otherwise be
        // invisible to the UI.
        let change = Self::reselect_after_delete(ctx, &kanban, &id).await?;
        if let Value::Object(map) = &mut result {
            map.insert("change".to_string(), change);
        }

        Ok(result)
    }
}

impl DeletePerspectiveCmd {
    /// Resolve the perspective id to delete: explicit `name` arg (resolved by
    /// name then id) wins, else the `perspective:{id}` scope-chain moniker.
    async fn resolve_delete_target(
        ctx: &CommandContext,
        kanban: &KanbanContext,
    ) -> crate::commands_core::Result<String> {
        if let Some(name) = ctx.arg("name").and_then(|v| v.as_str()) {
            let pctx = kanban
                .perspective_context()
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
            let pctx = pctx.read().await;
            if let Some(p) = pctx.get_by_name(name) {
                Ok(p.id.to_string())
            } else if pctx.get_by_id(name).is_some() {
                Ok(name.to_string())
            } else {
                Err(CommandError::ExecutionFailed(format!(
                    "perspective not found: {name}"
                )))
            }
        } else if let Some(scope_id) = ctx.resolve_entity_id("perspective") {
            Ok(scope_id.to_string())
        } else {
            Err(CommandError::MissingArg("name".into()))
        }
    }

    /// After a successful delete, re-point the window's active perspective at a
    /// survivor when the deleted id was the active one.
    ///
    /// Returns the reselection's `UiStateChange::PerspectiveSwitch` (as JSON)
    /// when a survivor was activated, so the caller can FORWARD it on the
    /// result envelope and the host's `ui-state-changed` emit fires for the
    /// new selection — the same `change` contract switch/next/prev ride.
    /// Returns `Value::Null` when there is nothing to forward: no
    /// [`UiState`](swissarmyhammer_ui_state::UiState) in scope, the deleted
    /// perspective was not the window's active selection, or no survivor
    /// existed (the active id is cleared, which carries no switch change).
    ///
    /// The survivor is the first perspective still belonging to the active
    /// view (id-scoped strictly, legacy by kind — see
    /// [`perspective_belongs_to_active_view`]); when none survive, the active
    /// id is cleared so the bar shows no stale selection.
    async fn reselect_after_delete(
        ctx: &CommandContext,
        kanban: &KanbanContext,
        deleted_id: &str,
    ) -> crate::commands_core::Result<Value> {
        let Some(ui) = ctx.ui_state.as_ref() else {
            return Ok(Value::Null);
        };
        let window_label = ctx.window_label_required()?;
        if ui.active_perspective_id(window_label) != deleted_id {
            return Ok(Value::Null);
        }

        let (view_kind, view_id) = resolve_active_view(ctx, kanban).await;
        let survivor = {
            let pctx = kanban
                .perspective_context()
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
            let pctx = pctx.read().await;
            pctx.all()
                .iter()
                .find(|p| perspective_belongs_to_active_view(p, view_id.as_deref(), &view_kind))
                .map(|p| p.id.clone())
        };

        match survivor {
            Some(new_id) => {
                // Forward the switch change so the UI's emit fires for the
                // new selection — not just the server-side write.
                switch_to_perspective(kanban, ui, window_label, &new_id).await
            }
            None => {
                // No survivor for this view — clear the dangling active id so
                // the bar shows no stale selection. There is no switch change
                // to forward (the never-zero recovery is an external concern).
                ui.set_active_perspective(window_label, "");
                Ok(Value::Null)
            }
        }
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

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
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
/// [`resolve_perspective_id`] (explicit arg → scope-chain moniker → UiState
/// active → first perspective for the active view kind). See
/// [`ClearFilterCmd`] for the same resolution semantics.
pub struct SetFilterCmd;

#[async_trait]
impl Command for SetFilterCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
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

/// Set a perspective's filter AND refresh the dispatching window when the
/// edited perspective is its active selection.
///
/// The event-driven counterpart to [`SetFilterCmd`]. `SetFilterCmd` writes
/// STORAGE only (`UpdatePerspective`) and returns the op JSON — that is the
/// resolution-only path the `views` server rides, which never recomputes
/// `filtered_task_ids` and never produces a `{ok,change}` the host can emit
/// as `ui-state-changed`. That is exactly why a filter edit used to require a
/// click-away/back to take effect: only the subsequent `perspective.switch`
/// re-evaluated the filter (card 01KV0MJYA58GW5PRXGVXWHQK32 — the same dead
/// `views` path switch/next/prev and delete were moved off of in cards
/// 01KTYQY0ZB62KHN6BPK3FBMBD7 / 01KTYVSA68WDFGXCEJ44T4VFNW).
///
/// This command lives on the board-bundle `entity` server (the only module
/// holding BOTH the [`KanbanContext`] and the [`UiState`]). It:
///
/// 1. Resolves + persists the target perspective id and validates the filter,
///    then writes the new filter to STORAGE via `UpdatePerspective` (same as
///    [`SetFilterCmd`]).
/// 2. If the edited perspective is the dispatching window's *active*
///    selection, re-runs the shared [`switch_to_perspective`] pipeline — which
///    re-reads the perspective's now-updated filter, re-evaluates it via
///    [`evaluate_perspective_filter`], and writes the window's
///    `filtered_task_ids` atomically via `UiState::switch_perspective`,
///    returning the resulting [`UiStateChange::PerspectiveSwitch`].
///    Otherwise it returns `Value::Null`: storage is updated but no window
///    needs refreshing.
///
/// Required args: `perspective_id` (resolved via [`resolve_perspective_id`]
/// when omitted) and `filter`. The `window:<label>` moniker in the scope chain
/// selects the window whose active-perspective comparison drives the refresh.
///
/// [`UiStateChange::PerspectiveSwitch`]: swissarmyhammer_ui_state::UiStateChange::PerspectiveSwitch
pub struct SetFilterAndRefreshCmd;

#[async_trait]
impl Command for SetFilterAndRefreshCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UiState not available".into()))?;

        let perspective_id = resolve_and_persist_perspective_id(ctx, &kanban).await?;

        let filter = ctx
            .arg("filter")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("filter".into()))?;

        validate_filter(filter)?;

        // Persist the new filter to storage (same write SetFilterCmd makes).
        let op = UpdatePerspective::new(&perspective_id).with_filter(Some(filter.to_string()));
        run_op(&op, &kanban).await?;

        // Drive the window refresh only when the edited perspective IS the
        // dispatching window's active selection — re-using the shared switch
        // pipeline so it re-reads the just-written filter and re-evaluates it.
        let window_label = ctx.window_label_required()?;
        if ui.active_perspective_id(window_label) == perspective_id {
            switch_to_perspective(&kanban, ui, window_label, &perspective_id).await
        } else {
            Ok(Value::Null)
        }
    }
}

/// Clear the filter on an active perspective.
///
/// Always available. The target perspective is resolved at execute time via
/// [`resolve_perspective_id`] (explicit arg → scope-chain moniker → UiState
/// active → first perspective for the active view kind).
pub struct ClearFilterCmd;

#[async_trait]
impl Command for ClearFilterCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let perspective_id = resolve_and_persist_perspective_id(ctx, &kanban).await?;

        let op = UpdatePerspective::new(&perspective_id).with_filter(None);
        run_op(&op, &kanban).await
    }
}

/// Set the group on an active perspective.
///
/// Always available. The target perspective is resolved at execute time via
/// [`resolve_perspective_id`] (explicit arg → scope-chain moniker → UiState
/// active → first perspective for the active view kind).
pub struct SetGroupCmd;

#[async_trait]
impl Command for SetGroupCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
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
/// [`resolve_perspective_id`] (explicit arg → scope-chain moniker → UiState
/// active → first perspective for the active view kind).
pub struct ClearGroupCmd;

#[async_trait]
impl Command for ClearGroupCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
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

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
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

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
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

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
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

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
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

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        cycle_perspective(ctx, CycleDirection::Prev).await
    }
}

/// Resolve the active view kind plus the active view id (when knowable).
///
/// Returns `(kind, view_id)`:
///
/// - `kind` is the active view kind as a lower-case string (e.g. `"board"`,
///   `"grid"`). Resolution order matches the legacy `resolve_view_kind`:
///   explicit `view_kind` arg > scope chain `view:{id}` looked up against the
///   views registry > `"board"` default.
/// - `view_id` is the active view *instance* id when one can be resolved.
///   Resolution order: explicit `view_id` arg > scope chain `view:{id}`
///   moniker. Returns `None` when neither source supplies one — callers
///   should treat that as "no scoped perspective leaks" (see
///   [`perspective_belongs_to_active_view`]).
///
/// Splitting kind and id is necessary because perspectives now scope by id
/// when present, and by kind otherwise. See task 01KRC1DRWA3PFC7NFX4WVF3DD8.
async fn resolve_active_view(
    ctx: &CommandContext,
    kanban: &KanbanContext,
) -> (String, Option<String>) {
    let explicit_kind = ctx.arg("view_kind").and_then(|v| v.as_str());
    let explicit_id = ctx
        .arg("view_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let scope_view_id = ctx
        .scope_chain
        .iter()
        .find_map(|m| m.strip_prefix("view:"))
        .map(str::to_string);

    let view_id = explicit_id.or(scope_view_id);

    let kind = if let Some(explicit) = explicit_kind {
        explicit.to_string()
    } else if let Some(kind) = resolve_kind_from_view_id(view_id.as_deref(), kanban).await {
        kind
    } else {
        "board".to_string()
    };

    (kind, view_id)
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
    Some(view_def.kind.as_kebab_str().to_string())
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
/// (wrapping). Updates UiState and returns the `UiStateChange`, or `null`
/// if cycling is not possible (fewer than 2 matching perspectives).
async fn cycle_perspective(
    ctx: &CommandContext,
    direction: CycleDirection,
) -> crate::commands_core::Result<Value> {
    let kanban = ctx.require_extension::<KanbanContext>()?;
    let ui = ctx
        .ui_state
        .as_ref()
        .ok_or_else(|| CommandError::ExecutionFailed("UiState not available".into()))?;

    let (view_kind, view_id) = resolve_active_view(ctx, &kanban).await;
    let window_label = ctx.window_label_required()?;
    let current_id = ui.active_perspective_id(window_label);

    // Get perspectives matching the requested view (by id when set, else kind).
    let matching: Vec<String> = {
        let pctx = kanban
            .perspective_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        let pctx = pctx.read().await;
        pctx.all()
            .iter()
            .filter(|p| perspective_belongs_to_active_view(p, view_id.as_deref(), &view_kind))
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
    // Full atomic switch — evaluate the target's filter and write both
    // per-window slots, exactly like `perspective.switch`. An id-only
    // `set_active_perspective` here would leave `filtered_task_ids` stale,
    // so the board would keep showing the PREVIOUS perspective's tasks
    // after a next/prev cycle.
    switch_to_perspective(&kanban, ui, window_label, new_id).await
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

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UiState not available".into()))?;

        let id = ctx.require_arg_str("id")?;
        let view_kind = ctx.arg("view_kind").and_then(|v| v.as_str());
        let view_id = ctx.arg("view_id").and_then(|v| v.as_str());
        let window_label = ctx.window_label_required()?;

        // Validate the perspective exists.
        let pctx = kanban
            .perspective_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        let pctx = pctx.read().await;

        let perspective = pctx
            .get_by_id(id)
            .ok_or_else(|| CommandError::ExecutionFailed(format!("perspective not found: {id}")))?;

        // If view_kind is specified, validate the perspective belongs to the
        // active view — id-scoped perspectives match strictly by id, legacy
        // ones fall back to kind. See `perspective_belongs_to_active_view`.
        if let Some(expected_kind) = view_kind {
            if !perspective_belongs_to_active_view(perspective, view_id, expected_kind) {
                return Err(CommandError::ExecutionFailed(format!(
                    "perspective '{}' (view='{}', view_id={:?}) does not belong to active view \
                     (view_kind='{expected_kind}', view_id={view_id:?})",
                    perspective.name, perspective.view, perspective.view_id
                )));
            }
        }

        // Release the lock before mutating UiState.
        drop(pctx);

        let change = ui.set_active_perspective(window_label, id);
        Ok(serde_json::to_value(change).unwrap_or(Value::Null))
    }
}

/// Switch to a perspective AND evaluate its filter in one backend command.
///
/// Replaces the prior `perspective.set` (and its predecessor
/// `ui.perspective.set`) which only mutated `UiState.active_perspective_id`.
/// That left filter evaluation to a follow-up `list_entities(filter=...)`
/// roundtrip driven by a frontend `useEffect`; clicking a tab dispatched a
/// command with no backend work, so the indeterminate progress bar tied to
/// `inflightCount` never fired for the real filtering cost.
///
/// `SwitchPerspectiveCmd` collapses that into one atomic step:
///
/// 1. Look up the perspective by id (errors cleanly when unknown).
/// 2. Load the board's tasks, enrich them (filter_tags + progress), and
///    evaluate the perspective's filter DSL against them via the local
///    [`evaluate_perspective_filter`] helper, which in turn delegates to
///    `filter_task_ids` in `task_helpers` so the DSL evaluator is the
///    same one `list_entities` uses — no duplication.
/// 3. Atomically write BOTH `active_perspective_id` and `filtered_task_ids`
///    on the window via [`UiState::switch_perspective`], producing exactly
///    one `UiStateChange::PerspectiveSwitch`. The frontend renders the
///    filtered task list directly out of UiState.
///
/// Required arg: `perspective_id`. Returns the [`UiStateChange`] as JSON,
/// or `Value::Null` when the switch is a no-op (id and filtered id list
/// both already match).
pub struct SwitchPerspectiveCmd;

#[async_trait]
impl Command for SwitchPerspectiveCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UiState not available".into()))?;

        let perspective_id = ctx.require_arg_str("perspective_id")?;
        let window_label = ctx.window_label_required()?;

        switch_to_perspective(&kanban, ui, window_label, perspective_id).await
    }
}

/// Switch a window's active perspective: look the perspective up, evaluate
/// its filter DSL, and atomically write BOTH `active_perspective_id` and
/// `filtered_task_ids` via [`UiState::switch_perspective`].
///
/// The single switch pipeline shared by [`SwitchPerspectiveCmd`] (switch by
/// explicit id) and [`cycle_perspective`] (next/prev) — cycling re-filters
/// the window exactly like a direct switch, producing the same atomic
/// `UiStateChange::PerspectiveSwitch`. Returns that change as JSON, or
/// `Value::Null` when the switch is a no-op.
///
/// [`UiState::switch_perspective`]: swissarmyhammer_ui_state::UiState::switch_perspective
pub async fn switch_to_perspective(
    kanban: &KanbanContext,
    ui: &swissarmyhammer_ui_state::UiState,
    window_label: &str,
    perspective_id: &str,
) -> crate::commands_core::Result<Value> {
    // Look up the perspective + capture its filter. Drop the read guard
    // before doing the (potentially long) filter evaluation so a
    // concurrent perspective mutation does not block on us.
    let filter = {
        let pctx = kanban
            .perspective_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        let pctx = pctx.read().await;
        let perspective = pctx.get_by_id(perspective_id).ok_or_else(|| {
            CommandError::ExecutionFailed(format!("perspective not found: {perspective_id}"))
        })?;
        perspective.filter.clone().unwrap_or_default()
    };

    let filtered_task_ids = evaluate_perspective_filter(kanban, filter.as_str()).await?;

    let change = ui.switch_perspective(window_label, perspective_id, filtered_task_ids);
    Ok(serde_json::to_value(change).unwrap_or(Value::Null))
}

/// Evaluate a perspective's filter DSL against the board's tasks and return
/// the matching task ids.
///
/// Reuses the same shared filter pipeline `list_entities` and `list_tasks`
/// drive — load tasks + columns + projects + actors, enrich tasks so
/// `#tag` / `@user` predicates resolve, build the slug registry, then
/// delegate to [`filter_task_ids`]. An empty / whitespace-only filter
/// returns every task id (no filter).
///
/// Public so the entity-cache bridge (`kanban-app`) can recompute a window's
/// `filtered_task_ids` on a live task change using the *same* DSL evaluator
/// `perspective.switch` uses — one pipeline, no divergence.
pub async fn evaluate_perspective_filter(
    kanban: &KanbanContext,
    filter: &str,
) -> crate::commands_core::Result<Vec<String>> {
    let ectx = kanban
        .entity_context()
        .await
        .map_err(|e| CommandError::ExecutionFailed(format!("entity_context: {e}")))?;

    let mut tasks = ectx
        .list("task")
        .await
        .map_err(|e| CommandError::ExecutionFailed(format!("list(task): {e}")))?;
    let columns = ectx
        .list("column")
        .await
        .map_err(|e| CommandError::ExecutionFailed(format!("list(column): {e}")))?;
    let projects = ectx
        .list("project")
        .await
        .map_err(|e| CommandError::ExecutionFailed(format!("list(project): {e}")))?;
    let actors = ectx
        .list("actor")
        .await
        .map_err(|e| CommandError::ExecutionFailed(format!("list(actor): {e}")))?;

    let terminal_column = columns
        .iter()
        .max_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
        .map(|c| c.id.as_str())
        .unwrap_or("done");

    let virtual_tag_registry = default_virtual_tag_registry();
    enrich_all_task_entities(&mut tasks, terminal_column, virtual_tag_registry);

    let slug_registry = EntitySlugRegistry::build(&projects, &actors, &tasks);
    filter_task_ids(&tasks, filter, &slug_registry).map_err(CommandError::ExecutionFailed)
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

    async fn execute(&self, ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let op = ListPerspectives::new();
        run_op(&op, &kanban).await
    }
}

/// UI-only marker for `perspective.filter.focus`.
///
/// The YAML entry declares `tab_button: { icon: filter }` so the
/// registry-driven `<RegistryTabButtons>` slot renders a Filter icon on
/// the active perspective's tab, and the `isActive` highlight is read
/// from the perspective's `filter` field. But the click itself does NOT
/// route through this backend command — `<FilterFocusCommandButton>`
/// (in `kanban-app/ui/src/components/perspective-tab-bar.tsx`)
/// overrides the dispatch to issue `nav.focus({ args: { fq } })`
/// against the formula bar's `filter_editor:${id}` spatial-nav scope.
///
/// This `execute` is therefore a deliberate no-op — it exists only to
/// satisfy the YAML ↔ Rust completeness invariant enforced by
/// `test_all_yaml_commands_have_rust_implementations` /
/// `test_no_orphan_rust_commands_without_yaml` in `commands/mod.rs`.
/// Reachable today only via palette / keybinding paths that would also
/// be funneled through `nav.focus` once those surfaces migrate; until
/// then a no-op is the correct behaviour (silently nothing happens, no
/// state mutation, no broadcast).
///
/// Refactor history: card `01KRGZY33P99J7CGG0XRQGZ352` replaced the
/// prior `FocusFilter` marker-envelope + `ui.focus.filter` Tauri event
/// channel with the `nav.focus` flow described above. The marker
/// envelope and the dispatcher's `handle_focus_filter` were deleted in
/// the same commit.
pub struct FocusFilterCmd;

#[async_trait]
impl Command for FocusFilterCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, _ctx: &CommandContext) -> crate::commands_core::Result<Value> {
        // UI-only command — focus claims flow through frontend
        // `nav.focus` (see this struct's doc comment). Returning `null`
        // signals "nothing for the dispatcher to do".
        Ok(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::context::KanbanContext;
    use crate::test_support::default_window_moniker;
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

    /// Build a CommandContext for a perspective command test.
    ///
    /// One parameterized helper for every test shape:
    /// - `scope_chain: None` — seed only the default `window:` moniker.
    /// - `scope_chain: Some(chain)` — use `chain`, appending the default
    ///   `window:` moniker if it carries none (per-window ops resolve a
    ///   window — they no longer fall back to a silent "main").
    /// - `ui: Some(state)` — attach the UiState the per-window ops mutate.
    fn make_ctx(
        kanban: Arc<KanbanContext>,
        args: HashMap<String, Value>,
        scope_chain: Option<Vec<String>>,
        ui: Option<Arc<swissarmyhammer_ui_state::UiState>>,
    ) -> CommandContext {
        let scope_chain = with_default_window(scope_chain.unwrap_or_default());
        let mut ctx = CommandContext::new("test", scope_chain, None, args);
        ctx.set_extension(kanban);
        if let Some(ui) = ui {
            ctx.ui_state = Some(ui);
        }
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
        let cmd_ctx = make_ctx(Arc::clone(kanban), args, None, None);
        let result = SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        result["id"].as_str().unwrap().to_string()
    }

    /// Helper: create a perspective pinned to a specific view instance (`view_id`).
    async fn create_perspective_scoped(
        kanban: &Arc<KanbanContext>,
        name: &str,
        view: &str,
        view_id: &str,
    ) -> String {
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String(name.into()));
        args.insert("view".into(), Value::String(view.into()));
        args.insert("view_id".into(), Value::String(view_id.into()));
        let cmd_ctx = make_ctx(Arc::clone(kanban), args, None, None);
        let result = SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        result["id"].as_str().unwrap().to_string()
    }

    /// Helper: create a perspective and return its ID.
    async fn create_perspective(kanban: &Arc<KanbanContext>, name: &str) -> String {
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String(name.into()));
        args.insert("view".into(), Value::String("grid".into()));
        let cmd_ctx = make_ctx(Arc::clone(kanban), args, None, None);
        let result = SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        result["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn test_list_perspectives_cmd_empty() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let cmd_ctx = make_ctx(Arc::clone(&kanban), HashMap::new(), None, None);

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
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, None);
        SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();

        // Now list
        let cmd_ctx = make_ctx(Arc::clone(&kanban), HashMap::new(), None, None);
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
    async fn test_save_if_absent_is_idempotent_for_view_kind() {
        // Regression: hot reload / boot races re-fired the frontend
        // auto-create of the "Default" perspective because the local list it
        // guards on was transiently empty, writing a fresh duplicate YAML
        // every reload. The `if_absent` ensure path short-circuits against the
        // backend's authoritative state, so repeated seeds return the existing
        // perspective instead of creating new ones.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        let seed = |kanban: Arc<KanbanContext>| async move {
            let mut args = HashMap::new();
            args.insert("name".into(), Value::String("Default".into()));
            args.insert("view".into(), Value::String("board".into()));
            args.insert("if_absent".into(), Value::Bool(true));
            let cmd_ctx = make_ctx(kanban, args, None, None);
            SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap()
        };

        // First seed creates the Default.
        let first = seed(Arc::clone(&kanban)).await;
        let first_id = first["id"].as_str().unwrap().to_string();

        // Subsequent seeds return the SAME perspective — no new writes.
        for _ in 0..3 {
            let again = seed(Arc::clone(&kanban)).await;
            assert_eq!(
                again["id"].as_str().unwrap(),
                first_id,
                "if_absent must return the existing perspective, not create a new one",
            );
        }

        // The board holds exactly one perspective for this view kind.
        let cmd_ctx = make_ctx(Arc::clone(&kanban), HashMap::new(), None, None);
        let result = ListPerspectivesCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["count"], 1, "no duplicate Default perspectives");
    }

    #[tokio::test]
    async fn test_save_if_absent_scopes_by_view_id() {
        // `if_absent` matches the existing perspective using the same
        // view_id-first / kind-fallback rule as the rest of the perspective
        // resolution. A seed scoped to a different view_id must still create.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        let seed = |kanban: Arc<KanbanContext>, view_id: &str| {
            let view_id = view_id.to_string();
            async move {
                let mut args = HashMap::new();
                args.insert("name".into(), Value::String("Default".into()));
                args.insert("view".into(), Value::String("board".into()));
                args.insert("view_id".into(), Value::String(view_id));
                args.insert("if_absent".into(), Value::Bool(true));
                let cmd_ctx = make_ctx(kanban, args, None, None);
                SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap()
            }
        };

        let a1 = seed(Arc::clone(&kanban), "view-a").await;
        let a2 = seed(Arc::clone(&kanban), "view-a").await;
        assert_eq!(
            a1["id"].as_str().unwrap(),
            a2["id"].as_str().unwrap(),
            "same view_id must be idempotent",
        );

        let b1 = seed(Arc::clone(&kanban), "view-b").await;
        assert_ne!(
            a1["id"].as_str().unwrap(),
            b1["id"].as_str().unwrap(),
            "a distinct view_id must seed its own perspective",
        );

        let cmd_ctx = make_ctx(Arc::clone(&kanban), HashMap::new(), None, None);
        let result = ListPerspectivesCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["count"], 2, "one Default per distinct view_id");
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
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            args,
            Some(vec![format!("perspective:{pid}")]),
            None,
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
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            args,
            Some(vec![format!("perspective:{pid}")]),
            None,
        );
        SetSortCmd.execute(&cmd_ctx).await.unwrap();

        // Now set desc — should replace, not append
        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("field".into(), Value::String("title".into()));
        args.insert("direction".into(), Value::String("desc".into()));
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            args,
            Some(vec![format!("perspective:{pid}")]),
            None,
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
            let cmd_ctx = make_ctx(
                Arc::clone(&kanban),
                args,
                Some(vec![format!("perspective:{pid}")]),
                None,
            );
            SetSortCmd.execute(&cmd_ctx).await.unwrap();
        }

        // Clear — no `field` arg supplied; the command must drop both entries.
        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            args,
            Some(vec![format!("perspective:{pid}")]),
            None,
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
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            args,
            Some(vec![format!("perspective:{pid}")]),
            None,
        );
        let result = ClearSortCmd.execute(&cmd_ctx).await.unwrap();
        let sort = result["sort"].as_array();
        assert!(sort.is_none() || sort.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_clear_sort_cmd_works_from_palette_with_no_perspective_id() {
        // Palette invocation: no args, no scope perspective moniker.
        // The resolver falls back to UiState.active_perspective_id("main"),
        // and the command must clear the sort list on that perspective.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let pid = create_perspective_with_view(&kanban, "Palette Sort", "grid").await;

        // Mark this perspective as UiState-active for the default window.
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());
        ui.set_active_perspective("main", &pid);

        // Seed a sort entry on it.
        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("field".into(), Value::String("title".into()));
        args.insert("direction".into(), Value::String("asc".into()));
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            args,
            Some(vec![format!("perspective:{pid}")]),
            None,
        );
        SetSortCmd.execute(&cmd_ctx).await.unwrap();

        // Palette dispatch: empty args, no scope moniker — UiState must win.
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            HashMap::new(),
            None,
            Some(Arc::clone(&ui)),
        );
        let result = ClearSortCmd.execute(&cmd_ctx).await.unwrap();
        let sort = result["sort"].as_array();
        assert!(
            sort.is_none() || sort.unwrap().is_empty(),
            "palette dispatch should resolve perspective via UiState and clear sort, got: {:?}",
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
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, Some(scope.clone()), None);
        let result = ToggleSortCmd.execute(&cmd_ctx).await.unwrap();
        let sort = result["sort"].as_array().unwrap();
        assert_eq!(sort.len(), 1);
        assert_eq!(sort[0]["direction"], "asc");

        // Toggle 2: asc → desc
        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("field".into(), Value::String("priority".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, Some(scope.clone()), None);
        let result = ToggleSortCmd.execute(&cmd_ctx).await.unwrap();
        let sort = result["sort"].as_array().unwrap();
        assert_eq!(sort.len(), 1);
        assert_eq!(sort[0]["direction"], "desc");

        // Toggle 3: desc → none
        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        args.insert("field".into(), Value::String("priority".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, Some(scope.clone()), None);
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
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            args,
            Some(vec![format!("perspective:{pid}")]),
            None,
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
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            args,
            Some(vec![format!("perspective:{pid}")]),
            None,
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
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            args,
            Some(vec![format!("perspective:{pid}")]),
            None,
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
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, None);
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
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, None);
        let result = SavePerspectiveCmd.execute(&cmd_ctx).await;
        assert!(result.is_err(), "invalid filter should be rejected on save");
    }

    #[tokio::test]
    async fn test_perspective_mutation_cmds_always_available() {
        // Commands that resolve `perspective_id` at execute time are always
        // available from the palette — scope-chain membership is no longer a
        // gate. See `resolve_perspective_id` for the resolution order.
        // After 01KRE21GJMPP289N1HSTMJG5HE, `SavePerspectiveCmd` is also
        // unconditionally available — the registry-rendered tab-button
        // popover collects the `name` arg at click time, so blocking
        // availability on `name` presence would hide the affordance.
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        assert!(SavePerspectiveCmd.available(&ctx));
        assert!(SetFilterCmd.available(&ctx));
        assert!(ClearFilterCmd.available(&ctx));
        assert!(SetGroupCmd.available(&ctx));
        assert!(ClearGroupCmd.available(&ctx));
        assert!(SetSortCmd.available(&ctx));
        assert!(ClearSortCmd.available(&ctx));
        assert!(ToggleSortCmd.available(&ctx));
    }

    /// `SavePerspectiveCmd::execute` with an empty / missing `name` arg
    /// falls back to generating a `"Untitled"` (or `"Untitled N+1"`) name.
    ///
    /// Pre-migration this fallback lived in the frontend
    /// `<AddPerspectiveButton>`, which inferred the name before
    /// dispatching. The registry-driven tab-button popover collects
    /// `name` from a text input that can be empty; moving the fallback
    /// into the dispatcher means every entry point (palette, keybind,
    /// tab button, etc.) gets the same defaulting behavior.
    ///
    /// Three submissions in this test:
    ///   1. Empty string → first call gets `"Untitled"`.
    ///   2. Missing name arg → second call gets `"Untitled 2"`.
    ///   3. Whitespace-only string → third call gets `"Untitled 3"`.
    #[tokio::test]
    async fn test_save_perspective_cmd_generates_untitled_name_when_empty() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        // 1. Empty string for `name` — fallback fires.
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("".into()));
        args.insert("view".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, None);
        let result = SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            result["name"], "Untitled",
            "empty name must fall back to 'Untitled' on a fresh board"
        );

        // 2. No `name` arg at all — fallback fires AND increments because
        // the previous Untitled is already in the perspective list.
        let mut args = HashMap::new();
        args.insert("view".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, None);
        let result = SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            result["name"], "Untitled 2",
            "missing name with one existing Untitled must increment to 'Untitled 2'"
        );

        // 3. Whitespace-only `name` — the dispatcher trims before
        // checking emptiness, so this also takes the fallback path.
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("   ".into()));
        args.insert("view".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, None);
        let result = SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            result["name"], "Untitled 3",
            "whitespace-only name must be treated as empty and increment to 'Untitled 3'"
        );
    }

    /// Lockstep drift pin for the generated-name convention — mirrored
    /// verbatim by the frontend's `generates_the_first_free_untitled_slot`
    /// in
    /// `apps/kanban-app/ui/src/components/perspective-tab-bar.add-create-rename.test.tsx`.
    /// The two generators (`first_free_untitled_name` here,
    /// `generateUntitledName` there) are cross-language mirrors of ONE
    /// convention: scan for the first free "Untitled" / "Untitled N" slot
    /// by EXACT name match. If either side changes, both tables must change
    /// together.
    #[test]
    fn first_free_untitled_name_matches_the_frontend_generator() {
        let table: &[(&[&str], &str)] = &[
            (&[], "Untitled"),
            (&["Untitled"], "Untitled 2"),
            (&["Untitled", "Untitled 2"], "Untitled 3"),
            // Gap shape: the first free slot is reused, never a colliding
            // re-mint.
            (&["Untitled 2"], "Untitled"),
            // Prefix-only names are user names, not generated slots — no
            // collision.
            (&["Untitled tasks"], "Untitled"),
            (&["Sprint", "Untitled", "Untitled 3"], "Untitled 2"),
        ];
        for (taken, expected) in table {
            assert_eq!(
                first_free_untitled_name(taken.iter().copied()),
                *expected,
                "taken={taken:?}"
            );
        }
    }

    /// The untitled fallback fills the first FREE slot instead of counting
    /// `Untitled`-prefixed names — counting re-mints an EXISTING name when
    /// the sequence has gaps (e.g. "Untitled" was deleted but "Untitled 2"
    /// survives), creating duplicate-named siblings (card
    /// 01KTYN8GB25ZFKSXWA0QA283PG review blocker B1).
    #[tokio::test]
    async fn test_save_perspective_cmd_untitled_fallback_fills_gaps_not_counts() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        // Seed an explicit "Untitled 2" — the gap shape ("Untitled" absent).
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("Untitled 2".into()));
        args.insert("view".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, None);
        SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();

        // Missing name → the generator must pick the first free slot
        // ("Untitled"), never the already-taken "Untitled 2".
        let mut args = HashMap::new();
        args.insert("view".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, None);
        let result = SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            result["name"], "Untitled",
            "a gap in the Untitled sequence must be filled, not re-minted as \
             a duplicate of the existing 'Untitled 2'"
        );
    }

    /// `SavePerspectiveCmd::execute` with a non-empty `name` arg uses
    /// it verbatim — the fallback only fires on empty / missing names.
    /// Guards against a regression that would silently swap user-typed
    /// names for "Untitled".
    #[tokio::test]
    async fn test_save_perspective_cmd_uses_supplied_name_when_non_empty() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("My Sprint".into()));
        args.insert("view".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, None);
        let result = SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            result["name"], "My Sprint",
            "non-empty name must be used verbatim"
        );
    }

    /// `SavePerspectiveCmd::execute` resolves `view_id` from the scope
    /// chain when the args bag does not supply one — mirroring the
    /// `from: scope_chain, entity_type: view` YAML annotation.
    ///
    /// Pre-fix: `SavePerspectiveCmd::execute` read `view_id` from
    /// `ctx.arg("view_id")` only and there is no automatic
    /// scope-chain-to-args injection in `build_dispatch_context`, so
    /// the `<BarRegistryTabButtons>` popover (which submits only
    /// `{ name }`) silently dropped `view_id`. Card
    /// `01KRE21GJMPP289N1HSTMJG5HE` review-finding blocker.
    ///
    /// The fixture uses `setup_with_views` so the views registry
    /// carries the builtin board view, and a scope chain with
    /// `view:01JMVIEW0000000000BOARD0` so `resolve_active_view` picks
    /// it up. The asserted invariant is `view_id: Some("01JMVIEW...")`
    /// on the persisted perspective.
    #[tokio::test]
    async fn test_save_perspective_cmd_resolves_view_id_from_scope_chain() {
        let (_temp, ctx) = setup_with_views().await;
        let kanban = Arc::new(ctx);

        // Args carry only `name` — same shape as the popover submission.
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("Pinned".into()));
        let scope = vec!["view:01JMVIEW0000000000BOARD0".to_string()];
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, Some(scope), None);

        let result = SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            result["view_id"], "01JMVIEW0000000000BOARD0",
            "view_id must be resolved from the scope chain's `view:` moniker \
             when no `view_id` arg is supplied; got: {result}"
        );
    }

    /// `SavePerspectiveCmd::execute` resolves the perspective's `view`
    /// kind from the scope chain when the args bag does not supply
    /// `view` — looks up the kind via the views registry.
    ///
    /// Pre-fix the dispatcher fell back to `"board"` regardless of the
    /// active view's kind, so clicking `+` on a grid view created a
    /// `view: "board"` perspective that did not appear in the grid
    /// view's `filteredPerspectives` (the bar filters by `p.view ===
    /// viewKind`). Card `01KRE21GJMPP289N1HSTMJG5HE` review-finding
    /// blocker.
    ///
    /// Uses the builtin grid view `01JMVIEW0000000000TGRID0` so the
    /// view registry resolves `kind: grid`.
    #[tokio::test]
    async fn test_save_perspective_cmd_resolves_view_kind_from_scope_chain() {
        let (_temp, ctx) = setup_with_views().await;
        let kanban = Arc::new(ctx);

        // Args carry only `name` — same shape as the popover submission.
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("From Grid".into()));
        let scope = vec!["view:01JMVIEW0000000000TGRID0".to_string()];
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, Some(scope), None);

        let result = SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            result["view"], "grid",
            "view kind must be resolved from the scope chain's `view:` \
             moniker (looked up against the views registry) when no `view` \
             arg is supplied; got: {result}"
        );
        assert_eq!(
            result["view_id"], "01JMVIEW0000000000TGRID0",
            "view_id from the same scope-chain moniker must round-trip too"
        );
    }

    /// Explicit `view_id` and `view` args still win over the scope-chain
    /// fallback — guards against a regression that would silently
    /// override caller-supplied values with scope-resolved ones.
    #[tokio::test]
    async fn test_save_perspective_cmd_explicit_view_args_override_scope_chain() {
        let (_temp, ctx) = setup_with_views().await;
        let kanban = Arc::new(ctx);

        // Scope chain says grid; args say board. Args win.
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("Pinned".into()));
        args.insert("view".into(), Value::String("board".into()));
        args.insert(
            "view_id".into(),
            Value::String("01JMVIEW0000000000BOARD0".into()),
        );
        let scope = vec!["view:01JMVIEW0000000000TGRID0".to_string()];
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, Some(scope), None);

        let result = SavePerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            result["view"], "board",
            "explicit `view` arg must override the scope-chain fallback"
        );
        assert_eq!(
            result["view_id"], "01JMVIEW0000000000BOARD0",
            "explicit `view_id` arg must override the scope-chain fallback"
        );
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

    #[tokio::test]
    async fn test_next_perspective_cycles_forward_with_wrapping() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let id_a = create_perspective_with_view(&kanban, "A", "board").await;
        let id_b = create_perspective_with_view(&kanban, "B", "board").await;
        let id_c = create_perspective_with_view(&kanban, "C", "board").await;

        // Set active to A
        ui.set_active_perspective("main", &id_a);

        // Next: A -> B
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        let result = NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert!(result != Value::Null);
        assert_eq!(ui.active_perspective_id("main"), id_b);

        // Next: B -> C
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(ui.active_perspective_id("main"), id_c);

        // Next: C -> A (wrap)
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(ui.active_perspective_id("main"), id_a);
    }

    #[tokio::test]
    async fn test_prev_perspective_cycles_backward_with_wrapping() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let id_a = create_perspective_with_view(&kanban, "A", "grid").await;
        let id_b = create_perspective_with_view(&kanban, "B", "grid").await;
        let id_c = create_perspective_with_view(&kanban, "C", "grid").await;

        // Set active to A
        ui.set_active_perspective("main", &id_a);

        // Prev: A -> C (wrap)
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("grid".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        PrevPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(ui.active_perspective_id("main"), id_c);

        // Prev: C -> B
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("grid".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        PrevPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(ui.active_perspective_id("main"), id_b);

        // Prev: B -> A
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("grid".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        PrevPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(ui.active_perspective_id("main"), id_a);
    }

    #[tokio::test]
    async fn test_cycle_noop_with_zero_matching_perspectives() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        // Create perspectives for "board" but query for "grid"
        create_perspective_with_view(&kanban, "A", "board").await;

        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("grid".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        let result = NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result, Value::Null);
    }

    #[tokio::test]
    async fn test_cycle_noop_with_one_matching_perspective() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let id_a = create_perspective_with_view(&kanban, "A", "board").await;
        ui.set_active_perspective("main", &id_a);

        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        let result = NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result, Value::Null);
    }

    #[tokio::test]
    async fn test_cycle_filters_by_view_kind() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let id_board = create_perspective_with_view(&kanban, "Board1", "board").await;
        let _id_grid = create_perspective_with_view(&kanban, "Grid1", "grid").await;
        let id_board2 = create_perspective_with_view(&kanban, "Board2", "board").await;

        // Set active to board perspective
        ui.set_active_perspective("main", &id_board);

        // Next should go to Board2, not Grid1
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("board".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
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

    /// Append a `window:<DEFAULT_TEST_WINDOW>` moniker to a scope chain when it
    /// carries no `window:` moniker, so per-window ops have a window to resolve
    /// in tests.
    fn with_default_window(mut scope_chain: Vec<String>) -> Vec<String> {
        if !scope_chain.iter().any(|m| m.starts_with("window:")) {
            scope_chain.push(default_window_moniker());
        }
        scope_chain
    }

    #[tokio::test]
    async fn test_next_perspective_derives_view_kind_from_scope_chain() {
        let (_temp, ctx) = setup_with_views().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        // Create board perspectives
        let id_a = create_perspective_with_view(&kanban, "A", "board").await;
        let id_b = create_perspective_with_view(&kanban, "B", "board").await;

        ui.set_active_perspective("main", &id_a);

        // Invoke without view_kind arg, but with view:01JMVIEW0000000000BOARD0 in scope chain
        let scope = vec!["view:01JMVIEW0000000000BOARD0".to_string()];
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            HashMap::new(),
            Some(scope),
            Some(Arc::clone(&ui)),
        );
        let result = NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();

        assert!(result != Value::Null, "should cycle, not return null");
        assert_eq!(ui.active_perspective_id("main"), id_b);
    }

    #[tokio::test]
    async fn test_next_perspective_explicit_view_kind_overrides_scope() {
        let (_temp, ctx) = setup_with_views().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

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
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            args,
            Some(scope),
            Some(Arc::clone(&ui)),
        );
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
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let id = create_perspective_with_view(&kanban, "Target", "board").await;

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(id.clone()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        let result = GotoPerspectiveCmd.execute(&cmd_ctx).await.unwrap();

        assert!(result != Value::Null);
        assert_eq!(ui.active_perspective_id("main"), id);
    }

    #[tokio::test]
    async fn test_goto_perspective_invalid_id_returns_error() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String("nonexistent".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        let result = GotoPerspectiveCmd.execute(&cmd_ctx).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_goto_perspective_mismatched_view_kind_returns_error() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let id = create_perspective_with_view(&kanban, "BoardView", "board").await;

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(id.clone()));
        args.insert("view_kind".into(), Value::String("grid".into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        let result = GotoPerspectiveCmd.execute(&cmd_ctx).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_goto_perspective_without_view_kind_succeeds() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let id = create_perspective_with_view(&kanban, "GridView", "grid").await;

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(id.clone()));
        // No view_kind arg — should succeed regardless of the perspective's view
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
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
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, None);
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
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, None);
        let result = RenamePerspectiveCmd.execute(&cmd_ctx).await;

        assert!(result.is_err());
    }

    // =========================================================================
    // resolve_perspective_id — four-tier fallback
    // =========================================================================

    /// Build a CommandContext with scope chain, args, UiState, and a
    /// KanbanContext extension. Used for resolver tests that need every
    /// input simultaneously.
    fn make_full_ctx(
        kanban: Arc<KanbanContext>,
        args: HashMap<String, Value>,
        scope_chain: Vec<String>,
        ui: Arc<swissarmyhammer_ui_state::UiState>,
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
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        // Populate UiState so every fallback path has an id available —
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
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());
        ui.set_active_perspective("main", "ui-id");

        // No arg — scope chain's perspective moniker should win over UiState.
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
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());
        ui.set_active_perspective("main", "ui-id");

        // No arg, no scope-chain perspective moniker — UiState wins.
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
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        // Two perspectives: one for each view kind.
        let board_pid = create_perspective_with_view(&kanban, "Board", "board").await;
        let _grid_pid = create_perspective_with_view(&kanban, "Grid", "grid").await;

        // No arg, no scope-chain perspective, empty UiState.
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
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let board_pid = create_perspective_with_view(&kanban, "Board", "board").await;

        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("board".into()));
        let cmd_ctx = make_full_ctx(
            Arc::clone(&kanban),
            args,
            vec!["window:main".into()],
            Arc::clone(&ui),
        );

        // Confirm UiState starts empty.
        assert_eq!(ui.active_perspective_id("main"), "");

        let resolved = resolve_and_persist_perspective_id(&cmd_ctx, &kanban)
            .await
            .unwrap();

        assert_eq!(resolved, board_pid);
        // Self-healing: the fallback choice should have been written back.
        assert_eq!(
            ui.active_perspective_id("main"),
            board_pid,
            "fallback resolution should persist to UiState so subsequent commands find it"
        );
    }

    #[tokio::test]
    async fn resolve_and_persist_does_not_touch_uistate_when_arg_supplied() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

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
        // Caller-supplied ids must not mutate UiState on the caller's behalf —
        // changing the active perspective is the caller's decision.
        assert_eq!(ui.active_perspective_id("main"), "");
    }

    #[tokio::test]
    async fn resolve_and_persist_does_not_touch_uistate_when_scope_supplied() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

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
        // No arg, no scope-chain perspective, empty UiState, no perspectives
        // registered for the view kind → the resolver must surface the
        // missing-arg error so the caller can report it.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

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
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        // Create a perspective with a filter, then mark it active in UiState.
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
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

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
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

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

    // =========================================================================
    // perspective_belongs_to_active_view — id-scoped vs legacy fallback
    // =========================================================================

    /// Build a [`Perspective`] in-memory for helper-rule unit tests. Bypasses
    /// the dispatch/YAML round-trip so the test focuses purely on the
    /// kind-vs-id matching rule.
    fn make_perspective(id: &str, view: &str, view_id: Option<&str>) -> Perspective {
        let mut p = Perspective::new(id.to_string(), id.to_string(), view.to_string());
        p.view_id = view_id.map(str::to_string);
        p
    }

    #[test]
    fn helper_matches_strictly_by_view_id_when_set() {
        let p = make_perspective("p", "grid", Some("view-a"));
        assert!(
            perspective_belongs_to_active_view(&p, Some("view-a"), "grid"),
            "id-scoped perspective matches when active id == its view_id"
        );
        assert!(
            !perspective_belongs_to_active_view(&p, Some("view-b"), "grid"),
            "id-scoped perspective must NOT match sibling view of same kind"
        );
    }

    #[test]
    fn helper_falls_back_to_kind_when_view_id_is_none() {
        let p = make_perspective("p", "grid", None);
        assert!(
            perspective_belongs_to_active_view(&p, Some("view-a"), "grid"),
            "legacy perspective shares by kind — grid matches grid"
        );
        assert!(
            perspective_belongs_to_active_view(&p, None, "grid"),
            "legacy perspective shares by kind even when active view id is unknown"
        );
        assert!(
            !perspective_belongs_to_active_view(&p, Some("view-a"), "board"),
            "legacy perspective with view=grid must NOT match a board-kind active view"
        );
    }

    #[test]
    fn helper_blocks_scoped_perspective_when_active_view_id_is_unknown() {
        // The headless dynamic-sources path may have a kind but no resolved
        // active view id (splash / pre-focus). Scoped perspectives must not
        // leak into that path.
        let p = make_perspective("p", "grid", Some("view-a"));
        assert!(
            !perspective_belongs_to_active_view(&p, None, "grid"),
            "scoped perspective must NOT leak when active view id is None"
        );
    }

    // =========================================================================
    // next_perspective / resolve_perspective_id — view_id scoping
    // =========================================================================

    /// View ids used by the per-id resolver tests. Both are grid-kind so the
    /// test can prove the resolver differentiates by id alone — kind is
    /// identical between them.
    const GRID_VIEW_A_ID: &str = "01JMVIEW0000000000TGRID0";
    const GRID_VIEW_B_ID: &str = "01JMVIEW0000000000PGRID0";

    #[tokio::test]
    async fn next_perspective_filters_by_view_id_when_arg_provided() {
        // Two grid-kind perspectives, each pinned to a different view_id.
        // With `view_id` arg pointing at GRID_VIEW_A_ID, only the matching
        // perspective should be selectable by the resolver — proving the
        // fallback walks the view_id-aware filter rather than kind-only.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let pid_a = create_perspective_scoped(&kanban, "GridA", "grid", GRID_VIEW_A_ID).await;
        let pid_b = create_perspective_scoped(&kanban, "GridB", "grid", GRID_VIEW_B_ID).await;

        // Resolver fallback path: no arg, no scope, empty UiState; `view_kind`
        // + `view_id` args steer the first-for-view-kind branch. The matching
        // perspective MUST be the one pinned to GRID_VIEW_A_ID.
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("grid".into()));
        args.insert("view_id".into(), Value::String(GRID_VIEW_A_ID.into()));
        let cmd_ctx = make_full_ctx(
            Arc::clone(&kanban),
            args,
            vec!["window:main".into()],
            Arc::clone(&ui),
        );

        let (id, source) = resolve_perspective_id(&cmd_ctx, &kanban).await.unwrap();
        assert_eq!(
            source,
            ResolvedFrom::FirstForViewKind,
            "expected fallback to first-for-view-kind branch"
        );
        assert_eq!(id, pid_a, "resolver must pick the view_id-A perspective");
        assert_ne!(
            id, pid_b,
            "resolver must NOT pick the sibling view_id-B perspective"
        );
    }

    #[tokio::test]
    async fn next_perspective_falls_back_to_legacy_perspectives_when_view_id_absent() {
        // Mixed roster: one legacy (view_id=None) grid perspective + one
        // id-scoped grid perspective. Caller provides only `view_kind` —
        // no `view_id` — so the resolver must NOT pick the scoped one,
        // it must fall back to the legacy perspective.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let pid_legacy = create_perspective_with_view(&kanban, "Legacy", "grid").await;
        let pid_scoped = create_perspective_scoped(&kanban, "Scoped", "grid", GRID_VIEW_A_ID).await;

        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("grid".into()));
        // Intentionally NO view_id arg.
        let cmd_ctx = make_full_ctx(
            Arc::clone(&kanban),
            args,
            vec!["window:main".into()],
            Arc::clone(&ui),
        );

        let (id, source) = resolve_perspective_id(&cmd_ctx, &kanban).await.unwrap();
        assert_eq!(source, ResolvedFrom::FirstForViewKind);
        assert_eq!(
            id, pid_legacy,
            "resolver must pick the legacy view_id=None perspective when view_id arg is absent"
        );
        assert_ne!(
            id, pid_scoped,
            "scoped perspective must NOT be picked when active view_id is unknown"
        );
    }

    #[tokio::test]
    async fn cycle_perspective_filters_by_view_id_when_arg_provided() {
        // End-to-end cycle test: two scoped grid perspectives on view A, one on
        // view B. With `view_id=A`, NextPerspective must cycle within A only.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let pid_a1 = create_perspective_scoped(&kanban, "A1", "grid", GRID_VIEW_A_ID).await;
        let pid_a2 = create_perspective_scoped(&kanban, "A2", "grid", GRID_VIEW_A_ID).await;
        let _pid_b = create_perspective_scoped(&kanban, "B1", "grid", GRID_VIEW_B_ID).await;

        ui.set_active_perspective("main", &pid_a1);

        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("grid".into()));
        args.insert("view_id".into(), Value::String(GRID_VIEW_A_ID.into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            ui.active_perspective_id("main"),
            pid_a2,
            "cycle within view A must land on the sibling pinned to view A"
        );

        // Wrap: A2 -> A1, not into view B's perspective.
        let mut args = HashMap::new();
        args.insert("view_kind".into(), Value::String("grid".into()));
        args.insert("view_id".into(), Value::String(GRID_VIEW_A_ID.into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        NextPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            ui.active_perspective_id("main"),
            pid_a1,
            "cycle must wrap within view A and NOT cross into view B"
        );
    }

    #[tokio::test]
    async fn goto_perspective_view_id_match_succeeds() {
        // Validation path: goto with view_kind + view_id where the
        // perspective is scoped to that view must succeed.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let pid = create_perspective_scoped(&kanban, "Pinned", "grid", GRID_VIEW_A_ID).await;

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(pid.clone()));
        args.insert("view_kind".into(), Value::String("grid".into()));
        args.insert("view_id".into(), Value::String(GRID_VIEW_A_ID.into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        let result = GotoPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert!(result != Value::Null);
        assert_eq!(ui.active_perspective_id("main"), pid);
    }

    #[tokio::test]
    async fn goto_perspective_view_id_mismatch_returns_error() {
        // Validation path: goto with view_kind + view_id where the
        // perspective is pinned to a DIFFERENT view_id (same kind!) must
        // return an error — this is the regression guard for the bug
        // where two grid views shared every scoped perspective.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let pid = create_perspective_scoped(&kanban, "Pinned to A", "grid", GRID_VIEW_A_ID).await;

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(pid));
        args.insert("view_kind".into(), Value::String("grid".into()));
        args.insert("view_id".into(), Value::String(GRID_VIEW_B_ID.into()));
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, None, Some(Arc::clone(&ui)));
        let result = GotoPerspectiveCmd.execute(&cmd_ctx).await;
        assert!(
            result.is_err(),
            "goto with mismatched view_id must error even when kinds match"
        );
    }

    // -----------------------------------------------------------------
    // FocusFilterCmd — UI-only marker (see struct doc).
    //
    // The pre-refactor tests pinned the `FocusFilter` marker envelope
    // and the resolver path that drove it. Card
    // `01KRGZY33P99J7CGG0XRQGZ352` deleted that channel; the command
    // is now a deliberate no-op kept only to satisfy the YAML ↔ Rust
    // completeness invariant. A single test pins the no-op contract so
    // a regression that re-introduces the old marker envelope (or any
    // other side-effect) is caught.
    // -----------------------------------------------------------------

    /// The command must execute to `Value::Null` — no marker envelope,
    /// no state mutation. The Filter tab button's click claims focus
    /// via the frontend `nav.focus` command instead (see
    /// `FilterFocusCommandButton`); this command exists only to keep
    /// the YAML registration valid for the `tab_button` icon emission.
    #[tokio::test]
    async fn focus_filter_command_is_a_noop() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let pid = create_perspective(&kanban, "Focus Test").await;

        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            HashMap::new(),
            Some(vec![format!("perspective:{pid}")]),
            None,
        );

        let result = FocusFilterCmd.execute(&cmd_ctx).await.unwrap();
        assert!(
            result.is_null(),
            "perspective.filter.focus must be a UI-only no-op (returned {result:?}) — \
             focus claims flow through the frontend nav.focus command",
        );
    }

    /// The command is always available — the tab-button slot needs to
    /// emit on every active perspective. Pinned so a future
    /// availability change doesn't silently drop the Filter icon from
    /// the tab bar.
    #[test]
    fn focus_filter_command_is_always_available() {
        let ctx = CommandContext::new("perspective.filter.focus", vec![], None, HashMap::new());
        assert!(FocusFilterCmd.available(&ctx));
    }

    // =========================================================================
    // SwitchPerspectiveCmd tests
    //
    // Covers the four contracts called out in 01KP3ERHEDP86C2JYYR7NM1593:
    //   (a) sets `active_perspective_id`
    //   (b) writes `filtered_task_ids` matching the perspective's filter
    //   (c) both changes land in one `UiStateChange::PerspectiveSwitch`
    //   (d) unknown perspective id surfaces as a clean `ExecutionFailed`
    // =========================================================================

    /// Build a CommandContext with KanbanContext + UiState + a single
    /// `perspective_id` arg — the minimal shape every `perspective.switch`
    /// test needs.
    fn switch_ctx(
        kanban: Arc<KanbanContext>,
        ui: Arc<swissarmyhammer_ui_state::UiState>,
        perspective_id: &str,
    ) -> CommandContext {
        let mut args = HashMap::new();
        args.insert(
            "perspective_id".into(),
            Value::String(perspective_id.into()),
        );
        // Per-window op: the scope chain must carry a `window:` moniker — the
        // silent "main" fallback was removed, so seed the default test window.
        let mut ctx =
            CommandContext::new("perspective.switch", vec!["window:main".into()], None, args);
        ctx.set_extension(kanban);
        ctx.ui_state = Some(ui);
        ctx
    }

    /// Add a task with a body — body carries `#tag` markers the
    /// `enrich_*` pipeline lifts into `filter_tags`, which is what the
    /// DSL evaluator reads on `#bug` / `#feature` queries.
    async fn add_task_with_body(kanban: &Arc<KanbanContext>, title: &str, body: &str) -> String {
        let task = crate::task::AddTask::new(title)
            .with_description(body)
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        task["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn switch_perspective_unknown_id_returns_execution_failed() {
        // Contract (d): an unknown perspective id must surface as a
        // clean `ExecutionFailed` error so the dispatcher can log it
        // and the frontend's `.catch(console.error)` can record it.
        // Crucially, it must NOT silently mutate UiState.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let cmd_ctx = switch_ctx(Arc::clone(&kanban), Arc::clone(&ui), "does-not-exist");
        let err = SwitchPerspectiveCmd
            .execute(&cmd_ctx)
            .await
            .expect_err("unknown perspective id must error");
        match err {
            CommandError::ExecutionFailed(msg) => {
                assert!(
                    msg.contains("perspective not found"),
                    "error message should mention perspective not found, got: {msg}",
                );
            }
            other => panic!("expected ExecutionFailed, got: {other:?}"),
        }
        // UiState must be untouched.
        assert!(ui.active_perspective_id("main").is_empty());
        assert!(ui.filtered_task_ids("main").is_empty());
    }

    #[tokio::test]
    async fn switch_perspective_sets_active_id_with_no_filter() {
        // Contract (a): the resolved id lands in `active_perspective_id`.
        // With no filter on the perspective, the filtered list is every
        // task on the board (filter empty → no-filter).
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        // Two tasks; no filter on the perspective.
        let t1 = add_task_with_body(&kanban, "Bug task", "#bug fix this").await;
        let t2 = add_task_with_body(&kanban, "Feature task", "#feature add that").await;
        let pid = create_perspective_with_view(&kanban, "All", "board").await;

        let cmd_ctx = switch_ctx(Arc::clone(&kanban), Arc::clone(&ui), &pid);
        let result = SwitchPerspectiveCmd.execute(&cmd_ctx).await.unwrap();
        assert!(!result.is_null(), "switch should produce a UiStateChange");

        assert_eq!(ui.active_perspective_id("main"), pid);
        let filtered = ui.filtered_task_ids("main");
        assert_eq!(filtered.len(), 2, "no-filter must include every task");
        assert!(filtered.contains(&t1));
        assert!(filtered.contains(&t2));
    }

    #[tokio::test]
    async fn switch_perspective_writes_filtered_ids_matching_filter() {
        // Contract (b): the filter DSL is evaluated server-side and only
        // matching task ids land in `filtered_task_ids`. Uses `#bug` —
        // the simplest tag predicate — to keep the test focused on the
        // wiring (the DSL evaluator itself is covered by
        // `task_helpers::tests`).
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let t_bug = add_task_with_body(&kanban, "Bug", "#bug top of body").await;
        let _t_feat = add_task_with_body(&kanban, "Feature", "#feature pretty").await;

        // Save a perspective whose filter narrows to `#bug`.
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("Bugs".into()));
        args.insert("view".into(), Value::String("board".into()));
        args.insert("filter".into(), Value::String("#bug".into()));
        let save_ctx = make_ctx(Arc::clone(&kanban), args, None, None);
        let saved = SavePerspectiveCmd.execute(&save_ctx).await.unwrap();
        let pid = saved["id"].as_str().unwrap().to_string();

        let cmd_ctx = switch_ctx(Arc::clone(&kanban), Arc::clone(&ui), &pid);
        SwitchPerspectiveCmd.execute(&cmd_ctx).await.unwrap();

        let filtered = ui.filtered_task_ids("main");
        assert_eq!(filtered, vec![t_bug], "only `#bug` tasks should survive");
    }

    #[tokio::test]
    async fn switch_perspective_emits_single_atomic_change() {
        // Contract (c): both `active_perspective_id` and
        // `filtered_task_ids` arrive in ONE `UiStateChange::PerspectiveSwitch`.
        // The frontend's `ui-state-changed` subscriber gets exactly one
        // event per click — never two.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let t = add_task_with_body(&kanban, "Bug", "#bug oops").await;
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("Bugs".into()));
        args.insert("view".into(), Value::String("board".into()));
        args.insert("filter".into(), Value::String("#bug".into()));
        let save_ctx = make_ctx(Arc::clone(&kanban), args, None, None);
        let pid = SavePerspectiveCmd.execute(&save_ctx).await.unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let cmd_ctx = switch_ctx(Arc::clone(&kanban), Arc::clone(&ui), &pid);
        let result = SwitchPerspectiveCmd.execute(&cmd_ctx).await.unwrap();

        // The result must deserialize as `PerspectiveSwitch` — not a pair
        // of `ActivePerspective` + something-else, and not as a tuple of
        // two changes.
        let change: swissarmyhammer_ui_state::UiStateChange =
            serde_json::from_value(result).expect("result must be a single UiStateChange");
        match change {
            swissarmyhammer_ui_state::UiStateChange::PerspectiveSwitch {
                perspective_id,
                filtered_task_ids,
            } => {
                assert_eq!(perspective_id, pid);
                assert_eq!(filtered_task_ids, vec![t]);
            }
            other => panic!("expected PerspectiveSwitch, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn switch_perspective_is_per_window() {
        // Window isolation guard: switching on `window:secondary` must
        // not bleed into the main window's slots. Drives the same
        // window_label_from_scope path the production app uses.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        add_task_with_body(&kanban, "Bug", "#bug").await;
        let pid = create_perspective_with_view(&kanban, "All", "board").await;

        let mut args = HashMap::new();
        args.insert("perspective_id".into(), Value::String(pid.clone()));
        let mut cmd_ctx = CommandContext::new(
            "perspective.switch",
            vec!["window:secondary".into()],
            None,
            args,
        );
        cmd_ctx.set_extension(Arc::clone(&kanban));
        cmd_ctx.ui_state = Some(Arc::clone(&ui));

        SwitchPerspectiveCmd.execute(&cmd_ctx).await.unwrap();

        assert_eq!(ui.active_perspective_id("secondary"), pid);
        assert_eq!(ui.filtered_task_ids("secondary").len(), 1);
        // Main window must be untouched.
        assert!(ui.active_perspective_id("main").is_empty());
        assert!(ui.filtered_task_ids("main").is_empty());
    }

    #[tokio::test]
    async fn switch_perspective_dispatches_via_registry_end_to_end() {
        // Integration: drive the command through the `register_commands()`
        // map exactly the way the production dispatcher does. Pins that
        // the YAML id → Rust handler wire is intact (no missing
        // registration, no name typo).
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let t = add_task_with_body(&kanban, "Bug", "#bug oops").await;
        let mut args = HashMap::new();
        args.insert("name".into(), Value::String("Bugs".into()));
        args.insert("view".into(), Value::String("board".into()));
        args.insert("filter".into(), Value::String("#bug".into()));
        let save_ctx = make_ctx(Arc::clone(&kanban), args, None, None);
        let pid = SavePerspectiveCmd.execute(&save_ctx).await.unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        // Pull the command from the registry — the same lookup path the
        // dispatcher uses at runtime.
        let registry = crate::commands::register_commands();
        let cmd = registry
            .get("perspective.switch")
            .expect("perspective.switch must be in the registry");

        let cmd_ctx = switch_ctx(Arc::clone(&kanban), Arc::clone(&ui), &pid);
        cmd.execute(&cmd_ctx).await.unwrap();

        assert_eq!(ui.active_perspective_id("main"), pid);
        assert_eq!(ui.filtered_task_ids("main"), vec![t]);
    }

    #[tokio::test]
    async fn switch_perspective_missing_arg_returns_missing_arg_error() {
        // Calling the command with no `perspective_id` arg must error
        // cleanly — the YAML declares the param as required, but the
        // handler itself enforces it too so a programmatic dispatch
        // can't silently pass an empty switch.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let mut cmd_ctx = CommandContext::new("perspective.switch", vec![], None, HashMap::new());
        cmd_ctx.set_extension(Arc::clone(&kanban));
        cmd_ctx.ui_state = Some(Arc::clone(&ui));

        let err = SwitchPerspectiveCmd
            .execute(&cmd_ctx)
            .await
            .expect_err("missing arg must error");
        match err {
            CommandError::MissingArg(name) => assert_eq!(name, "perspective_id"),
            other => panic!("expected MissingArg(perspective_id), got: {other:?}"),
        }
    }

    // =========================================================================
    // SetFilterAndRefreshCmd tests
    //
    // The filter-edit refresh seam (card 01KV0MJYA58GW5PRXGVXWHQK32): editing
    // the ACTIVE perspective's filter must recompute the window's
    // `filtered_task_ids` and emit a `PerspectiveSwitch` change — the same
    // event-driven refresh switch/next/prev/delete ride — so the view
    // re-filters without a click-away/back. Editing a NON-active perspective
    // only persists STORAGE (no window refresh to drive).
    // =========================================================================

    /// Build a CommandContext with KanbanContext + UiState + `perspective_id`
    /// and `filter` args and a `window:<label>` scope — the shape the entity
    /// server's `handle_set_filter` produces.
    fn set_filter_ctx(
        kanban: Arc<KanbanContext>,
        ui: Arc<swissarmyhammer_ui_state::UiState>,
        perspective_id: &str,
        filter: &str,
        window_label: &str,
    ) -> CommandContext {
        let mut args = HashMap::new();
        args.insert(
            "perspective_id".into(),
            Value::String(perspective_id.into()),
        );
        args.insert("filter".into(), Value::String(filter.into()));
        let mut ctx = CommandContext::new(
            "perspective.filter",
            vec![format!("window:{window_label}")],
            None,
            args,
        );
        ctx.set_extension(kanban);
        ctx.ui_state = Some(ui);
        ctx
    }

    #[tokio::test]
    async fn set_filter_on_active_perspective_recomputes_filtered_task_ids_and_emits_change() {
        // The dispatching window's active perspective is the one being
        // edited: persisting the new filter must recompute the window's
        // `filtered_task_ids` and return a `PerspectiveSwitch` change so the
        // host's `ui-state-changed` emit fires — no click-away required.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let t_bug = add_task_with_body(&kanban, "Bug", "#bug top").await;
        let _t_feat = add_task_with_body(&kanban, "Feature", "#feature pretty").await;

        // The active perspective starts with no filter (shows everything).
        let pid = create_perspective_with_view(&kanban, "All", "board").await;
        let switch_ctx = switch_ctx(Arc::clone(&kanban), Arc::clone(&ui), &pid);
        SwitchPerspectiveCmd.execute(&switch_ctx).await.unwrap();
        assert_eq!(
            ui.filtered_task_ids("main").len(),
            2,
            "precondition: no-filter perspective shows every task"
        );

        // Now edit that ACTIVE perspective's filter to `#bug`.
        let cmd_ctx = set_filter_ctx(Arc::clone(&kanban), Arc::clone(&ui), &pid, "#bug", "main");
        let result = SetFilterAndRefreshCmd.execute(&cmd_ctx).await.unwrap();

        // The result must be a single `PerspectiveSwitch` change carrying the
        // recomputed id list.
        let change: swissarmyhammer_ui_state::UiStateChange =
            serde_json::from_value(result).expect("result must be a single UiStateChange");
        match change {
            swissarmyhammer_ui_state::UiStateChange::PerspectiveSwitch {
                perspective_id,
                filtered_task_ids,
            } => {
                assert_eq!(perspective_id, pid);
                assert_eq!(filtered_task_ids, vec![t_bug.clone()]);
            }
            other => panic!("expected PerspectiveSwitch, got: {other:?}"),
        }

        // And the window's slot is now narrowed to the `#bug` task.
        assert_eq!(ui.filtered_task_ids("main"), vec![t_bug]);
    }

    #[tokio::test]
    async fn set_filter_on_non_active_perspective_returns_null_and_leaves_active_window_unchanged()
    {
        // Editing a perspective that is NOT the dispatching window's active
        // selection only persists STORAGE — there is no window to refresh, so
        // the change is null and the active window's `filtered_task_ids` is
        // untouched.
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);
        let ui = Arc::new(swissarmyhammer_ui_state::UiState::new());

        let t_bug = add_task_with_body(&kanban, "Bug", "#bug top").await;
        let t_feat = add_task_with_body(&kanban, "Feature", "#feature pretty").await;

        // Active perspective (no filter) — shows everything.
        let active_pid = create_perspective_with_view(&kanban, "Active", "board").await;
        // A second, NON-active perspective we will edit.
        let other_pid = create_perspective_with_view(&kanban, "Other", "board").await;

        let switch_ctx = switch_ctx(Arc::clone(&kanban), Arc::clone(&ui), &active_pid);
        SwitchPerspectiveCmd.execute(&switch_ctx).await.unwrap();
        let before = ui.filtered_task_ids("main");
        assert_eq!(before.len(), 2);

        // Edit the OTHER perspective's filter.
        let cmd_ctx = set_filter_ctx(
            Arc::clone(&kanban),
            Arc::clone(&ui),
            &other_pid,
            "#bug",
            "main",
        );
        let result = SetFilterAndRefreshCmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(
            result,
            Value::Null,
            "editing a non-active perspective must not produce a window-refresh change"
        );

        // The active window is unchanged.
        let after = ui.filtered_task_ids("main");
        assert_eq!(after.len(), 2);
        assert!(after.contains(&t_bug));
        assert!(after.contains(&t_feat));
        assert_eq!(ui.active_perspective_id("main"), active_pid);

        // But the storage write landed: the other perspective now carries the
        // `#bug` filter.
        let pctx = kanban.perspective_context().await.unwrap();
        let pctx = pctx.read().await;
        let other = pctx.get_by_id(&other_pid).unwrap();
        assert_eq!(other.filter.as_deref(), Some("#bug"));
    }

    // =========================================================================
    // evaluate_perspective_filter tests
    //
    // The helper is `pub` so the kanban-app entity-cache bridge can recompute a
    // window's `filtered_task_ids` on a live task change with the same DSL
    // evaluator `perspective.switch` uses. These pin its two contracts: a
    // non-empty filter narrows to matching ids; an empty filter is "no filter".
    // =========================================================================

    #[tokio::test]
    async fn evaluate_perspective_filter_narrows_to_matching_tasks() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        let t_bug = add_task_with_body(&kanban, "Bug", "#bug body").await;
        let _t_feat = add_task_with_body(&kanban, "Feature", "#feature body").await;

        let ids = evaluate_perspective_filter(&kanban, "#bug").await.unwrap();
        assert_eq!(
            ids,
            vec![t_bug],
            "only `#bug` tasks should match the filter"
        );
    }

    #[tokio::test]
    async fn evaluate_perspective_filter_empty_filter_returns_all_tasks() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        let t1 = add_task_with_body(&kanban, "One", "#bug").await;
        let t2 = add_task_with_body(&kanban, "Two", "#feature").await;

        let ids = evaluate_perspective_filter(&kanban, "").await.unwrap();
        assert_eq!(ids.len(), 2, "an empty filter must return every task id");
        assert!(ids.contains(&t1));
        assert!(ids.contains(&t2));
    }
}
