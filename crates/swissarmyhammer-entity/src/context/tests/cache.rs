//! The entity cache, and the events undo and redo emit.

use super::*;

// =========================================================================
// EntityCache integration tests
// =========================================================================

/// When a cache is attached, `list()` should serve from the in-memory map
/// and not hit `io::read_entity_dir` — beyond the single preload call
/// issued by `EntityCache::load_all`.
///
/// This is asserted behaviorally: after the cache is loaded, we plant a
/// new entity file directly on disk, bypassing the cache. If subsequent
/// `list()` calls returned that file, they must have re-scanned the
/// directory; if they continue to return only the originally-cached
/// entries, the cache successfully short-circuited disk I/O. This
/// avoids depending on a process-global counter, which would race
/// against parallel tests that also call `read_entity_dir`.
#[tokio::test]
async fn test_list_hits_cache_not_disk() {
    use crate::cache::EntityCache;

    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();

    // Seed some entities on disk through a bare context.
    let seed_ctx = EntityContext::new(dir.path(), Arc::clone(&fields));
    for i in 0..5 {
        let mut tag = Entity::new("tag", format!("t{i}"));
        tag.set("tag_name", json!(format!("Tag {i}")));
        seed_ctx.write(&tag).await.unwrap();
    }
    drop(seed_ctx);

    // Build a fresh cache-wired context.
    let ctx = Arc::new(EntityContext::new(dir.path(), Arc::clone(&fields)));
    let cache = Arc::new(EntityCache::new(Arc::clone(&ctx)));
    ctx.attach_cache(&cache);

    cache.load_all("tag").await.unwrap();

    // Sanity: list() sees the 5 preloaded tags.
    assert_eq!(ctx.list("tag").await.unwrap().len(), 5);

    // Plant a 6th tag directly on disk, bypassing the cache. A
    // disk-reading `list()` would observe this file; a cache-served
    // `list()` would not.
    let tag_def = fields.get_entity("tag").expect("tag def must exist");
    let tag_dir = ctx.entity_dir("tag");
    let mut planted = Entity::new("tag", "t-planted");
    planted.set("tag_name", json!("Planted"));
    let path = crate::io::entity_file_path(&tag_dir, planted.id.as_str(), tag_def);
    crate::io::write_entity(&path, &planted, tag_def)
        .await
        .unwrap();

    // 100 list calls must serve from cache — they must not see the
    // planted file, because the cache short-circuits disk reads.
    for _ in 0..100 {
        let tags = ctx.list("tag").await.unwrap();
        assert_eq!(
            tags.len(),
            5,
            "list() after load_all must serve from cache and not re-scan disk"
        );
        assert!(
            tags.iter().all(|t| t.id.as_str() != "t-planted"),
            "list() must not observe the disk-planted entity"
        );
    }
}

/// When a cache is attached, `EntityContext::write` delegates to
/// `EntityCache::write`, which emits an `EntityChanged` event with the
/// field-level diff (the sub-card 1 shape).
#[tokio::test]
async fn test_write_goes_through_cache_when_attached() {
    use crate::cache::EntityCache;
    use crate::events::EntityEvent;

    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();

    let ctx = Arc::new(EntityContext::new(dir.path(), Arc::clone(&fields)));
    let cache = Arc::new(EntityCache::new(Arc::clone(&ctx)));
    ctx.attach_cache(&cache);

    // Subscribe before the write so we catch the event.
    let mut rx = cache.subscribe();

    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    tag.set("color", json!("#ff0000"));
    ctx.write(&tag).await.unwrap();

    // Exactly one EntityChanged event with a non-empty `changes` vec.
    let evt = rx.try_recv().expect("expected EntityChanged event");
    match evt {
        EntityEvent::EntityChanged {
            entity_type,
            id,
            changes,
            ..
        } => {
            assert_eq!(entity_type, "tag");
            assert_eq!(id, "bug");
            assert!(
                !changes.is_empty(),
                "new entity should report fields in `changes`"
            );
        }
        other => panic!("expected EntityChanged, got {other:?}"),
    }

    // And the cache is populated — subsequent reads don't hit disk.
    let cached = cache.get("tag", "bug").await.unwrap();
    assert_eq!(cached.get_str("tag_name"), Some("Bug"));
}

/// Attaching the cache twice is a programming error — second call panics.
#[tokio::test]
#[should_panic(expected = "attach_cache called more than once")]
async fn test_attach_cache_twice_panics() {
    use crate::cache::EntityCache;

    let dir = TempDir::new().unwrap();
    let fields = test_fields_context();
    let ctx = Arc::new(EntityContext::new(dir.path(), fields));
    let cache1 = Arc::new(EntityCache::new(Arc::clone(&ctx)));
    let cache2 = Arc::new(EntityCache::new(Arc::clone(&ctx)));
    ctx.attach_cache(&cache1);
    ctx.attach_cache(&cache2); // should panic
}

// =========================================================================
// Delete / archive + undo / redo round-trip tests.
//
// After card 01KQ5FJ0VXEQZVKHZBN49Q5GFS removes the dual-writer, the
// sole source of changelog data is the store layer. These tests pin
// the contract that delete + undo round-trips still emit the correct
// events on the cache's broadcast channel and that the entity is
// re-readable after undo (and not after redo).
// =========================================================================

/// Round-trip harness: an `EntityContext` with a registered `StoreHandle`
/// for "tag", a shared `StoreContext`, and an attached `EntityCache`.
/// This mirrors the production wiring used by the kanban app.
async fn cache_ctx_store(
    dir: &TempDir,
    entity_type: &str,
) -> (
    Arc<EntityContext>,
    Arc<crate::cache::EntityCache>,
    Arc<StoreContext>,
) {
    let fields = test_fields_context();
    let ctx = Arc::new(EntityContext::new(dir.path(), fields.clone()));

    let entity_dir = dir.path().join(format!("{entity_type}s"));
    std::fs::create_dir_all(&entity_dir).unwrap();
    let entity_def = fields.get_entity(entity_type).unwrap();
    let field_defs: Vec<_> = fields
        .fields_for_entity(entity_type)
        .into_iter()
        .cloned()
        .collect();
    let store = crate::store::EntityTypeStore::new(
        &entity_dir,
        entity_type,
        Arc::new(entity_def.clone()),
        Arc::new(field_defs),
    );
    let handle = Arc::new(swissarmyhammer_store::StoreHandle::new(Arc::new(store)));
    ctx.register_store(entity_type, Arc::clone(&handle)).await;

    let store_context = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    store_context.register(handle).await;
    ctx.set_store_context(Arc::clone(&store_context));

    let cache = Arc::new(crate::cache::EntityCache::new(Arc::clone(&ctx)));
    ctx.attach_cache(&cache);

    (ctx, cache, store_context)
}

/// Helper: drain any already-buffered events from the cache receiver so
/// subsequent assertions see only the event we are interested in.
fn drain_events(rx: &mut tokio::sync::broadcast::Receiver<crate::events::EntityEvent>) {
    while rx.try_recv().is_ok() {}
}

#[tokio::test]
async fn delete_then_undo_round_trip_emits_correct_events() {
    use crate::events::EntityEvent;

    let dir = TempDir::new().unwrap();
    let (ctx, cache, store_context) = cache_ctx_store(&dir, "tag").await;
    let mut rx = cache.subscribe();

    // Write — must fire EntityChanged.
    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    tag.set("color", json!("#ff0000"));
    ctx.write(&tag).await.unwrap();

    let evt = rx.recv().await.expect("write must emit an event");
    assert!(
        matches!(evt, EntityEvent::EntityChanged { ref id, .. } if id == "bug"),
        "expected EntityChanged from write, got {evt:?}"
    );

    // Delete — must fire EntityDeleted and remove the entity from cache.
    ctx.delete("tag", "bug").await.unwrap();
    let evt = rx.recv().await.expect("delete must emit an event");
    assert!(
        matches!(evt, EntityEvent::EntityDeleted { ref id, .. } if id == "bug"),
        "expected EntityDeleted from delete, got {evt:?}"
    );
    assert!(
        ctx.read("tag", "bug").await.is_err(),
        "tag must be unreadable after delete"
    );

    // Undo — restores the entity on disk. The undo command-layer
    // glue calls `sync_entity_cache_from_disk`, which we invoke
    // directly here since this test exercises the entity layer alone.
    let outcome = store_context.undo().await.expect("undo must succeed");
    ctx.sync_entity_cache_from_disk(&outcome.store_name, outcome.item_id.as_str())
        .await;

    let evt = rx
        .recv()
        .await
        .expect("undo must emit an event via refresh_from_disk");
    assert!(
        matches!(evt, EntityEvent::EntityChanged { ref id, .. } if id == "bug"),
        "expected EntityChanged from undo of delete, got {evt:?}"
    );
    assert!(
        ctx.read("tag", "bug").await.is_ok(),
        "tag must be readable after undo of delete"
    );

    // Redo — re-deletes the entity. Must fire EntityDeleted again.
    drain_events(&mut rx);
    let outcome = store_context.redo().await.expect("redo must succeed");
    ctx.sync_entity_cache_from_disk(&outcome.store_name, outcome.item_id.as_str())
        .await;

    let evt = rx.recv().await.expect("redo must emit an event");
    assert!(
        matches!(evt, EntityEvent::EntityDeleted { ref id, .. } if id == "bug"),
        "expected EntityDeleted from redo of delete, got {evt:?}"
    );
    assert!(
        ctx.read("tag", "bug").await.is_err(),
        "tag must be unreadable again after redo of delete"
    );
}

#[tokio::test]
async fn archive_then_undo_round_trip_emits_correct_events() {
    use crate::events::EntityEvent;

    let dir = TempDir::new().unwrap();
    let (ctx, cache, store_context) = cache_ctx_store(&dir, "tag").await;
    let mut rx = cache.subscribe();

    // Write the tag and drain the EntityChanged event.
    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    ctx.write(&tag).await.unwrap();
    let _ = rx.recv().await.unwrap();

    // Archive — the entity disappears from the live cache.
    ctx.archive("tag", "bug").await.unwrap();
    let evt = rx.recv().await.expect("archive must emit an event");
    assert!(
        matches!(evt, EntityEvent::EntityDeleted { ref id, .. } if id == "bug"),
        "expected EntityDeleted from archive, got {evt:?}"
    );
    assert!(
        ctx.read("tag", "bug").await.is_err(),
        "tag must not be readable from live storage after archive"
    );

    // Undo — restores the tag from `.archive/` back to live storage.
    let outcome = store_context.undo().await.expect("undo must succeed");
    ctx.sync_entity_cache_from_disk(&outcome.store_name, outcome.item_id.as_str())
        .await;

    let evt = rx.recv().await.expect("undo must emit an event");
    assert!(
        matches!(evt, EntityEvent::EntityChanged { ref id, .. } if id == "bug"),
        "expected EntityChanged from undo of archive, got {evt:?}"
    );
    assert!(
        ctx.read("tag", "bug").await.is_ok(),
        "tag must be readable after undo of archive"
    );

    // Redo — re-archives the tag.
    drain_events(&mut rx);
    let outcome = store_context.redo().await.expect("redo must succeed");
    ctx.sync_entity_cache_from_disk(&outcome.store_name, outcome.item_id.as_str())
        .await;

    let evt = rx.recv().await.expect("redo must emit an event");
    assert!(
        matches!(evt, EntityEvent::EntityDeleted { ref id, .. } if id == "bug"),
        "expected EntityDeleted from redo of archive, got {evt:?}"
    );
    assert!(
        ctx.read("tag", "bug").await.is_err(),
        "tag must not be readable after redo of archive"
    );
}
