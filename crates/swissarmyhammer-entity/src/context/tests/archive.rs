//! Archive, unarchive, and reading an archived entity.

use super::*;

// ===========================================================================
// Additional tests from main
// ===========================================================================

#[tokio::test]
async fn archive_dir_correct() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    assert_eq!(
        ctx.archive_dir("tag"),
        dir.path().join("tags").join(".archive")
    );
    assert_eq!(
        ctx.archive_dir("task"),
        dir.path().join("tasks").join(".archive")
    );
}

#[tokio::test]
async fn list_archived_returns_archived_only() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    // Create two tags
    let mut t1 = Entity::new("tag", "bug");
    t1.set("tag_name", json!("Bug"));
    let mut t2 = Entity::new("tag", "feature");
    t2.set("tag_name", json!("Feature"));

    ctx.write(&t1).await.unwrap();
    ctx.write(&t2).await.unwrap();

    // Archive only "bug"
    ctx.archive("tag", "bug").await.unwrap();

    // list() should only return "feature"
    let live = ctx.list("tag").await.unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].id, "feature");

    // list_archived() should only return "bug"
    let archived = ctx.list_archived("tag").await.unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].id, "bug");
}

#[tokio::test]
async fn read_archived_returns_entity() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    tag.set("color", json!("#ff0000"));
    ctx.write(&tag).await.unwrap();

    ctx.archive("tag", "bug").await.unwrap();

    // read() on archived entity should fail
    assert!(ctx.read("tag", "bug").await.is_err());

    // read_archived() should succeed
    let archived = ctx.read_archived("tag", "bug").await.unwrap();
    assert_eq!(archived.get_str("tag_name"), Some("Bug"));
    assert_eq!(archived.get_str("color"), Some("#ff0000"));
}

#[tokio::test]
async fn archive_writes_changelog() {
    let dir = TempDir::new().unwrap();
    let ctx = ctx_with_tag_store(&dir).await;

    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    ctx.write(&tag).await.unwrap();

    ctx.archive("tag", "bug").await.unwrap();

    // Archive is recorded as a store-format `ChangelogEntry` under
    // `{type}s/.archive/` with a versioned filename. The serialized
    // record contains `"op":"archive"` (lowercase, per the store layer's
    // `#[serde(rename_all = "lowercase")]` on `ChangeOp`).
    let archive_dir = dir.path().join("tags").join(".archive");
    let archived_logs: Vec<_> = std::fs::read_dir(&archive_dir)
        .expect("archive dir must exist")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
        .map(|e| e.path())
        .collect();
    assert!(
        !archived_logs.is_empty(),
        "at least one archived changelog must exist"
    );
    let has_archive_op = archived_logs.iter().any(|p| {
        std::fs::read_to_string(p)
            .map(|s| s.contains("\"op\":\"archive\""))
            .unwrap_or(false)
    });
    assert!(has_archive_op, "archive changelog must contain archive op");
}

#[tokio::test]
async fn unarchive_writes_changelog() {
    let dir = TempDir::new().unwrap();
    let ctx = ctx_with_tag_store(&dir).await;

    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    ctx.write(&tag).await.unwrap();

    ctx.archive("tag", "bug").await.unwrap();
    ctx.unarchive("tag", "bug").await.unwrap();

    // The store layer restores the live `.jsonl` and appends an
    // `unarchive` store-format record. The projecting reader is
    // exercised separately; here we just verify the on-disk shape.
    let live_log = dir.path().join("tags").join("bug.jsonl");
    let content = tokio::fs::read_to_string(&live_log).await.unwrap();
    assert!(
        content.contains("\"op\":\"unarchive\""),
        "live changelog must contain an unarchive store-format record"
    );
}

#[tokio::test]
async fn root_and_fields_accessors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    assert_eq!(ctx.root(), dir.path());
    // fields() should return the same FieldsContext
    assert!(ctx.fields().get_entity("tag").is_some());
    assert!(ctx.fields().get_entity("task").is_some());
}

#[tokio::test]
async fn list_archived_with_compute_engine() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let compute = Arc::new(swissarmyhammer_fields::ComputeEngine::new());
    let ctx = EntityContext::new(dir.path(), fields.clone()).with_compute(compute);

    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    ctx.write(&tag).await.unwrap();
    ctx.archive("tag", "bug").await.unwrap();

    // list_archived with compute engine
    let archived = ctx.list_archived("tag").await.unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].get_str("tag_name"), Some("Bug"));
}
