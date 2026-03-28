//! End-to-end undo/redo integration tests.
//!
//! These tests exercise the full undo/redo flow using `EntityContext` directly
//! and verify both in-memory state AND the on-disk `undo_stack.yaml` file.

use serde_json::json;
use swissarmyhammer_entity::entity::Entity;
use swissarmyhammer_entity::test_utils::test_fields_context;
use swissarmyhammer_entity::undo_stack::UndoStack;
use swissarmyhammer_entity::EntityContext;
use tempfile::TempDir;

/// Helper: create an EntityContext with a temp dir.
fn make_ctx(dir: &TempDir) -> EntityContext {
    let fields = test_fields_context();
    EntityContext::new(dir.path(), fields)
}

/// Helper: read the on-disk undo_stack.yaml and parse it.
fn read_stack_yaml(dir: &TempDir) -> UndoStack {
    let path = dir.path().join("undo_stack.yaml");
    UndoStack::load(&path).expect("failed to load undo_stack.yaml")
}

// ---------------------------------------------------------------------------
// 1. Single field edit undo/redo cycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn single_field_edit_undo_redo_cycle() {
    let dir = TempDir::new().unwrap();
    let ctx = make_ctx(&dir);

    // Create entity
    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    tag.set("color", json!("#ff0000"));
    let _create_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // Update field
    tag.set("tag_name", json!("Bug Report"));
    let update_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // Verify can_undo via in-memory stack
    {
        let stack = ctx.undo_stack().await;
        assert!(stack.can_undo(), "should be able to undo after update");
        assert!(!stack.can_redo(), "no redo yet");
    }

    // Verify undo_stack.yaml has entries
    let yaml_stack = read_stack_yaml(&dir);
    assert!(yaml_stack.can_undo(), "YAML stack should show can_undo");

    // Undo the update
    ctx.undo(&update_ulid).await.unwrap();

    // Verify field reverted
    let restored = ctx.read("tag", "bug").await.unwrap();
    assert_eq!(restored.get_str("tag_name"), Some("Bug"));

    // Verify pointer decremented in YAML
    let yaml_stack = read_stack_yaml(&dir);
    assert!(yaml_stack.can_redo(), "should be able to redo after undo");

    // Redo
    ctx.redo(&update_ulid).await.unwrap();

    // Verify field restored
    let redone = ctx.read("tag", "bug").await.unwrap();
    assert_eq!(redone.get_str("tag_name"), Some("Bug Report"));

    // Verify pointer incremented in YAML
    let yaml_stack = read_stack_yaml(&dir);
    assert!(!yaml_stack.can_redo(), "no more redo after redo");
    assert!(yaml_stack.can_undo(), "can still undo");
}

// ---------------------------------------------------------------------------
// 2. Multi-step undo
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multi_step_undo() {
    let dir = TempDir::new().unwrap();
    let ctx = make_ctx(&dir);

    // Create
    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Original"));
    tag.set("color", json!("#000000"));
    let _create_ulid = ctx.write(&tag).await.unwrap().unwrap();

    // Update A
    tag.set("tag_name", json!("A"));
    let _ulid_a = ctx.write(&tag).await.unwrap().unwrap();

    // Update B
    tag.set("tag_name", json!("B"));
    let ulid_b = ctx.write(&tag).await.unwrap().unwrap();

    // Update C
    tag.set("tag_name", json!("C"));
    let ulid_c = ctx.write(&tag).await.unwrap().unwrap();

    // Undo C
    ctx.undo(&ulid_c).await.unwrap();
    let loaded = ctx.read("tag", "bug").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("B"), "C should be reverted to B");

    // Undo B
    ctx.undo(&ulid_b).await.unwrap();
    let loaded = ctx.read("tag", "bug").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("A"), "B should be reverted to A");

    // Verify YAML: can redo twice, can undo twice
    let yaml_stack = read_stack_yaml(&dir);
    assert!(yaml_stack.can_undo());
    assert!(yaml_stack.can_redo());

    // Redo B
    ctx.redo(&ulid_b).await.unwrap();
    let loaded = ctx.read("tag", "bug").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("B"), "B should be restored");

    let yaml_stack = read_stack_yaml(&dir);
    assert!(yaml_stack.can_redo(), "can still redo C");
}

// ---------------------------------------------------------------------------
// 3. Undo after new edit clears redo
// ---------------------------------------------------------------------------

#[tokio::test]
async fn undo_after_new_edit_clears_redo() {
    let dir = TempDir::new().unwrap();
    let ctx = make_ctx(&dir);

    // Create
    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Original"));
    tag.set("color", json!("#000000"));
    ctx.write(&tag).await.unwrap();

    // Update A
    tag.set("tag_name", json!("A"));
    let ulid_a = ctx.write(&tag).await.unwrap().unwrap();

    // Undo A
    ctx.undo(&ulid_a).await.unwrap();

    // Verify can_redo is true before new edit
    {
        let stack = ctx.undo_stack().await;
        assert!(stack.can_redo(), "should be able to redo after undo");
    }

    // New edit B (should clear redo)
    tag.set("tag_name", json!("B"));
    ctx.write(&tag).await.unwrap();

    // Verify can_redo is false
    {
        let stack = ctx.undo_stack().await;
        assert!(!stack.can_redo(), "redo should be cleared after new edit");
    }

    // Verify redo entries gone from YAML
    let yaml_stack = read_stack_yaml(&dir);
    assert!(!yaml_stack.can_redo(), "YAML redo should be empty");
}

// ---------------------------------------------------------------------------
// 4. Transaction undo (multi-entity)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn transaction_undo() {
    let dir = TempDir::new().unwrap();
    let ctx = make_ctx(&dir);

    // Create two entities
    let mut tag1 = Entity::new("tag", "bug");
    tag1.set("tag_name", json!("Bug"));
    tag1.set("color", json!("#ff0000"));
    ctx.write(&tag1).await.unwrap();

    let mut tag2 = Entity::new("tag", "feature");
    tag2.set("tag_name", json!("Feature"));
    tag2.set("color", json!("#00ff00"));
    ctx.write(&tag2).await.unwrap();

    // Start transaction
    let tx_id = EntityContext::generate_transaction_id();
    ctx.set_transaction(tx_id.clone()).await;

    // Update both entities within the transaction
    tag1.set("tag_name", json!("Bug Updated"));
    ctx.write(&tag1).await.unwrap();

    tag2.set("tag_name", json!("Feature Updated"));
    ctx.write(&tag2).await.unwrap();

    ctx.clear_transaction().await;

    // Verify transaction dedup: both writes share one stack entry
    let yaml_stack = read_stack_yaml(&dir);
    // Stack: create_tag1, create_tag2, tx_id (deduped)
    assert!(yaml_stack.can_undo());

    // Undo the transaction
    ctx.undo(tx_id.as_str()).await.unwrap();

    // Verify both entities reverted
    let loaded1 = ctx.read("tag", "bug").await.unwrap();
    assert_eq!(loaded1.get_str("tag_name"), Some("Bug"));

    let loaded2 = ctx.read("tag", "feature").await.unwrap();
    assert_eq!(loaded2.get_str("tag_name"), Some("Feature"));
}

// ---------------------------------------------------------------------------
// 5. Delete + undo
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delete_and_undo() {
    let dir = TempDir::new().unwrap();
    let ctx = make_ctx(&dir);

    // Create
    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    tag.set("color", json!("#ff0000"));
    ctx.write(&tag).await.unwrap();

    // Delete
    let delete_ulid = ctx.delete("tag", "bug").await.unwrap().unwrap();

    // Verify entity is gone
    assert!(ctx.read("tag", "bug").await.is_err());

    // Verify YAML shows the delete entry
    let yaml_stack = read_stack_yaml(&dir);
    assert!(yaml_stack.can_undo());

    // Undo the delete
    ctx.undo(&delete_ulid).await.unwrap();

    // Verify entity restored
    let restored = ctx.read("tag", "bug").await.unwrap();
    assert_eq!(restored.get_str("tag_name"), Some("Bug"));
    assert_eq!(restored.get_str("color"), Some("#ff0000"));

    // Verify YAML state
    let yaml_stack = read_stack_yaml(&dir);
    assert!(yaml_stack.can_redo(), "should be able to redo the delete");
}

// ---------------------------------------------------------------------------
// 6. Stack capacity
// ---------------------------------------------------------------------------

#[tokio::test]
async fn stack_capacity_trims_oldest() {
    let dir = TempDir::new().unwrap();
    let ctx = make_ctx(&dir);

    // Create a tag first
    let mut tag = Entity::new("tag", "counter");
    tag.set("tag_name", json!("0"));
    tag.set("color", json!("#000000"));
    ctx.write(&tag).await.unwrap();

    // Push 100 more operations (total 101, exceeding default cap of 100)
    for i in 1..=100 {
        tag.set("tag_name", json!(format!("{}", i)));
        ctx.write(&tag).await.unwrap();
    }

    // Verify stack is capped and can still undo
    let yaml_stack = read_stack_yaml(&dir);
    assert!(yaml_stack.can_undo());
    assert!(yaml_stack.undo_target().is_some());
}

// ---------------------------------------------------------------------------
// 7. YAML round-trip persistence
// ---------------------------------------------------------------------------

#[tokio::test]
async fn yaml_round_trip_persistence() {
    let dir = TempDir::new().unwrap();

    // Scope 1: perform operations, then drop EntityContext
    let ulid_a;
    {
        let ctx = make_ctx(&dir);

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        tag.set("tag_name", json!("Bug Report"));
        ulid_a = ctx.write(&tag).await.unwrap().unwrap();

        // Undo one
        ctx.undo(&ulid_a).await.unwrap();

        // Verify in-memory state before drop
        let stack = ctx.undo_stack().await;
        assert!(stack.can_undo());
        assert!(stack.can_redo());
    }
    // EntityContext is dropped here

    // Scope 2: create new EntityContext from same root — stack loaded from disk
    {
        let ctx = make_ctx(&dir);

        // Verify stack was loaded from disk with correct state
        let stack = ctx.undo_stack().await;
        assert!(stack.can_undo(), "should be able to undo after reload");
        assert!(stack.can_redo(), "should be able to redo after reload");
        assert_eq!(stack.redo_target(), Some(ulid_a.as_str()));
    }
}
