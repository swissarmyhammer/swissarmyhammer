//! `enrich_attachment_fields` and `resolve_attachment_value` on their own.

use super::*;

// -----------------------------------------------------------------------
// enrich_attachment_fields — targeted tests
// -----------------------------------------------------------------------

#[tokio::test]
async fn enrich_attachment_fields_populates_full_metadata() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    // Write two source files as a multiple attachment
    let src1 = dir.path().join("image.png");
    let src2 = dir.path().join("readme.md");
    tokio::fs::write(&src1, b"PNG data bytes").await.unwrap();
    tokio::fs::write(&src2, b"# Hello").await.unwrap();

    let mut entity = Entity::new("item", "01ENRICH");
    entity.set("title", json!("Enrich Test"));
    entity.set(
        "files",
        json!([
            src1.to_string_lossy().to_string(),
            src2.to_string_lossy().to_string()
        ]),
    );
    ctx.write(&entity).await.unwrap();

    // Read — enrichment happens via apply_compute → enrich_attachment_fields
    let read = ctx.read("item", "01ENRICH").await.unwrap();
    let files = read.fields.get("files").unwrap().as_array().unwrap();
    assert_eq!(files.len(), 2);

    // Verify each enriched object has the required shape
    for meta in files {
        assert!(meta["id"].is_string(), "should have id");
        assert!(meta["name"].is_string(), "should have name");
        assert!(meta["size"].is_number(), "should have size");
        assert!(meta["mime_type"].is_string(), "should have mime_type");
        assert!(meta["path"].is_string(), "should have path");
    }

    assert_eq!(files[0]["name"], "image.png");
    assert_eq!(files[0]["size"], 14); // b"PNG data bytes".len()
    assert_eq!(files[0]["mime_type"], "image/png");

    assert_eq!(files[1]["name"], "readme.md");
    assert_eq!(files[1]["size"], 7); // b"# Hello".len()
}

#[tokio::test]
async fn enrich_attachment_fields_missing_file_silently_drops() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    // Write one real attachment first
    let src = dir.path().join("real.txt");
    tokio::fs::write(&src, b"exists").await.unwrap();

    let mut entity = Entity::new("item", "01MISS");
    entity.set("title", json!("Missing File Test"));
    entity.set("files", json!([src.to_string_lossy().to_string()]));
    ctx.write(&entity).await.unwrap();

    // Get the stored filename, then manually add a bogus one to the YAML
    let def = ctx.entity_def("item").unwrap();
    let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01MISS", def);
    let raw = crate::io::read_entity(&path, "item", "01MISS", def)
        .await
        .unwrap();
    let stored = raw.fields.get("files").unwrap().as_array().unwrap()[0]
        .as_str()
        .unwrap()
        .to_string();

    // Rewrite entity with the real stored filename plus a nonexistent one
    let mut entity2 = Entity::new("item", "01MISS");
    entity2.set("title", json!("Missing File Test"));
    entity2.set("files", json!([stored.clone(), "01BOGUS-nonexistent.txt"]));
    // Write raw to disk (bypass resolve so the bogus name stays as-is)
    crate::io::write_entity(&path, &entity2, def).await.unwrap();

    // Read with enrichment — missing file should be silently dropped
    let read = ctx.read("item", "01MISS").await.unwrap();
    let files = read.fields.get("files").unwrap().as_array().unwrap();

    // Only the real file should appear (missing one silently skipped)
    assert_eq!(
        files.len(),
        1,
        "missing attachment should be silently dropped during enrichment"
    );
    assert_eq!(files[0]["name"], "real.txt");
}

#[tokio::test]
async fn enrich_attachment_fields_empty_array_unchanged() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    let mut entity = Entity::new("item", "01EMPTY");
    entity.set("title", json!("Empty Attachments"));
    entity.set("files", json!([]));
    ctx.write(&entity).await.unwrap();

    let read = ctx.read("item", "01EMPTY").await.unwrap();
    let files = read.fields.get("files").unwrap().as_array().unwrap();
    assert!(
        files.is_empty(),
        "empty attachment array should remain empty after enrichment"
    );
}

#[tokio::test]
async fn resolve_attachment_value_copies_source_to_attachments_dir() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    let source = dir.path().join("document.pdf");
    tokio::fs::write(&source, b"PDF content here")
        .await
        .unwrap();

    let mut entity = Entity::new("item", "01RESOLVE");
    entity.set("title", json!("Resolve Test"));
    entity.set("avatar", json!(source.to_string_lossy().to_string()));
    ctx.write(&entity).await.unwrap();

    // Verify file landed in .attachments/
    let att_dir = dir.path().join("items").join(".attachments");
    let mut entries = tokio::fs::read_dir(&att_dir).await.unwrap();
    let mut found = false;
    while let Some(entry) = entries.next_entry().await.unwrap() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with("-document.pdf") {
            found = true;
            let contents = tokio::fs::read(entry.path()).await.unwrap();
            assert_eq!(contents, b"PDF content here");
        }
    }
    assert!(
        found,
        "source file should be copied to .attachments/ with ULID prefix"
    );
}

#[tokio::test]
async fn resolve_attachment_value_existing_filename_returned_as_is() {
    let dir = TempDir::new().unwrap();
    let fields = attachment_fields_context();
    let ctx = EntityContext::new(dir.path(), fields);

    // First write: copy a source file
    let source = dir.path().join("notes.txt");
    tokio::fs::write(&source, b"my notes").await.unwrap();

    let mut entity = Entity::new("item", "01EXIST");
    entity.set("title", json!("Existing Test"));
    entity.set("avatar", json!(source.to_string_lossy().to_string()));
    ctx.write(&entity).await.unwrap();

    // Get the stored filename from raw YAML
    let def = ctx.entity_def("item").unwrap();
    let path = crate::io::entity_file_path(&ctx.entity_dir("item"), "01EXIST", def);
    let raw = crate::io::read_entity(&path, "item", "01EXIST", def)
        .await
        .unwrap();
    let stored = raw
        .fields
        .get("avatar")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    // Second write: use the stored filename directly
    let mut entity2 = Entity::new("item", "01EXIST");
    entity2.set("title", json!("Existing Test v2"));
    entity2.set("avatar", json!(stored.clone()));
    ctx.write(&entity2).await.unwrap();

    // Verify the stored filename didn't change (no new copy)
    let raw2 = crate::io::read_entity(&path, "item", "01EXIST", def)
        .await
        .unwrap();
    let stored2 = raw2
        .fields
        .get("avatar")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        stored, stored2,
        "stored filename should be returned as-is without creating a new copy"
    );

    // Only one file should exist in .attachments/ (no duplicates)
    let att_dir = dir.path().join("items").join(".attachments");
    let mut count = 0;
    let mut entries = tokio::fs::read_dir(&att_dir).await.unwrap();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with('.') {
            count += 1;
        }
    }
    assert_eq!(count, 1, "should have exactly one attachment file");
}
