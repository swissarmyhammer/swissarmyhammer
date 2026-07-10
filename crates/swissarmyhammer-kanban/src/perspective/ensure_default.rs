//! Default-perspective invariants: deterministic ids and board-open
//! reconciliation (dedup + zero-state recovery).
//!
//! # The live bug this guards against
//!
//! The "Default" perspective used to be created by a frontend auto-create
//! hook minting a fresh ULID per create, guarded only by in-memory state.
//! That state is loaded once per process, so every hot reload, extra window,
//! or sibling process with a stale cache created another duplicate — hundreds
//! of `Default` YAML files accumulated on one board. Conversely, a stale empty
//! cache presented as ZERO perspectives while files existed on disk.
//!
//! The cross-window staleness for rename/delete is now also closed: the entity
//! file watcher routes `.kanban/perspectives/*.yaml` events to a
//! [`PerspectiveReloader`](swissarmyhammer_entity::PerspectiveReloader) that
//! calls [`PerspectiveContext::reload_from_disk_with`], converging a sibling
//! process's perspective cache (and the frontend tab bar) on an external edit.
//! The deterministic-id + reconciliation mechanisms below remain the
//! convergence guarantee for the CREATE path.
//!
//! Two storage-layer mechanisms fix the class of bug:
//!
//! 1. **Deterministic ids** — an ensure-created default's id derives from
//!    its scope ([`default_perspective_id`]), so a second create (stale
//!    cache, concurrent window, sibling process) writes the SAME file and
//!    upserts instead of accumulating. Idempotent by construction, with no
//!    cross-process locking needed.
//! 2. **Board-open reconciliation** — [`reconcile_default_perspectives`]
//!    runs in `KanbanContext::open`: it converges existing duplicates,
//!    prunes unreachable vanilla defaults, and recreates the default when a
//!    board has zero perspectives.
//!
//! Extra duplicate defaults are **removed**, not merged — but only the
//! VANILLA ones. A customized default (any filter/group/sort/fields set)
//! is always preferred as the keeper, and customized or user-named
//! perspectives are never deleted: when two or more customized Defaults
//! share a scope, all of them survive — one becomes the keeper for the
//! well-known id, the rest remain as extra perspectives for the user to
//! resolve (rename, merge, or delete) themselves.

use std::collections::HashMap;

use swissarmyhammer_perspectives::default_scope::is_customized;
use swissarmyhammer_perspectives::{Perspective, PerspectiveContext};
use swissarmyhammer_views::ViewsContext;

use crate::error::Result;

// The scope invariants (deterministic id, matching rule, filename safety,
// customization test) are shared with every other Default-perspective writer
// — most importantly the `views` MCP server's `save perspective` ensure path,
// which is what production dispatches through the `perspective-commands`
// plugin. They live in `swissarmyhammer_perspectives::default_scope` (the
// crate every writer already depends on) so the writers cannot drift; these
// re-exports keep this module the kanban-crate-local facade.
pub use swissarmyhammer_perspectives::default_scope::{
    default_perspective_id, DEFAULT_PERSPECTIVE_NAME,
};
pub(crate) use swissarmyhammer_perspectives::default_scope::{
    is_safe_scope_component, matches_scope, perspective_scope,
};

/// Reconcile the default-perspective invariants at board open.
///
/// Three passes, in order:
///
/// 1. **Dedup** — delete the *vanilla* duplicates among perspectives named
///    [`DEFAULT_PERSPECTIVE_NAME`] that share a scope, keeping one keeper
///    (customized beats vanilla, then the deterministic well-known id,
///    then the oldest id). Customized duplicates are user data and are
///    all kept.
/// 2. **Prune** (only when a view registry is supplied) — delete *vanilla*
///    defaults that can never render: ones pinned to a view id that no
///    longer exists, and legacy kind-shared ones fully shadowed because
///    every view of that kind already has its own pinned default.
/// 3. **Recover** — a board with zero perspectives gets its default
///    recreated (view kind `"board"`, pinned to the single board view when
///    unambiguous) under the deterministic id.
///
/// User-created perspectives (any other name) are never touched.
pub async fn reconcile_default_perspectives(
    pctx: &mut PerspectiveContext,
    views: Option<&ViewsContext>,
) -> Result<()> {
    dedup_defaults_per_scope(pctx).await?;
    if let Some(views) = views {
        prune_unreachable_defaults(pctx, views).await?;
    }
    recover_zero_state(pctx, views).await?;
    Ok(())
}

/// Pass 1: delete the *vanilla* same-scope duplicate defaults, converging
/// on a single keeper.
///
/// Customized duplicates are user data (filter/group/sort/fields the user
/// set on them) and are never deleted: when several customized Defaults
/// share a scope, one becomes the keeper and the others simply remain as
/// extra perspectives for the user to resolve.
async fn dedup_defaults_per_scope(pctx: &mut PerspectiveContext) -> Result<()> {
    let mut groups: HashMap<String, Vec<Perspective>> = HashMap::new();
    for p in pctx.all() {
        if p.name == DEFAULT_PERSPECTIVE_NAME {
            groups
                .entry(perspective_scope(p).to_string())
                .or_default()
                .push(p.clone());
        }
    }

    for (scope, mut group) in groups {
        if group.len() < 2 {
            continue;
        }
        let well_known = default_perspective_id(&scope);
        // Keeper preference: customized > well-known id > oldest (smallest
        // ULID). Sort so the keeper lands first.
        group.sort_by_key(|p| (!is_customized(p), p.id != well_known, p.id.clone()));
        let keeper = &group[0];
        // Only vanilla duplicates are deletable; customized ones carry
        // user-authored state and all survive alongside the keeper.
        let doomed: Vec<&Perspective> = group[1..].iter().filter(|p| !is_customized(p)).collect();
        tracing::info!(
            scope = %scope,
            keeper = %keeper.id,
            removed = doomed.len(),
            kept_customized = group.len() - 1 - doomed.len(),
            "converging duplicate default perspectives"
        );
        for dup in doomed {
            delete_perspective_with_changelog(pctx, &dup.id).await?;
        }
    }
    Ok(())
}

/// Pass 2: delete vanilla defaults that can never appear in any view.
///
/// - A default pinned to a `view_id` missing from the registry is
///   unreachable (its view was deleted or its id was never real).
/// - A legacy kind-shared default is fully shadowed when every view of its
///   kind already has a pinned default; it would only render as a
///   duplicate tab next to those.
///
/// Shadowing is deliberately **all-or-nothing**: a legacy kind-shared
/// default is ONE file serving every view of its kind, so it cannot be
/// deleted "per view". While only SOME views of the kind have pinned
/// defaults (partial shadowing), the legacy default survives — it is the
/// only Default the still-unpinned views have — and renders as a residual
/// duplicate "Default" tab in the views that do have a pinned one. That
/// residual resolves once every view of the kind gains its own pinned
/// default (this prune then removes the legacy file) or the user deletes
/// the legacy perspective themselves.
async fn prune_unreachable_defaults(
    pctx: &mut PerspectiveContext,
    views: &ViewsContext,
) -> Result<()> {
    let all_views = views.all_views();
    if all_views.is_empty() {
        return Ok(());
    }

    let mut doomed: Vec<String> = Vec::new();
    for p in pctx.all() {
        if p.name != DEFAULT_PERSPECTIVE_NAME || is_customized(p) {
            continue;
        }
        match p.view_id.as_deref() {
            Some(vid) => {
                if views.get_by_id(vid).is_none() {
                    tracing::info!(
                        perspective_id = %p.id,
                        view_id = %vid,
                        "removing default perspective pinned to a nonexistent view"
                    );
                    doomed.push(p.id.clone());
                }
            }
            None => {
                let kind_views: Vec<&str> = all_views
                    .iter()
                    .filter(|v| v.kind.as_kebab_str() == p.view)
                    .map(|v| v.id.as_str())
                    .collect();
                let fully_shadowed = !kind_views.is_empty()
                    && kind_views.iter().all(|vid| {
                        pctx.all().iter().any(|q| {
                            q.name == DEFAULT_PERSPECTIVE_NAME && q.view_id.as_deref() == Some(vid)
                        })
                    });
                if fully_shadowed {
                    tracing::info!(
                        perspective_id = %p.id,
                        view_kind = %p.view,
                        "removing legacy default shadowed by per-view defaults"
                    );
                    doomed.push(p.id.clone());
                }
            }
        }
    }

    for id in doomed {
        delete_perspective_with_changelog(pctx, &id).await?;
    }
    Ok(())
}

/// Pass 3: a board with zero perspectives gets its default recreated so the
/// user never sees an empty perspective bar.
async fn recover_zero_state(
    pctx: &mut PerspectiveContext,
    views: Option<&ViewsContext>,
) -> Result<()> {
    if !pctx.all().is_empty() {
        return Ok(());
    }

    let mut perspective = Perspective::new("pending", DEFAULT_PERSPECTIVE_NAME, "board");
    if let Some(views) = views {
        crate::perspective::migrate::maybe_pin_view_id_on_save(&mut perspective, views);
    }
    perspective.id = default_perspective_id(perspective_scope(&perspective));

    tracing::info!(
        perspective_id = %perspective.id,
        "recovering default perspective for a board with zero perspectives"
    );
    pctx.write(&perspective).await?;
    Ok(())
}

/// Delete a duplicate/unreachable default plus its sibling changelog file.
///
/// At board open the store handle is not wired yet, so
/// [`PerspectiveContext::delete`] removes only the `{id}.yaml`; the
/// `{id}.jsonl` changelog would otherwise be left orphaned on disk.
async fn delete_perspective_with_changelog(pctx: &mut PerspectiveContext, id: &str) -> Result<()> {
    pctx.delete(id).await?;
    let changelog = pctx.root().join(format!("{id}.jsonl"));
    if let Err(e) = tokio::fs::remove_file(&changelog).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(path = %changelog.display(), error = %e, "failed to remove perspective changelog");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vanilla(id: &str, view: &str, view_id: Option<&str>) -> Perspective {
        let mut p = Perspective::new(id, DEFAULT_PERSPECTIVE_NAME, view);
        p.view_id = view_id.map(String::from);
        p
    }

    async fn open_ctx(dir: &std::path::Path) -> PerspectiveContext {
        PerspectiveContext::open(dir).await.unwrap()
    }

    #[tokio::test]
    async fn dedup_prefers_customized_keeper_over_older_vanilla() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut pctx = open_ctx(temp.path()).await;

        // Older vanilla duplicate (smaller id sorts first by age)...
        pctx.write(&vanilla("01AAA", "board", None)).await.unwrap();
        // ...and a newer duplicate the user customized.
        let mut customized = vanilla("01BBB", "board", None);
        customized.filter = Some("#bug".into());
        pctx.write(&customized).await.unwrap();

        reconcile_default_perspectives(&mut pctx, None)
            .await
            .unwrap();

        assert_eq!(pctx.all().len(), 1);
        assert_eq!(
            pctx.all()[0].id,
            "01BBB",
            "the customized default must win over an older vanilla one"
        );
        assert_eq!(pctx.all()[0].filter.as_deref(), Some("#bug"));
    }

    #[tokio::test]
    async fn dedup_preserves_all_customized_defaults_sharing_one_scope() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut pctx = open_ctx(temp.path()).await;

        // Two CUSTOMIZED Defaults in the same scope — the state the
        // production duplicate-minting bug breeds (user customizes the
        // Default tab in window A while window B's stale cache mints
        // another, which the user also customizes). Both are user data.
        let mut filtered = vanilla("01AAA", "board", None);
        filtered.filter = Some("#bug".into());
        pctx.write(&filtered).await.unwrap();

        let mut grouped = vanilla("01BBB", "board", None);
        grouped.group = Some("assignee".into());
        pctx.write(&grouped).await.unwrap();

        reconcile_default_perspectives(&mut pctx, None)
            .await
            .unwrap();

        assert_eq!(
            pctx.all().len(),
            2,
            "customized duplicate defaults are user data and must never be deleted: {:?}",
            pctx.all()
        );
        let kept_filter = pctx.get_by_id("01AAA").expect("01AAA must survive");
        assert_eq!(kept_filter.filter.as_deref(), Some("#bug"));
        let kept_group = pctx.get_by_id("01BBB").expect("01BBB must survive");
        assert_eq!(kept_group.group.as_deref(), Some("assignee"));
    }

    #[tokio::test]
    async fn dedup_keeps_defaults_in_distinct_scopes() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut pctx = open_ctx(temp.path()).await;

        pctx.write(&vanilla("01AAA", "board", Some("view-1")))
            .await
            .unwrap();
        pctx.write(&vanilla("01BBB", "grid", Some("view-2")))
            .await
            .unwrap();

        reconcile_default_perspectives(&mut pctx, None)
            .await
            .unwrap();

        assert_eq!(
            pctx.all().len(),
            2,
            "defaults scoped to different views are not duplicates"
        );
    }

    #[tokio::test]
    async fn zero_state_recovery_uses_the_deterministic_id() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut pctx = open_ctx(temp.path()).await;

        reconcile_default_perspectives(&mut pctx, None)
            .await
            .unwrap();

        assert_eq!(pctx.all().len(), 1);
        let p = &pctx.all()[0];
        assert_eq!(p.name, DEFAULT_PERSPECTIVE_NAME);
        assert_eq!(p.view, "board");
        assert_eq!(p.id, default_perspective_id("board"));
    }

    // The `matches_scope` / scope-helper unit tests moved with the helpers to
    // `swissarmyhammer_perspectives::default_scope`.
}
