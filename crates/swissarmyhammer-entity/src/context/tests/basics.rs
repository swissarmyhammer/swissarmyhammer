//! The shape of the store on disk.
//!
//! The entity directory and path, the round trip of a plain entity and of
//! one that carries a body, listing, delete to trash, and the migration
//! that moves an old trash layout.

use super::*;

#[tokio::test]
async fn entity_dir_pluralizes() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    assert_eq!(ctx.entity_dir("task"), dir.path().join("tasks"));
    assert_eq!(ctx.entity_dir("tag"), dir.path().join("tags"));
    assert_eq!(ctx.entity_dir("board"), dir.path().join("boards"));
}

#[tokio::test]
async fn entity_path_uses_correct_extension() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    // task has body_field → .md
    let p = ctx.entity_path("task", "01ABC").unwrap();
    assert_eq!(p, dir.path().join("tasks").join("01ABC.md"));

    // tag has no body_field → .yaml
    let p = ctx.entity_path("tag", "bug").unwrap();
    assert_eq!(p, dir.path().join("tags").join("bug.yaml"));
}

#[tokio::test]
async fn unknown_entity_type_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    assert!(ctx.entity_path("unicorn", "x").is_err());
    assert!(ctx.read("unicorn", "x").await.is_err());
}

#[tokio::test]
async fn round_trip_plain_yaml() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    tag.set("color", json!("#ff0000"));

    ctx.write(&tag).await.unwrap();

    let loaded = ctx.read("tag", "bug").await.unwrap();
    assert_eq!(loaded.get_str("tag_name"), Some("Bug"));
    assert_eq!(loaded.get_str("color"), Some("#ff0000"));
}

#[tokio::test]
async fn round_trip_with_body() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let mut task = Entity::new("task", "01ABC");
    task.set("title", json!("Fix bug"));
    task.set("body", json!("Details here.\n\n- [ ] Step 1"));

    ctx.write(&task).await.unwrap();

    let loaded = ctx.read("task", "01ABC").await.unwrap();
    assert_eq!(loaded.get_str("title"), Some("Fix bug"));
    assert!(loaded.get_str("body").unwrap().contains("Step 1"));
}

#[tokio::test]
async fn list_entities() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let mut t1 = Entity::new("tag", "bug");
    t1.set("tag_name", json!("Bug"));
    let mut t2 = Entity::new("tag", "feature");
    t2.set("tag_name", json!("Feature"));

    ctx.write(&t1).await.unwrap();
    ctx.write(&t2).await.unwrap();

    let tags = ctx.list("tag").await.unwrap();
    assert_eq!(tags.len(), 2);
}

#[tokio::test]
async fn delete_moves_to_trash() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    ctx.write(&tag).await.unwrap();

    assert!(ctx.read("tag", "bug").await.is_ok());
    ctx.delete("tag", "bug").await.unwrap();

    // No longer readable from live storage
    assert!(ctx.read("tag", "bug").await.is_err());

    // Files moved to trash (new layout: {type}s/.trash/)
    let trash_dir = dir.path().join("tags").join(".trash");
    assert!(trash_dir.join("bug.yaml").exists());
}

#[tokio::test]
async fn trash_dir_correct() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    // New layout: {root}/{type}s/.trash/
    assert_eq!(ctx.trash_dir("tag"), dir.path().join("tags").join(".trash"));
    assert_eq!(
        ctx.trash_dir("task"),
        dir.path().join("tasks").join(".trash")
    );
}

#[tokio::test]
async fn migration_moves_old_trash() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    // Simulate old-style trash layout: {root}/.trash/{type}s/
    let old_trash = dir.path().join(".trash").join("tags");
    tokio::fs::create_dir_all(&old_trash).await.unwrap();
    tokio::fs::write(old_trash.join("bug.yaml"), "tag_name: Bug\n")
        .await
        .unwrap();
    tokio::fs::write(old_trash.join("bug.jsonl"), "{}\n")
        .await
        .unwrap();

    // Run migration
    ctx.migrate_trash_layout("tag").await.unwrap();

    // Files should now be in the new location: {type}s/.trash/
    let new_trash = dir.path().join("tags").join(".trash");
    assert!(new_trash.join("bug.yaml").exists());
    assert!(new_trash.join("bug.jsonl").exists());

    // Old location should be gone
    assert!(!old_trash.exists());
    // Old root .trash/ should also be gone
    assert!(!dir.path().join(".trash").exists());
}
