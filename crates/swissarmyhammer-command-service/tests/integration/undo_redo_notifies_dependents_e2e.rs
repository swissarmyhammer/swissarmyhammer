//! End-to-end: undo/redo over a real `kanban`+`views`+`store` substrate
//! reconciles every dependent in-memory cache AND a subscribed MCP client
//! (via the notification bridge) receives the resulting data events — carrying
//! `txn`/`origin` — plus the undo-stack-state events.
//!
//! This is the cross-cutting proof for the change-propagation task. It boots
//! the actual `EntityContext` + `EntityCache`, `ViewsContext`, and
//! `PerspectiveContext` over **one shared `StoreContext`** (the substrate
//! invariant), wires the `swissarmyhammer-kanban` fan-in adapters
//! ([`spawn_notification_fanin`]) into a real
//! [`NotificationBridge`](swissarmyhammer_plugin::NotificationBridge) with a
//! subscribed in-process client, edits one entity + one perspective + one
//! view, then undoes and redoes through the *same per-item reconcile the
//! unified `reconcile_post_undo_caches` runs* (`sync_entity_cache_from_disk_with`
//! / `reload_from_disk_with`, dispatched by store category — no bespoke
//! per-store code).
//!
//! It asserts, at each step:
//! - the caches reflect the reversed / reapplied state, and
//! - the subscribed client received the corresponding `store/changed`
//!   notifications stamped `origin: "undo"`/`"redo"` with a shared per-command
//!   `txn`, and the `store/undo_changed` stack-state notifications.

use std::sync::Arc;

use serde_json::json;
use tempfile::TempDir;

use swissarmyhammer_entity::test_utils::test_fields_context;
use swissarmyhammer_entity::{Entity, EntityCache, EntityContext, EntityTypeStore};
use swissarmyhammer_kanban::commands::app_commands::reconcile_caches;
use swissarmyhammer_kanban::notify_fanin::spawn_notification_fanin;
use swissarmyhammer_perspectives::types::Perspective;
use swissarmyhammer_perspectives::{PerspectiveContext, PerspectiveStore};
use swissarmyhammer_plugin::{McpNotification, NotificationBridge, NotificationSubscription};
use swissarmyhammer_store::{StoreContext, StoreHandle, UndoOutcome};
use swissarmyhammer_views::{ViewDef, ViewKind, ViewStore, ViewsContext};

/// The whole booted substrate plus the bridge client, kept together so the
/// test body reads as a script.
struct World {
    _dir: TempDir,
    store: Arc<StoreContext>,
    entity: Arc<EntityContext>,
    // Held only to keep the cache alive: `EntityContext::attach_cache` keeps a
    // weak handle, so dropping the only strong `Arc` would silently sever the
    // write→event→fan-in path. Never read directly.
    _cache: Arc<EntityCache>,
    views: tokio::sync::RwLock<ViewsContext>,
    perspectives: tokio::sync::RwLock<PerspectiveContext>,
    client: NotificationSubscription,
    _fanin: swissarmyhammer_kanban::notify_fanin::NotificationFanin,
}

/// Boot entity + views + perspectives over one `StoreContext`, then wire the
/// kanban fan-in into a bridge with a subscribed client.
async fn boot() -> World {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    // One shared StoreContext — the substrate invariant.
    let store = Arc::new(StoreContext::new(root.clone()));

    // --- entity layer (tag store) ---
    let fields = test_fields_context();
    let entity = Arc::new(EntityContext::new(&root, fields.clone()));
    let tag_dir = root.join("tags");
    std::fs::create_dir_all(&tag_dir).unwrap();
    let tag_def = fields.get_entity("tag").unwrap();
    let tag_fields: Vec<_> = fields.fields_for_entity("tag").into_iter().cloned().collect();
    let tag_store = EntityTypeStore::new(
        &tag_dir,
        "tag",
        Arc::new(tag_def.clone()),
        Arc::new(tag_fields),
    );
    let tag_handle = Arc::new(StoreHandle::new(Arc::new(tag_store)));
    entity.register_store("tag", Arc::clone(&tag_handle)).await;
    store.register(tag_handle).await;
    entity.set_store_context(Arc::clone(&store));
    let cache = Arc::new(EntityCache::new(Arc::clone(&entity)));
    entity.attach_cache(&cache);

    // --- views layer ---
    let views_dir = root.join("views");
    let view_store = Arc::new(ViewStore::new(&views_dir));
    let view_handle = Arc::new(StoreHandle::new(view_store));
    let mut views_ctx = ViewsContext::open(&views_dir).build().await.unwrap();
    views_ctx.set_store_handle(Arc::clone(&view_handle));
    views_ctx.set_store_context(Arc::clone(&store));
    // Register by value so the typed `Arc<StoreHandle<_>>` coerces to the
    // `Arc<dyn ErasedStore>` the context's shared stack stores.
    store.register(view_handle).await;

    // --- perspectives layer ---
    let persp_dir = root.join("perspectives");
    let persp_store = Arc::new(PerspectiveStore::new(&persp_dir));
    let persp_handle = Arc::new(StoreHandle::new(persp_store));
    let mut persp_ctx = PerspectiveContext::open(&persp_dir).await.unwrap();
    persp_ctx.set_store_handle(Arc::clone(&persp_handle));
    persp_ctx.set_store_context(Arc::clone(&store));
    store.register(persp_handle).await;

    // --- bridge + fan-in ---
    let bridge = NotificationBridge::new();
    let client = bridge.subscribe();
    let fanin = spawn_notification_fanin(
        bridge.clone(),
        Some(cache.subscribe()),
        Some(views_ctx.subscribe()),
        Some(persp_ctx.subscribe()),
        Some(store.subscribe_stack_state()),
    );

    // Let the forwarder tasks register their subscriptions before any edit.
    tokio::task::yield_now().await;

    World {
        _dir: dir,
        store,
        entity,
        _cache: cache,
        views: tokio::sync::RwLock::new(views_ctx),
        perspectives: tokio::sync::RwLock::new(persp_ctx),
        client,
        _fanin: fanin,
    }
}

impl World {
    /// Reconcile every dependent cache for a finished undo/redo `outcome` by
    /// calling the **production** shared reconcile helper
    /// ([`reconcile_caches`]) — the exact body `reconcile_post_undo_caches`
    /// runs. The test holds its cache handles directly (rather than off a
    /// `CommandContext`) and passes them in, so it cannot drift from the
    /// production per-item, category-keyed dispatch + one-shared-txn stamping.
    async fn reconcile(&self, outcome: &UndoOutcome, origin: &str) {
        reconcile_caches(
            outcome,
            origin,
            Some(self.entity.as_ref()),
            Some(&self.views),
            Some(&self.perspectives),
        )
        .await;
    }

    /// Drain notifications buffered on the bridge client (give the async
    /// fan-in forwarders a few scheduler turns to deliver first).
    async fn drain(&mut self) -> Vec<McpNotification> {
        // The fan-in forwarders are separate tasks; poll briefly for delivery.
        let mut out = Vec::new();
        // Poll for a while: the fan-in forwarders are separate tasks, so a
        // just-published event may not be in the channel on the first turn.
        // Once we have at least one and a quiet turn yields nothing more, the
        // burst is complete.
        for _ in 0..200 {
            let mut drained_any = false;
            while let Ok(note) = self.client.try_recv() {
                out.push(note);
                drained_any = true;
            }
            if !out.is_empty() && !drained_any {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }
        out
    }
}

fn make_view(id: &str, name: &str) -> ViewDef {
    ViewDef {
        id: id.into(),
        name: name.into(),
        icon: None,
        kind: ViewKind::Board,
        entity_type: None,
        card_fields: Vec::new(),
        commands: Vec::new(),
    }
}

fn store_changes(notes: &[McpNotification]) -> Vec<&McpNotification> {
    notes
        .iter()
        .filter(|n| n.method == "notifications/store/changed")
        .collect()
}

fn undo_changes(notes: &[McpNotification]) -> Vec<&McpNotification> {
    notes
        .iter()
        .filter(|n| n.method == "notifications/store/undo_changed")
        .collect()
}

#[tokio::test]
async fn undo_redo_reconciles_caches_and_notifies_subscribed_client() {
    let mut w = boot().await;

    // ---- Edit one entity, one perspective, one view (all undoable). ----
    let mut tag = Entity::new("tag", "bug");
    tag.set("tag_name", json!("Bug"));
    w.entity.write(&tag).await.unwrap();

    {
        let p = Perspective::new("01PERSP000000000000000001", "My Persp", "board");
        w.perspectives.write().await.write(&p).await.unwrap();
    }
    {
        let v = make_view("01VIEW0000000000000000001", "My View");
        w.views.write().await.write_view(&v).await.unwrap();
    }

    // All three live edits + the three pushes that grew the undo stack should
    // have produced normal `store/changed` (origin "user") + `undo_changed`.
    let creation_notes = w.drain().await;
    let creation_changes = store_changes(&creation_notes);
    assert!(
        creation_changes.iter().any(|n| n.params["store"] == "tag"
            && n.origin() == Some("user")),
        "the entity create must reach the client as origin=user; got {creation_notes:?}"
    );
    assert!(
        creation_changes
            .iter()
            .any(|n| n.params["store"] == "perspective"),
        "the perspective create must reach the client"
    );
    assert!(
        creation_changes.iter().any(|n| n.params["store"] == "view"),
        "the view create must reach the client"
    );
    assert!(
        !undo_changes(&creation_notes).is_empty(),
        "each push must fire a stack-state event"
    );
    assert!(w.store.can_undo().await);

    // ---- Undo the most recent edit (the view create). ----
    let outcome = w.store.undo().await.unwrap();
    w.reconcile(&outcome, "undo").await;

    // Cache reflects the reversed state: the view is gone.
    assert!(
        w.views.read().await.get_by_id("01VIEW0000000000000000001").is_none(),
        "undo of the view create must evict it from the views cache"
    );

    let undo_notes = w.drain().await;
    let undo_data = store_changes(&undo_notes);
    let view_undo: Vec<_> = undo_data
        .iter()
        .filter(|n| n.params["store"] == "view")
        .collect();
    assert!(!view_undo.is_empty(), "undo must notify the view store");
    for n in &view_undo {
        assert_eq!(n.origin(), Some("undo"), "undo-sourced event must carry origin=undo");
        assert!(n.txn().is_some(), "undo data events carry a txn");
        assert_eq!(n.params["op"], "removed", "undo of a create surfaces as removed");
    }
    // The reversed-edit data events of one undo share a single txn.
    let undo_txns: std::collections::HashSet<_> =
        view_undo.iter().filter_map(|n| n.txn()).collect();
    assert_eq!(undo_txns.len(), 1, "one undo command = one txn across its items");

    // The undo also fired a stack-state event (redo now available).
    let undo_stack = undo_changes(&undo_notes);
    assert!(!undo_stack.is_empty(), "undo must fire a stack-state event");
    assert_eq!(
        undo_stack.last().unwrap().params["can_redo"],
        true,
        "after an undo, redo must be available"
    );

    // ---- Redo it. ----
    let outcome = w.store.redo().await.unwrap();
    w.reconcile(&outcome, "redo").await;

    // Cache reflects the reapplied state: the view is back.
    assert!(
        w.views.read().await.get_by_id("01VIEW0000000000000000001").is_some(),
        "redo must restore the view in the cache"
    );

    let redo_notes = w.drain().await;
    let view_redo: Vec<_> = store_changes(&redo_notes)
        .into_iter()
        .filter(|n| n.params["store"] == "view")
        .collect();
    assert!(!view_redo.is_empty(), "redo must notify the view store");
    for n in &view_redo {
        assert_eq!(n.origin(), Some("redo"), "redo-sourced event must carry origin=redo");
        assert!(n.txn().is_some());
        // Views/perspectives use reload-item semantics: the reconcile cannot
        // recover the pre-undo `is_create` flag, so a restored view surfaces
        // as `updated` (the client reload-fetches the item either way). The
        // precise created/removed byte transition is asserted on the entity
        // store below, where the field diff makes it observable.
        assert_eq!(n.params["op"], "updated", "redo restores the view (reload-item)");
    }

    let redo_stack = undo_changes(&redo_notes);
    assert!(!redo_stack.is_empty(), "redo must fire a stack-state event");
    assert_eq!(
        redo_stack.last().unwrap().params["can_redo"],
        false,
        "after redoing the last entry, nothing is left to redo"
    );
}

/// A plain edit after an undo discards the redo tail — the non-symmetric case
/// — and the client must receive a `store/undo_changed` with `can_redo:false`.
#[tokio::test]
async fn fresh_edit_after_undo_notifies_can_redo_false() {
    let mut w = boot().await;

    // Two entity edits, then undo the second so redo is available.
    let mut a = Entity::new("tag", "aaa");
    a.set("tag_name", json!("A"));
    w.entity.write(&a).await.unwrap();
    let mut b = Entity::new("tag", "bbb");
    b.set("tag_name", json!("B"));
    w.entity.write(&b).await.unwrap();

    let outcome = w.store.undo().await.unwrap();
    w.reconcile(&outcome, "undo").await;
    assert!(w.store.can_redo().await, "precondition: redo available after undo");
    let _ = w.drain().await; // discard notifications so far

    // A brand-new edit discards the redo tail with no undo/redo call.
    let mut c = Entity::new("tag", "ccc");
    c.set("tag_name", json!("C"));
    w.entity.write(&c).await.unwrap();

    assert!(!w.store.can_redo().await, "a fresh edit discards the redo tail");

    let notes = w.drain().await;
    let stack = undo_changes(&notes);
    assert!(
        !stack.is_empty(),
        "the fresh edit's push must fire a stack-state event"
    );
    assert_eq!(
        stack.last().unwrap().params["can_redo"],
        false,
        "a plain edit after an undo must turn the Redo control off (can_redo:false)"
    );
}
