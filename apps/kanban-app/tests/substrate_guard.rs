//! Substrate guard: prove that all command-backing stores share **one**
//! `Arc<StoreContext>` — i.e. one undo stack per board.
//!
//! # What this test pins
//!
//! Production wiring lives at `apps/kanban-app/src/state.rs::BoardHandle::open`.
//! That function constructs exactly one `Arc<StoreContext>`, then passes it
//! by `Arc::clone` to three registration helpers that wire it into the
//! `EntityContext`, `PerspectiveContext`, and `ViewsContext`. Every
//! `TrackedStore` (entity-type stores, perspective store, view store) is
//! registered into that one `StoreContext`, so `store.undo` / `store.redo`
//! revert across heterogeneous stores on a single LIFO stack.
//!
//! This test mirrors that wiring step-by-step against a temp `.kanban` dir
//! and then verifies, via `Arc::ptr_eq`, that the `StoreContext` each
//! subsystem holds is the *same* allocation as the one constructed at the
//! top of the substrate. If a future change ever splits the substrate —
//! e.g. a setter that quietly constructs its own `StoreContext` instead of
//! storing the one it was handed, or a registration helper that builds a
//! second context — this test fails loudly.
//!
//! The wiring below intentionally tracks
//! `state.rs::{register_entity_stores, register_perspective_store,
//! register_view_store}` line-for-line. If those change shape, this guard
//! must be updated in lock-step.

use std::sync::Arc;

use swissarmyhammer_entity::EntityTypeStore;
use swissarmyhammer_kanban::board::InitBoard;
use swissarmyhammer_kanban::{KanbanContext, KanbanOperationProcessor, OperationProcessor};
use swissarmyhammer_perspectives::PerspectiveStore;
use swissarmyhammer_store::{StoreContext, StoreHandle};
use swissarmyhammer_views::ViewStore;
use tempfile::TempDir;

/// Open a production-shape `KanbanContext` with the same three-store
/// registration sequence `BoardHandle::open` performs. Returns the kanban
/// context and the single `Arc<StoreContext>` the test will check against.
async fn open_production_shape() -> (TempDir, Arc<KanbanContext>, Arc<StoreContext>) {
    let temp = TempDir::new().expect("temp dir");
    let kanban_dir = temp.path().join(".kanban");

    // Initialize the board so the entity context can be opened later.
    let init_ctx = KanbanContext::new(&kanban_dir);
    let processor = KanbanOperationProcessor::new();
    processor
        .process(&InitBoard::new("Substrate Guard"), &init_ctx)
        .await
        .expect("board init");
    drop(init_ctx);

    // Reopen to populate the views context, mirroring `BoardHandle::open`.
    let kanban = Arc::new(
        KanbanContext::open(&kanban_dir)
            .await
            .expect("reopen kanban context"),
    );

    // === The one-and-only StoreContext for this board. ===
    let store_context = Arc::new(StoreContext::new(kanban.root().to_path_buf()));

    // --- Mirror `register_entity_stores` ---
    let ectx = kanban.entity_context().await.expect("entity context");
    ectx.set_store_context(Arc::clone(&store_context));
    let fields_ctx = ectx.fields();
    for entity_def in fields_ctx.all_entities() {
        let entity_type = entity_def.name.as_str();
        let owned_defs: Vec<_> = fields_ctx
            .fields_for_entity(entity_type)
            .into_iter()
            .cloned()
            .collect();
        let entity_type_store = EntityTypeStore::new(
            ectx.entity_dir(entity_type),
            entity_type,
            Arc::new(entity_def.clone()),
            Arc::new(owned_defs),
        );
        let handle = Arc::new(StoreHandle::new(Arc::new(entity_type_store)));
        ectx.register_store(entity_type, handle.clone()).await;
        store_context.register(handle).await;
    }

    // --- Mirror `register_perspective_store` ---
    let perspectives_dir = kanban.root().join("perspectives");
    let perspective_store = PerspectiveStore::new(&perspectives_dir);
    let perspective_handle = Arc::new(StoreHandle::new(Arc::new(perspective_store)));
    store_context.register(perspective_handle.clone()).await;
    {
        let pctx = kanban
            .perspective_context()
            .await
            .expect("perspective context");
        let mut pctx = pctx.write().await;
        pctx.set_store_handle(perspective_handle);
        pctx.set_store_context(Arc::clone(&store_context));
    }

    // --- Mirror `register_view_store` ---
    let views_dir = kanban.root().join("views");
    let view_store = ViewStore::new(&views_dir);
    let view_handle = Arc::new(StoreHandle::new(Arc::new(view_store)));
    store_context.register(view_handle.clone()).await;
    if let Some(views_lock) = kanban.views() {
        let mut views = views_lock.write().await;
        views.set_store_handle(view_handle);
        views.set_store_context(Arc::clone(&store_context));
    }

    (temp, kanban, store_context)
}

/// After the production-shape wiring runs, the entity, perspective, and
/// views contexts must each hold an `Arc<StoreContext>` whose allocation is
/// the same as the one the board owns. `Arc::ptr_eq` returns true iff the
/// two `Arc`s point at the *same* allocation, so this is the strict
/// "no-fork" check the substrate invariant demands.
#[tokio::test(flavor = "multi_thread")]
async fn all_subsystems_share_one_store_context() {
    let (_temp, kanban, board_store_context) = open_production_shape().await;

    // --- EntityContext ---
    let ectx = kanban.entity_context().await.expect("entity context");
    let entity_sc = ectx
        .store_context()
        .expect("entity context must have a StoreContext after wiring");
    assert!(
        Arc::ptr_eq(&entity_sc, &board_store_context),
        "EntityContext holds a different StoreContext than the board — \
         someone forked the undo substrate"
    );

    // --- PerspectiveContext ---
    let pctx = kanban
        .perspective_context()
        .await
        .expect("perspective context");
    let perspective_sc = {
        let pctx = pctx.read().await;
        pctx.store_context()
            .expect("PerspectiveContext must have a StoreContext after wiring")
    };
    assert!(
        Arc::ptr_eq(&perspective_sc, &board_store_context),
        "PerspectiveContext holds a different StoreContext than the board — \
         someone forked the undo substrate"
    );

    // --- ViewsContext ---
    let views_lock = kanban.views().expect("views context");
    let views_sc = {
        let views = views_lock.read().await;
        views
            .store_context()
            .expect("ViewsContext must have a StoreContext after wiring")
    };
    assert!(
        Arc::ptr_eq(&views_sc, &board_store_context),
        "ViewsContext holds a different StoreContext than the board — \
         someone forked the undo substrate"
    );
}
