//! Substrate guard: prove that all command-backing stores share **one**
//! `Arc<StoreContext>` — i.e. one undo stack per board.
//!
//! # What this test pins
//!
//! Production wiring lives at `apps/kanban-app/src/state.rs::BoardHandle::open`,
//! which delegates the entire substrate wiring to
//! `swissarmyhammer_kanban::wire_store_substrate`. That function constructs
//! exactly one `Arc<StoreContext>`, then wires it into the `EntityContext`,
//! `PerspectiveContext`, and `ViewsContext`. Every `TrackedStore` (entity-type
//! stores, perspective store, view store) is registered into that one
//! `StoreContext`, so `store.undo` / `store.redo` revert across heterogeneous
//! stores on a single LIFO stack.
//!
//! This test does NOT mirror that wiring — it **calls** `wire_store_substrate`
//! directly, the exact same function production runs, against a temp `.kanban`
//! dir. It then verifies, via `Arc::ptr_eq`, that the `StoreContext` each
//! subsystem holds is the *same* allocation as the one the helper returned. If
//! a future change ever splits the substrate — e.g. a setter that quietly
//! constructs its own `StoreContext` instead of storing the one it was handed,
//! or the helper building a second context — this test fails loudly, because
//! it exercises the production code path rather than a copy of it.

use std::sync::Arc;

use swissarmyhammer_kanban::board::InitBoard;
use swissarmyhammer_kanban::{
    wire_store_substrate, KanbanContext, KanbanOperationProcessor, OperationProcessor,
};
use swissarmyhammer_store::StoreContext;
use tempfile::TempDir;

/// Open a production-shape `KanbanContext` and run the real production
/// substrate wiring (`wire_store_substrate`) against it. Returns the kanban
/// context and the single `Arc<StoreContext>` the helper produced, which the
/// test checks every subsystem shares.
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

    // Run the EXACT production wiring — the same call `BoardHandle::open`
    // makes. The returned Arc is the one-and-only StoreContext for this board.
    let store_context = wire_store_substrate(&kanban).await;

    (temp, kanban, store_context)
}

/// After the production wiring runs, the entity, perspective, and views
/// contexts must each hold an `Arc<StoreContext>` whose allocation is the same
/// as the one the helper returned. `Arc::ptr_eq` returns true iff the two
/// `Arc`s point at the *same* allocation, so this is the strict "no-fork"
/// check the substrate invariant demands.
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
