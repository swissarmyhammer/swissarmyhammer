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

use swissarmyhammer_commands::{WindowInfo};
use swissarmyhammer_ui_state::{UIState};
use swissarmyhammer_perspectives::{PerspectiveFieldInfo, PerspectiveInfo};
use swissarmyhammer_views::ViewInfo;

use crate::commands::perspective_commands::perspective_belongs_to_active_view;
use crate::context::KanbanContext;
use crate::scope_commands::{BoardInfo, DynamicSources};

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
    /// Selectable AI models — caller-supplied for the same reason as
    /// `windows`: the model set is discovered by `swissarmyhammer-config`'s
    /// `ModelManager`, which the pure-domain kanban crate does not depend
    /// on. The GUI runtime enumerates the models (via `ai_list_models`)
    /// and passes the projected list in here so the `ai.models` options
    /// resolver can fill the `ai.model` command's model picker. Headless
    /// tests fabricate the list (often empty).
    pub ai_models: Vec<crate::commands::options_resolvers::AiModelInfo>,
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
/// 5. **AI models** pass through from the caller (same rationale as
///    windows — the model set is GUI-runtime data the kanban crate
///    cannot derive itself).
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
        ai_models: inputs.ai_models,
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
    let Some(ctx) = ctx else {
        return Vec::new();
    };
    let Ok(pctx) = ctx.perspective_context().await else {
        return Vec::new();
    };
    let Ok(pc) = pctx.try_read() else {
        return Vec::new();
    };

    // Hold the views lock for the whole iteration so each perspective can
    // resolve its bound view's `entity_type` (the input to
    // `denormalize_perspective_fields`'s entity-schema lookup). Held
    // sync because the views lock is rarely contended and the iteration
    // is bounded by the perspective count.
    let views_guard = match ctx.views() {
        Some(views_lock) => views_lock.try_read().ok(),
        None => None,
    };
    let views_slice: &[swissarmyhammer_views::ViewDef] = views_guard
        .as_deref()
        .map(|vc| vc.all_views())
        .unwrap_or(&[]);

    // One-time discovery log for legacy view-id-less perspectives. The
    // helper guards against repeated emissions, so calling it on every
    // `list_commands_for_scope` invocation is cheap. See
    // `perspective::migrate` for the placement rationale.
    if let Some(vc) = views_guard.as_deref() {
        crate::perspective::migrate::log_legacy_perspectives_once(pc.all(), vc);
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
            fields: denormalize_perspective_fields(p, fields_ctx, views_slice, active_view_id),
        })
        .collect()
}

/// Resolve the entity-type a perspective is bound to via its view.
///
/// A perspective points at a view in one of three shapes, tried in order:
///
/// - **Strict (preferred)**: `view_id: Some(id)` pins the perspective to
///   exactly that view instance — we look up the matching `ViewDef` by
///   id and read its `entity_type`.
/// - **Active-view tiebreaker**: `view_id: None` AND the caller-supplied
///   `active_view_id` names a view whose kind matches the perspective's
///   `view`. The perspective is currently *being shown in* that view, so
///   its picker should answer for that view's entity_type. Without this
///   tier, every view-id-less perspective on a workspace with multiple
///   views of the same kind (e.g. three grid-kind builtins each pointing
///   at a different entity) collapsed to ambiguous → empty picker. This
///   is the regression task `01KRGW1DYD0T05PSTEDPT5D076` fixes — the
///   prior fix only sourced from the entity schema, but the entity-type
///   derivation itself was returning `None` for the user's actual setup.
/// - **Legacy (shared-by-kind)**: `view_id: None` AND no active-view
///   tiebreaker resolves — fall back to matching by `view` kind. When
///   multiple views share that kind but disagree on entity_type, the
///   resolver returns `None` rather than guess. Preserves the original
///   legacy-perspective semantics for command palettes / context menus
///   that don't carry an active view in scope.
///
/// Returns `None` when no view matches, the matching view has no
/// `entity_type`, or the legacy fallback resolves to multiple views with
/// conflicting entity types and no active-view tiebreaker disambiguates.
fn entity_type_for_perspective<'a>(
    perspective: &swissarmyhammer_perspectives::Perspective,
    views: &'a [swissarmyhammer_views::ViewDef],
    active_view_id: Option<&str>,
) -> Option<&'a str> {
    // Strict path: view_id pinpoints exactly one view.
    if let Some(view_id) = perspective.view_id.as_deref() {
        return views
            .iter()
            .find(|v| v.id.as_str() == view_id)
            .and_then(|v| v.entity_type.as_deref());
    }

    // Active-view tiebreaker: if the caller passed an active view id and
    // it matches the perspective's `view` kind, prefer that view's
    // entity_type. The perspective is being rendered IN that view right
    // now, so the picker should answer for that view's schema even when
    // multiple views of the same kind disagree on entity_type. This is
    // the user's production case — view-id-less perspectives on a
    // workspace with builtin tasks-grid / projects-grid / tags-grid
    // (all grid-kind, different entity types) were collapsing to
    // ambiguous → empty.
    if let Some(active_id) = active_view_id {
        if let Some(active_view) = views.iter().find(|v| v.id.as_str() == active_id) {
            if active_view.kind.as_kebab_str() == perspective.view {
                if let Some(et) = active_view.entity_type.as_deref() {
                    return Some(et);
                }
            }
        }
    }

    // Legacy path: match by view kind. Only safe when every matching
    // view agrees on entity_type; otherwise the picker can't pick one.
    let mut matched_entity: Option<&str> = None;
    for v in views {
        if v.kind.as_kebab_str() != perspective.view {
            continue;
        }
        let Some(et) = v.entity_type.as_deref() else {
            continue;
        };
        match matched_entity {
            None => matched_entity = Some(et),
            Some(prior) if prior == et => {}
            Some(_) => {
                // conflicting entity types — bail out
                return None;
            }
        }
    }
    matched_entity
}

/// Build the Group By picker's option list for a perspective.
///
/// Enumerates every field on the perspective's bound entity type and
/// keeps only those marked `groupable: true` in the entity schema.
/// This mirrors the legacy `<GroupSelector>` source — pre-migration the
/// component received `fields={schemaFields}` where `schemaFields`
/// came from `getSchema(entityType)?.fields ?? []`, then filtered by
/// `f.groupable === true`. The command-driven-ui migration must
/// preserve that semantics so a user perspective with an empty
/// `fields[]` (the common case — every real perspective at
/// `.kanban/perspectives/*.yaml` has no column overrides) still
/// surfaces every groupable schema field in the popover.
///
/// # Why source from the schema, not the perspective's `fields[]`
///
/// `perspective.fields[]` is the user's *visible column list* — the
/// columns rendered in the grid view. It is unrelated to "what is a
/// valid group key". Most user perspectives leave it empty (relying on
/// view defaults), so sourcing the picker from it would empty the
/// popover. The entity schema is the right source: every field that
/// exists on the entity can in principle be a group key, and the
/// `groupable` annotation selects the subset that *should* be shown.
///
/// Entity resolution falls through to [`entity_type_for_perspective`]:
/// strict `view_id` lookup first, active-view tiebreaker second, legacy
/// by-kind fallback third. When the entity cannot be resolved (no views
/// supplied, view not found, or every fallback is ambiguous), the
/// function returns an empty `Vec` — matches the legacy behavior of "no
/// schema, no options".
///
/// `active_view_id` lets the resolver disambiguate view-id-less
/// perspectives against the currently-focused view: a user perspective
/// with `view: "grid"` and `view_id: None` on a workspace with three
/// grid-kind builtins (tasks-grid → task, projects-grid → project,
/// tags-grid → tag) is ambiguous by kind alone, but the active view's
/// entity_type makes the picker answerable. Pass `None` from contexts
/// without a focused view (e.g. command palette before any view opens).
///
/// When `fields_ctx` is `None` the function returns an empty `Vec` (we
/// have no field registry to enumerate); production code always
/// threads a context in, and the `None` arm is exercised only by
/// minimal headless fixtures that don't care about the picker payload.
fn denormalize_perspective_fields(
    perspective: &swissarmyhammer_perspectives::Perspective,
    fields_ctx: Option<&swissarmyhammer_fields::FieldsContext>,
    views: &[swissarmyhammer_views::ViewDef],
    active_view_id: Option<&str>,
) -> Vec<PerspectiveFieldInfo> {
    let Some(fc) = fields_ctx else {
        return Vec::new();
    };
    let Some(entity_type) = entity_type_for_perspective(perspective, views, active_view_id) else {
        return Vec::new();
    };
    fc.fields_for_entity(entity_type)
        .into_iter()
        .filter(|fd| fd.groupable == Some(true))
        .map(|fd| PerspectiveFieldInfo {
            id: fd.id.as_str().to_string(),
            // Field name (schema slug) — the wire value the
            // `perspective.fields` picker emits and the key tasks use
            // in their `fields` map. Without a separate display caption
            // on `FieldDef`, the slug is also the fallback for
            // `display_name`. See `PerspectiveFieldInfo` for the full
            // round-trip contract.
            name: fd.name.as_str().to_string(),
            display_name: fd.name.as_str().to_string(),
        })
        .collect()
}

/// Read the board entity's `name` field from the entity store.
///
/// Returns the `name` field of the board entity (entity type `board`, id
/// `board`) — the canonical display name set during `init board` and
/// editable by the user. Returns `None` when the entity store isn't
/// reachable, the `board/board` entity doesn't exist yet, or the entity
/// has no non-empty `name` field.
///
/// Used by both the headless [`gather_boards`] assembly here and the GUI
/// crate (window titles, the open-boards list, board-switch handlers).
pub async fn board_display_name(ctx: &KanbanContext) -> Option<String> {
    let ectx = ctx.entity_context().await.ok()?;
    let entity = ectx.read("board", "board").await.ok()?;
    entity
        .fields
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use swissarmyhammer_fields::FieldsContext;
    use swissarmyhammer_perspectives::Perspective;
    use swissarmyhammer_views::{ViewDef, ViewKind};

    /// Build a [`FieldsContext`] for a single entity (`thing`) with three
    /// fields: two groupable (`title`, `status`), one non-groupable
    /// (`body`). The fixture matches the new `denormalize_perspective_fields`
    /// contract — its source is the entity schema, not the perspective's
    /// column list.
    fn fields_ctx_with_groupable_mix() -> FieldsContext {
        // Field ULIDs are stable test sentinels chosen so the assertions
        // below can match on exact id strings.
        let title = r#"
id: 00000000000000000000000F01
name: title
type:
  kind: text
  single_line: true
groupable: true
"#;
        let body = r#"
id: 00000000000000000000000F02
name: body
type:
  kind: text
  single_line: true
"#;
        let status = r#"
id: 00000000000000000000000F03
name: status
type:
  kind: text
  single_line: true
groupable: true
"#;
        // Entity definition wires the three fields onto a single `thing`
        // entity so `fields_for_entity("thing")` returns them in
        // template order.
        let thing = r#"
name: thing
fields:
  - title
  - body
  - status
"#;
        FieldsContext::from_yaml_sources(
            PathBuf::from("/tmp/dynamic_sources_test"),
            &[("title", title), ("body", body), ("status", status)],
            &[("thing", thing)],
        )
        .expect("FieldsContext fixture must build")
    }

    /// Build a single grid-kind [`ViewDef`] for the `thing` entity with
    /// the given id, so a perspective with `view_id = Some(id)` resolves
    /// to `entity_type = thing`.
    fn view_for_thing(id: &str) -> ViewDef {
        ViewDef {
            id: id.into(),
            name: "Things".into(),
            icon: None,
            kind: ViewKind::Grid,
            entity_type: Some("thing".into()),
            card_fields: Vec::new(),
            commands: Vec::new(),
        }
    }

    /// Happy path: a perspective bound to a view whose entity_type is
    /// `thing` projects the two groupable schema fields (`title`,
    /// `status`) onto [`PerspectiveFieldInfo`] entries, in entity-schema
    /// order. The non-groupable `body` field is dropped.
    #[test]
    fn denormalize_emits_entity_groupable_fields_in_schema_order() {
        let fc = fields_ctx_with_groupable_mix();
        let view = view_for_thing("01VIEW0");
        let p = Perspective::new("01P", "Active Sprint", "grid").with_view_id(view.id.clone());
        let out = denormalize_perspective_fields(&p, Some(&fc), &[view], None);
        assert_eq!(
            out.len(),
            2,
            "two of three schema fields are groupable; got {out:?}"
        );
        // Entity defines fields in order [title, body, status]; the
        // non-groupable `body` is dropped, so survivors keep schema
        // order [title, status].
        assert_eq!(out[0].id, "00000000000000000000000F01");
        assert_eq!(out[0].display_name, "title");
        assert_eq!(out[1].id, "00000000000000000000000F03");
        assert_eq!(out[1].display_name, "status");
    }

    /// Negative case: non-groupable schema fields must not surface in
    /// the denormalised output. Pins the "Group By picker only shows
    /// fields the user can group on" contract — same intent as the
    /// legacy `<GroupSelector>`'s `f.groupable === true` filter, applied
    /// against the entity schema rather than the (often empty)
    /// perspective `fields[]` column list.
    #[test]
    fn denormalize_drops_non_groupable_schema_fields() {
        let fc = fields_ctx_with_groupable_mix();
        let view = view_for_thing("01VIEW0");
        let p = Perspective::new("01P", "Active Sprint", "grid").with_view_id(view.id.clone());
        let out = denormalize_perspective_fields(&p, Some(&fc), &[view], None);
        let ids: Vec<&str> = out.iter().map(|f| f.id.as_str()).collect();
        assert!(
            !ids.contains(&"00000000000000000000000F02"),
            "non-groupable schema field must not surface in the picker; got {ids:?}"
        );
    }

    /// A perspective with empty `fields[]` (the common case — every
    /// real user perspective at `.kanban/perspectives/*.yaml` has no
    /// column overrides) still produces a non-empty picker because the
    /// source is the entity schema, not `perspective.fields`. This is
    /// the regression the task `01KRGW1DYD0T05PSTEDPT5D076` fixes:
    /// pre-fix the picker was sourced from `perspective.fields[]` and
    /// was empty for every real perspective.
    #[test]
    fn denormalize_picker_is_non_empty_when_perspective_fields_is_empty() {
        let fc = fields_ctx_with_groupable_mix();
        let view = view_for_thing("01VIEW0");
        // No `with_fields` call — the perspective's column list is the
        // default empty Vec.
        let p = Perspective::new("01P", "Active Sprint", "grid").with_view_id(view.id.clone());
        assert!(
            p.fields.is_empty(),
            "test precondition: perspective.fields must be empty"
        );
        let out = denormalize_perspective_fields(&p, Some(&fc), &[view], None);
        assert!(
            !out.is_empty(),
            "picker must enumerate entity schema's groupable fields, not the \
             perspective's (empty) column list; got {out:?}"
        );
    }

    /// Legacy fallback: a perspective with `view_id: None` resolves its
    /// entity type by matching `view` (kind string) against the
    /// supplied views. When exactly one view of that kind carries an
    /// `entity_type`, the picker is populated from that entity's
    /// groupable fields. Mirrors the by-kind perspective-binding path.
    #[test]
    fn denormalize_legacy_view_id_none_resolves_by_kind() {
        let fc = fields_ctx_with_groupable_mix();
        let view = view_for_thing("01VIEW0");
        // No `with_view_id` call — view_id stays None and the kebab
        // `view` field ("grid") is used for kind matching.
        let p = Perspective::new("01P", "Active Sprint", "grid");
        let out = denormalize_perspective_fields(&p, Some(&fc), &[view], None);
        assert_eq!(
            out.len(),
            2,
            "two groupable fields from the kind-matched entity; got {out:?}"
        );
    }

    /// Legacy fallback with ambiguity: when multiple views of the same
    /// kind point at conflicting entity types, the resolver refuses to
    /// guess and returns an empty list. The frontend treats this as
    /// "no schema selected → empty picker" — matches the legacy
    /// behavior of `getSchema(entityType)?.fields ?? []` when
    /// `entityType` couldn't be resolved.
    #[test]
    fn denormalize_legacy_by_kind_empty_when_ambiguous() {
        let fc = fields_ctx_with_groupable_mix();
        let view_thing = view_for_thing("01VIEW0");
        let view_other = ViewDef {
            id: "01VIEW1".into(),
            name: "Other".into(),
            icon: None,
            kind: ViewKind::Grid,
            entity_type: Some("other".into()),
            card_fields: Vec::new(),
            commands: Vec::new(),
        };
        let p = Perspective::new("01P", "Active Sprint", "grid");
        let out = denormalize_perspective_fields(&p, Some(&fc), &[view_thing, view_other], None);
        assert!(
            out.is_empty(),
            "ambiguous by-kind entity resolution must yield no options; got {out:?}"
        );
    }

    /// A perspective bound by `view_id` to an id no view carries
    /// resolves to no entity type and the picker is empty.
    #[test]
    fn denormalize_unknown_view_id_yields_empty() {
        let fc = fields_ctx_with_groupable_mix();
        let view = view_for_thing("01VIEW0");
        let p = Perspective::new("01P", "Active Sprint", "grid").with_view_id("does-not-exist");
        let out = denormalize_perspective_fields(&p, Some(&fc), &[view], None);
        assert!(
            out.is_empty(),
            "unknown view_id must yield no options; got {out:?}"
        );
    }

    /// When no `FieldsContext` is supplied the function returns an empty
    /// vec — without a registry we cannot enumerate the entity schema
    /// or read `groupable` on its fields.
    #[test]
    fn denormalize_returns_empty_without_fields_context() {
        let view = view_for_thing("01VIEW0");
        let p = Perspective::new("01P", "Active Sprint", "grid").with_view_id(view.id.clone());
        let out = denormalize_perspective_fields(&p, None, &[view], None);
        assert!(out.is_empty());
    }

    /// Active-view tiebreaker path: a perspective with `view_id: None`
    /// AND multiple views of the same kind with conflicting entity
    /// types still resolves to a non-empty picker when the caller
    /// supplies an `active_view_id` whose kind matches the
    /// perspective's `view`. The resolver prefers the active view's
    /// entity_type over the ambiguous by-kind fallback.
    ///
    /// This is the regression that task `01KRGW1DYD0T05PSTEDPT5D076`
    /// (iteration 2) fixes — the user's actual setup has multiple
    /// grid-kind views (tasks-grid → task, projects-grid → project,
    /// tags-grid → tag) plus a legacy `view: "grid"` perspective with
    /// no `view_id`. Pre-fix the picker was empty because the by-kind
    /// fallback found conflicting entity types and bailed out; the
    /// fix uses the active view in scope to disambiguate.
    #[test]
    fn denormalize_active_view_disambiguates_legacy_by_kind() {
        let fc = fields_ctx_with_groupable_mix();
        let view_thing = view_for_thing("01VIEW0");
        let view_other = ViewDef {
            id: "01VIEW1".into(),
            name: "Other".into(),
            icon: None,
            kind: ViewKind::Grid,
            entity_type: Some("other".into()),
            card_fields: Vec::new(),
            commands: Vec::new(),
        };
        // Legacy perspective: view: "grid", view_id: None — the user's
        // production shape for every perspective saved before the
        // save-time pin migration.
        let p = Perspective::new("01P", "Active Sprint", "grid");
        // Active view is the `thing`-entity grid; the picker should
        // answer for `thing`'s groupable fields even though a sibling
        // grid view points at `other`.
        let out = denormalize_perspective_fields(
            &p,
            Some(&fc),
            &[view_thing, view_other],
            Some("01VIEW0"),
        );
        assert_eq!(
            out.len(),
            2,
            "active-view tiebreaker must populate the picker even when \
             by-kind matching alone is ambiguous; got {out:?}"
        );
    }

    /// Active-view tiebreaker is ignored when the active view's kind
    /// does NOT match the perspective's `view`. The perspective is
    /// being viewed cross-kind (legacy data hygiene case) — falling
    /// back to by-kind matching is still the right answer, so an
    /// unambiguous match should still resolve.
    #[test]
    fn denormalize_active_view_ignored_when_kind_mismatches() {
        let fc = fields_ctx_with_groupable_mix();
        let view_grid = view_for_thing("01VIEW0");
        let view_board = ViewDef {
            id: "01VIEW2".into(),
            name: "Board".into(),
            icon: None,
            kind: ViewKind::Board,
            entity_type: Some("other".into()),
            card_fields: Vec::new(),
            commands: Vec::new(),
        };
        // Perspective declares `view: "grid"` but the active view in
        // scope is a `board`-kind view — the tiebreaker can't apply,
        // and the legacy by-kind fallback finds exactly one grid view
        // → resolves to `thing`'s groupable fields.
        let p = Perspective::new("01P", "Active Sprint", "grid");
        let out = denormalize_perspective_fields(
            &p,
            Some(&fc),
            &[view_grid, view_board],
            Some("01VIEW2"),
        );
        assert_eq!(
            out.len(),
            2,
            "kind-mismatched active view must fall through to by-kind \
             matching; got {out:?}"
        );
    }
}
