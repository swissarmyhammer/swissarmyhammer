//! An attachment field through write, read, update and delete.

use super::*;

// --- Attachment tests ---

#[tokio::test]
async fn write_attachment_copies_file_and_stores_filename() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    // Create a source file to attach
    let source = dir.path().join("photo.jpg");
    tokio::fs::write(&source, b"fake image data").await.unwrap();

    let mut entity = Entity::new("item", "01TEST");
    entity.set("title", json!("Test Item"));
    entity.set("avatar", json!(source.to_string_lossy().to_string()));

    ctx.write(&entity).await.unwrap();

    // Read raw (without compute) to check stored filename
    let def = ctx.entity_def("item").unwrap();
    let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01TEST", def);
    let raw = crate::io::read_entity(&path, "item", "01TEST", def)
        .await
        .unwrap();

    let stored = raw.fields.get("avatar").unwrap().as_str().unwrap();
    assert!(
        stored.contains("photo.jpg"),
        "stored name should contain original filename"
    );
    assert!(
        stored.len() > "photo.jpg".len(),
        "stored name should have ULID prefix"
    );

    // Verify the file was copied to .attachments/
    let att_dir = dir.path().join("items").join(".attachments");
    assert!(att_dir.join(stored).exists());

    // Verify contents match
    let copied = tokio::fs::read(att_dir.join(stored)).await.unwrap();
    assert_eq!(copied, b"fake image data");
}

#[tokio::test]
async fn write_existing_attachment_filename_leaves_file_untouched() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    // Create a source file and write it as an attachment
    let source = dir.path().join("photo.jpg");
    tokio::fs::write(&source, b"original data").await.unwrap();

    let mut entity = Entity::new("item", "01TEST");
    entity.set("title", json!("Test"));
    entity.set("avatar", json!(source.to_string_lossy().to_string()));
    ctx.write(&entity).await.unwrap();

    // Get the stored filename
    let def = ctx.entity_def("item").unwrap();
    let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01TEST", def);
    let raw = crate::io::read_entity(&path, "item", "01TEST", def)
        .await
        .unwrap();
    let stored = raw
        .fields
        .get("avatar")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    // Write again with the stored filename (not a source path)
    let mut entity2 = Entity::new("item", "01TEST");
    entity2.set("title", json!("Updated Title"));
    entity2.set("avatar", json!(stored.clone()));
    ctx.write(&entity2).await.unwrap();

    // Verify the file still exists and contents unchanged
    let att_dir = dir.path().join("items").join(".attachments");
    let contents = tokio::fs::read(att_dir.join(&stored)).await.unwrap();
    assert_eq!(contents, b"original data");
}

#[tokio::test]
async fn read_attachment_returns_metadata_with_path() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    let source = dir.path().join("photo.png");
    tokio::fs::write(&source, b"png data here").await.unwrap();

    let mut entity = Entity::new("item", "01TEST");
    entity.set("title", json!("Test"));
    entity.set("avatar", json!(source.to_string_lossy().to_string()));
    ctx.write(&entity).await.unwrap();

    // Read with compute (should enrich attachment fields)
    let read = ctx.read("item", "01TEST").await.unwrap();
    let meta = read.fields.get("avatar").unwrap();

    assert!(
        meta.is_object(),
        "attachment field should be a metadata object"
    );
    assert_eq!(meta["name"], "photo.png");
    assert_eq!(meta["mime_type"], "image/png");
    assert_eq!(meta["size"], 13); // b"png data here".len()
    assert!(meta["id"].is_string());
    assert!(meta["path"].as_str().unwrap().contains(".attachments"));

    // Verify the path is readable and content matches
    let resolved_path = meta["path"].as_str().unwrap();
    let contents = tokio::fs::read(resolved_path).await.unwrap();
    assert_eq!(contents, b"png data here");
}

#[tokio::test]
async fn write_attachment_exceeding_max_bytes_errors() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    // max_bytes is 1MB; create a file slightly over
    let source = dir.path().join("huge.bin");
    let data = vec![0u8; 1_048_577]; // 1MB + 1
    tokio::fs::write(&source, &data).await.unwrap();

    let mut entity = Entity::new("item", "01TEST");
    entity.set("title", json!("Test"));
    entity.set("avatar", json!(source.to_string_lossy().to_string()));

    let result = ctx.write(&entity).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("too large"),
        "error should mention file too large: {}",
        err
    );
}

#[tokio::test]
async fn update_removing_attachment_trashes_file() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    let source = dir.path().join("photo.jpg");
    tokio::fs::write(&source, b"image data").await.unwrap();

    let mut entity = Entity::new("item", "01TEST");
    entity.set("title", json!("Test"));
    entity.set("avatar", json!(source.to_string_lossy().to_string()));
    ctx.write(&entity).await.unwrap();

    // Get the stored filename
    let def = ctx.entity_def("item").unwrap();
    let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01TEST", def);
    let raw = crate::io::read_entity(&path, "item", "01TEST", def)
        .await
        .unwrap();
    let stored = raw
        .fields
        .get("avatar")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    // Update entity removing the avatar field
    let mut entity2 = Entity::new("item", "01TEST");
    entity2.set("title", json!("No Avatar"));
    // avatar field is absent → attachment should be trashed
    ctx.write(&entity2).await.unwrap();

    // Verify file moved to .trash
    let att_dir = dir.path().join("items").join(".attachments");
    assert!(
        !att_dir.join(&stored).exists(),
        "attachment should be removed from .attachments/"
    );
    let trash_dir = att_dir.join(".trash");
    assert!(
        trash_dir.join(&stored).exists(),
        "attachment should be in .attachments/.trash/"
    );
}

#[tokio::test]
async fn delete_entity_trashes_attachment_files() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    let source = dir.path().join("doc.pdf");
    tokio::fs::write(&source, b"pdf content").await.unwrap();

    let mut entity = Entity::new("item", "01TEST");
    entity.set("title", json!("Test"));
    entity.set("avatar", json!(source.to_string_lossy().to_string()));
    ctx.write(&entity).await.unwrap();

    // Get stored filename
    let def = ctx.entity_def("item").unwrap();
    let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01TEST", def);
    let raw = crate::io::read_entity(&path, "item", "01TEST", def)
        .await
        .unwrap();
    let stored = raw
        .fields
        .get("avatar")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    // Delete the entity
    ctx.delete("item", "01TEST").await.unwrap();

    // Attachment file should be in .attachments/.trash/
    let att_dir = dir.path().join("items").join(".attachments");
    assert!(!att_dir.join(&stored).exists());
    assert!(att_dir.join(".trash").join(&stored).exists());
}

#[tokio::test]
async fn multiple_attachments_add_read_remove() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    // Create two source files
    let src1 = dir.path().join("file1.txt");
    let src2 = dir.path().join("file2.txt");
    tokio::fs::write(&src1, b"content one").await.unwrap();
    tokio::fs::write(&src2, b"content two").await.unwrap();

    // Write entity with two attachments in the `files` (multiple) field
    let mut entity = Entity::new("item", "01MULTI");
    entity.set("title", json!("Multi"));
    entity.set(
        "files",
        json!([
            src1.to_string_lossy().to_string(),
            src2.to_string_lossy().to_string()
        ]),
    );
    ctx.write(&entity).await.unwrap();

    // Read raw to get stored filenames
    let def = ctx.entity_def("item").unwrap();
    let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01MULTI", def);
    let raw = crate::io::read_entity(&path, "item", "01MULTI", def)
        .await
        .unwrap();
    let stored_arr = raw.fields.get("files").unwrap().as_array().unwrap();
    assert_eq!(stored_arr.len(), 2);
    let stored1 = stored_arr[0].as_str().unwrap().to_string();
    let stored2 = stored_arr[1].as_str().unwrap().to_string();

    // Read with compute — should get metadata array
    let read = ctx.read("item", "01MULTI").await.unwrap();
    let meta_arr = read.fields.get("files").unwrap().as_array().unwrap();
    assert_eq!(meta_arr.len(), 2);
    assert_eq!(meta_arr[0]["name"], "file1.txt");
    assert_eq!(meta_arr[1]["name"], "file2.txt");

    // Update removing one attachment (keep stored2, drop stored1)
    let mut entity2 = Entity::new("item", "01MULTI");
    entity2.set("title", json!("Multi"));
    entity2.set("files", json!([stored2.clone()]));
    ctx.write(&entity2).await.unwrap();

    // stored1 should be trashed, stored2 should remain
    let att_dir = dir.path().join("items").join(".attachments");
    assert!(!att_dir.join(&stored1).exists());
    assert!(att_dir.join(".trash").join(&stored1).exists());
    assert!(att_dir.join(&stored2).exists());
}

#[tokio::test]
async fn write_attachment_source_not_found_errors() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    let mut entity = Entity::new("item", "01TEST");
    entity.set("title", json!("Test"));
    entity.set("avatar", json!("/nonexistent/path/photo.jpg"));

    let result = ctx.write(&entity).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("not found"),
        "error should mention source not found: {}",
        err
    );
}

#[tokio::test]
async fn write_enriched_attachment_object_preserves_file() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    // Create and write an entity with an attachment
    let source = dir.path().join("photo.png");
    tokio::fs::write(&source, b"png data").await.unwrap();

    let mut entity = Entity::new("item", "01ENRICH");
    entity.set("title", json!("Test"));
    entity.set("avatar", json!(source.to_string_lossy().to_string()));
    ctx.write(&entity).await.unwrap();

    // Read — avatar is now an enriched metadata object
    let read = ctx.read("item", "01ENRICH").await.unwrap();
    let meta = read.fields.get("avatar").unwrap().clone();
    assert!(meta.is_object(), "should be enriched");

    // Write back unchanged — the enriched object should round-trip
    let mut entity2 = Entity::new("item", "01ENRICH");
    entity2.set("title", json!("Updated Title"));
    entity2.set("avatar", meta);
    ctx.write(&entity2).await.unwrap();

    // Verify the attachment file still exists and data is intact
    let att_dir = dir.path().join("items").join(".attachments");
    let entries: Vec<_> = std::fs::read_dir(&att_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| !e.file_name().to_str().unwrap_or("").starts_with('.'))
        .collect();
    assert_eq!(entries.len(), 1, "attachment file should still exist");
    let contents = tokio::fs::read(entries[0].path()).await.unwrap();
    assert_eq!(contents, b"png data");
}

#[tokio::test]
async fn write_enriched_objects_mixed_with_new_paths() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    // Create and attach first file
    let src1 = dir.path().join("file1.txt");
    tokio::fs::write(&src1, b"content one").await.unwrap();

    let mut entity = Entity::new("item", "01MIX");
    entity.set("title", json!("Mixed"));
    entity.set("files", json!([src1.to_string_lossy().to_string()]));
    ctx.write(&entity).await.unwrap();

    // Read to get enriched metadata
    let read = ctx.read("item", "01MIX").await.unwrap();
    let enriched_arr = read
        .fields
        .get("files")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(enriched_arr.len(), 1);

    // Create a second source file to append
    let src2 = dir.path().join("file2.txt");
    tokio::fs::write(&src2, b"content two").await.unwrap();

    // Write back with mixed array: enriched object + new source path
    let mut entity2 = Entity::new("item", "01MIX");
    entity2.set("title", json!("Mixed"));
    entity2.set(
        "files",
        json!([enriched_arr[0], src2.to_string_lossy().to_string()]),
    );
    ctx.write(&entity2).await.unwrap();

    // Read again — should have two attachments
    let read2 = ctx.read("item", "01MIX").await.unwrap();
    let files = read2.fields.get("files").unwrap().as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0]["name"], "file1.txt");
    assert_eq!(files[1]["name"], "file2.txt");
}

#[tokio::test]
async fn write_mixed_enriched_stored_and_source_paths() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    // Create two source files and write them
    let src1 = dir.path().join("a.txt");
    let src2 = dir.path().join("b.txt");
    tokio::fs::write(&src1, b"aaa").await.unwrap();
    tokio::fs::write(&src2, b"bbb").await.unwrap();

    let mut entity = Entity::new("item", "01ALL3");
    entity.set("title", json!("Three Shapes"));
    entity.set(
        "files",
        json!([
            src1.to_string_lossy().to_string(),
            src2.to_string_lossy().to_string()
        ]),
    );
    ctx.write(&entity).await.unwrap();

    // Read to get enriched metadata and raw stored filenames
    let read = ctx.read("item", "01ALL3").await.unwrap();
    let enriched = read.fields.get("files").unwrap().as_array().unwrap();
    let enriched_obj = enriched[0].clone(); // enriched metadata object

    let def = ctx.entity_def("item").unwrap();
    let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01ALL3", def);
    let raw = crate::io::read_entity(&path, "item", "01ALL3", def)
        .await
        .unwrap();
    let stored_filename = raw.fields.get("files").unwrap().as_array().unwrap()[1]
        .as_str()
        .unwrap()
        .to_string(); // raw stored filename string

    // Create a third file to add as source path
    let src3 = dir.path().join("c.txt");
    tokio::fs::write(&src3, b"ccc").await.unwrap();

    // Write with all three shapes: enriched object, stored filename, source path
    let mut entity2 = Entity::new("item", "01ALL3");
    entity2.set("title", json!("Three Shapes"));
    entity2.set(
        "files",
        json!([
            enriched_obj,
            stored_filename,
            src3.to_string_lossy().to_string()
        ]),
    );
    ctx.write(&entity2).await.unwrap();

    // Read — should have three attachments
    let read2 = ctx.read("item", "01ALL3").await.unwrap();
    let files = read2.fields.get("files").unwrap().as_array().unwrap();
    assert_eq!(files.len(), 3);
    assert_eq!(files[0]["name"], "a.txt");
    assert_eq!(files[1]["name"], "b.txt");
    assert_eq!(files[2]["name"], "c.txt");
}
