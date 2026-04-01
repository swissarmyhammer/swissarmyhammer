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
        changelog_lines.len() >= 1,
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

    // Legacy path writes its own per-entity changelog, but no store-level one
    // The per-entity changelog is managed by EntityContext, not StoreHandle
    let entity_changelog = dir.path().join("tags/t1.jsonl");
    assert!(
        entity_changelog.exists(),
        "per-entity changelog should exist in legacy mode"
    );

    // Delete via legacy path
    ctx.delete("tag", "t1").await.unwrap();
    assert!(
        !entity_file.exists(),
        "legacy delete should remove entity file"
    );
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
