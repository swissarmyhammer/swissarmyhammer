//! Undo/redo substrate wiring for a board.
//!
//! A board's command-backing stores (per-entity-type stores, the perspective
//! store, the view store) must all register into **one** `Arc<StoreContext>`
//! so that `store.undo` / `store.redo` revert across heterogeneous stores on a
//! single LIFO stack. [`wire_store_substrate`] constructs that single context
//! and wires every subsystem into it.
//!
//! This is the *one* place the substrate is wired. Both production
//! (`BoardHandle::open` in kanban-app) and the substrate guard test
//! (`apps/kanban-app/tests/substrate_guard.rs`) call this function, so the guard
//! exercises the real production code path — if the wiring here ever forks the
//! `StoreContext`, the guard fails directly.

use std::sync::Arc;

use swissarmyhammer_store::{StoreContext, StoreHandle};

use crate::KanbanContext;

/// Wire the complete undo/redo substrate for a board and return the single
/// `Arc<StoreContext>` every subsystem now shares.
///
/// Constructs exactly one `StoreContext` rooted at `ctx.root()`, then registers
/// the per-entity-type stores, the perspective store, and the view store into
/// it (each via `Arc::clone`), attaching the shared context to the
/// `EntityContext`, `PerspectiveContext`, and `ViewsContext` that
/// `KanbanContext::open` already constructed.
///
/// Callers must construct **no other** `StoreContext` for the same board:
/// doing so would fork the undo stack. The returned `Arc` is the board's owner
/// handle on the substrate.
pub async fn wire_store_substrate(ctx: &KanbanContext) -> Arc<StoreContext> {
    let store_context = Arc::new(StoreContext::new(ctx.root().to_path_buf()));
    register_entity_stores(ctx, &store_context).await;
    register_perspective_store(ctx, &store_context).await;
    register_view_store(ctx, &store_context).await;
    store_context
}

/// Register a per-entity-type store for each entity type discovered on disk.
///
/// Wires the shared `StoreContext` into `EntityContext` so writes/deletes push
/// onto the undo stack, then creates an `EntityTypeStore` for every entity def
/// and registers it with both contexts.
async fn register_entity_stores(ctx: &KanbanContext, store_context: &Arc<StoreContext>) {
    let Ok(ectx) = ctx.entity_context().await else {
        return;
    };
    ectx.set_store_context(Arc::clone(store_context));

    let fields_ctx = ectx.fields();
    for entity_def in fields_ctx.all_entities() {
        let entity_type = entity_def.name.as_str();
        let field_defs = fields_ctx.fields_for_entity(entity_type);
        let owned_defs: Vec<_> = field_defs.into_iter().cloned().collect();
        let entity_type_store = swissarmyhammer_entity::EntityTypeStore::new(
            ectx.entity_dir(entity_type),
            entity_type,
            Arc::new(entity_def.clone()),
            Arc::new(owned_defs),
        );
        let handle = Arc::new(StoreHandle::new(Arc::new(entity_type_store)));
        ectx.register_store(entity_type, handle.clone()).await;
        store_context.register(handle).await;
    }
}

/// Register the perspective store for undo/redo and wire it into
/// `PerspectiveContext` so writes delegate to it and push onto the undo stack.
async fn register_perspective_store(ctx: &KanbanContext, store_context: &Arc<StoreContext>) {
    let perspectives_dir = ctx.root().join("perspectives");
    let perspective_store = swissarmyhammer_perspectives::PerspectiveStore::new(&perspectives_dir);
    let handle = Arc::new(StoreHandle::new(Arc::new(perspective_store)));
    store_context.register(handle.clone()).await;

    if let Ok(pctx) = ctx.perspective_context().await {
        let mut pctx = pctx.write().await;
        pctx.set_store_handle(handle);
        pctx.set_store_context(Arc::clone(store_context));
    }
}

/// Register the view store for undo/redo and wire it into `ViewsContext` so
/// writes delegate to it and push onto the undo stack.
///
/// Mirrors [`register_perspective_store`]: creates a `ViewStore` rooted at the
/// `views/` directory, wraps it in a `StoreHandle`, registers the handle with
/// the shared `StoreContext`, and attaches both to the `ViewsContext` that
/// `KanbanContext::open` already constructed.
async fn register_view_store(ctx: &KanbanContext, store_context: &Arc<StoreContext>) {
    let views_dir = ctx.root().join("views");
    let view_store = swissarmyhammer_views::ViewStore::new(&views_dir);
    let handle = Arc::new(StoreHandle::new(Arc::new(view_store)));
    store_context.register(handle.clone()).await;

    if let Some(views_lock) = ctx.views() {
        let mut views = views_lock.write().await;
        views.set_store_handle(handle);
        views.set_store_context(Arc::clone(store_context));
    }
}
