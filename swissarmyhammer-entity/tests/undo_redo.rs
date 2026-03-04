//! Comprehensive undo/redo integration tests for the entity layer.
//!
//! These tests exercise EntityContext.undo() and EntityContext.redo() through
//! multi-step sequences, field type coverage, edge cases, and delete/restore
//! cycles. Basic round-trip tests already exist in `context.rs` unit tests;
//! this file adds thorough coverage for sequences and edge cases.

use serde_json::json;
use swissarmyhammer_entity::test_utils::test_fields_context;
use swissarmyhammer_entity::{Entity, EntityContext};
use tempfile::TempDir;

// =========================================================================
// Basic round-trips
// =========================================================================

/// Set a field, undo it, verify the field is restored to the previous value.
#[tokio::test]
async fn set_field_undo_restores_previous_value() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Original"));
    ctx.write(&tag).await.unwrap();

    tag.set("tag_name", json!("Modified"));
    let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

    ctx.undo(&update_ulid).await.unwrap();

    let restored = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(restored.get_str("tag_name"), Some("Original"));
}

/// Set a field, undo it, redo it, verify the field has the new value again.
#[tokio::test]
async fn set_field_undo_redo_restores_new_value() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Original"));
    ctx.write(&tag).await.unwrap();

    tag.set("tag_name", json!("Modified"));
    let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

    ctx.undo(&update_ulid).await.unwrap();
    let after_undo = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(after_undo.get_str("tag_name"), Some("Original"));

    ctx.redo(&update_ulid).await.unwrap();
    let after_redo = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(after_redo.get_str("tag_name"), Some("Modified"));
}

/// Create entity, undo (entity gone), redo (entity back).
#[tokio::test]
async fn create_undo_redo_cycle() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Bug"));
    tag.set("color", json!("#ff0000"));
    let create_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // Undo create: entity should be gone
    ctx.undo(&create_ulid).await.unwrap();
    assert!(ctx.read("tag", "t1").await.is_err());

    // Redo create: entity should be back
    ctx.redo(&create_ulid).await.unwrap();
    let restored = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(restored.get_str("tag_name"), Some("Bug"));
    assert_eq!(restored.get_str("color"), Some("#ff0000"));
}

/// Delete entity, undo (entity restored), redo (entity gone again).
#[tokio::test]
async fn delete_undo_redo_cycle() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Feature"));
    tag.set("color", json!("#00ff00"));
    ctx.write(&tag).await.unwrap();

    let delete_ulid = ctx.delete("tag", "t1").await.unwrap().unwrap();
    assert!(ctx.read("tag", "t1").await.is_err());

    // Undo delete: entity should be restored
    ctx.undo(&delete_ulid).await.unwrap();
    let restored = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(restored.get_str("tag_name"), Some("Feature"));
    assert_eq!(restored.get_str("color"), Some("#00ff00"));

    // Redo delete: entity should be gone again
    ctx.redo(&delete_ulid).await.unwrap();
    assert!(ctx.read("tag", "t1").await.is_err());
}

// =========================================================================
// Multi-step sequences
// =========================================================================

/// set A -> set B -> undo (B reverts) -> undo (A reverts) -> redo (A reapplied) -> redo (B reapplied)
#[tokio::test]
async fn multi_step_undo_redo_sequence() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Initial"));
    ctx.write(&tag).await.unwrap();

    // Step A: change to "StepA"
    tag.set("tag_name", json!("StepA"));
    let ulid_a = ctx.write(&tag).await.unwrap().unwrap();

    // Step B: change to "StepB"
    tag.set("tag_name", json!("StepB"));
    let ulid_b = ctx.write(&tag).await.unwrap().unwrap();

    // Verify current state
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("StepB"));

    // Undo B -> should be "StepA"
    ctx.undo(&ulid_b).await.unwrap();
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("StepA"));

    // Undo A -> should be "Initial"
    ctx.undo(&ulid_a).await.unwrap();
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("Initial"));

    // Redo A -> should be "StepA"
    ctx.redo(&ulid_a).await.unwrap();
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("StepA"));

    // Redo B -> should be "StepB"
    ctx.redo(&ulid_b).await.unwrap();
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("StepB"));
}

/// Set same field twice, undo second set -> intermediate value restored (not original).
#[tokio::test]
async fn undo_restores_intermediate_not_original() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("V1"));
    ctx.write(&tag).await.unwrap();

    tag.set("tag_name", json!("V2"));
    ctx.write(&tag).await.unwrap();

    tag.set("tag_name", json!("V3"));
    let ulid_v3 = ctx.write(&tag).await.unwrap().unwrap();

    // Undo V3 -> should restore to V2, NOT V1
    ctx.undo(&ulid_v3).await.unwrap();
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("V2"));
}

/// Set field1, set field2, undo field2 change -> field1 still has its new value.
#[tokio::test]
async fn undo_one_field_does_not_affect_other_fields() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Name"));
    tag.set("color", json!("#000000"));
    ctx.write(&tag).await.unwrap();

    // Update tag_name
    tag.set("tag_name", json!("NewName"));
    ctx.write(&tag).await.unwrap();

    // Update color
    tag.set("color", json!("#ffffff"));
    let color_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // Undo the color change
    ctx.undo(&color_ulid).await.unwrap();

    let loaded = ctx.read("tag", "t1").await.unwrap();
    // tag_name should still be the updated value
    assert_eq!(loaded.get_str("tag_name"), Some("NewName"));
    // color should be reverted
    assert_eq!(loaded.get_str("color"), Some("#000000"));
}

// =========================================================================
// Field type coverage
// =========================================================================

/// Undo/redo of multi-line text changes (TextDiff patches).
#[tokio::test]
async fn text_diff_undo_redo_multiline() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let original_body = "line 1\nline 2\nline 3\nline 4\nline 5";
    let modified_body = "line 1\nMODIFIED\nline 3\nINSERTED\nline 4\nline 5";

    let mut task = Entity::new("task", "01ABC");
    task.set("title", json!("Test"));
    task.set("body", json!(original_body));
    ctx.write(&task).await.unwrap();

    task.set("body", json!(modified_body));
    let update_ulid = ctx.write(&task).await.unwrap().unwrap();

    // Verify modification applied
    let loaded = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded.get_str("body"), Some(modified_body));

    // Undo -> back to original
    ctx.undo(&update_ulid).await.unwrap();
    let loaded = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded.get_str("body"), Some(original_body));

    // Redo -> back to modified
    ctx.redo(&update_ulid).await.unwrap();
    let loaded = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded.get_str("body"), Some(modified_body));
}

/// Undo/redo of non-string fields: numbers, booleans, arrays (Changed with old/new JSON).
#[tokio::test]
async fn non_string_field_undo_redo() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    // Use a task with frontmatter fields to test non-string values
    let mut task = Entity::new("task", "01ABC");
    task.set("title", json!("Test"));
    task.set("body", json!("body text"));
    ctx.write(&task).await.unwrap();

    // Add a non-string field (stored in frontmatter as JSON value)
    // Note: although "title" is defined as text, we can test with the
    // raw entity API which doesn't enforce types without a validation engine.
    // Using a tag with color (stored as plain yaml) to test Changed for non-string.
    let dir2 = TempDir::new().unwrap();
    let ctx2 = EntityContext::new(dir2.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Test"));
    tag.set("color", json!("#000000"));
    ctx2.write(&tag).await.unwrap();

    tag.set("color", json!("#ffffff"));
    let update_ulid = ctx2.write(&tag).await.unwrap().unwrap();

    let loaded = ctx2.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("color"), Some("#ffffff"));

    // Undo
    ctx2.undo(&update_ulid).await.unwrap();
    let loaded = ctx2.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("color"), Some("#000000"));

    // Redo
    ctx2.redo(&update_ulid).await.unwrap();
    let loaded = ctx2.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("color"), Some("#ffffff"));
}

/// Field addition (Set) -> undo removes field -> redo adds it back.
#[tokio::test]
async fn field_addition_undo_removes_redo_readds() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    // Create tag with only tag_name (no color)
    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Bug"));
    ctx.write(&tag).await.unwrap();

    // Add the color field
    tag.set("color", json!("#ff0000"));
    let add_ulid = ctx.write(&tag).await.unwrap().unwrap();

    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("color"), Some("#ff0000"));

    // Undo -> color field should be removed
    ctx.undo(&add_ulid).await.unwrap();
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get("color"), None);
    assert_eq!(loaded.get_str("tag_name"), Some("Bug"));

    // Redo -> color field should be back
    ctx.redo(&add_ulid).await.unwrap();
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("color"), Some("#ff0000"));
}

/// Field removal -> undo re-adds field -> redo removes it again.
#[tokio::test]
async fn field_removal_undo_readds_redo_removes() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    // Create tag with both fields
    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Bug"));
    tag.set("color", json!("#ff0000"));
    ctx.write(&tag).await.unwrap();

    // Remove the color field
    tag.remove("color");
    let remove_ulid = ctx.write(&tag).await.unwrap().unwrap();

    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get("color"), None);

    // Undo -> color field should be back
    ctx.undo(&remove_ulid).await.unwrap();
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("color"), Some("#ff0000"));

    // Redo -> color field removed again
    ctx.redo(&remove_ulid).await.unwrap();
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get("color"), None);
}

/// Multi-line text with scattered edits produces TextDiff that round-trips through undo/redo.
#[tokio::test]
async fn scattered_multiline_text_edits_undo_redo() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let old_lines: Vec<String> = (1..=30).map(|i| format!("line {}", i)).collect();
    let original_body = old_lines.join("\n");

    let mut new_lines = old_lines.clone();
    new_lines[2] = "MODIFIED line 3".into(); // edit near top
    new_lines.insert(10, "INSERTED after line 10".into()); // insert in middle
    new_lines[25] = "MODIFIED line 25".into(); // edit near bottom (shifted by insert)
    new_lines.push("APPENDED line 31".into()); // append at end
    let modified_body = new_lines.join("\n");

    let mut task = Entity::new("task", "01ABC");
    task.set("title", json!("Scattered edits"));
    task.set("body", json!(original_body));
    ctx.write(&task).await.unwrap();

    task.set("body", json!(modified_body));
    let update_ulid = ctx.write(&task).await.unwrap().unwrap();

    // Verify modification
    let loaded = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded.get_str("body"), Some(modified_body.as_str()));

    // Undo -> original
    ctx.undo(&update_ulid).await.unwrap();
    let loaded = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded.get_str("body"), Some(original_body.as_str()));

    // Redo -> modified
    ctx.redo(&update_ulid).await.unwrap();
    let loaded = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded.get_str("body"), Some(modified_body.as_str()));
}

// =========================================================================
// Edge cases
// =========================================================================

/// Undo after entity was modified by another operation (stale undo) should error.
#[tokio::test]
async fn stale_undo_errors() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("V1"));
    ctx.write(&tag).await.unwrap();

    tag.set("tag_name", json!("V2"));
    let v2_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // Another modification on top
    tag.set("tag_name", json!("V3"));
    ctx.write(&tag).await.unwrap();

    // Attempting to undo V2 when entity is at V3 should fail
    // because the reverse TextDiff expects the entity to be at V2
    let result = ctx.undo(&v2_ulid).await;
    assert!(
        result.is_err(),
        "undoing a stale update should return an error"
    );
}

/// Undo the same operation twice should error (entity state changed after first undo).
#[tokio::test]
async fn double_undo_same_operation_errors() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("V1"));
    ctx.write(&tag).await.unwrap();

    tag.set("tag_name", json!("V2"));
    let v2_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // First undo succeeds
    ctx.undo(&v2_ulid).await.unwrap();
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("V1"));

    // Second undo of the same ULID should fail because entity is now at V1,
    // and the reverse diff expects V2 as the current state
    let result = ctx.undo(&v2_ulid).await;
    assert!(
        result.is_err(),
        "undoing the same operation twice should error"
    );
}

/// Undo of create when entity has been modified since creation.
/// The undo still deletes/trashes the entity, and the changelog shows
/// all current fields as Removed.
#[tokio::test]
async fn undo_create_after_modification_trashes_modified_entity() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Original"));
    let create_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // Modify the entity after creation
    tag.set("tag_name", json!("Modified"));
    tag.set("color", json!("#ff0000"));
    ctx.write(&tag).await.unwrap();

    // Undo the create -- should still trash the entity
    ctx.undo(&create_ulid).await.unwrap();
    assert!(ctx.read("tag", "t1").await.is_err());

    // Verify the trash contains the modified version
    let trash_dir = dir.path().join(".trash").join("tags");
    assert!(trash_dir.join("t1.yaml").exists());
}

/// Undo of delete when trash data file has been manually removed should error.
///
/// The restore_entity_files function requires the data file to be present
/// in trash. If someone manually deletes it, undo must fail with a clear error
/// rather than silently succeeding with nothing restored.
#[tokio::test]
async fn undo_delete_missing_trash_files_returns_error_or_empty() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Bug"));
    ctx.write(&tag).await.unwrap();

    let delete_ulid = ctx.delete("tag", "t1").await.unwrap().unwrap();

    // Manually remove both trash files
    let trash_dir = dir.path().join(".trash").join("tags");
    let _ = tokio::fs::remove_file(trash_dir.join("t1.yaml")).await;
    let _ = tokio::fs::remove_file(trash_dir.join("t1.jsonl")).await;

    // Attempting to undo must fail — the trash files are gone.
    // May error on changelog lookup or on restore_entity_files depending
    // on which file is checked first.
    let result = ctx.undo(&delete_ulid).await;
    assert!(
        result.is_err(),
        "undo of delete should fail when trash files are missing"
    );
}

/// Undo of delete when only the trash data file is removed (changelog still present)
/// should error with RestoreFromTrashFailed.
#[tokio::test]
async fn undo_delete_missing_trash_data_file_returns_restore_error() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Bug"));
    ctx.write(&tag).await.unwrap();

    let delete_ulid = ctx.delete("tag", "t1").await.unwrap().unwrap();

    // Remove only the data file — leave the changelog so undo_single can
    // find the original entry and reach restore_entity_files
    let trash_dir = dir.path().join(".trash").join("tags");
    let _ = tokio::fs::remove_file(trash_dir.join("t1.yaml")).await;

    let result = ctx.undo(&delete_ulid).await;
    assert!(
        result.is_err(),
        "undo of delete should fail when trash data file is missing"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("cannot restore from trash"),
        "error should mention restore from trash, got: {err}"
    );
}

/// Empty changes (write with no actual diff) -- no changelog entry, returns None.
#[tokio::test]
async fn idempotent_write_produces_no_changelog_entry() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Bug"));
    tag.set("color", json!("#ff0000"));
    ctx.write(&tag).await.unwrap();

    // Write the exact same entity again
    let result = ctx.write(&tag).await.unwrap();
    assert_eq!(
        result, None,
        "writing identical entity should return None (no changes)"
    );

    // Changelog should have exactly 1 entry (the create), not 2
    let log = ctx.read_changelog("tag", "t1").await.unwrap();
    assert_eq!(log.len(), 1, "should have only the create entry");
    assert_eq!(log[0].op, "create");
}

// =========================================================================
// Delete/restore cycles
// =========================================================================

/// delete -> undo -> entity readable with all original fields.
#[tokio::test]
async fn delete_then_undo_restores_all_fields() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Bug"));
    tag.set("color", json!("#ff0000"));
    ctx.write(&tag).await.unwrap();

    let delete_ulid = ctx.delete("tag", "t1").await.unwrap().unwrap();
    assert!(ctx.read("tag", "t1").await.is_err());

    ctx.undo(&delete_ulid).await.unwrap();
    let restored = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(restored.get_str("tag_name"), Some("Bug"));
    assert_eq!(restored.get_str("color"), Some("#ff0000"));
}

/// delete -> undo -> delete again (new op) -> undo new delete -> entity back again.
#[tokio::test]
async fn delete_undo_delete_again_undo_again_cycle() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Bug"));
    tag.set("color", json!("#ff0000"));
    ctx.write(&tag).await.unwrap();

    // First delete
    let delete1_ulid = ctx.delete("tag", "t1").await.unwrap().unwrap();
    assert!(ctx.read("tag", "t1").await.is_err());

    // Undo first delete
    ctx.undo(&delete1_ulid).await.unwrap();
    let restored = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(restored.get_str("tag_name"), Some("Bug"));

    // Delete again (a new operation)
    let delete2_ulid = ctx.delete("tag", "t1").await.unwrap().unwrap();
    assert!(ctx.read("tag", "t1").await.is_err());

    // Undo the new delete
    ctx.undo(&delete2_ulid).await.unwrap();
    let restored = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(restored.get_str("tag_name"), Some("Bug"));
    assert_eq!(restored.get_str("color"), Some("#ff0000"));
}

// =========================================================================
// Changelog entry correctness
// =========================================================================

/// Undo changelog entry has op="undo" and undone_id pointing to the original.
#[tokio::test]
async fn undo_changelog_entry_references_original() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Bug"));
    ctx.write(&tag).await.unwrap();

    tag.set("tag_name", json!("Bug Report"));
    let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

    let undo_ulid = ctx.undo(&update_ulid).await.unwrap().unwrap();

    let log = ctx.read_changelog("tag", "t1").await.unwrap();
    let undo_entry = log.iter().find(|e| e.id == undo_ulid).unwrap();

    assert_eq!(undo_entry.op, "undo");
    assert_eq!(undo_entry.undone_id.as_deref(), Some(update_ulid.as_str()));
    assert_eq!(undo_entry.entity_type, "tag");
    assert_eq!(undo_entry.entity_id, "t1");
}

/// Redo changelog entry has op="redo" and redone_id pointing to the original.
#[tokio::test]
async fn redo_changelog_entry_references_original() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Bug"));
    ctx.write(&tag).await.unwrap();

    tag.set("tag_name", json!("Bug Report"));
    let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

    ctx.undo(&update_ulid).await.unwrap();
    let redo_ulid = ctx.redo(&update_ulid).await.unwrap().unwrap();

    let log = ctx.read_changelog("tag", "t1").await.unwrap();
    let redo_entry = log.iter().find(|e| e.id == redo_ulid).unwrap();

    assert_eq!(redo_entry.op, "redo");
    assert_eq!(redo_entry.redone_id.as_deref(), Some(update_ulid.as_str()));
    assert_eq!(redo_entry.entity_type, "tag");
    assert_eq!(redo_entry.entity_id, "t1");
}

/// Changelog grows correctly through a write/undo/redo sequence.
#[tokio::test]
async fn changelog_grows_through_undo_redo_sequence() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("V1"));
    ctx.write(&tag).await.unwrap();

    tag.set("tag_name", json!("V2"));
    let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

    ctx.undo(&update_ulid).await.unwrap();
    ctx.redo(&update_ulid).await.unwrap();

    let log = ctx.read_changelog("tag", "t1").await.unwrap();
    // Should have: create, update, undo, redo = 4 entries
    assert_eq!(log.len(), 4);
    assert_eq!(log[0].op, "create");
    assert_eq!(log[1].op, "update");
    assert_eq!(log[2].op, "undo");
    assert_eq!(log[3].op, "redo");
}

// =========================================================================
// Undo/redo with body-field entities (task with markdown body)
// =========================================================================

/// Full undo/redo cycle on a task with frontmatter + markdown body.
#[tokio::test]
async fn task_body_undo_redo_preserves_both_frontmatter_and_body() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut task = Entity::new("task", "01ABC");
    task.set("title", json!("Original Title"));
    task.set("body", json!("Original body\nwith multiple\nlines."));
    ctx.write(&task).await.unwrap();

    // Update both title and body
    task.set("title", json!("Updated Title"));
    task.set("body", json!("Updated body\nwith different\ncontent."));
    let update_ulid = ctx.write(&task).await.unwrap().unwrap();

    // Undo
    ctx.undo(&update_ulid).await.unwrap();
    let loaded = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded.get_str("title"), Some("Original Title"));
    assert_eq!(
        loaded.get_str("body"),
        Some("Original body\nwith multiple\nlines.")
    );

    // Redo
    ctx.redo(&update_ulid).await.unwrap();
    let loaded = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded.get_str("title"), Some("Updated Title"));
    assert_eq!(
        loaded.get_str("body"),
        Some("Updated body\nwith different\ncontent.")
    );
}

/// Undo/redo of body-only change (title unchanged).
#[tokio::test]
async fn task_body_only_change_undo_redo() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut task = Entity::new("task", "01ABC");
    task.set("title", json!("Fixed Title"));
    task.set("body", json!("Version 1 of the body."));
    ctx.write(&task).await.unwrap();

    task.set("body", json!("Version 2 of the body."));
    let update_ulid = ctx.write(&task).await.unwrap().unwrap();

    ctx.undo(&update_ulid).await.unwrap();
    let loaded = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded.get_str("title"), Some("Fixed Title"));
    assert_eq!(loaded.get_str("body"), Some("Version 1 of the body."));

    ctx.redo(&update_ulid).await.unwrap();
    let loaded = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded.get_str("title"), Some("Fixed Title"));
    assert_eq!(loaded.get_str("body"), Some("Version 2 of the body."));
}

// =========================================================================
// Undo/redo of create and delete with body-field entities
// =========================================================================

/// Create task -> undo (task gone) -> redo (task back with all fields).
#[tokio::test]
async fn task_create_undo_redo() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut task = Entity::new("task", "01ABC");
    task.set("title", json!("My Task"));
    task.set("body", json!("Task body content."));
    let create_ulid = ctx.write(&task).await.unwrap().unwrap();

    ctx.undo(&create_ulid).await.unwrap();
    assert!(ctx.read("task", "01ABC").await.is_err());

    ctx.redo(&create_ulid).await.unwrap();
    let loaded = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded.get_str("title"), Some("My Task"));
    assert_eq!(loaded.get_str("body"), Some("Task body content."));
}

/// Delete task -> undo (task back) -> redo (task gone).
#[tokio::test]
async fn task_delete_undo_redo() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut task = Entity::new("task", "01ABC");
    task.set("title", json!("Deletable Task"));
    task.set("body", json!("This will be deleted."));
    ctx.write(&task).await.unwrap();

    let delete_ulid = ctx.delete("task", "01ABC").await.unwrap().unwrap();
    assert!(ctx.read("task", "01ABC").await.is_err());

    ctx.undo(&delete_ulid).await.unwrap();
    let loaded = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded.get_str("title"), Some("Deletable Task"));
    assert_eq!(loaded.get_str("body"), Some("This will be deleted."));

    ctx.redo(&delete_ulid).await.unwrap();
    assert!(ctx.read("task", "01ABC").await.is_err());
}

// =========================================================================
// Stale redo
// =========================================================================

/// Redo after entity was modified by another operation (stale redo) should error.
#[tokio::test]
async fn stale_redo_errors() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("V1"));
    ctx.write(&tag).await.unwrap();

    tag.set("tag_name", json!("V2"));
    let v2_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // Undo back to V1
    ctx.undo(&v2_ulid).await.unwrap();

    // Manually modify to V3 (a different path)
    tag.set("tag_name", json!("V3"));
    ctx.write(&tag).await.unwrap();

    // Attempting to redo V2 when entity is at V3 should fail
    let result = ctx.redo(&v2_ulid).await;
    assert!(
        result.is_err(),
        "redoing a stale operation should return an error"
    );
}

/// Redo unknown ULID should error.
#[tokio::test]
async fn redo_unknown_ulid_errors() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let result = ctx.redo("01NONEXISTENT000000000000").await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not found"));
}

/// Undo unknown ULID should error.
#[tokio::test]
async fn undo_unknown_ulid_errors() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let result = ctx.undo("01NONEXISTENT000000000000").await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not found"));
}

// =========================================================================
// Multiple entities are independent
// =========================================================================

/// Undo on one entity does not affect a different entity.
#[tokio::test]
async fn undo_on_one_entity_does_not_affect_another() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    // Create two tags
    let mut tag1 = Entity::new("tag", "t1");
    tag1.set("tag_name", json!("Tag1"));
    ctx.write(&tag1).await.unwrap();

    let mut tag2 = Entity::new("tag", "t2");
    tag2.set("tag_name", json!("Tag2"));
    ctx.write(&tag2).await.unwrap();

    // Update tag1
    tag1.set("tag_name", json!("Tag1-Updated"));
    let t1_ulid = ctx.write(&tag1).await.unwrap().unwrap();

    // Update tag2
    tag2.set("tag_name", json!("Tag2-Updated"));
    ctx.write(&tag2).await.unwrap();

    // Undo only tag1's update
    ctx.undo(&t1_ulid).await.unwrap();

    // tag1 should be reverted
    let loaded1 = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded1.get_str("tag_name"), Some("Tag1"));

    // tag2 should be unchanged
    let loaded2 = ctx.read("tag", "t2").await.unwrap();
    assert_eq!(loaded2.get_str("tag_name"), Some("Tag2-Updated"));
}

/// Undo/redo of entities across different entity types (tag vs task).
#[tokio::test]
async fn undo_redo_across_entity_types() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    // Create a tag and a task
    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    ctx.write(&tag).await.unwrap();

    let mut task = Entity::new("task", "01ABC");
    task.set("title", json!("Fix bug"));
    task.set("body", json!("Details"));
    ctx.write(&task).await.unwrap();

    // Update both
    tag.set("tag_name", json!("Bug Report"));
    let tag_ulid = ctx.write(&tag).await.unwrap().unwrap();

    task.set("title", json!("Fix important bug"));
    let task_ulid = ctx.write(&task).await.unwrap().unwrap();

    // Undo the tag update
    ctx.undo(&tag_ulid).await.unwrap();
    let loaded_tag = ctx.read("tag", "bug").await.unwrap();
    assert_eq!(loaded_tag.get_str("tag_name"), Some("Bug"));

    // Task should be unaffected
    let loaded_task = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded_task.get_str("title"), Some("Fix important bug"));

    // Undo the task update
    ctx.undo(&task_ulid).await.unwrap();
    let loaded_task = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded_task.get_str("title"), Some("Fix bug"));

    // Redo both
    ctx.redo(&tag_ulid).await.unwrap();
    ctx.redo(&task_ulid).await.unwrap();

    let loaded_tag = ctx.read("tag", "bug").await.unwrap();
    assert_eq!(loaded_tag.get_str("tag_name"), Some("Bug Report"));
    let loaded_task = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded_task.get_str("title"), Some("Fix important bug"));
}

// =========================================================================
// Full undo-redo-undo-redo cycle
// =========================================================================

/// Full cycle: update -> undo -> redo -> undo -> redo verifying state at each step.
#[tokio::test]
async fn full_undo_redo_undo_redo_cycle() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Original"));
    tag.set("color", json!("#000000"));
    ctx.write(&tag).await.unwrap();

    tag.set("tag_name", json!("Modified"));
    let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // State: Modified
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("Modified"));

    // Undo -> Original
    ctx.undo(&update_ulid).await.unwrap();
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("Original"));
    assert_eq!(loaded.get_str("color"), Some("#000000"));

    // Redo -> Modified
    ctx.redo(&update_ulid).await.unwrap();
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("Modified"));
    assert_eq!(loaded.get_str("color"), Some("#000000"));

    // Undo again -> Original
    ctx.undo(&update_ulid).await.unwrap();
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("Original"));

    // Redo again -> Modified
    ctx.redo(&update_ulid).await.unwrap();
    let loaded = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("Modified"));
}

// =========================================================================
// Listing after undo/redo
// =========================================================================

/// After undoing a create, the entity should not appear in list().
#[tokio::test]
async fn list_excludes_undone_created_entity() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag1 = Entity::new("tag", "t1");
    tag1.set("tag_name", json!("Tag1"));
    ctx.write(&tag1).await.unwrap();

    let mut tag2 = Entity::new("tag", "t2");
    tag2.set("tag_name", json!("Tag2"));
    let t2_create_ulid = ctx.write(&tag2).await.unwrap().unwrap();

    let tags = ctx.list("tag").await.unwrap();
    assert_eq!(tags.len(), 2);

    // Undo the creation of t2
    ctx.undo(&t2_create_ulid).await.unwrap();

    let tags = ctx.list("tag").await.unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].id, "t1");
}

/// After undoing a delete, the entity should reappear in list().
#[tokio::test]
async fn list_includes_undone_deleted_entity() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    let mut tag1 = Entity::new("tag", "t1");
    tag1.set("tag_name", json!("Tag1"));
    ctx.write(&tag1).await.unwrap();

    let mut tag2 = Entity::new("tag", "t2");
    tag2.set("tag_name", json!("Tag2"));
    ctx.write(&tag2).await.unwrap();

    // Delete t2
    let delete_ulid = ctx.delete("tag", "t2").await.unwrap().unwrap();
    let tags = ctx.list("tag").await.unwrap();
    assert_eq!(tags.len(), 1);

    // Undo the delete
    ctx.undo(&delete_ulid).await.unwrap();
    let tags = ctx.list("tag").await.unwrap();
    assert_eq!(tags.len(), 2);
}

// =========================================================================
// Partial transaction undo/redo failure with rollback
// =========================================================================

/// Transaction undo where one entry fails midway should roll back completed
/// entries and return TransactionPartialFailure with rollback_succeeded = true.
///
/// Setup: create a transaction with two updates across two entities. Make one
/// entity stale so its undo fails. The other entity's undo should be rolled back.
#[tokio::test]
async fn partial_transaction_undo_rolls_back_on_failure() {
    use swissarmyhammer_entity::EntityError;

    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    // Create two tags
    let mut tag_a = Entity::new("tag", "a");
    tag_a.set("tag_name", json!("AlphaOrig"));
    ctx.write(&tag_a).await.unwrap();

    let mut tag_b = Entity::new("tag", "b");
    tag_b.set("tag_name", json!("BetaOrig"));
    ctx.write(&tag_b).await.unwrap();

    // Create a transaction that updates both tags
    let tx_id = EntityContext::generate_transaction_id();
    ctx.set_transaction(tx_id.clone()).await;

    tag_a.set("tag_name", json!("AlphaNew"));
    ctx.write(&tag_a).await.unwrap();

    tag_b.set("tag_name", json!("BetaNew"));
    ctx.write(&tag_b).await.unwrap();

    ctx.clear_transaction().await;

    // Verify transaction state
    let loaded_a = ctx.read("tag", "a").await.unwrap();
    assert_eq!(loaded_a.get_str("tag_name"), Some("AlphaNew"));
    let loaded_b = ctx.read("tag", "b").await.unwrap();
    assert_eq!(loaded_b.get_str("tag_name"), Some("BetaNew"));

    // Make tag_a stale by writing another update OUTSIDE the transaction.
    // Transaction undo reverses entries in reverse order: first undo B, then A.
    // A is stale so its undo will fail after B has been undone.
    tag_a.set("tag_name", json!("AlphaStale"));
    ctx.write(&tag_a).await.unwrap();

    // Attempt to undo the transaction — should fail with TransactionPartialFailure
    let result = ctx.undo(&tx_id).await;
    assert!(result.is_err(), "undo should fail due to stale entry");

    let err = result.unwrap_err();
    match &err {
        EntityError::TransactionPartialFailure {
            original_error,
            completed,
            failed_entry: _,
            rollback_succeeded,
        } => {
            // B was undone first (reverse order), then A failed
            assert_eq!(completed.len(), 1, "one entry should have been completed before failure");
            assert!(
                original_error.contains("patch"),
                "original error should be about patch failure, got: {}",
                original_error
            );
            assert!(
                *rollback_succeeded,
                "rollback should succeed (re-redo the undone B entry)"
            );
        }
        other => panic!(
            "expected TransactionPartialFailure, got: {:?}",
            other
        ),
    }

    // After successful rollback, both entities should be in their
    // post-transaction state (plus the stale write on A)
    let loaded_a = ctx.read("tag", "a").await.unwrap();
    assert_eq!(
        loaded_a.get_str("tag_name"),
        Some("AlphaStale"),
        "tag A should still have the stale value"
    );

    let loaded_b = ctx.read("tag", "b").await.unwrap();
    assert_eq!(
        loaded_b.get_str("tag_name"),
        Some("BetaNew"),
        "tag B should be rolled back to post-transaction state"
    );
}

/// Transaction redo where one entry fails midway should roll back completed
/// entries and return TransactionPartialFailure.
#[tokio::test]
async fn partial_transaction_redo_rolls_back_on_failure() {
    use swissarmyhammer_entity::EntityError;

    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    // Create two tags
    let mut tag_a = Entity::new("tag", "a");
    tag_a.set("tag_name", json!("AlphaOrig"));
    ctx.write(&tag_a).await.unwrap();

    let mut tag_b = Entity::new("tag", "b");
    tag_b.set("tag_name", json!("BetaOrig"));
    ctx.write(&tag_b).await.unwrap();

    // Create a transaction that updates both tags
    let tx_id = EntityContext::generate_transaction_id();
    ctx.set_transaction(tx_id.clone()).await;

    tag_a.set("tag_name", json!("AlphaNew"));
    ctx.write(&tag_a).await.unwrap();

    tag_b.set("tag_name", json!("BetaNew"));
    ctx.write(&tag_b).await.unwrap();

    ctx.clear_transaction().await;

    // Undo the whole transaction (clean undo, no staleness)
    ctx.undo(&tx_id).await.unwrap();

    // Verify undo worked
    let loaded_a = ctx.read("tag", "a").await.unwrap();
    assert_eq!(loaded_a.get_str("tag_name"), Some("AlphaOrig"));
    let loaded_b = ctx.read("tag", "b").await.unwrap();
    assert_eq!(loaded_b.get_str("tag_name"), Some("BetaOrig"));

    // Make tag_b stale by writing another update.
    // Redo processes entries in forward order: A first, then B.
    // B is stale so its redo will fail after A has been redone.
    tag_b.set("tag_name", json!("BetaStale"));
    ctx.write(&tag_b).await.unwrap();

    // Attempt to redo the transaction — should fail with TransactionPartialFailure
    let result = ctx.redo(&tx_id).await;
    assert!(result.is_err(), "redo should fail due to stale entry");

    let err = result.unwrap_err();
    match &err {
        EntityError::TransactionPartialFailure {
            original_error,
            completed,
            failed_entry: _,
            rollback_succeeded,
        } => {
            // A was redone first (forward order), then B failed
            assert_eq!(completed.len(), 1, "one entry should have been completed before failure");
            assert!(
                original_error.contains("patch"),
                "original error should be about patch failure, got: {}",
                original_error
            );
            assert!(
                *rollback_succeeded,
                "rollback should succeed (re-undo the redone A entry)"
            );
        }
        other => panic!(
            "expected TransactionPartialFailure, got: {:?}",
            other
        ),
    }

    // After successful rollback, entities should be in their pre-redo state
    let loaded_a = ctx.read("tag", "a").await.unwrap();
    assert_eq!(
        loaded_a.get_str("tag_name"),
        Some("AlphaOrig"),
        "tag A should be rolled back to pre-redo state"
    );

    let loaded_b = ctx.read("tag", "b").await.unwrap();
    assert_eq!(
        loaded_b.get_str("tag_name"),
        Some("BetaStale"),
        "tag B should still have the stale value"
    );
}

/// Transaction undo where a single entry succeeds cleanly should
/// still work as before (no rollback needed).
#[tokio::test]
async fn transaction_undo_succeeds_when_all_entries_are_clean() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    // Create two tags
    let mut tag_a = Entity::new("tag", "a");
    tag_a.set("tag_name", json!("AlphaOrig"));
    ctx.write(&tag_a).await.unwrap();

    let mut tag_b = Entity::new("tag", "b");
    tag_b.set("tag_name", json!("BetaOrig"));
    ctx.write(&tag_b).await.unwrap();

    // Create a transaction that updates both
    let tx_id = EntityContext::generate_transaction_id();
    ctx.set_transaction(tx_id.clone()).await;

    tag_a.set("tag_name", json!("AlphaNew"));
    ctx.write(&tag_a).await.unwrap();

    tag_b.set("tag_name", json!("BetaNew"));
    ctx.write(&tag_b).await.unwrap();

    ctx.clear_transaction().await;

    // Undo the transaction (no staleness — should succeed cleanly)
    ctx.undo(&tx_id).await.unwrap();

    let loaded_a = ctx.read("tag", "a").await.unwrap();
    assert_eq!(loaded_a.get_str("tag_name"), Some("AlphaOrig"));
    let loaded_b = ctx.read("tag", "b").await.unwrap();
    assert_eq!(loaded_b.get_str("tag_name"), Some("BetaOrig"));

    // Redo should also work cleanly
    ctx.redo(&tx_id).await.unwrap();

    let loaded_a = ctx.read("tag", "a").await.unwrap();
    assert_eq!(loaded_a.get_str("tag_name"), Some("AlphaNew"));
    let loaded_b = ctx.read("tag", "b").await.unwrap();
    assert_eq!(loaded_b.get_str("tag_name"), Some("BetaNew"));
}

/// TransactionPartialFailure error message includes useful information.
#[tokio::test]
async fn partial_failure_error_display_is_informative() {
    use swissarmyhammer_entity::EntityError;

    let err = EntityError::TransactionPartialFailure {
        original_error: "patch apply error: context mismatch".to_string(),
        completed: vec!["ULID_A".to_string(), "ULID_B".to_string()],
        failed_entry: "ULID_C".to_string(),
        rollback_succeeded: true,
    };
    let msg = err.to_string();
    assert!(msg.contains("ULID_C"), "should mention the failed entry");
    assert!(msg.contains("2 entries"), "should mention completed count");
    assert!(msg.contains("succeeded"), "should mention rollback status");

    let err_failed = EntityError::TransactionPartialFailure {
        original_error: "some error".to_string(),
        completed: vec!["ULID_X".to_string()],
        failed_entry: "ULID_Y".to_string(),
        rollback_succeeded: false,
    };
    let msg_failed = err_failed.to_string();
    assert!(msg_failed.contains("failed"), "should mention rollback failed");
}

// =========================================================================
// Non-string stale detection
// =========================================================================

/// Update a non-string field (JSON number), modify the entity behind the
/// changelog's back, then undo. The undo should fail because the current value
/// no longer matches what the changelog entry expects.
///
/// Uses JSON numbers to ensure the diff produces `Changed` (not `TextDiff`),
/// exercising the new stale detection on the Changed variant.
#[tokio::test]
async fn non_string_stale_undo_errors() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    // Create tag with a numeric "color" value (non-string → Changed diff)
    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Red"));
    tag.set("color", json!(100));
    ctx.write(&tag).await.unwrap();

    // Update color from 100 → 200 (produces a Changed changelog entry)
    tag.set("color", json!(200));
    let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // Modify color to 999 behind the changelog's back.
    // The entity now has color=999 but the changelog expects color=200 for undo.
    tag.set("color", json!(999));
    ctx.write(&tag).await.unwrap();

    // Attempt undo — should fail because current (999) != expected (200)
    let result = ctx.undo(&update_ulid).await;
    assert!(
        result.is_err(),
        "undo of a stale non-string field should fail"
    );

    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("stale"),
        "error should mention stale change, got: {}",
        msg
    );
    assert!(
        msg.contains("color"),
        "error should mention the field name, got: {}",
        msg
    );
}

/// Update a non-string field, undo it, modify the entity behind the
/// changelog's back, then redo. The redo should fail because the current
/// value no longer matches what the changelog entry expects.
#[tokio::test]
async fn non_string_stale_redo_errors() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    // Create tag with numeric color
    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Red"));
    tag.set("color", json!(100));
    ctx.write(&tag).await.unwrap();

    // Update color from 100 → 200
    tag.set("color", json!(200));
    let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // Undo the update (clean — should succeed, restores color to 100)
    ctx.undo(&update_ulid).await.unwrap();
    let after_undo = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(after_undo.get_i64("color"), Some(100));

    // Modify color to 999 behind the changelog's back
    let mut stale_tag = after_undo.clone();
    stale_tag.set("color", json!(999));
    ctx.write(&stale_tag).await.unwrap();

    // Attempt redo — should fail because current (999) != expected (100)
    let result = ctx.redo(&update_ulid).await;
    assert!(
        result.is_err(),
        "redo of a stale non-string field should fail"
    );

    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("stale"),
        "error should mention stale change, got: {}",
        msg
    );
}

/// Update a non-string field, undo, verify undo succeeds (happy path).
/// Ensures the stale detection check does not break clean undo/redo.
#[tokio::test]
async fn non_string_clean_undo_succeeds() {
    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    // Create tag with numeric color
    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Red"));
    tag.set("color", json!(100));
    ctx.write(&tag).await.unwrap();

    // Update color from 100 → 200
    tag.set("color", json!(200));
    let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // Undo — no intervening changes, should succeed
    ctx.undo(&update_ulid).await.unwrap();
    let restored = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(restored.get_i64("color"), Some(100));

    // Redo — should also succeed cleanly
    ctx.redo(&update_ulid).await.unwrap();
    let re_applied = ctx.read("tag", "t1").await.unwrap();
    assert_eq!(re_applied.get_i64("color"), Some(200));
}

// =========================================================================
// Unsupported undo/redo op type errors
// =========================================================================

/// Attempting to undo an "undo" changelog entry should return an
/// `UnsupportedUndoOp` error instead of silently succeeding.
#[tokio::test]
async fn undo_of_undo_entry_returns_error() {
    use swissarmyhammer_entity::EntityError;

    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    // Create a tag entity.
    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Red"));
    tag.set("color", json!("ff0000"));
    ctx.write(&tag).await.unwrap();

    // Update the tag (produces an "update" changelog entry).
    tag.set("color", json!("00ff00"));
    let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // Undo the update — this creates an "undo" changelog entry and returns its ULID.
    let undo_ulid = ctx.undo(&update_ulid).await.unwrap().unwrap();

    // Now attempt to undo the undo entry itself — should error.
    let result = ctx.undo(&undo_ulid).await;
    assert!(
        result.is_err(),
        "undoing an 'undo' entry should return an error"
    );

    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unsupported undo/redo operation type"),
        "error should mention unsupported op type, got: {msg}"
    );
    assert!(
        msg.contains("undo"),
        "error should mention the 'undo' op, got: {msg}"
    );

    // Verify it's the right variant.
    assert!(
        matches!(err, EntityError::UnsupportedUndoOp { ref op } if op == "undo"),
        "expected UnsupportedUndoOp with op='undo', got: {err:?}"
    );
}

/// Attempting to undo a "redo" changelog entry should return an
/// `UnsupportedUndoOp` error instead of silently succeeding.
#[tokio::test]
async fn undo_of_redo_entry_returns_error() {
    use swissarmyhammer_entity::EntityError;

    let dir = TempDir::new().unwrap();
    let ctx = EntityContext::new(dir.path(), test_fields_context());

    // Create a tag entity.
    let mut tag = Entity::new("tag", "t1");
    tag.set("tag_name", json!("Blue"));
    tag.set("color", json!("0000ff"));
    ctx.write(&tag).await.unwrap();

    // Update the tag.
    tag.set("color", json!("ff00ff"));
    let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // Undo the update.
    ctx.undo(&update_ulid).await.unwrap();

    // Redo the update — this creates a "redo" changelog entry and returns its ULID.
    let redo_ulid = ctx.redo(&update_ulid).await.unwrap().unwrap();

    // Now attempt to undo the redo entry itself — should error.
    let result = ctx.undo(&redo_ulid).await;
    assert!(
        result.is_err(),
        "undoing a 'redo' entry should return an error"
    );

    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unsupported undo/redo operation type"),
        "error should mention unsupported op type, got: {msg}"
    );
    assert!(
        msg.contains("redo"),
        "error should mention the 'redo' op, got: {msg}"
    );

    // Verify it's the right variant.
    assert!(
        matches!(err, EntityError::UnsupportedUndoOp { ref op } if op == "redo"),
        "expected UnsupportedUndoOp with op='redo', got: {err:?}"
    );
}
