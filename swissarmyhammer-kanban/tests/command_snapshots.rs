//! Command snapshot regression tests.
//!
//! For each canonical scope, capture the full `commands_for_scope` output as
//! a JSON snapshot committed to `tests/snapshots/`. A future refactor that
//! silently reshapes menus (adds/removes a command, changes a name, drifts
//! a keybinding, reorders a group) produces a diff on re-run, which is the
//! signal that a surface change happened.
//!
//! Each canonical scope is captured twice — once with `context_menu_only:
//! true` and once with `false` — so the filter path and the full surface
//! both have regression coverage.
//!
//! # Regenerating snapshots
//!
//! Set `UPDATE_SNAPSHOTS=1` when running the test suite to rewrite every
//! snapshot file. Review the diff by hand (`git diff tests/snapshots/`),
//! commit intentional changes, and push. Never write these files by hand.
//!
//! ```text
//! UPDATE_SNAPSHOTS=1 cargo test -p swissarmyhammer-kanban --test command_snapshots
//! ```
//!
//! # Why id-stripping matters
//!
//! `commands_for_scope` embeds runtime-generated ULIDs in target monikers
//! (e.g. `task:01KPG7FABC...`). A naive snapshot would churn on every run.
//! Before serializing, [`SnapshotHarness::capture`] replaces live ULIDs with
//! the stable placeholder tokens used in the scope fixture (`TASK`, `TAG`,
//! etc.) — so the snapshot is a function of the scope shape alone.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_commands::{builtin_yaml_sources, Command, CommandsRegistry, UIState};
use swissarmyhammer_entity::EntityTypeStore;
use swissarmyhammer_fields::FieldsContext;
use swissarmyhammer_kanban::commands::register_commands;
use swissarmyhammer_kanban::defaults::{builtin_entity_definitions, builtin_field_definitions};
use swissarmyhammer_kanban::scope_commands::commands_for_scope;
use swissarmyhammer_kanban::{
    board::InitBoard, KanbanContext, KanbanOperationProcessor, OperationProcessor,
};
use swissarmyhammer_store::StoreHandle;
use tempfile::TempDir;

// =============================================================================
// Snapshot harness
// =============================================================================

struct SnapshotHarness {
    _temp: TempDir,
    kanban: Arc<KanbanContext>,
    commands: HashMap<String, Arc<dyn Command>>,
    registry: CommandsRegistry,
    fields: FieldsContext,
    ui_state: Arc<UIState>,
}

impl SnapshotHarness {
    /// Build a fresh harness with an initialized board.
    async fn new() -> Self {
        let temp = TempDir::new().expect("tempdir");
        let kanban_dir = temp.path().join(".kanban");
        let kanban = KanbanContext::new(&kanban_dir);

        let processor = KanbanOperationProcessor::new();
        processor
            .process(&InitBoard::new("Snapshot Test"), &kanban)
            .await
            .expect("board init failed");

        let kanban = Arc::new(kanban);

        // Register a StoreHandle for every entity type — production-like path.
        let ectx = kanban.entity_context().await.expect("entity_context");
        let fields_ctx = ectx.fields();
        for entity_def in fields_ctx.all_entities() {
            let entity_type = entity_def.name.as_str();
            let field_defs: Vec<_> = fields_ctx
                .fields_for_entity(entity_type)
                .into_iter()
                .cloned()
                .collect();
            let entity_type_store = EntityTypeStore::new(
                ectx.entity_dir(entity_type),
                entity_type,
                Arc::new(entity_def.clone()),
                Arc::new(field_defs),
            );
            let handle = Arc::new(StoreHandle::new(Arc::new(entity_type_store)));
            ectx.register_store(entity_type, handle).await;
        }

        let registry = CommandsRegistry::from_yaml_sources(&builtin_yaml_sources());
        let commands = register_commands();
        let fields = FieldsContext::from_yaml_sources(
            kanban.root().to_path_buf(),
            &builtin_field_definitions(),
            &builtin_entity_definitions(),
        )
        .expect("fields ctx");
        let ui_state = Arc::new(UIState::new());

        Self {
            _temp: temp,
            kanban,
            commands,
            registry,
            fields,
            ui_state,
        }
    }

    /// Capture the surface for `scope` with the given context_menu filter.
    ///
    /// Strips live (post-InitBoard) board ids out of the JSON so snapshots
    /// are stable across runs — otherwise every `board:{ULID}` embedded in a
    /// name or target changes per invocation.
    async fn capture(&self, scope: &[&str], context_menu_only: bool) -> Value {
        let scope_vec: Vec<String> = scope.iter().map(|s| s.to_string()).collect();
        let cmds = commands_for_scope(
            &scope_vec,
            &self.registry,
            &self.commands,
            Some(&self.fields),
            &self.ui_state,
            context_menu_only,
            None,
        );

        let mut rendered: Vec<Value> = cmds
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "name": c.name,
                    "menu_name": c.menu_name,
                    "target": c.target,
                    "group": c.group,
                    "context_menu": c.context_menu,
                    "keys": c.keys,
                    "available": c.available,
                })
            })
            .collect();

        // `commands_for_scope` iterates over HashMap-backed collections —
        // its output order is non-deterministic across runs. Sort by
        // (group, id, target) so the snapshot is a stable function of the
        // command surface, not of hash-iteration whim. We lose "insertion
        // order matches menu order" in snapshots but keep regression
        // coverage for presence/shape/availability; ordering is still
        // verified by the separate scope_commands unit tests that pin
        // specific orderings.
        rendered.sort_by(|a, b| {
            let ka = (
                a["group"].as_str().unwrap_or(""),
                a["id"].as_str().unwrap_or(""),
                a["target"].as_str().unwrap_or(""),
            );
            let kb = (
                b["group"].as_str().unwrap_or(""),
                b["id"].as_str().unwrap_or(""),
                b["target"].as_str().unwrap_or(""),
            );
            ka.cmp(&kb)
        });

        let mut snap = json!({
            "scope": scope,
            "context_menu_only": context_menu_only,
            "command_count": rendered.len(),
            "commands": rendered,
        });

        // Replace the live board id with a stable placeholder so snapshots
        // don't churn on the per-run ULID (or the `"board"` slug used by
        // InitBoard). We scope the replacement to `board:{id}` monikers so
        // we never touch unrelated literals.
        let board_id = self.board_id().await;
        let raw = serde_json::to_string_pretty(&snap).unwrap();
        let needle = format!("board:{board_id}");
        let stable = raw.replace(&needle, "board:BOARD_ID_STABLE");
        snap = serde_json::from_str(&stable).unwrap();

        snap
    }

    async fn board_id(&self) -> String {
        let ectx = self.kanban.entity_context().await.unwrap();
        let boards = ectx.list("board").await.unwrap();
        boards
            .first()
            .expect("board exists after InitBoard")
            .id
            .to_string()
    }
}

// =============================================================================
// Snapshot file IO
// =============================================================================

/// Path to the committed snapshot for a given name.
fn snapshot_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join(format!("{name}.json"))
}

/// Return `true` when the test run is generating fresh snapshots.
///
/// Controlled by `UPDATE_SNAPSHOTS=1`. Without it, a missing snapshot file
/// fails the test — the regeneration mode is explicit so CI can't quietly
/// invent new snapshots.
fn update_mode() -> bool {
    std::env::var("UPDATE_SNAPSHOTS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Compare `snapshot` against the committed file at `tests/snapshots/{name}.json`.
///
/// - If `UPDATE_SNAPSHOTS=1` is set: (over)writes the file.
/// - If the file is missing: fails with instructions to run with
///   `UPDATE_SNAPSHOTS=1`.
/// - Otherwise: asserts byte-identical JSON (after pretty-printing) and
///   fails with a human-readable diff on mismatch.
#[track_caller]
fn assert_snapshot(name: &str, snapshot: &Value) {
    let path = snapshot_path(name);
    let pretty = serde_json::to_string_pretty(snapshot).expect("serialize");

    if update_mode() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create snapshot dir");
        }
        // Preserve a trailing newline so `git diff` is friendly.
        let with_newline = format!("{pretty}\n");
        std::fs::write(&path, with_newline).expect("write snapshot");
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "snapshot file missing at {}: {e}\n\
             Run `UPDATE_SNAPSHOTS=1 cargo test -p swissarmyhammer-kanban --test command_snapshots` \
             to generate it.",
            path.display()
        )
    });
    // Strip the trailing newline introduced by our writer for comparison.
    let expected_trimmed = expected.trim_end_matches('\n');
    if expected_trimmed != pretty {
        panic!(
            "snapshot mismatch for {name}\n\
             File: {path}\n\
             \n\
             --- expected ---\n{expected_trimmed}\n\
             --- actual ---\n{pretty}\n\
             \n\
             If this is an intentional change, regenerate with:\n\
             UPDATE_SNAPSHOTS=1 cargo test -p swissarmyhammer-kanban --test command_snapshots\n",
            path = path.display(),
        );
    }
}

// =============================================================================
// Canonical scopes
// =============================================================================
//
// The 7 canonical scope shapes × 2 context_menu_only modes = 14 snapshot
// files committed under `tests/snapshots/`.
//
// Target monikers use stable placeholder IDs (`TASK`, `TAG`, etc.) so the
// snapshot is stable across runs. The `board:BOARD_ID_STABLE` token is
// produced by `SnapshotHarness::capture` — the live board id is rewritten
// after serialization.

const BOARD_SCOPE: &[&str] = &["board:BOARD_ID_STABLE"];
const COLUMN_SCOPE: &[&str] = &["column:todo", "board:BOARD_ID_STABLE"];
const TASK_SCOPE: &[&str] = &["task:TASK", "column:todo", "board:BOARD_ID_STABLE"];
const TAG_ON_TASK_SCOPE: &[&str] = &[
    "tag:TAG",
    "task:TASK",
    "column:todo",
    "board:BOARD_ID_STABLE",
];
const PROJECT_SCOPE: &[&str] = &["project:backend"];
const ACTOR_SCOPE: &[&str] = &["actor:alice"];
const ATTACHMENT_SCOPE: &[&str] = &["attachment:/tmp/x.png", "task:TASK", "column:todo"];

#[tokio::test]
async fn snapshot_board_full() {
    let h = SnapshotHarness::new().await;
    let snap = h.capture(BOARD_SCOPE, false).await;
    assert_snapshot("board_full", &snap);
}

#[tokio::test]
async fn snapshot_board_context_menu_only() {
    let h = SnapshotHarness::new().await;
    let snap = h.capture(BOARD_SCOPE, true).await;
    assert_snapshot("board_context_menu_only", &snap);
}

#[tokio::test]
async fn snapshot_column_full() {
    let h = SnapshotHarness::new().await;
    let snap = h.capture(COLUMN_SCOPE, false).await;
    assert_snapshot("column_full", &snap);
}

#[tokio::test]
async fn snapshot_column_context_menu_only() {
    let h = SnapshotHarness::new().await;
    let snap = h.capture(COLUMN_SCOPE, true).await;
    assert_snapshot("column_context_menu_only", &snap);
}

#[tokio::test]
async fn snapshot_task_full() {
    let h = SnapshotHarness::new().await;
    let snap = h.capture(TASK_SCOPE, false).await;
    assert_snapshot("task_full", &snap);
}

#[tokio::test]
async fn snapshot_task_context_menu_only() {
    let h = SnapshotHarness::new().await;
    let snap = h.capture(TASK_SCOPE, true).await;
    assert_snapshot("task_context_menu_only", &snap);
}

#[tokio::test]
async fn snapshot_tag_on_task_full() {
    let h = SnapshotHarness::new().await;
    let snap = h.capture(TAG_ON_TASK_SCOPE, false).await;
    assert_snapshot("tag_on_task_full", &snap);
}

#[tokio::test]
async fn snapshot_tag_on_task_context_menu_only() {
    let h = SnapshotHarness::new().await;
    let snap = h.capture(TAG_ON_TASK_SCOPE, true).await;
    assert_snapshot("tag_on_task_context_menu_only", &snap);
}

#[tokio::test]
async fn snapshot_project_full() {
    let h = SnapshotHarness::new().await;
    let snap = h.capture(PROJECT_SCOPE, false).await;
    assert_snapshot("project_full", &snap);
}

#[tokio::test]
async fn snapshot_project_context_menu_only() {
    let h = SnapshotHarness::new().await;
    let snap = h.capture(PROJECT_SCOPE, true).await;
    assert_snapshot("project_context_menu_only", &snap);
}

#[tokio::test]
async fn snapshot_actor_full() {
    let h = SnapshotHarness::new().await;
    let snap = h.capture(ACTOR_SCOPE, false).await;
    assert_snapshot("actor_full", &snap);
}

#[tokio::test]
async fn snapshot_actor_context_menu_only() {
    let h = SnapshotHarness::new().await;
    let snap = h.capture(ACTOR_SCOPE, true).await;
    assert_snapshot("actor_context_menu_only", &snap);
}

#[tokio::test]
async fn snapshot_attachment_full() {
    let h = SnapshotHarness::new().await;
    let snap = h.capture(ATTACHMENT_SCOPE, false).await;
    assert_snapshot("attachment_full", &snap);
}

#[tokio::test]
async fn snapshot_attachment_context_menu_only() {
    let h = SnapshotHarness::new().await;
    let snap = h.capture(ATTACHMENT_SCOPE, true).await;
    assert_snapshot("attachment_context_menu_only", &snap);
}
