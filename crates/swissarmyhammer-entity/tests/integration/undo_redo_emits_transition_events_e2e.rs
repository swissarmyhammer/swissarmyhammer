//! End-to-end: undo/redo emit *edit-shaped* `EntityEvent`s carrying the
//! correct byte transition plus `txn` + `origin` provenance.
//!
//! These exercise the **reconcile path** (the `UndoCmd`/`RedoCmd` command
//! glue → `sync_entity_cache_from_disk_with` → `cache.refresh_from_disk` /
//! `evict`), NOT a store-level emitter. `swissarmyhammer-store` rewrites the
//! bytes on disk and returns an `UndoOutcome`; the entity layer derives the
//! event from the *post-rewrite* state. So:
//!
//! - undo of a **create** → the file is gone → `EntityDeleted` (`removed`),
//! - redo of that create → the file is back → `EntityChanged` (`created`),
//! - undo of an **update** → old field values in `changes`,
//! - redo of that update → new field values in `changes`,
//!
//! and every reconcile-sourced event carries `origin: "undo"` / `"redo"`
//! and a single shared `txn` across the command's reconciled items.

use std::sync::Arc;

use serde_json::json;
use swissarmyhammer_entity::events::EntityEvent;
use swissarmyhammer_entity::test_utils::test_fields_context;
use swissarmyhammer_entity::{Entity, EntityContext, EntityTypeStore};
use swissarmyhammer_store::{EventProvenance, StoreContext, StoreHandle, UndoEntryId, UndoOutcome};
use tempfile::TempDir;

/// Build an `EntityContext` + `EntityCache` + `StoreContext` for one entity
/// type, all sharing the single store substrate. Returns the pieces a test
/// needs to drive writes, subscribe to events, and run undo/redo commands.
async fn setup(
    entity_type: &str,
) -> (
    TempDir,
    Arc<EntityContext>,
    Arc<swissarmyhammer_entity::cache::EntityCache>,
    Arc<StoreContext>,
) {
    let dir = TempDir::new().unwrap();
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
    let store = EntityTypeStore::new(
        &entity_dir,
        entity_type,
        Arc::new(entity_def.clone()),
        Arc::new(field_defs),
    );
    let handle = Arc::new(StoreHandle::new(Arc::new(store)));
    ctx.register_store(entity_type, Arc::clone(&handle)).await;

    let store_context = Arc::new(StoreContext::new(dir.path().to_path_buf()));
    store_context.register(handle).await;
    ctx.set_store_context(Arc::clone(&store_context));

    let cache = Arc::new(swissarmyhammer_entity::cache::EntityCache::new(Arc::clone(&ctx)));
    ctx.attach_cache(&cache);

    (dir, ctx, cache, store_context)
}

/// Reconcile the entity cache against post-undo/redo disk state — the same
/// loop the deleted `UndoCmd`/`RedoCmd` glue ran. All items of one
/// undo/redo call share a single fresh `txn`.
async fn reconcile(ctx: &EntityContext, outcome: &UndoOutcome, origin: &str) {
    let txn = UndoEntryId::new().to_string();
    for (store_name, item_id) in &outcome.items {
        let prov = EventProvenance::new(Some(txn.clone()), origin);
        ctx.sync_entity_cache_from_disk_with(store_name, item_id.as_str(), prov)
            .await;
    }
}

async fn undo_and_reconcile(ctx: &EntityContext, store: &StoreContext) {
    let outcome = store.undo().await.expect("undo must succeed");
    reconcile(ctx, &outcome, "undo").await;
}

async fn redo_and_reconcile(ctx: &EntityContext, store: &StoreContext) {
    let outcome = store.redo().await.expect("redo must succeed");
    reconcile(ctx, &outcome, "redo").await;
}

fn drain(rx: &mut tokio::sync::broadcast::Receiver<EntityEvent>) {
    while rx.try_recv().is_ok() {}
}

/// undo of a create emits `removed` (EntityDeleted) with `origin: "undo"`;
/// redo emits `created` (EntityChanged) with `origin: "redo"`. Each carries a
/// `txn`.
#[tokio::test]
async fn create_undo_emits_removed_redo_emits_created_with_provenance() {
    let (_dir, ctx, cache, store_context) = setup("tag").await;

    // Create the tag through the entity context (write-through cache).
    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    ctx.write(&tag).await.unwrap();

    // Subscribe only now so we observe undo/redo events in isolation.
    let mut rx = cache.subscribe();

    // Undo the create — the file is trashed, so the reconcile must emit
    // EntityDeleted (`removed`) with origin "undo".
    undo_and_reconcile(&ctx, &store_context).await;
    let evt = rx.recv().await.expect("undo of create must emit an event");
    let undo_txn = match evt {
        EntityEvent::EntityDeleted {
            ref id,
            ref txn,
            ref origin,
            ..
        } => {
            assert_eq!(id, "bug");
            assert_eq!(origin, "undo", "undo-sourced event must carry origin=undo");
            assert!(txn.is_some(), "undo event must carry a txn");
            txn.clone()
        }
        other => panic!("expected EntityDeleted from undo of create, got {other:?}"),
    };

    // Redo the create — the file is restored, so the reconcile must emit
    // EntityChanged (`created`) with origin "redo".
    redo_and_reconcile(&ctx, &store_context).await;
    let evt = rx.recv().await.expect("redo of create must emit an event");
    match evt {
        EntityEvent::EntityChanged {
            ref id,
            ref origin,
            ref txn,
            ref changes,
            ..
        } => {
            assert_eq!(id, "bug");
            assert_eq!(origin, "redo", "redo-sourced event must carry origin=redo");
            assert!(txn.is_some(), "redo event must carry a txn");
            assert_ne!(
                txn, &undo_txn,
                "a distinct command (redo) gets its own txn, not the undo's"
            );
            assert!(
                changes.iter().any(|c| c.field == "tag_name"),
                "redo of create must re-surface the entity's fields"
            );
        }
        other => panic!("expected EntityChanged from redo of create, got {other:?}"),
    }
}

/// undo of an update emits the OLD field values; redo emits the NEW ones.
/// Both carry the correct origin, and a single command's reconciled items
/// share one txn.
#[tokio::test]
async fn update_undo_emits_old_values_redo_emits_new_values() {
    let (_dir, ctx, cache, store_context) = setup("tag").await;

    // Create then update so the undo stack has an update entry on top.
    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Old Name"));
    ctx.write(&tag).await.unwrap();

    tag.set("tag_name", json!("New Name"));
    ctx.write(&tag).await.unwrap();

    let mut rx = cache.subscribe();
    drain(&mut rx);

    // Undo the update — disk reverts to "Old Name"; the reconcile diffs the
    // cached "New Name" against disk and emits the OLD value.
    undo_and_reconcile(&ctx, &store_context).await;
    let evt = rx.recv().await.expect("undo of update must emit an event");
    let undo_txn = match evt {
        EntityEvent::EntityChanged {
            ref id,
            ref origin,
            ref txn,
            ref changes,
            ..
        } => {
            assert_eq!(id, "bug");
            assert_eq!(origin, "undo");
            let name = changes
                .iter()
                .find(|c| c.field == "tag_name")
                .expect("tag_name must be in the undo diff");
            assert_eq!(name.value, json!("Old Name"), "undo must surface the OLD value");
            txn.clone()
        }
        other => panic!("expected EntityChanged from undo of update, got {other:?}"),
    };

    // Redo the update — disk goes back to "New Name".
    redo_and_reconcile(&ctx, &store_context).await;
    let evt = rx.recv().await.expect("redo of update must emit an event");
    match evt {
        EntityEvent::EntityChanged {
            ref origin,
            ref txn,
            ref changes,
            ..
        } => {
            assert_eq!(origin, "redo");
            assert!(txn.is_some());
            assert_ne!(txn, &undo_txn, "redo is its own command → its own txn");
            let name = changes
                .iter()
                .find(|c| c.field == "tag_name")
                .expect("tag_name must be in the redo diff");
            assert_eq!(name.value, json!("New Name"), "redo must surface the NEW value");
        }
        other => panic!("expected EntityChanged from redo of update, got {other:?}"),
    }
}

/// A grouped command's N reconciled items share one `txn` per undo call.
/// Two entities written under one transaction are undone as one group; the
/// two reconcile events must carry the same txn.
#[tokio::test]
async fn grouped_undo_shares_one_txn_across_items() {
    let (_dir, ctx, cache, store_context) = setup("tag").await;

    // Write two tags inside one transaction so they form one undo group.
    let guard = store_context.begin_undo_group().await;
    let mut a = Entity::new("tag", "aaa");
    a.set("tag_name", json!("A"));
    ctx.write(&a).await.unwrap();
    let mut b = Entity::new("tag", "bbb");
    b.set("tag_name", json!("B"));
    ctx.write(&b).await.unwrap();
    guard.end().await;

    let mut rx = cache.subscribe();
    drain(&mut rx);

    // One undo reverses the whole group → two reconcile events, one txn.
    undo_and_reconcile(&ctx, &store_context).await;

    let e1 = rx.recv().await.expect("first grouped undo event");
    let e2 = rx.recv().await.expect("second grouped undo event");
    let txn_of = |e: &EntityEvent| match e {
        EntityEvent::EntityDeleted { txn, origin, .. }
        | EntityEvent::EntityChanged { txn, origin, .. } => {
            assert_eq!(origin, "undo");
            txn.clone()
        }
        other => panic!("unexpected event {other:?}"),
    };
    let t1 = txn_of(&e1);
    let t2 = txn_of(&e2);
    assert!(t1.is_some(), "grouped undo events must carry a txn");
    assert_eq!(t1, t2, "all items of one undo command must share one txn");
}
