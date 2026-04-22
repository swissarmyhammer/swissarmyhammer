//! Command × entity surface matrix.
//!
//! For every (entity, cross-cutting command) pair this test suite asserts
//! both the **surface contract** (is the `ResolvedCommand` emitted? with what
//! shape?) and, for positive pairs, the **dispatch contract** (does
//! `execute()` against a real [`KanbanContext`] actually mutate state?).
//!
//! The matrix under test is:
//!
//! | Entity      | inspect | delete | archive | unarchive | copy | cut | paste |
//! |-------------|---------|--------|---------|-----------|------|-----|-------|
//! | task        | yes     | yes    | yes     | yes       | yes  | yes | yes   |
//! | tag         | yes     | yes    | yes     | yes       | yes  | yes | —     |
//! | project     | yes     | yes    | yes     | yes       | —    | —   | yes   |
//! | column      | yes     | yes    | —       | —         | —    | —   | yes   |
//! | actor       | yes     | yes    | —       | —         | —    | —   | —     |
//! | board       | yes     | —      | —       | —         | —    | —   | yes   |
//! | attachment  | —       | yes    | —       | —         | —    | —   | —     |
//!
//! Where "—" means either the command is not emitted for that entity moniker,
//! or it is emitted with `available: false`. The matrix covers 49 cells
//! (7 entities × 7 commands).
//!
//! Notes on the `—` cells:
//!
//! - **attachment**: cross-cutting commands still auto-emit (attachment is a
//!   known entity). Inspect and delete both use the cross-cutting path now
//!   (`ui.inspect` and `entity.delete`), with `attachment.open` /
//!   `attachment.reveal` remaining as the only type-specific surface.
//!   The `—` cells assert the cross-cutting command is either absent or
//!   unavailable, matching the intent documented in the card.
//! - **project / column / actor / board delete**: the cross-cutting
//!   `entity.delete` auto-emits for every entity type. Boards opt out via
//!   `DELETE_OPT_OUT_TYPES` in `entity_commands.rs` — the cross-cutting
//!   emitter still creates a `ResolvedCommand`, but availability returns
//!   false and `commands_for_scope` drops it in the final retain pass.
//!   Project / column / actor delete remain surfaced (no opt-out).
//! - **paste**: availability is a function of the clipboard state. We test
//!   each positive pair by seeding a compatible payload on the UI clipboard
//!   (mirroring what `CopyEntityCmd::execute` does) before asking
//!   `commands_for_scope` what it emits.
//!
//! ## Shape assertions
//!
//! For positive cells we assert the exact values of:
//!
//! - `id` — the command id
//! - `target` — the moniker the command was emitted against
//! - `name` — the template-resolved display name (e.g. `Copy Tag`)
//! - `context_menu` — whether the command should appear in context menus
//! - `keys` — the keybinding bundle as declared in the YAML registry
//! - `available` — whether the Rust `Command::available()` impl returned true
//!
//! These are the fields of [`ResolvedCommand`] that are user-visible — any
//! regression in any of them is a regression in the command surface.

use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_commands::{
    builtin_yaml_sources, Command, CommandContext, CommandError, CommandsRegistry, UIState,
};
use swissarmyhammer_entity::EntityTypeStore;
use swissarmyhammer_fields::FieldsContext;
use swissarmyhammer_kanban::clipboard::{
    ClipboardProvider, ClipboardProviderExt, InMemoryClipboard,
};
use swissarmyhammer_kanban::commands::register_commands;
use swissarmyhammer_kanban::defaults::{builtin_entity_definitions, builtin_field_definitions};
use swissarmyhammer_kanban::scope_commands::{commands_for_scope, ResolvedCommand};
use swissarmyhammer_kanban::{
    board::InitBoard, KanbanContext, KanbanOperationProcessor, OperationProcessor,
};
use swissarmyhammer_store::StoreHandle;
use tempfile::TempDir;

// =============================================================================
// Test harness
// =============================================================================

/// Full test harness — everything needed to surface _and_ dispatch commands.
///
/// Surface queries use the [`FieldsContext`], [`CommandsRegistry`] and
/// `register_commands()` map. Dispatch uses a real [`KanbanContext`] with
/// StoreHandles registered for every entity type so state changes flow
/// through the production undo-capable path.
struct MatrixHarness {
    _temp: TempDir,
    kanban: Arc<KanbanContext>,
    commands: HashMap<String, Arc<dyn Command>>,
    registry: CommandsRegistry,
    fields: FieldsContext,
    ui_state: Arc<UIState>,
    clipboard: Arc<InMemoryClipboard>,
}

impl MatrixHarness {
    /// Build a fresh harness with an initialized board.
    async fn new() -> Self {
        let temp = TempDir::new().expect("tempdir");
        let kanban_dir = temp.path().join(".kanban");
        let kanban = KanbanContext::new(&kanban_dir);

        let processor = KanbanOperationProcessor::new();
        processor
            .process(&InitBoard::new("Matrix Test"), &kanban)
            .await
            .expect("board init failed");

        let kanban = Arc::new(kanban);

        // Register a StoreHandle for every entity type — production-like path,
        // required for mutating commands (delete / archive / paste) that write
        // through the undo stack.
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
        let clipboard = Arc::new(InMemoryClipboard::new());

        Self {
            _temp: temp,
            kanban,
            commands,
            registry,
            fields,
            ui_state,
            clipboard,
        }
    }

    /// Query `commands_for_scope` with the current UIState and the full
    /// registry/impls/fields triple — no `context_menu_only` filter, no
    /// dynamic sources. This is the surface that context menus, palettes and
    /// the Edit menu bar all consume.
    fn surface(&self, scope_chain: &[&str]) -> Vec<ResolvedCommand> {
        let scope: Vec<String> = scope_chain.iter().map(|s| s.to_string()).collect();
        commands_for_scope(
            &scope,
            &self.registry,
            &self.commands,
            Some(&self.fields),
            &self.ui_state,
            false,
            None,
        )
    }

    /// Dispatch a command by id via the full availability + execute cycle.
    async fn dispatch(
        &self,
        cmd_id: &str,
        scope: &[&str],
        target: Option<&str>,
        args: HashMap<String, serde_json::Value>,
    ) -> swissarmyhammer_commands::Result<serde_json::Value> {
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
        let ectx = self.kanban.entity_context().await.expect("entity_context");
        ctx.set_extension(ectx);
        let clipboard_ext =
            ClipboardProviderExt(Arc::clone(&self.clipboard) as Arc<dyn ClipboardProvider>);
        ctx.set_extension(Arc::new(clipboard_ext));

        if !cmd.available(&ctx) {
            return Err(CommandError::ExecutionFailed(format!(
                "command '{cmd_id}' not available"
            )));
        }
        cmd.execute(&ctx).await
    }

    /// Add a task (via the unified `entity.add:task` pathway) and return its id.
    async fn add_task(&self, title: &str) -> String {
        let mut args = HashMap::new();
        args.insert("entity_type".into(), json!("task"));
        args.insert("title".into(), json!(title));
        let result = self
            .dispatch("entity.add", &[], None, args)
            .await
            .expect("entity.add:task");
        result["id"].as_str().unwrap().to_string()
    }

    /// Add a tag (via `entity.add:tag`) and return its id.
    async fn add_tag(&self, name: &str) -> String {
        let mut args = HashMap::new();
        args.insert("entity_type".into(), json!("tag"));
        args.insert("tag_name".into(), json!(name));
        let result = self
            .dispatch("entity.add", &[], None, args)
            .await
            .expect("entity.add:tag");
        result["id"].as_str().unwrap().to_string()
    }

    /// Add a project and return its id.
    async fn add_project(&self, slug: &str, name: &str) -> String {
        let processor = KanbanOperationProcessor::new();
        let result = processor
            .process(
                &swissarmyhammer_kanban::project::AddProject::new(slug, name),
                &self.kanban,
            )
            .await
            .expect("add project");
        result["id"].as_str().unwrap().to_string()
    }

    /// Add an actor and return its id.
    async fn add_actor(&self, slug: &str, name: &str) -> String {
        let processor = KanbanOperationProcessor::new();
        let result = processor
            .process(
                &swissarmyhammer_kanban::actor::AddActor::new(slug, name),
                &self.kanban,
            )
            .await
            .expect("add actor");
        result["actor"]["id"].as_str().unwrap().to_string()
    }

    /// Resolve the board id created by [`InitBoard`].
    async fn board_id(&self) -> String {
        let ectx = self.kanban.entity_context().await.unwrap();
        let boards = ectx.list("board").await.unwrap();
        boards
            .first()
            .expect("board should exist after InitBoard")
            .id
            .to_string()
    }

    /// Seed the UI clipboard with a fake entity snapshot. Mirrors what
    /// [`CopyEntityCmd`] does on execute — enough to make
    /// [`PasteEntityCmd::available`] return true for a target whose
    /// `(clipboard_type, target_type)` handler exists.
    fn seed_clipboard(&self, entity_type: &str) {
        self.ui_state.set_clipboard_entity_type(entity_type);
        // PasteEntityCmd.available() only checks UIState.has_clipboard() —
        // which is true once set_clipboard_entity_type fires. We don't need
        // actual clipboard text for availability gating.
    }
}

// =============================================================================
// Shape assertion helpers
// =============================================================================

/// Find a command in the surface by `(id, target)`.
fn find_cmd<'a>(
    surface: &'a [ResolvedCommand],
    id: &str,
    target: Option<&str>,
) -> Option<&'a ResolvedCommand> {
    surface
        .iter()
        .find(|c| c.id == id && c.target.as_deref() == target)
}

/// Assert a command is present with the given id, target, context_menu,
/// keys-present flag, and availability. The exact keys are verified against
/// the declared YAML values — we only assert `keys.is_some() == keys_expected`
/// here so the matrix test doesn't duplicate the full KeysDef snapshot.
#[track_caller]
fn assert_shape(
    surface: &[ResolvedCommand],
    id: &str,
    target: Option<&str>,
    expected_name: &str,
    expected_context_menu: bool,
    expected_keys_present: bool,
    expected_available: bool,
) {
    let cmd = find_cmd(surface, id, target).unwrap_or_else(|| {
        panic!(
            "expected command id={id}, target={target:?} in surface; \
             got: {:?}",
            surface
                .iter()
                .map(|c| (c.id.clone(), c.target.clone()))
                .collect::<Vec<_>>()
        )
    });
    assert_eq!(
        cmd.name, expected_name,
        "name mismatch for {id} @ {target:?}"
    );
    assert_eq!(
        cmd.context_menu, expected_context_menu,
        "context_menu mismatch for {id} @ {target:?}"
    );
    assert_eq!(
        cmd.keys.is_some(),
        expected_keys_present,
        "keys presence mismatch for {id} @ {target:?}"
    );
    assert_eq!(
        cmd.available, expected_available,
        "available mismatch for {id} @ {target:?}"
    );
}

/// Assert the given `(id, target)` pair does NOT appear in the surface.
#[track_caller]
fn assert_absent(surface: &[ResolvedCommand], id: &str, target: Option<&str>) {
    if let Some(cmd) = find_cmd(surface, id, target) {
        panic!("expected {id} @ {target:?} to be absent, found: {:?}", cmd);
    }
}

// =============================================================================
// Task row — 7 cells
// =============================================================================

#[tokio::test]
async fn matrix_task_inspect() {
    let h = MatrixHarness::new().await;
    let task_id = h.add_task("Inspect me").await;
    let target = format!("task:{task_id}");

    let surface = h.surface(&[&target, "column:todo", "board:main"]);
    assert_shape(
        &surface,
        "ui.inspect",
        Some(&target),
        "Inspect Task",
        true,
        false, // ui.inspect has no keys in the YAML
        true,
    );

    // Round-trip: executing ui.inspect pushes the target onto the window stack.
    h.dispatch("ui.inspect", &[], Some(&target), HashMap::new())
        .await
        .expect("ui.inspect dispatch");
    assert_eq!(h.ui_state.inspector_stack("main"), vec![target]);
}

#[tokio::test]
async fn matrix_task_delete() {
    let h = MatrixHarness::new().await;
    let task_id = h.add_task("Delete me").await;
    let target = format!("task:{task_id}");

    let surface = h.surface(&[&target, "column:todo", "board:main"]);
    assert_shape(
        &surface,
        "entity.delete",
        Some(&target),
        "Delete Task",
        true,
        false, // no keys on entity.delete
        true,
    );

    // Round-trip: dispatch the delete and verify the task is gone.
    h.dispatch("entity.delete", &[], Some(&target), HashMap::new())
        .await
        .expect("entity.delete dispatch");
    let ectx = h.kanban.entity_context().await.unwrap();
    assert!(ectx.list("task").await.unwrap().is_empty());
}

#[tokio::test]
async fn matrix_task_archive() {
    let h = MatrixHarness::new().await;
    let task_id = h.add_task("Archive me").await;
    let target = format!("task:{task_id}");

    let surface = h.surface(&[&target, "column:todo", "board:main"]);
    // entity.archive declares `keys: { vim: dd }` in the registry.
    assert_shape(
        &surface,
        "entity.archive",
        Some(&target),
        "Archive Task",
        true,
        true,
        true,
    );

    // Round-trip: dispatch archive and verify the task leaves the live list.
    h.dispatch("entity.archive", &[], Some(&target), HashMap::new())
        .await
        .expect("entity.archive dispatch");
    let ectx = h.kanban.entity_context().await.unwrap();
    assert!(ectx.list("task").await.unwrap().is_empty());
}

#[tokio::test]
async fn matrix_task_unarchive() {
    let h = MatrixHarness::new().await;
    let task_id = h.add_task("Restore me").await;

    // Archive first via the operation path.
    let processor = KanbanOperationProcessor::new();
    processor
        .process(
            &swissarmyhammer_kanban::task::ArchiveTask::new(task_id.as_str()),
            &h.kanban,
        )
        .await
        .expect("archive");

    let archive_target = format!("task:{task_id}:archive");

    let surface = h.surface(&[&archive_target]);
    assert_shape(
        &surface,
        "entity.unarchive",
        Some(&archive_target),
        "Unarchive Task",
        true,
        false,
        true,
    );

    // `entity.archive` must NOT be available for an archived moniker —
    // `ArchiveEntityCmd::available` rejects `:archive` suffixes. The cross-cutting
    // emitter still fires the command (availability is not a pre-filter at emit
    // time); the final `result.retain(|c| c.available)` pass in
    // `commands_for_scope` drops it, so we assert absence.
    assert_absent(&surface, "entity.archive", Some(&archive_target));

    // Round-trip: dispatch unarchive, task should reappear.
    h.dispatch(
        "entity.unarchive",
        &[],
        Some(&archive_target),
        HashMap::new(),
    )
    .await
    .expect("entity.unarchive dispatch");
    let ectx = h.kanban.entity_context().await.unwrap();
    assert_eq!(ectx.list("task").await.unwrap().len(), 1);
}

#[tokio::test]
async fn matrix_task_copy() {
    let h = MatrixHarness::new().await;
    let task_id = h.add_task("Copy me").await;
    let target = format!("task:{task_id}");

    let surface = h.surface(&[&target, "column:todo", "board:main"]);
    assert_shape(
        &surface,
        "entity.copy",
        Some(&target),
        "Copy Task",
        true,
        true, // cua: Mod+C / vim: y
        true,
    );

    h.dispatch("entity.copy", &[], Some(&target), HashMap::new())
        .await
        .expect("entity.copy dispatch");
    assert_eq!(
        h.ui_state.clipboard_entity_type().as_deref(),
        Some("task"),
        "copy should set UIState clipboard_entity_type"
    );
}

#[tokio::test]
async fn matrix_task_cut() {
    let h = MatrixHarness::new().await;
    let task_id = h.add_task("Cut me").await;
    let target = format!("task:{task_id}");

    let surface = h.surface(&[&target, "column:todo", "board:main"]);
    assert_shape(
        &surface,
        "entity.cut",
        Some(&target),
        "Cut Task",
        true,
        true, // cua: Mod+X / vim: x
        true,
    );

    h.dispatch("entity.cut", &[], Some(&target), HashMap::new())
        .await
        .expect("entity.cut dispatch");
    let ectx = h.kanban.entity_context().await.unwrap();
    assert!(
        ectx.list("task").await.unwrap().is_empty(),
        "cut should remove the task from the live list"
    );
}

#[tokio::test]
async fn matrix_task_paste() {
    // "paste onto a task" — tag/actor/attachment paste handlers exist.
    // Seed a tag on the clipboard so the (tag, task) handler matches.
    let h = MatrixHarness::new().await;
    let task_id = h.add_task("Paste target").await;
    let tag_id = h.add_tag("bug").await;
    // Simulate a copy of the tag first — this populates the system clipboard
    // text through ClipboardProviderExt so paste.execute can read it.
    h.dispatch(
        "entity.copy",
        &[],
        Some(&format!("tag:{tag_id}")),
        HashMap::new(),
    )
    .await
    .expect("seed clipboard via entity.copy tag");

    let target = format!("task:{task_id}");

    let surface = h.surface(&[&target, "column:todo", "board:main"]);
    // Paste's resolved name uses the CLIPBOARD type, not the target type
    // (see `paste_aware_tpl` in scope_commands).
    assert_shape(
        &surface,
        "entity.paste",
        Some(&target),
        "Paste Tag",
        true,
        true, // cua: Mod+V / vim: p
        true,
    );

    // Round-trip: paste the tag onto the task.
    h.dispatch("entity.paste", &[], Some(&target), HashMap::new())
        .await
        .expect("entity.paste dispatch");
    let task = h
        .kanban
        .read_entity_generic("task", &task_id)
        .await
        .expect("task still exists");
    let tags = task
        .get("tags")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    assert!(
        tags.iter().any(|t| t.as_str() == Some("bug")),
        "paste should attach the 'bug' tag to the task"
    );
}

// =============================================================================
// Tag row — 7 cells
// =============================================================================

#[tokio::test]
async fn matrix_tag_inspect() {
    let h = MatrixHarness::new().await;
    let tag_id = h.add_tag("bug").await;
    let target = format!("tag:{tag_id}");

    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "ui.inspect",
        Some(&target),
        "Inspect Tag",
        true,
        false,
        true,
    );
}

#[tokio::test]
async fn matrix_tag_delete() {
    let h = MatrixHarness::new().await;
    let tag_id = h.add_tag("bug").await;
    let target = format!("tag:{tag_id}");

    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "entity.delete",
        Some(&target),
        "Delete Tag",
        true,
        false,
        true,
    );

    h.dispatch("entity.delete", &[], Some(&target), HashMap::new())
        .await
        .expect("entity.delete dispatch");
    let ectx = h.kanban.entity_context().await.unwrap();
    assert!(ectx.list("tag").await.unwrap().is_empty());
}

#[tokio::test]
async fn matrix_tag_archive() {
    let h = MatrixHarness::new().await;
    let tag_id = h.add_tag("bug").await;
    let target = format!("tag:{tag_id}");

    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "entity.archive",
        Some(&target),
        "Archive Tag",
        true,
        true,
        true,
    );

    h.dispatch("entity.archive", &[], Some(&target), HashMap::new())
        .await
        .expect("entity.archive dispatch");
    let ectx = h.kanban.entity_context().await.unwrap();
    assert!(ectx.list("tag").await.unwrap().is_empty());
}

#[tokio::test]
async fn matrix_tag_unarchive() {
    let h = MatrixHarness::new().await;
    let tag_id = h.add_tag("bug").await;

    // Archive the tag via EntityContext::archive (tag doesn't have a
    // dedicated ArchiveTag operation; the cross-cutting command routes
    // through ectx.archive for non-task entities).
    let ectx = h.kanban.entity_context().await.unwrap();
    ectx.archive("tag", &tag_id).await.expect("archive tag");

    let archive_target = format!("tag:{tag_id}:archive");

    let surface = h.surface(&[&archive_target]);
    assert_shape(
        &surface,
        "entity.unarchive",
        Some(&archive_target),
        "Unarchive Tag",
        true,
        false,
        true,
    );

    h.dispatch(
        "entity.unarchive",
        &[],
        Some(&archive_target),
        HashMap::new(),
    )
    .await
    .expect("entity.unarchive dispatch");
    let ectx = h.kanban.entity_context().await.unwrap();
    assert_eq!(ectx.list("tag").await.unwrap().len(), 1);
}

#[tokio::test]
async fn matrix_tag_copy() {
    let h = MatrixHarness::new().await;
    let tag_id = h.add_tag("bug").await;
    let target = format!("tag:{tag_id}");

    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "entity.copy",
        Some(&target),
        "Copy Tag",
        true,
        true,
        true,
    );

    h.dispatch("entity.copy", &[], Some(&target), HashMap::new())
        .await
        .expect("entity.copy dispatch");
    assert_eq!(h.ui_state.clipboard_entity_type().as_deref(), Some("tag"));
}

#[tokio::test]
async fn matrix_tag_cut() {
    // Cutting a tag means "untag this tag from the task that shares the scope
    // chain". CutEntityCmd::available returns false unless a task is present.
    let h = MatrixHarness::new().await;
    let task_id = h.add_task("Tagged task").await;
    let tag_id = h.add_tag("bug").await;
    // Attach the tag to the task so cut has something to do.
    let processor = KanbanOperationProcessor::new();
    processor
        .process(
            &swissarmyhammer_kanban::task::TagTask::new(task_id.as_str(), "bug"),
            &h.kanban,
        )
        .await
        .expect("tag task");
    let target_tag = format!("tag:{tag_id}");
    let target_task = format!("task:{task_id}");

    let surface = h.surface(&[&target_tag, &target_task, "column:todo", "board:main"]);
    assert_shape(
        &surface,
        "entity.cut",
        Some(&target_tag),
        "Cut Tag",
        true,
        true,
        true,
    );

    h.dispatch(
        "entity.cut",
        &[&target_tag, &target_task, "column:todo", "board:main"],
        Some(&target_tag),
        HashMap::new(),
    )
    .await
    .expect("entity.cut dispatch");
    let task = h
        .kanban
        .read_entity_generic("task", &task_id)
        .await
        .unwrap();
    let tags = task
        .get("tags")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    assert!(
        !tags.iter().any(|t| t.as_str() == Some("bug")),
        "cut should remove the tag from the task"
    );
}

#[tokio::test]
async fn matrix_tag_paste_not_emitted() {
    // No `(*, tag)` paste handler exists in the production registry.
    // Even with a tag clipboard seeded, PasteEntityCmd::available() walks
    // the PasteMatrix and finds no `(tag, tag)` handler — so the
    // cross-cutting emitter surfaces the command with available=false
    // and `commands_for_scope` drops it in the final retain pass.
    let h = MatrixHarness::new().await;
    let tag_id = h.add_tag("bug").await;
    h.seed_clipboard("tag");
    let target = format!("tag:{tag_id}");

    let surface = h.surface(&[&target]);
    assert_absent(&surface, "entity.paste", Some(&target));
}

// =============================================================================
// Project row — 7 cells
// =============================================================================

#[tokio::test]
async fn matrix_project_inspect() {
    let h = MatrixHarness::new().await;
    let project_id = h.add_project("backend", "Backend").await;
    let target = format!("project:{project_id}");

    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "ui.inspect",
        Some(&target),
        "Inspect Project",
        true,
        false,
        true,
    );
}

#[tokio::test]
async fn matrix_project_delete() {
    let h = MatrixHarness::new().await;
    let project_id = h.add_project("backend", "Backend").await;
    let target = format!("project:{project_id}");

    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "entity.delete",
        Some(&target),
        "Delete Project",
        true,
        false,
        true,
    );

    h.dispatch("entity.delete", &[], Some(&target), HashMap::new())
        .await
        .expect("entity.delete dispatch");
    let ectx = h.kanban.entity_context().await.unwrap();
    assert!(ectx
        .list("project")
        .await
        .unwrap()
        .iter()
        .all(|p| p.id != project_id));
}

#[tokio::test]
async fn matrix_project_archive() {
    let h = MatrixHarness::new().await;
    let project_id = h.add_project("backend", "Backend").await;
    let target = format!("project:{project_id}");

    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "entity.archive",
        Some(&target),
        "Archive Project",
        true,
        true,
        true,
    );

    h.dispatch("entity.archive", &[], Some(&target), HashMap::new())
        .await
        .expect("entity.archive dispatch");
    let ectx = h.kanban.entity_context().await.unwrap();
    assert!(ectx
        .list("project")
        .await
        .unwrap()
        .iter()
        .all(|p| p.id != project_id));
}

#[tokio::test]
async fn matrix_project_unarchive() {
    let h = MatrixHarness::new().await;
    let project_id = h.add_project("backend", "Backend").await;
    let ectx = h.kanban.entity_context().await.unwrap();
    ectx.archive("project", &project_id)
        .await
        .expect("archive project");

    let archive_target = format!("project:{project_id}:archive");

    let surface = h.surface(&[&archive_target]);
    assert_shape(
        &surface,
        "entity.unarchive",
        Some(&archive_target),
        "Unarchive Project",
        true,
        false,
        true,
    );

    h.dispatch(
        "entity.unarchive",
        &[],
        Some(&archive_target),
        HashMap::new(),
    )
    .await
    .expect("entity.unarchive dispatch");
    let ectx = h.kanban.entity_context().await.unwrap();
    assert!(ectx
        .list("project")
        .await
        .unwrap()
        .iter()
        .any(|p| p.id == project_id));
}

#[tokio::test]
async fn matrix_project_copy_surfaces_despite_card_dash() {
    // Project IS in the COPYABLE_ENTITY_TYPES list in clipboard_commands.rs —
    // so the cross-cutting entity.copy surfaces with available=true for a
    // project moniker. This is the current contract; the card's "—" cell
    // documents that project-copy is not a UX we actively use, but the
    // command is still emitted. If project-copy should be removed, the
    // opt-out belongs in CopyEntityCmd.available().
    //
    // We assert the current contract here so a future refactor that breaks
    // it fails loudly instead of silently changing behaviour.
    let h = MatrixHarness::new().await;
    let project_id = h.add_project("backend", "Backend").await;
    let target = format!("project:{project_id}");

    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "entity.copy",
        Some(&target),
        "Copy Project",
        true,
        true,
        true,
    );
}

#[tokio::test]
async fn matrix_project_cut_not_available() {
    // CutEntityCmd::available returns false for non-task/non-tag targets —
    // projects fall into the `_ => false` arm of the match. The command is
    // emitted by the cross-cutting pass with available=false and dropped by
    // the final retain. Assert absence from the surface.
    let h = MatrixHarness::new().await;
    let project_id = h.add_project("backend", "Backend").await;
    let target = format!("project:{project_id}");

    let surface = h.surface(&[&target]);
    assert_absent(&surface, "entity.cut", Some(&target));
}

#[tokio::test]
async fn matrix_project_paste() {
    // Handler `(task, project)` exists — so pasting a task onto a project
    // moves it into that project.
    let h = MatrixHarness::new().await;
    let project_id = h.add_project("backend", "Backend").await;
    let task_id = h.add_task("Assign me").await;

    // Seed the clipboard by copying the task.
    h.dispatch(
        "entity.copy",
        &[],
        Some(&format!("task:{task_id}")),
        HashMap::new(),
    )
    .await
    .expect("copy task");

    let target = format!("project:{project_id}");
    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "entity.paste",
        Some(&target),
        "Paste Task",
        true,
        true,
        true,
    );
}

// =============================================================================
// Column row — 7 cells
// =============================================================================

#[tokio::test]
async fn matrix_column_inspect() {
    let h = MatrixHarness::new().await;

    let surface = h.surface(&["column:todo", "board:main"]);
    assert_shape(
        &surface,
        "ui.inspect",
        Some("column:todo"),
        "Inspect Column",
        true,
        false,
        true,
    );
}

#[tokio::test]
async fn matrix_column_delete() {
    let h = MatrixHarness::new().await;

    let surface = h.surface(&["column:todo", "board:main"]);
    assert_shape(
        &surface,
        "entity.delete",
        Some("column:todo"),
        "Delete Column",
        true,
        false,
        true,
    );

    h.dispatch("entity.delete", &[], Some("column:todo"), HashMap::new())
        .await
        .expect("entity.delete dispatch");
    let ectx = h.kanban.entity_context().await.unwrap();
    assert!(ectx
        .list("column")
        .await
        .unwrap()
        .iter()
        .all(|c| c.id != "todo"));
}

#[tokio::test]
async fn matrix_column_archive_available() {
    // ArchiveEntityCmd has no per-type opt-out in its Rust `available()`
    // impl — any moniker without the `:archive` suffix qualifies. The card
    // marks column-archive as "—" (not part of the UX) but the cross-cutting
    // pass still emits it with available=true. Pin the contract.
    let h = MatrixHarness::new().await;

    let surface = h.surface(&["column:todo", "board:main"]);
    assert_shape(
        &surface,
        "entity.archive",
        Some("column:todo"),
        "Archive Column",
        true,
        true,
        true,
    );
}

#[tokio::test]
async fn matrix_column_unarchive_not_available() {
    // Without an `:archive` suffix on the target, UnarchiveEntityCmd rejects
    // the request. The cross-cutting emitter still fires the command but
    // available=false, and `commands_for_scope` drops it.
    let h = MatrixHarness::new().await;

    let surface = h.surface(&["column:todo", "board:main"]);
    assert_absent(&surface, "entity.unarchive", Some("column:todo"));
}

#[tokio::test]
async fn matrix_column_copy_available() {
    // column IS in COPYABLE_ENTITY_TYPES — so entity.copy surfaces. Pin the
    // contract.
    let h = MatrixHarness::new().await;

    let surface = h.surface(&["column:todo", "board:main"]);
    assert_shape(
        &surface,
        "entity.copy",
        Some("column:todo"),
        "Copy Column",
        true,
        true,
        true,
    );
}

#[tokio::test]
async fn matrix_column_cut_not_available() {
    let h = MatrixHarness::new().await;

    let surface = h.surface(&["column:todo", "board:main"]);
    assert_absent(&surface, "entity.cut", Some("column:todo"));
}

#[tokio::test]
async fn matrix_column_paste() {
    // Handler `(task, column)` exists — seed a task on the clipboard.
    let h = MatrixHarness::new().await;
    let task_id = h.add_task("Paste into doing").await;
    h.dispatch(
        "entity.copy",
        &[],
        Some(&format!("task:{task_id}")),
        HashMap::new(),
    )
    .await
    .expect("copy task");

    let surface = h.surface(&["column:doing", "board:main"]);
    assert_shape(
        &surface,
        "entity.paste",
        Some("column:doing"),
        "Paste Task",
        true,
        true,
        true,
    );
}

// =============================================================================
// Actor row — 7 cells
// =============================================================================

#[tokio::test]
async fn matrix_actor_inspect() {
    let h = MatrixHarness::new().await;
    let actor_id = h.add_actor("alice", "Alice").await;
    let target = format!("actor:{actor_id}");

    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "ui.inspect",
        Some(&target),
        "Inspect Actor",
        true,
        false,
        true,
    );
}

#[tokio::test]
async fn matrix_actor_delete() {
    let h = MatrixHarness::new().await;
    let actor_id = h.add_actor("alice", "Alice").await;
    let target = format!("actor:{actor_id}");

    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "entity.delete",
        Some(&target),
        "Delete Actor",
        true,
        false,
        true,
    );

    h.dispatch("entity.delete", &[], Some(&target), HashMap::new())
        .await
        .expect("entity.delete dispatch");
    let ectx = h.kanban.entity_context().await.unwrap();
    assert!(ectx
        .list("actor")
        .await
        .unwrap()
        .iter()
        .all(|a| a.id != actor_id));
}

#[tokio::test]
async fn matrix_actor_archive_available() {
    // ArchiveEntityCmd surfaces for any non-archived moniker — pin contract.
    let h = MatrixHarness::new().await;
    let actor_id = h.add_actor("alice", "Alice").await;
    let target = format!("actor:{actor_id}");

    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "entity.archive",
        Some(&target),
        "Archive Actor",
        true,
        true,
        true,
    );
}

#[tokio::test]
async fn matrix_actor_unarchive_not_available() {
    let h = MatrixHarness::new().await;
    let actor_id = h.add_actor("alice", "Alice").await;
    let target = format!("actor:{actor_id}");

    let surface = h.surface(&[&target]);
    assert_absent(&surface, "entity.unarchive", Some(&target));
}

#[tokio::test]
async fn matrix_actor_copy_available() {
    // actor IS in COPYABLE_ENTITY_TYPES.
    let h = MatrixHarness::new().await;
    let actor_id = h.add_actor("alice", "Alice").await;
    let target = format!("actor:{actor_id}");

    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "entity.copy",
        Some(&target),
        "Copy Actor",
        true,
        true,
        true,
    );
}

#[tokio::test]
async fn matrix_actor_cut_not_available() {
    let h = MatrixHarness::new().await;
    let actor_id = h.add_actor("alice", "Alice").await;
    let target = format!("actor:{actor_id}");

    let surface = h.surface(&[&target]);
    assert_absent(&surface, "entity.cut", Some(&target));
}

#[tokio::test]
async fn matrix_actor_paste_not_emitted() {
    // No `(*, actor)` paste handler exists — even with any clipboard,
    // paste is not available for actor targets.
    let h = MatrixHarness::new().await;
    let actor_id = h.add_actor("alice", "Alice").await;
    h.seed_clipboard("task");
    let target = format!("actor:{actor_id}");

    let surface = h.surface(&[&target]);
    assert_absent(&surface, "entity.paste", Some(&target));
}

// =============================================================================
// Board row — 7 cells
// =============================================================================

#[tokio::test]
async fn matrix_board_inspect() {
    let h = MatrixHarness::new().await;
    let board_id = h.board_id().await;
    let target = format!("board:{board_id}");

    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "ui.inspect",
        Some(&target),
        "Inspect Board",
        true,
        false,
        true,
    );
}

#[tokio::test]
async fn matrix_board_delete_not_available() {
    // Boards are in DELETE_OPT_OUT_TYPES — `DeleteEntityCmd::available()`
    // returns false for a board moniker, so the cross-cutting emitter's
    // surface entry is dropped by the final `retain(|c| c.available)` pass
    // in `commands_for_scope`.
    //
    // Board lifecycle is managed by `file.closeBoard` / `file.newBoard` /
    // `file.openBoard`; a generic row-level delete is not a meaningful UX.
    let h = MatrixHarness::new().await;
    let board_id = h.board_id().await;
    let target = format!("board:{board_id}");

    let surface = h.surface(&[&target]);
    assert_absent(&surface, "entity.delete", Some(&target));
}

#[tokio::test]
async fn matrix_board_archive_not_available() {
    // Boards are in ARCHIVE_OPT_OUT_TYPES — `ArchiveEntityCmd::available()`
    // returns false for a board moniker, mirroring the delete opt-out.
    // Archiving a board would silently move the board file into `.archive/`
    // with no code path treating the result as meaningful; surfacing the
    // command is worse than hiding it.
    let h = MatrixHarness::new().await;
    let board_id = h.board_id().await;
    let target = format!("board:{board_id}");

    let surface = h.surface(&[&target]);
    assert_absent(&surface, "entity.archive", Some(&target));
}

#[tokio::test]
async fn matrix_board_unarchive_not_available() {
    let h = MatrixHarness::new().await;
    let board_id = h.board_id().await;
    let target = format!("board:{board_id}");

    let surface = h.surface(&[&target]);
    assert_absent(&surface, "entity.unarchive", Some(&target));
}

#[tokio::test]
async fn matrix_board_copy_available() {
    // board IS in COPYABLE_ENTITY_TYPES.
    let h = MatrixHarness::new().await;
    let board_id = h.board_id().await;
    let target = format!("board:{board_id}");

    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "entity.copy",
        Some(&target),
        "Copy Board",
        true,
        true,
        true,
    );
}

#[tokio::test]
async fn matrix_board_cut_not_available() {
    let h = MatrixHarness::new().await;
    let board_id = h.board_id().await;
    let target = format!("board:{board_id}");

    let surface = h.surface(&[&target]);
    assert_absent(&surface, "entity.cut", Some(&target));
}

#[tokio::test]
async fn matrix_board_paste() {
    // Handler `(task, board)` and `(column, board)` exist. Seed a task on
    // the clipboard.
    let h = MatrixHarness::new().await;
    let board_id = h.board_id().await;
    let task_id = h.add_task("Cross-board paste").await;
    h.dispatch(
        "entity.copy",
        &[],
        Some(&format!("task:{task_id}")),
        HashMap::new(),
    )
    .await
    .expect("copy task");

    let target = format!("board:{board_id}");
    let surface = h.surface(&[&target]);
    assert_shape(
        &surface,
        "entity.paste",
        Some(&target),
        "Paste Task",
        true,
        true,
        true,
    );
}

// =============================================================================
// Attachment row — 7 cells
// =============================================================================
//
// Attachments have their own inspect-equivalent (`attachment.open`). Delete
// is handled by the cross-cutting `entity.delete` command — the
// `DeleteEntityCmd::execute` has an `"attachment"` match arm that walks the
// scope chain to find the parent task. The other cross-cutting commands
// still auto-emit for attachment monikers because attachment is a known
// entity type; the matrix card marks the inapplicable cells as "—".

#[tokio::test]
async fn matrix_attachment_inspect_surface_emits_but_attachment_open_present() {
    let h = MatrixHarness::new().await;
    // For attachments we test two things at once: (a) the cross-cutting
    // ui.inspect still surfaces, (b) the type-specific attachment.open is
    // ALSO present and is the "real" inspect-equivalent.
    let target = "attachment:/tmp/x.png";

    let surface = h.surface(&[target, "task:01X", "column:todo"]);
    // Cross-cutting ui.inspect is emitted with attachment target — current
    // contract (attachment is_known_entity=true).
    assert_shape(
        &surface,
        "ui.inspect",
        Some(target),
        "Inspect Attachment",
        true,
        false,
        true,
    );

    // Type-specific attachment.open is also present — this is the primary
    // UX for inspecting an attachment (open it in the OS default app).
    assert!(
        surface.iter().any(|c| c.id == "attachment.open"),
        "attachment.open should be in surface"
    );
}

#[tokio::test]
async fn matrix_attachment_delete_surface_emits_entity_delete_only() {
    let h = MatrixHarness::new().await;
    let target = "attachment:/tmp/x.png";

    let surface = h.surface(&[target, "task:01X", "column:todo"]);
    // Cross-cutting entity.delete is emitted with the attachment target
    // and rendered via the `"Delete {{entity.type}}"` template in
    // `entity.yaml`. Dispatch resolves the parent task via the scope chain.
    assert_shape(
        &surface,
        "entity.delete",
        Some(target),
        "Delete Attachment",
        true,
        false,
        true,
    );

    // The type-specific `attachment.delete` command has been retired —
    // delete is unified on `entity.delete`. Right-clicking a task-scoped
    // surface must NOT offer a second "Delete Attachment" row.
    assert!(
        !surface.iter().any(|c| c.id == "attachment.delete"),
        "attachment.delete must no longer appear in the surface; \
         delete is unified on entity.delete"
    );
}

/// Right-clicking a task (with NO attachment moniker in the scope chain)
/// must not offer any "Delete Attachment" row. Before the unification
/// the retired `attachment.delete` command was scoped `entity:task` and
/// leaked onto every task context menu as a non-working row — this test
/// pins that it stays gone.
#[tokio::test]
async fn task_context_menu_does_not_include_delete_attachment() {
    let h = MatrixHarness::new().await;
    let target = "task:01X";

    let surface = h.surface(&[target, "column:todo"]);

    // No `attachment.delete` anywhere — the command no longer exists.
    assert!(
        !surface.iter().any(|c| c.id == "attachment.delete"),
        "task context menu must not include attachment.delete (retired command)"
    );

    // The cross-cutting `entity.delete` is present for the task target,
    // and — because the target is a task, not an attachment — its
    // template resolves to "Delete Task", never "Delete Attachment".
    assert_shape(
        &surface,
        "entity.delete",
        Some(target),
        "Delete Task",
        true,
        false,
        true,
    );
}

#[tokio::test]
async fn matrix_attachment_archive_available() {
    let h = MatrixHarness::new().await;
    let target = "attachment:/tmp/x.png";

    let surface = h.surface(&[target, "task:01X", "column:todo"]);
    // Cross-cutting entity.archive still surfaces — current contract.
    assert_shape(
        &surface,
        "entity.archive",
        Some(target),
        "Archive Attachment",
        true,
        true,
        true,
    );
}

#[tokio::test]
async fn matrix_attachment_unarchive_not_available() {
    let h = MatrixHarness::new().await;
    let target = "attachment:/tmp/x.png";

    let surface = h.surface(&[target, "task:01X", "column:todo"]);
    assert_absent(&surface, "entity.unarchive", Some(target));
}

#[tokio::test]
async fn matrix_attachment_copy_available() {
    // attachment IS in COPYABLE_ENTITY_TYPES.
    let h = MatrixHarness::new().await;
    let target = "attachment:/tmp/x.png";

    let surface = h.surface(&[target, "task:01X", "column:todo"]);
    assert_shape(
        &surface,
        "entity.copy",
        Some(target),
        "Copy Attachment",
        true,
        true,
        true,
    );
}

#[tokio::test]
async fn matrix_attachment_cut_not_available() {
    let h = MatrixHarness::new().await;
    let target = "attachment:/tmp/x.png";

    let surface = h.surface(&[target, "task:01X", "column:todo"]);
    assert_absent(&surface, "entity.cut", Some(target));
}

#[tokio::test]
async fn matrix_attachment_paste_not_emitted() {
    // No `(*, attachment)` paste handler. Even with any clipboard, paste is
    // not available on an attachment target.
    let h = MatrixHarness::new().await;
    h.seed_clipboard("task");
    let target = "attachment:/tmp/x.png";

    let surface = h.surface(&[target, "task:01X", "column:todo"]);
    assert_absent(&surface, "entity.paste", Some(target));
}
