//! End-to-end undo verification for cross-cutting mutating commands.
//!
//! Every cross-cutting mutating command (`entity.delete`, `entity.archive`,
//! `entity.unarchive`, `entity.paste`) is declared `undoable: true` in
//! `entity.yaml`. The contract is that these mutations flow through the
//! `KanbanOperationProcessor` (via `commands::run_op` or, for the
//! polymorphic non-task archive/unarchive paths, through
//! `EntityContext::archive` / `EntityContext::unarchive`) and the underlying
//! `StoreHandle::write` / `delete` / `archive` / `unarchive_latest`
//! pushes onto the shared `StoreContext` undo stack.
//!
//! These tests exercise that loop end-to-end: dispatch the command via the
//! registry → assert the on-disk state changed → invoke `app.undo` →
//! assert the state was restored → invoke `app.redo` → assert it was
//! reapplied. They also verify the negative — `entity.copy` is
//! non-mutating at the entity layer and must NOT push anything onto the
//! undo stack.
//!
//! The harness mirrors `command_dispatch_integration.rs::TestEngine` but
//! always wires up a `StoreContext`, since undo is a no-op without one.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_commands::{
    builtin_yaml_sources, Command, CommandContext, CommandError, CommandsRegistry, UIState,
};
use swissarmyhammer_entity::EntityTypeStore;
use swissarmyhammer_kanban::clipboard::{
    ClipboardProvider, ClipboardProviderExt, InMemoryClipboard,
};
use swissarmyhammer_kanban::commands::register_commands;
use swissarmyhammer_kanban::{
    board::InitBoard, KanbanContext, KanbanOperationProcessor, OperationProcessor,
};
use swissarmyhammer_store::{StoreContext, StoreHandle};
use tempfile::TempDir;

// ===========================================================================
// Test harness — full production-like wiring with StoreContext + StoreHandles.
// ===========================================================================

/// Production-like test engine: temp board, registered command map, in-memory
/// clipboard, shared `UIState`, and a `StoreContext` with a `StoreHandle`
/// registered for every entity type.
///
/// Every write/delete/archive/unarchive performed via the entity context
/// pushes a `UndoEntryId` onto `StoreContext`'s undo stack, so the
/// `app.undo` / `app.redo` commands can reverse the mutation.
struct UndoEngine {
    _temp: TempDir,
    kanban: Arc<KanbanContext>,
    commands: HashMap<String, Arc<dyn Command>>,
    ui_state: Arc<UIState>,
    clipboard: Arc<InMemoryClipboard>,
    store_context: Arc<StoreContext>,
}

impl UndoEngine {
    /// Set up a fresh kanban board with `StoreContext` + per-type
    /// `StoreHandle`s wired into the entity context.
    async fn new() -> Self {
        let temp = TempDir::new().expect("temp dir");
        let kanban_dir = temp.path().join(".kanban");
        let kanban = KanbanContext::new(&kanban_dir);

        // Initialize the board (creates default columns + board entity)
        let processor = KanbanOperationProcessor::new();
        processor
            .process(&InitBoard::new("Undo Test Board"), &kanban)
            .await
            .expect("board init");

        let kanban = Arc::new(kanban);

        // Wire StoreContext into the entity context so writes push to the
        // shared undo stack.
        let store_context = Arc::new(StoreContext::new(kanban.root().to_path_buf()));
        let ectx = kanban.entity_context().await.expect("entity context");
        ectx.set_store_context(Arc::clone(&store_context));

        // Register a StoreHandle for every entity type — same shape as
        // `kanban-app/src/state.rs` production setup.
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
            ectx.register_store(entity_type, Arc::clone(&handle)).await;
            store_context.register(handle).await;
        }

        // Sanity: the canonical YAML registry must declare these commands
        // with their expected `undoable` flags. If a future YAML edit drops
        // `undoable: true` from a mutating command, the auto-emit dispatch
        // would silently bypass the operation processor and undo would
        // become a no-op for that command — these regression guards fail
        // fast if that happens.
        let registry = CommandsRegistry::from_yaml_sources(&builtin_yaml_sources());
        assert!(
            registry
                .get("entity.delete")
                .expect("entity.delete in YAML")
                .undoable,
            "entity.delete must be undoable in YAML"
        );
        assert!(
            registry
                .get("entity.archive")
                .expect("entity.archive in YAML")
                .undoable,
            "entity.archive must be undoable in YAML"
        );
        assert!(
            registry
                .get("entity.unarchive")
                .expect("entity.unarchive in YAML")
                .undoable,
            "entity.unarchive must be undoable in YAML"
        );
        assert!(
            registry
                .get("entity.paste")
                .expect("entity.paste in YAML")
                .undoable,
            "entity.paste must be undoable in YAML"
        );
        assert!(
            !registry
                .get("entity.copy")
                .expect("entity.copy in YAML")
                .undoable,
            "entity.copy must NOT be undoable in YAML — it does not mutate the entity layer"
        );

        Self {
            _temp: temp,
            kanban,
            commands: register_commands(),
            ui_state: Arc::new(UIState::new()),
            clipboard: Arc::new(InMemoryClipboard::new()),
            store_context,
        }
    }

    /// Dispatch a command through the full availability + execute cycle.
    ///
    /// Mirrors `command_dispatch_integration::TestEngine::dispatch` — wires
    /// every extension (kanban context, entity context, clipboard, store
    /// context) the production dispatcher attaches.
    async fn dispatch(
        &self,
        cmd_id: &str,
        scope: &[&str],
        target: Option<&str>,
        args: HashMap<String, Value>,
    ) -> swissarmyhammer_commands::Result<Value> {
        let cmd = self
            .commands
            .get(cmd_id)
            .ok_or_else(|| CommandError::ExecutionFailed(format!("unknown command: {cmd_id}")))?;

        let mut ctx = CommandContext::new(
            cmd_id,
            scope.iter().map(|s| s.to_string()).collect(),
            target.map(|s| s.to_string()),
            args,
        );
        ctx.ui_state = Some(Arc::clone(&self.ui_state));
        ctx.set_extension(Arc::clone(&self.kanban));
        let ectx = self.kanban.entity_context().await.expect("entity context");
        ctx.set_extension(ectx);
        let clipboard_ext = ClipboardProviderExt(Arc::clone(&self.clipboard)
            as Arc<dyn swissarmyhammer_kanban::clipboard::ClipboardProvider>);
        ctx.set_extension(Arc::new(clipboard_ext));
        ctx.set_extension(Arc::clone(&self.store_context));

        if !cmd.available(&ctx) {
            return Err(CommandError::ExecutionFailed(format!(
                "command '{cmd_id}' not available in this context"
            )));
        }

        cmd.execute(&ctx).await
    }

    /// Shorthand for `dispatch` with no extra args.
    async fn dispatch_simple(
        &self,
        cmd_id: &str,
        scope: &[&str],
        target: Option<&str>,
    ) -> swissarmyhammer_commands::Result<Value> {
        self.dispatch(cmd_id, scope, target, HashMap::new()).await
    }

    /// Add a task via the unified `entity.add` path. Returns the task's id.
    async fn add_task(&self, title: &str) -> String {
        let mut args = HashMap::new();
        args.insert("entity_type".into(), json!("task"));
        args.insert("title".into(), json!(title));
        let result = self
            .dispatch("entity.add", &[], None, args)
            .await
            .expect("entity.add:task");
        result["id"]
            .as_str()
            .expect("entity.add must return an id")
            .to_string()
    }

    /// Add a tag via the unified `entity.add` path. Returns the tag's id.
    async fn add_tag(&self, name: &str) -> String {
        let mut args = HashMap::new();
        args.insert("entity_type".into(), json!("tag"));
        args.insert("tag_name".into(), json!(name));
        let result = self
            .dispatch("entity.add", &[], None, args)
            .await
            .expect("entity.add:tag");
        result["id"]
            .as_str()
            .expect("entity.add:tag must return an id")
            .to_string()
    }

    /// Add a project via the unified `entity.add` path. Returns the project's id.
    async fn add_project(&self, name: &str) -> String {
        let mut args = HashMap::new();
        args.insert("entity_type".into(), json!("project"));
        args.insert("name".into(), json!(name));
        let result = self
            .dispatch("entity.add", &[], None, args)
            .await
            .expect("entity.add:project");
        result["id"]
            .as_str()
            .expect("entity.add:project must return an id")
            .to_string()
    }

    /// Set the UIState's `can_undo` / `can_redo` flags from the live store
    /// stack. Production wires this through a state listener; tests must
    /// poke it manually before each `app.undo` / `app.redo` dispatch since
    /// `UndoCmd::available` reads UIState rather than the stack directly.
    async fn sync_undo_state(&self) {
        let can_undo = self.store_context.can_undo().await;
        let can_redo = self.store_context.can_redo().await;
        self.ui_state.set_undo_redo_state(can_undo, can_redo);
    }

    /// Drive `app.undo` and assert it succeeded (returns `{"undone": true}`).
    async fn undo(&self) {
        self.sync_undo_state().await;
        let result = self
            .dispatch_simple("app.undo", &[], None)
            .await
            .expect("app.undo");
        assert_eq!(
            result["undone"], true,
            "app.undo must return {{\"undone\": true}}; got {result}"
        );
    }

    /// Drive `app.redo` and assert it succeeded (returns `{"redone": true}`).
    async fn redo(&self) {
        self.sync_undo_state().await;
        let result = self
            .dispatch_simple("app.redo", &[], None)
            .await
            .expect("app.redo");
        assert_eq!(
            result["redone"], true,
            "app.redo must return {{\"redone\": true}}; got {result}"
        );
    }
}

// ===========================================================================
// Tests
// ===========================================================================

/// Deleting a tag via `entity.delete` (the cross-cutting auto-emit) must
/// land on the shared undo stack. Undo restores the tag; redo deletes it
/// again.
#[tokio::test]
async fn undo_entity_delete_restores_tag() {
    let engine = UndoEngine::new().await;
    let tag_id = engine.add_tag("bug").await;

    // Sanity — the tag exists before deletion.
    let ectx = engine.kanban.entity_context().await.unwrap();
    assert!(
        ectx.read("tag", &tag_id).await.is_ok(),
        "tag must exist before delete"
    );

    // Delete via cross-cutting `entity.delete`.
    engine
        .dispatch_simple("entity.delete", &[], Some(&format!("tag:{tag_id}")))
        .await
        .expect("entity.delete should succeed for tag");
    assert!(
        ectx.read("tag", &tag_id).await.is_err(),
        "tag must be gone after entity.delete"
    );

    // Undo — tag is restored.
    engine.undo().await;
    assert!(
        ectx.read("tag", &tag_id).await.is_ok(),
        "tag must be restored after undo"
    );

    // Redo — tag is deleted again.
    engine.redo().await;
    assert!(
        ectx.read("tag", &tag_id).await.is_err(),
        "tag must be gone again after redo"
    );
}

/// Same as `undo_entity_delete_restores_tag` but for tasks — guards the
/// task-specific delete branch in `DeleteEntityCmd` (which routes through
/// `crate::task::DeleteTask` rather than the generic entity delete).
#[tokio::test]
async fn undo_entity_delete_restores_task() {
    let engine = UndoEngine::new().await;
    let task_id = engine.add_task("Restore me").await;

    let ectx = engine.kanban.entity_context().await.unwrap();
    assert!(
        ectx.read("task", &task_id).await.is_ok(),
        "task must exist before delete"
    );

    engine
        .dispatch_simple("entity.delete", &[], Some(&format!("task:{task_id}")))
        .await
        .expect("entity.delete should succeed for task");
    assert!(
        ectx.read("task", &task_id).await.is_err(),
        "task must be gone after entity.delete"
    );

    engine.undo().await;
    assert!(
        ectx.read("task", &task_id).await.is_ok(),
        "task must be restored after undo"
    );
}

/// Archiving a project via `entity.archive` (the non-task branch — calls
/// `EntityContext::archive` directly) must land on the undo stack. Undo
/// restores the project to live storage; redo archives it again.
#[tokio::test]
async fn undo_entity_archive_restores_project() {
    let engine = UndoEngine::new().await;
    let project_id = engine.add_project("Backend").await;

    let ectx = engine.kanban.entity_context().await.unwrap();
    assert!(
        ectx.read("project", &project_id).await.is_ok(),
        "project must exist before archive"
    );

    engine
        .dispatch_simple(
            "entity.archive",
            &[],
            Some(&format!("project:{project_id}")),
        )
        .await
        .expect("entity.archive should succeed for project");
    assert!(
        ectx.read("project", &project_id).await.is_err(),
        "project must be gone from live storage after archive"
    );

    engine.undo().await;
    assert!(
        ectx.read("project", &project_id).await.is_ok(),
        "project must be restored to live storage after undo of archive"
    );

    engine.redo().await;
    assert!(
        ectx.read("project", &project_id).await.is_err(),
        "project must be archived again after redo"
    );
}

/// Archiving a task via `entity.archive` (the task branch — runs through
/// `crate::task::ArchiveTask`) must be undoable end-to-end.
#[tokio::test]
async fn undo_entity_archive_restores_task() {
    let engine = UndoEngine::new().await;
    let task_id = engine.add_task("Archive me").await;

    let ectx = engine.kanban.entity_context().await.unwrap();
    assert!(
        ectx.read("task", &task_id).await.is_ok(),
        "task must exist before archive"
    );

    engine
        .dispatch_simple("entity.archive", &[], Some(&format!("task:{task_id}")))
        .await
        .expect("entity.archive should succeed for task");
    assert!(
        ectx.read("task", &task_id).await.is_err(),
        "task must be gone from live storage after archive"
    );

    engine.undo().await;
    assert!(
        ectx.read("task", &task_id).await.is_ok(),
        "task must be restored to live storage after undo of archive"
    );
}

/// Unarchiving a project via `entity.unarchive` must be undoable: undo
/// returns the project to the archive.
#[tokio::test]
async fn undo_entity_unarchive_returns_project_to_archive() {
    let engine = UndoEngine::new().await;
    let project_id = engine.add_project("Backend").await;

    // Archive the project first via the cross-cutting command so the
    // unarchive operates on a real `.archive/` artifact with a versioned
    // filename. (Calling `EntityContext::archive` directly would skip the
    // dispatch path; we want to exercise it end-to-end.)
    engine
        .dispatch_simple(
            "entity.archive",
            &[],
            Some(&format!("project:{project_id}")),
        )
        .await
        .expect("entity.archive (setup)");

    let ectx = engine.kanban.entity_context().await.unwrap();
    assert!(
        ectx.read("project", &project_id).await.is_err(),
        "project must be archived before unarchive"
    );

    // Unarchive via the cross-cutting command. The auto-emit pass in
    // production fires `entity.unarchive` with `target = "{type}:{id}:archive"`.
    engine
        .dispatch_simple(
            "entity.unarchive",
            &[],
            Some(&format!("project:{project_id}:archive")),
        )
        .await
        .expect("entity.unarchive should succeed for archived project");
    assert!(
        ectx.read("project", &project_id).await.is_ok(),
        "project must be live after unarchive"
    );

    engine.undo().await;
    assert!(
        ectx.read("project", &project_id).await.is_err(),
        "project must be archived again after undo of unarchive"
    );
}

/// Pasting a copied task into a column via `entity.paste` creates a new
/// task. Undo must remove it without disturbing the source.
#[tokio::test]
async fn undo_entity_paste_removes_created_task() {
    let engine = UndoEngine::new().await;
    let source_id = engine.add_task("Source").await;

    // Copy the source task to the clipboard.
    engine
        .dispatch_simple("entity.copy", &[], Some(&format!("task:{source_id}")))
        .await
        .expect("entity.copy should succeed");

    let ectx = engine.kanban.entity_context().await.unwrap();
    let before = ectx.list("task").await.unwrap().len();
    assert_eq!(before, 1, "exactly one task before paste");

    // Paste into the `doing` column. Result carries the new task's id.
    let paste_result = engine
        .dispatch_simple("entity.paste", &[], Some("column:doing"))
        .await
        .expect("entity.paste should succeed");
    let new_id = paste_result["id"]
        .as_str()
        .expect("paste must return new task id")
        .to_string();
    assert_ne!(
        new_id, source_id,
        "pasted task must have a fresh id distinct from the source"
    );
    assert_eq!(
        ectx.list("task").await.unwrap().len(),
        2,
        "paste must add a second task"
    );
    assert!(
        ectx.read("task", &new_id).await.is_ok(),
        "the new task must exist after paste"
    );

    // Undo — only the create from paste is reversed (copy is non-undoable).
    // The newly-created task must be gone; the source must remain.
    engine.undo().await;
    assert!(
        ectx.read("task", &new_id).await.is_err(),
        "newly-pasted task must be removed by undo"
    );
    assert!(
        ectx.read("task", &source_id).await.is_ok(),
        "source task must remain after undoing a copy-paste"
    );
    assert_eq!(
        ectx.list("task").await.unwrap().len(),
        1,
        "only the source task remains after undo"
    );
}

/// Pasting a `cut` task into a column creates the new task and deletes
/// the source. Undo runs in reverse order (LIFO): the most recent
/// operation (the source delete) is undone first, restoring the source.
/// A second undo removes the newly-pasted task. After both undos the
/// board is back to its pre-paste state.
///
/// Note: we cannot drive this via `entity.cut` + `entity.paste` because
/// `entity.cut` already deletes the source up front; by the time the
/// paste handler reaches its source-delete branch there is nothing to
/// delete and the inner [`DeleteTask`](crate::task::DeleteTask) raises a
/// hard error. Instead we stage a synthetic cut-mode clipboard payload
/// pointing at a still-live source (the same on-wire shape
/// `entity.cut` writes) and dispatch `entity.paste` against it. This
/// exercises the exact [`TaskIntoColumnHandler`] path that production
/// runs — create-then-delete with both operations flowing through
/// `run_op` and onto the shared undo stack.
#[tokio::test]
async fn undo_entity_paste_cut_restores_source_task() {
    use swissarmyhammer_kanban::clipboard::{serialize_to_clipboard, ClipboardProvider};

    let engine = UndoEngine::new().await;
    let source_id = engine.add_task("Source").await;

    let ectx = engine.kanban.entity_context().await.unwrap();
    assert!(
        ectx.read("task", &source_id).await.is_ok(),
        "source must exist before paste-cut"
    );

    // Stage a cut-mode clipboard payload manually. Going through
    // `entity.cut` would delete the source before paste runs, starving
    // the handler's delete-source branch. The on-wire payload shape
    // matches what `entity.cut` writes: mode="cut", entity_id=source_id,
    // and the source's current field snapshot.
    let source_entity = ectx.read("task", &source_id).await.unwrap();
    let fields = serde_json::to_value(&source_entity.fields).unwrap();
    let cut_payload_json = serialize_to_clipboard("task", &source_id, "cut", fields);
    engine
        .clipboard
        .write_text(&cut_payload_json)
        .await
        .expect("seed cut clipboard");
    engine.ui_state.set_clipboard_entity_type("task");

    // Paste-cut into a column. The handler creates a new task AND
    // deletes the (still-live) source. Both mutations push to the shared
    // undo stack through `run_op`.
    let paste_result = engine
        .dispatch_simple("entity.paste", &[], Some("column:doing"))
        .await
        .expect("entity.paste (cut) should succeed");
    let new_id = paste_result["id"]
        .as_str()
        .expect("paste must return new task id")
        .to_string();

    assert_ne!(new_id, source_id, "paste must produce a fresh ULID");
    assert!(
        ectx.read("task", &new_id).await.is_ok(),
        "new task must exist after paste-cut"
    );
    assert!(
        ectx.read("task", &source_id).await.is_err(),
        "source must be deleted by paste-cut"
    );
    assert_eq!(
        ectx.list("task").await.unwrap().len(),
        1,
        "paste-cut leaves exactly one task on the board (the new one)"
    );

    // LIFO: the most recent undoable op is the source-delete. One undo
    // restores the source; the newly-pasted task is still present.
    engine.undo().await;
    assert!(
        ectx.read("task", &source_id).await.is_ok(),
        "source must be restored after one undo (delete is reversed)"
    );
    assert!(
        ectx.read("task", &new_id).await.is_ok(),
        "newly-pasted task must still exist after only one undo"
    );
    assert_eq!(
        ectx.list("task").await.unwrap().len(),
        2,
        "after one undo both tasks coexist briefly"
    );

    // A second undo reverses the create — the newly-pasted task is
    // gone; only the source remains. The board is back to its
    // pre-paste state.
    engine.undo().await;
    assert!(
        ectx.read("task", &new_id).await.is_err(),
        "newly-pasted task must be removed after second undo"
    );
    assert!(
        ectx.read("task", &source_id).await.is_ok(),
        "source must remain present after full undo of paste-cut"
    );
    assert_eq!(
        ectx.list("task").await.unwrap().len(),
        1,
        "only the source task remains after both undos"
    );
}

/// `entity.copy` is declared `undoable: false` in the YAML registry —
/// it is a clipboard-only operation and does not mutate the entity layer.
/// This test pins both invariants:
///
///   1. The YAML metadata says `undoable: false` (regression guard for
///      a future YAML edit that tries to mark copy as undoable).
///   2. Dispatching `entity.copy` does not push anything onto the
///      shared undo stack (regression guard for a future Rust impl
///      that accidentally writes through `EntityContext`).
#[tokio::test]
async fn entity_copy_is_not_undoable() {
    let engine = UndoEngine::new().await;
    let task_id = engine.add_task("Copy me").await;

    // Snapshot the depth — we only care about the *delta* produced by
    // `entity.copy`, not the absolute stack state. `undo_depth()` is a
    // read-only accessor: no filesystem churn, no mutation of the stack
    // while probing.
    let depth_before = engine.store_context.undo_depth().await;

    // Dispatch the copy.
    engine
        .dispatch_simple("entity.copy", &[], Some(&format!("task:{task_id}")))
        .await
        .expect("entity.copy should succeed");

    let depth_after = engine.store_context.undo_depth().await;
    assert_eq!(
        depth_before, depth_after,
        "entity.copy must not push onto the undo stack — \
         depth before={depth_before}, after={depth_after}"
    );

    // The clipboard does have data (copy succeeded), and the source is
    // untouched.
    assert!(
        engine.clipboard.read_text().await.unwrap().is_some(),
        "entity.copy must populate the system clipboard"
    );
    let ectx = engine.kanban.entity_context().await.unwrap();
    assert!(
        ectx.read("task", &task_id).await.is_ok(),
        "source task must remain after entity.copy"
    );
}
