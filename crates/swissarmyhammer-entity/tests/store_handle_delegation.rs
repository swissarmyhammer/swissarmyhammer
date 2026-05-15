//! Integration tests verifying that `EntityContext::write()` and `delete()`
//! delegate file I/O to a registered `StoreHandle<EntityTypeStore>` when one
//! is available, while the old per-entity changelog still works for activity.

use std::sync::Arc;

use serde_json::json;
use swissarmyhammer_entity::test_utils::test_fields_context;
use swissarmyhammer_entity::{Entity, EntityContext, EntityTypeStore};
use swissarmyhammer_store::StoreHandle;
use tempfile::TempDir;

/// Helper: build an EntityContext and register a StoreHandle for the "tag" entity type.
///
/// Returns `(TempDir, EntityContext, Arc<StoreHandle<EntityTypeStore>>)`.
/// The tag entity type uses plain YAML (no body_field), so files are `.yaml`.
async fn setup_with_store_handle() -> (TempDir, EntityContext, Arc<StoreHandle<EntityTypeStore>>) {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    // Build EntityTypeStore for "tag" — root is {dir}/tags/ (entity_dir convention)
    let entity_dir = dir.path().join("tags");
    std::fs::create_dir_all(&entity_dir).unwrap();

    let entity_def = fields.get_entity("tag").unwrap();
    let field_defs: Vec<_> = fields
        .fields_for_entity("tag")
        .into_iter()
        .cloned()
        .collect();

    let store = EntityTypeStore::new(
        &entity_dir,
        "tag",
        Arc::new(entity_def.clone()),
        Arc::new(field_defs),
    );
    let handle = Arc::new(StoreHandle::new(Arc::new(store)));

    ctx.register_store("tag", handle.clone()).await;

    (dir, ctx, handle)
}

/// Write through EntityContext with a registered StoreHandle.
/// Verifies the file was written AND the store changelog.jsonl was created.
#[tokio::test]
async fn write_delegates_to_store_handle() {
    let (dir, ctx, _handle) = setup_with_store_handle().await;

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Blue"));
    tag.set("color", json!("#0000ff"));

    let result = ctx.write(&tag).await.unwrap();
    assert!(result.is_some(), "write should produce a changelog entry");

    // Entity file should exist in the tags directory
    let entity_file = dir.path().join("tags/t1.yaml");
    assert!(
        entity_file.exists(),
        "entity file should be written by StoreHandle"
    );

    // Per-item changelog should exist (written by both StoreHandle and EntityContext)
    let item_changelog = dir.path().join("tags/t1.jsonl");
    assert!(
        item_changelog.exists(),
        "per-item changelog should be created by StoreHandle"
    );

    // Verify the per-item changelog has entries (read the JSONL file directly).
    // StoreHandle writes a Create entry, EntityContext also appends its own entry.
    let changelog_content = std::fs::read_to_string(&item_changelog).unwrap();
    let changelog_lines: Vec<&str> = changelog_content.lines().collect();
    assert!(
        !changelog_lines.is_empty(),
        "per-item changelog should have at least one entry"
    );

    // Verify the entity can be read back through EntityContext
    let read_back = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(read_back.get_str("tag_name"), Some("Blue"));
    assert_eq!(read_back.get_str("color"), Some("#0000ff"));
}

/// Delete through EntityContext with a registered StoreHandle.
/// Verifies the file was moved to the store's .trash/ directory.
#[tokio::test]
async fn delete_delegates_to_store_handle() {
    let (dir, ctx, _handle) = setup_with_store_handle().await;

    // Create an entity first
    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Red"));
    tag.set("color", json!("#ff0000"));
    ctx.write(&tag).await.unwrap();

    let entity_file = dir.path().join("tags/t1.yaml");
    assert!(
        entity_file.exists(),
        "entity file should exist before delete"
    );

    // Delete it
    let result = ctx.delete("tag", "t1").await.unwrap();
    assert!(result.is_some(), "delete should produce a changelog entry");

    // Entity file should be gone
    assert!(
        !entity_file.exists(),
        "entity file should be removed after delete"
    );

    // The .trash directory should exist with the trashed data file and changelog
    let trash_dir = dir.path().join("tags/.trash");
    assert!(trash_dir.exists(), "store .trash/ directory should exist");

    // Per-item changelog should be trashed alongside the data file
    assert!(
        !dir.path().join("tags/t1.jsonl").exists(),
        "per-item changelog should be trashed after delete"
    );
}

/// Without a registered StoreHandle, write() and delete() use the legacy path.
/// This verifies backwards compatibility.
#[tokio::test]
async fn write_and_delete_without_store_handle_uses_legacy_path() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    // No store handle registered — should use legacy io::write_entity
    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Green"));
    ctx.write(&tag).await.unwrap();

    let entity_file = dir.path().join("tags/t1.yaml");
    assert!(
        entity_file.exists(),
        "legacy write should create entity file"
    );

    // Legacy path does not write changelogs (no StoreHandle, no changelog code)
    let entity_changelog = dir.path().join("tags/t1.jsonl");
    assert!(
        !entity_changelog.exists(),
        "no changelog in legacy fallback mode"
    );

    // Delete via legacy path
    ctx.delete("tag", "t1").await.unwrap();
    assert!(
        !entity_file.exists(),
        "legacy delete should remove entity file"
    );
}

/// Helper: build an EntityContext and register a StoreHandle for the "task" entity type.
///
/// Returns `(TempDir, EntityContext, Arc<StoreHandle<EntityTypeStore>>)`.
/// The task entity type uses MD+YAML (body_field = "body"), so files are `.md`.
/// Includes a computed "tags" field to simulate the real kanban setup.
async fn setup_task_with_store_handle(
) -> (TempDir, EntityContext, Arc<StoreHandle<EntityTypeStore>>) {
    use swissarmyhammer_fields::FieldsContext;

    // Build a FieldsContext with a computed "tags" field on task
    let defs = vec![
        (
            "title",
            "id: 00000000000000000000000TTL\nname: title\ntype:\n  kind: text\n  single_line: true\n",
        ),
        (
            "body",
            "id: 00000000000000000000000BDY\nname: body\ntype:\n  kind: markdown\n",
        ),
        (
            "tags",
            "id: 00000000000000000000000TAG\nname: tags\ntype:\n  kind: computed\n  derive: parse-body-tags\n  depends_on:\n    - body\n",
        ),
    ];
    let entities = vec![(
        "task",
        "name: task\nbody_field: body\nfields:\n  - title\n  - body\n  - tags\n",
    )];

    let dir = TempDir::new().unwrap();
    let fields = std::sync::Arc::new(
        FieldsContext::from_yaml_sources(dir.path(), &defs, &entities).unwrap(),
    );
    let ctx = EntityContext::new(dir.path(), fields.clone());

    // Build EntityTypeStore for "task" — root is {dir}/tasks/
    let entity_dir = dir.path().join("tasks");
    std::fs::create_dir_all(&entity_dir).unwrap();

    let entity_def = fields.get_entity("task").unwrap();
    let field_defs: Vec<_> = fields
        .fields_for_entity("task")
        .into_iter()
        .cloned()
        .collect();

    let store = EntityTypeStore::new(
        &entity_dir,
        "task",
        Arc::new(entity_def.clone()),
        Arc::new(field_defs),
    );
    let handle = Arc::new(StoreHandle::new(Arc::new(store)));

    ctx.register_store("task", handle.clone()).await;

    (dir, ctx, handle)
}

/// Write a task with body containing `#bug`, then update body to remove it.
/// Verifies StoreHandle correctly persists body changes (untag scenario).
#[tokio::test]
async fn body_change_persisted_through_store_handle() {
    let (dir, ctx, _handle) = setup_task_with_store_handle().await;

    // Step 1: Create a task with #bug in the body
    let mut task = Entity::new("task", "t1");
    task.set("title", json!("Fix something"));
    task.set("body", json!("#bug some text"));
    ctx.write(&task).await.unwrap();

    // Verify it was written to disk
    let entity_file = dir.path().join("tasks/t1.md");
    assert!(
        entity_file.exists(),
        "task file should exist after first write"
    );
    let content = std::fs::read_to_string(&entity_file).unwrap();
    assert!(
        content.contains("#bug some text"),
        "file should contain '#bug some text', got: {}",
        content
    );

    // Step 2: Read back, simulate what apply_compute would do (add tags),
    // then remove #bug from body and write again.
    let read_back = ctx.read("task", "t1").await.unwrap();
    assert_eq!(read_back.get_str("body"), Some("#bug some text"));

    let mut updated = read_back;
    // Simulate apply_compute populating the computed tags field
    updated.set("tags", json!(["bug"]));
    // Simulate what untag does: modify the body field directly
    updated.set("body", json!("some text"));
    let result = ctx.write(&updated).await.unwrap();

    // The write should have detected a change (not idempotent)
    assert!(
        result.is_some(),
        "write should produce a changelog entry when body changes"
    );

    // Step 3: Verify the file on disk has the updated body
    let content_after = std::fs::read_to_string(&entity_file).unwrap();
    assert!(
        !content_after.contains("#bug"),
        "file should no longer contain '#bug' after update, got: {}",
        content_after
    );
    assert!(
        content_after.contains("some text"),
        "file should still contain 'some text', got: {}",
        content_after
    );

    // Step 4: Verify read-back is correct
    let final_read = ctx.read("task", "t1").await.unwrap();
    assert_eq!(final_read.get_str("body"), Some("some text"));
}

/// Update (second write) through StoreHandle produces an Update entry.
#[tokio::test]
async fn update_through_store_handle_produces_update_entry() {
    let (dir, ctx, _handle) = setup_with_store_handle().await;

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("V1"));
    ctx.write(&tag).await.unwrap();

    tag.set("tag_name", json!("V2"));
    ctx.write(&tag).await.unwrap();

    // Per-item changelog should have entries from both StoreHandle and EntityContext
    let item_changelog = dir.path().join("tags/t1.jsonl");
    let changelog_content = std::fs::read_to_string(&item_changelog).unwrap();
    let changelog_lines: Vec<&str> = changelog_content.lines().collect();
    assert!(
        changelog_lines.len() >= 2,
        "per-item changelog should have at least Create + Update"
    );

    // Verify entity file has V2
    let read_back = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(read_back.get_str("tag_name"), Some("V2"));

    // Per-item changelog has entries from both StoreHandle and EntityContext.
    // StoreHandle writes Create + Update, EntityContext also writes its own entries.
    let entity_changelog = dir.path().join("tags/t1.jsonl");
    let content = std::fs::read_to_string(&entity_changelog).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    assert!(
        lines.len() >= 2,
        "per-item changelog should have at least create + update entries"
    );
}
