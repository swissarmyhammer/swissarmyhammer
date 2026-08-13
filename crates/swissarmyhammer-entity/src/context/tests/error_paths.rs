//! What each operation returns when the input is wrong.
//!
//! An unknown entity type, a missing entity, an empty store, and a
//! changelog read with no writes behind it.

use super::*;

#[tokio::test]
async fn extract_attachment_filenames_edge_cases() {
    // None value returns empty
    let empty: Vec<String> = EntityContext::extract_attachment_filenames(None, false);
    assert!(empty.is_empty());

    let empty_multi: Vec<String> = EntityContext::extract_attachment_filenames(None, true);
    assert!(empty_multi.is_empty());

    // Non-string single value returns empty
    let num = json!(42);
    let result = EntityContext::extract_attachment_filenames(Some(&num), false);
    assert!(result.is_empty());

    // Non-string/non-array multiple value returns empty
    let result_multi = EntityContext::extract_attachment_filenames(Some(&num), true);
    assert!(result_multi.is_empty());

    // String value for multiple returns single-element vec
    let s = json!("filename.txt");
    let result = EntityContext::extract_attachment_filenames(Some(&s), true);
    assert_eq!(result, vec!["filename.txt".to_string()]);

    // Array with mixed types filters non-strings
    let arr = json!(["file1.txt", 42, "file2.txt"]);
    let result = EntityContext::extract_attachment_filenames(Some(&arr), true);
    assert_eq!(
        result,
        vec!["file1.txt".to_string(), "file2.txt".to_string()]
    );
}

#[tokio::test]
async fn migrate_trash_no_op_when_old_layout_absent() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    // No old-style trash exists; migration should be a no-op
    ctx.migrate_trash_layout("tag").await.unwrap();
    // Nothing should be created
    assert!(!dir.path().join("tags").join(".trash").exists());
}

#[tokio::test]
async fn entity_def_returns_correct_definition() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let def = ctx.entity_def("tag").unwrap();
    assert_eq!(def.name, "tag");
    assert!(def.body_field.is_none());

    let def = ctx.entity_def("task").unwrap();
    assert_eq!(def.name, "task");
    assert_eq!(def.body_field.as_deref(), Some("body"));
}

#[tokio::test]
async fn entity_def_unknown_type_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.entity_def("nonexistent");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("unknown entity type"));
}

#[tokio::test]
async fn read_changelog_empty_when_no_writes() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let log = ctx.read_changelog("tag", "nonexistent").await.unwrap();
    assert!(log.is_empty());
}

#[tokio::test]
async fn read_changelog_unknown_entity_type_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.read_changelog("unicorn", "x").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn read_changelog_after_three_writes_yields_three_field_diffs() {
    // Integration check for the replay reader: three successive writes
    // through a registered `EntityTypeStore` produce three store-format
    // changelog records on disk. `read_changelog` must project those into
    // `ChangeEntry`s — the create, then two updates — each carrying a
    // field-level diff on the `title` field.
    //
    // Note: until the next card (`01KQ5FJ0VXEQZVKHZBN49Q5GFS`) removes
    // the entity-layer dual-writer, every `EntityContext::write` also
    // appends a legacy entity-format `ChangeEntry`. The reader returns
    // the union (store-projected + legacy) merged by timestamp, so the
    // observed total is 2 × N. This test still pins the property the
    // card adds — the projection works and surfaces text-level diffs on
    // `title` — by verifying every `update` entry carries one. After the
    // writer-removal card lands, the count tightens to exactly 3.
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    // Wire a store handle so writes are routed through the store layer
    // and emit store-format changelog records.
    let entity_dir = dir.path().join("tasks");
    std::fs::create_dir_all(&entity_dir).unwrap();
    let entity_def = fields.get_entity("task").unwrap();
    let field_defs: Vec<_> = fields
        .fields_for_entity("task")
        .into_iter()
        .cloned()
        .collect();
    let store = crate::store::EntityTypeStore::new(
        &entity_dir,
        "task",
        Arc::new(entity_def.clone()),
        Arc::new(field_defs),
    );
    let handle = Arc::new(swissarmyhammer_store::StoreHandle::new(Arc::new(store)));
    ctx.register_store("task", handle).await;

    // Three writes with three different titles. Body kept constant so the
    // only field-level change between updates is `title`.
    let mut task = Entity::new("task", "01ABC");
    task.set("title", json!("First"));
    task.set("body", json!("constant body"));
    ctx.write(&task).await.unwrap();

    task.set("title", json!("Second"));
    ctx.write(&task).await.unwrap();

    task.set("title", json!("Third"));
    ctx.write(&task).await.unwrap();

    let log = ctx.read_changelog("task", "01ABC").await.unwrap();

    // Every write contributes at least one entry; with the dual-writer
    // still active, every write contributes two. Either way there are at
    // least 3 (the post-projection store-derived entries), one create
    // followed by updates.
    assert!(
        log.len() >= 3,
        "expected at least 3 changelog entries, got {}",
        log.len()
    );

    // Each create entry — at least one — has every field surfaced as Set
    // against the empty before-state.
    let creates: Vec<_> = log.iter().filter(|e| e.op == "create").collect();
    assert!(!creates.is_empty(), "expected at least one create entry");
    for create in &creates {
        let keys: Vec<&str> = create.changes.iter().map(|(k, _)| k.as_str()).collect();
        assert!(
            keys.contains(&"title"),
            "create missing title: {:?}",
            create
        );
        assert!(keys.contains(&"body"), "create missing body: {:?}", create);
    }

    // Each `update` entry must carry a TextDiff on `title`. This is the
    // core property the projection adds: even though the on-disk store
    // record is a text patch, the reader synthesises a field-level diff.
    let updates: Vec<_> = log.iter().filter(|e| e.op == "update").collect();
    assert!(
        !updates.is_empty(),
        "expected at least one update entry, got log: {:?}",
        log
    );
    for entry in &updates {
        let title_change = entry
            .changes
            .iter()
            .find(|(k, _)| k == "title")
            .map(|(_, c)| c)
            .unwrap_or_else(|| panic!("update entry missing `title` change: {:?}", entry));
        assert!(
            matches!(title_change, FieldChange::TextDiff { .. }),
            "expected TextDiff on title, got {:?}",
            title_change
        );
    }
}

#[tokio::test]
async fn changelog_path_unknown_entity_type_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.changelog_path("unicorn", "x");
    assert!(result.is_err());
}

#[tokio::test]
async fn entity_path_unknown_entity_type_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.entity_path("unicorn", "x");
    assert!(result.is_err());
}

#[tokio::test]
async fn list_empty_entity_type() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.list("tag").await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn list_unknown_entity_type_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.list("unicorn").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn list_archived_empty() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.list_archived("tag").await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn list_archived_unknown_entity_type_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.list_archived("unicorn").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn read_archived_unknown_entity_type_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.read_archived("unicorn", "x").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn read_archived_not_found_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.read_archived("tag", "nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn delete_unknown_entity_type_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.delete("unicorn", "x").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn archive_unknown_entity_type_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.archive("unicorn", "x").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn unarchive_unknown_entity_type_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.unarchive("unicorn", "x").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn restore_from_trash_unknown_entity_type_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.restore_from_trash("unicorn", "x").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn restore_from_archive_unknown_entity_type_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.restore_from_archive("unicorn", "x").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn migration_handles_already_existing_dest() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    // Create old-style trash with a file
    let old_trash = dir.path().join(".trash").join("tags");
    tokio::fs::create_dir_all(&old_trash).await.unwrap();
    tokio::fs::write(old_trash.join("dup.yaml"), "tag_name: Dup\n")
        .await
        .unwrap();

    // Also create new-style trash with the same filename already present
    let new_trash = dir.path().join("tags").join(".trash");
    tokio::fs::create_dir_all(&new_trash).await.unwrap();
    tokio::fs::write(new_trash.join("dup.yaml"), "tag_name: Existing\n")
        .await
        .unwrap();

    // Migration should handle the AlreadyExists case gracefully
    ctx.migrate_trash_layout("tag").await.unwrap();

    // The new trash file should still exist (migration skips on AlreadyExists)
    assert!(new_trash.join("dup.yaml").exists());
}

#[tokio::test]
async fn write_task_with_body_round_trips_through_context() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let mut task = Entity::new("task", "01TEST");
    task.set("title", json!("Test Task"));
    task.set(
        "body",
        json!("# Heading\n\nParagraph text.\n\n- Item 1\n- Item 2"),
    );
    ctx.write(&task).await.unwrap();

    let loaded = ctx.read("task", "01TEST").await.unwrap();
    assert_eq!(loaded.get_str("title"), Some("Test Task"));
    assert!(loaded.get_str("body").unwrap().contains("# Heading"));
}

#[tokio::test]
async fn delete_nonexistent_entity_does_not_error() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    // Deleting an entity that doesn't exist should succeed (moves to trash, nothing found)
    let result = ctx.delete("tag", "nonexistent").await;
    // It succeeds but returns None (no changelog entry since entity had no fields)
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn archive_nonexistent_entity_succeeds_with_none() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    // Archiving entity that doesn't exist should succeed with None
    let result = ctx.archive("tag", "nonexistent").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn read_changelog_with_trash_fallback_unknown_entity_type_errors() {
    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = EntityContext::new(dir.path(), fields.clone());

    let result = ctx.read_changelog_with_trash_fallback("unicorn", "x").await;
    assert!(result.is_err());
}
