//! Integration tests for opt-in legacy-perspective view-id migration.
//!
//! These tests exercise the save-time helper
//! `perspective::migrate::maybe_pin_view_id_on_save` end-to-end through the
//! `add perspective` + `update perspective` dispatch path so the on-disk YAML
//! state is the source of truth: we re-read the YAML after the migration runs
//! and assert against the file contents directly.
//!
//! Three scenarios are covered:
//!
//! 1. Unambiguous kind → next `update perspective` writes `view_id:` back to
//!    disk (the typical migration path).
//! 2. Ambiguous kind (multiple views of the same kind) → `update perspective`
//!    leaves the YAML untouched; the perspective stays legacy.
//! 3. Ambiguous kind also emits a one-time `info!` so the user understands
//!    why the migration did not happen.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::json;
use swissarmyhammer_ui_state::UIState;
use swissarmyhammer_kanban::dynamic_sources::{build_dynamic_sources, DynamicSourcesInputs};
use swissarmyhammer_kanban::{
    board::InitBoard, dispatch::execute_operation, parse::parse_input, Execute, KanbanContext,
};
use swissarmyhammer_views::{ViewDef, ViewKind};
use tempfile::TempDir;
use tracing_test::traced_test;

/// Open a fresh board under a temp dir with views + perspectives initialised.
/// Returns the temp guard (kept alive for the test), the context, and the
/// canonical kanban directory.
async fn open_board(name: &str) -> (TempDir, Arc<KanbanContext>, PathBuf) {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::open(&kanban_dir)
        .await
        .expect("KanbanContext::open must succeed");

    InitBoard::new(name)
        .execute(&ctx)
        .await
        .into_result()
        .expect("InitBoard must succeed");

    let canonical = kanban_dir
        .canonicalize()
        .unwrap_or_else(|_| kanban_dir.clone());
    (temp, Arc::new(ctx), canonical)
}

/// Register a view of the given kind+id+name with the board's view registry.
async fn register_view(ctx: &KanbanContext, id: &str, name: &str, kind: ViewKind) {
    let views_lock = ctx.views().expect("KanbanContext must have a views ctx");
    let mut views = views_lock.write().await;
    let def = ViewDef {
        id: id.to_string(),
        name: name.to_string(),
        icon: None,
        kind,
        entity_type: Some("task".into()),
        card_fields: Vec::new(),
        commands: Vec::new(),
    };
    views
        .write_view(&def)
        .await
        .expect("write_view must succeed");
}

/// Write a perspective YAML directly to disk *without* going through the
/// `add perspective` dispatch path. This simulates a legacy on-disk
/// perspective from before `view_id` existed: the YAML has no `view_id:`
/// line at all.
///
/// The kanban context must be reopened afterwards (callers do this) so the
/// in-memory `PerspectiveContext` picks up the new file.
async fn seed_legacy_perspective_yaml(
    kanban_dir: &std::path::Path,
    id: &str,
    name: &str,
    view: &str,
) {
    let perspectives_dir = kanban_dir.join("perspectives");
    tokio::fs::create_dir_all(&perspectives_dir).await.unwrap();
    let path = perspectives_dir.join(format!("{id}.yaml"));
    let yaml = format!("id: {id}\nname: {name}\nview: {view}\n");
    tokio::fs::write(&path, yaml).await.unwrap();
}

/// Read a perspective YAML back from disk.
async fn read_perspective_yaml(kanban_dir: &std::path::Path, id: &str) -> String {
    let path = kanban_dir.join("perspectives").join(format!("{id}.yaml"));
    tokio::fs::read_to_string(&path).await.unwrap()
}

/// Dispatch one operation through the canonical parse → execute pipeline.
async fn dispatch(ctx: &KanbanContext, payload: serde_json::Value) -> serde_json::Value {
    let ops = parse_input(payload).expect("parse_input must succeed");
    assert_eq!(ops.len(), 1, "expected exactly one parsed operation");
    execute_operation(ctx, &ops[0])
        .await
        .expect("execute_operation must succeed")
}

/// A legacy perspective whose `view` kind matches exactly one registered view
/// gets `view_id:` auto-written to disk on the next `update perspective` —
/// the typical migration path.
#[tokio::test]
async fn legacy_perspective_unambiguous_kind_migrates_on_save() {
    let (_tmp, _bootstrap_ctx, kanban_dir) = open_board("Sample").await;

    // Register exactly one custom board-kind view. Note: the workspace's
    // built-in `board` view is also kind=board, so to make the kind truly
    // unambiguous we delete the on-disk builtin board YAML before seeding
    // the legacy perspective. The simplest path is to use a unique kind
    // string: pick a perspective `view` value matching a kind that has
    // exactly one view registered.
    //
    // We use `list` (kind=list): the builtin set does not include a list
    // view, so registering one makes it the sole list-kind view.
    let persp_id = "01JPERSP00000000000UNAMBIG";
    let view_id = "01VIEW00000000000000LIST1";
    seed_legacy_perspective_yaml(&kanban_dir, persp_id, "Legacy List", "list").await;

    // Reopen so the seeded perspective is loaded.
    let ctx = Arc::new(
        KanbanContext::open(&kanban_dir)
            .await
            .expect("reopen must succeed"),
    );
    register_view(&ctx, view_id, "My List", ViewKind::List).await;

    // Sanity-check the seeded YAML has no view_id line.
    let before = read_perspective_yaml(&kanban_dir, persp_id).await;
    assert!(
        !before.contains("view_id"),
        "seed yaml must not contain view_id; got:\n{before}"
    );

    // Trigger a re-save via `update perspective` (rename via the `name`
    // field is enough — the migration helper runs unconditionally on save).
    let updated = dispatch(
        &ctx,
        json!({
            "op": "update perspective",
            "id": persp_id,
            "name": "Legacy List (renamed)"
        }),
    )
    .await;

    // The in-memory result reflects the pinned id.
    assert_eq!(
        updated["view_id"].as_str(),
        Some(view_id),
        "update result must echo the pinned view_id; got {updated:?}"
    );

    // And, more importantly, the on-disk YAML carries the new view_id.
    let after = read_perspective_yaml(&kanban_dir, persp_id).await;
    assert!(
        after.contains(&format!("view_id: {view_id}")),
        "migrated YAML must include `view_id: {view_id}`; got:\n{after}"
    );
}

/// A legacy perspective whose kind matches multiple views must NOT auto-pin
/// on save — the user has to explicitly open it in a specific view and
/// re-save to disambiguate.
#[tokio::test]
async fn legacy_perspective_ambiguous_kind_stays_legacy() {
    let (_tmp, _bootstrap_ctx, kanban_dir) = open_board("Sample").await;

    let persp_id = "01JPERSP00000000000AMBIGS";
    seed_legacy_perspective_yaml(&kanban_dir, persp_id, "Legacy Grid", "grid").await;

    let ctx = Arc::new(
        KanbanContext::open(&kanban_dir)
            .await
            .expect("reopen must succeed"),
    );

    // Two grid-kind views — the kind match is ambiguous.
    register_view(&ctx, "01VIEW000000000000GRIDA0", "Grid A", ViewKind::Grid).await;
    register_view(&ctx, "01VIEW000000000000GRIDB0", "Grid B", ViewKind::Grid).await;

    // Re-save via `update perspective`.
    let updated = dispatch(
        &ctx,
        json!({
            "op": "update perspective",
            "id": persp_id,
            "name": "Legacy Grid (renamed)"
        }),
    )
    .await;

    // The result must not include a `view_id` key — the migration helper
    // declined to pin (or, if present, must be null/absent). `skip_serializing_if`
    // on the Perspective type drops the field entirely when `None`.
    assert!(
        updated.get("view_id").is_none() || updated["view_id"].is_null(),
        "ambiguous update result must not carry a view_id; got {updated:?}"
    );

    // YAML on disk must still be view-id-less.
    let after = read_perspective_yaml(&kanban_dir, persp_id).await;
    assert!(
        !after.contains("view_id"),
        "ambiguous YAML must remain free of view_id; got:\n{after}"
    );
}

/// A legacy perspective whose kind matches multiple views emits exactly one
/// `info!` log line per process so the user discovers why the migration did
/// not run. Repeated invocations of the gather path do not re-emit.
#[tokio::test]
#[traced_test]
async fn legacy_perspective_ambiguous_emits_one_time_log() {
    // Reset the once-per-process guard so this test sees a fresh state
    // regardless of which other tests ran first.
    swissarmyhammer_kanban::perspective::migrate::reset_legacy_log_guard_for_test();

    let (_tmp, _bootstrap_ctx, kanban_dir) = open_board("Sample").await;

    let persp_id = "01JPERSP00000000000LOGAMB";
    seed_legacy_perspective_yaml(&kanban_dir, persp_id, "Legacy Grid Log", "grid").await;

    let ctx = Arc::new(
        KanbanContext::open(&kanban_dir)
            .await
            .expect("reopen must succeed"),
    );

    // Two grid-kind views, so the perspective is ambiguous.
    register_view(&ctx, "01VIEW000000000000GRIDX0", "Grid X", ViewKind::Grid).await;
    register_view(&ctx, "01VIEW000000000000GRIDY0", "Grid Y", ViewKind::Grid).await;

    // Walk the gather path twice — the log must fire exactly once even
    // though both calls inspect the same legacy perspective.
    let board_path_str = kanban_dir.display().to_string();
    for _ in 0..2 {
        let ui = UIState::new();
        ui.add_open_board(&board_path_str);
        ui.set_active_view("main", "01VIEW000000000000GRIDX0");

        let mut open_boards: HashMap<PathBuf, Arc<KanbanContext>> = HashMap::new();
        open_boards.insert(kanban_dir.clone(), Arc::clone(&ctx));

        let _ = build_dynamic_sources(DynamicSourcesInputs {
            ui_state: &ui,
            active_ctx: Some(&ctx),
            open_board_ctxs: &open_boards,
            active_window_label: Some("main"),
            windows: vec![],
            ai_models: vec![],
        })
        .await;
    }

    // `tracing-test` exposes `logs_contain` (case-sensitive substring) so we
    // can assert on the body of the emitted line. The text must match the
    // canonical message from `perspective::migrate::log_legacy_perspectives_once`.
    assert!(
        logs_contain(persp_id),
        "info! line for the ambiguous perspective must be emitted"
    );
    assert!(
        logs_contain("remains shared across all grid views"),
        "log body must explain why the perspective did not migrate"
    );
    // Exact-count assertion via `logs_assert`: count lines that mention
    // BOTH the perspective id and the canonical migration log body. This
    // tightens the predicate so unrelated diagnostic tracing that mentions
    // the perspective id (e.g. iter-3 `[group-debug]` lines in
    // `dynamic_sources`) does not bleed into this assertion. The intent is
    // still "the migration log fires exactly once per process", which is
    // what the canonical body identifies.
    let needle = persp_id.to_string();
    let body_marker = "remains shared across all grid views";
    logs_assert(|lines: &[&str]| {
        let n = lines
            .iter()
            .filter(|line| line.contains(&needle) && line.contains(body_marker))
            .count();
        if n == 1 {
            Ok(())
        } else {
            Err(format!(
                "expected exactly one migration log line mentioning {needle}, got {n}"
            ))
        }
    });
}
