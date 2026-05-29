//! `execute` brackets its callback in one ambient store transaction: the
//! command-as-unit contract.
//!
//! A command whose `execute` callback makes TWO store writes must land both
//! writes in ONE undo group, so a single `store.undo` reverts the whole
//! command as one step. This is the headline grouping test the card calls for
//! (`/tdd` — written first): it pins that one action → one txn → one undo
//! group.
//!
//! ## What is real here
//!
//! - A real `StoreContext` + `EntityContext` over one shared substrate (the
//!   same shape the kanban app boots).
//! - A real store-backed [`TransactionSeam`] wrapping
//!   `StoreContext::begin_transaction` / `end_transaction`.
//! - A fake [`CallbackDispatcher`] whose `execute` callback writes two
//!   entities through the entity layer — exactly the downstream store writes a
//!   production callback would make, on the same tokio task the seam opened the
//!   transaction on (the handler `.await`s the dispatcher inline, never
//!   spawning), so the writes inherit the ambient `txn`.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use swissarmyhammer_command_service::{
    CallbackDispatcher, CallbackHandle, CallbackInvokeError, CommandService, TransactionSeam,
};
use swissarmyhammer_entity::test_utils::test_fields_context;
use swissarmyhammer_entity::{Entity, EntityContext, EntityTypeStore};
use swissarmyhammer_plugin::CallerId;
use swissarmyhammer_store::{StoreContext, StoreHandle, UndoEntryId};
use tempfile::TempDir;

use super::support::{call_command, execute_args, register_args};

/// Store-backed transaction seam: opens/closes the per-task ambient
/// transaction on the shared `StoreContext`, exactly as the production seam
/// (the `store` server's `BeginTransaction`/`EndTransaction`) does.
struct StoreTransactionSeam {
    store: Arc<StoreContext>,
}

impl std::fmt::Debug for StoreTransactionSeam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreTransactionSeam").finish()
    }
}

impl TransactionSeam for StoreTransactionSeam {
    fn begin(&self, origin: &str) -> Option<String> {
        Some(
            self.store
                .begin_transaction_with_origin(Some(origin.to_string()))
                .to_string(),
        )
    }

    fn end(&self, txn: &str) {
        if let Ok(id) = txn.parse::<UndoEntryId>() {
            self.store.end_transaction(id);
        }
    }
}

/// A dispatcher whose single `execute` callback writes two tag entities.
///
/// Both writes go through the entity layer onto the shared `StoreContext`, so
/// each pushes an undo entry that picks up whatever ambient transaction is open
/// on the calling task.
struct TwoWriteDispatcher {
    entity: Arc<EntityContext>,
}

impl std::fmt::Debug for TwoWriteDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TwoWriteDispatcher").finish()
    }
}

#[async_trait]
impl CallbackDispatcher for TwoWriteDispatcher {
    async fn invoke(
        &self,
        _handle: &CallbackHandle,
        _args: Value,
    ) -> Result<Value, CallbackInvokeError> {
        let mut a = Entity::new("tag", "alpha");
        a.set("tag_name", json!("Alpha"));
        self.entity
            .write(&a)
            .await
            .map_err(|e| CallbackInvokeError::new(e.to_string()))?;

        let mut b = Entity::new("tag", "bravo");
        b.set("tag_name", json!("Bravo"));
        self.entity
            .write(&b)
            .await
            .map_err(|e| CallbackInvokeError::new(e.to_string()))?;

        Ok(json!({ "wrote": 2 }))
    }
}

/// Boot a real entity substrate over one shared `StoreContext` and return the
/// pieces the test needs.
async fn boot() -> (TempDir, Arc<StoreContext>, Arc<EntityContext>) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    let store = Arc::new(StoreContext::new(root.clone()));

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

    (dir, store, entity)
}

#[tokio::test]
async fn two_writes_share_one_txn_and_undo_reverts_them_as_one_group() {
    let (_dir, store, entity) = boot().await;

    let dispatcher = Arc::new(TwoWriteDispatcher {
        entity: Arc::clone(&entity),
    });
    let transaction = Arc::new(StoreTransactionSeam {
        store: Arc::clone(&store),
    });

    let service = CommandService::new()
        .with_dispatcher(dispatcher)
        .with_transaction(transaction);

    // Register a command whose execute callback makes the two writes.
    call_command(
        &service,
        CallerId::HostInternal,
        register_args("tag.makeTwo", "Make Two Tags", "cb_two"),
    )
    .await;

    // Precondition: nothing on the undo stack yet.
    assert_eq!(store.undo_depth().await, 0, "no writes before execute");

    // Execute the command. The two callback writes happen inside the
    // bracketed transaction.
    call_command(&service, CallerId::HostInternal, execute_args("tag.makeTwo")).await;

    // Both entities are on disk.
    assert!(entity.read("tag", "alpha").await.is_ok());
    assert!(entity.read("tag", "bravo").await.is_ok());

    // The two writes are ONE undo group: a single `undo()` reverts both.
    assert!(store.can_undo().await, "the command produced an undo group");
    store.undo().await.expect("undo reverts the group");

    assert!(
        entity.read("tag", "alpha").await.is_err() && entity.read("tag", "bravo").await.is_err(),
        "one undo must revert BOTH writes the command made — they share one txn/group"
    );
    assert!(
        !store.can_undo().await,
        "after reverting the single command group nothing remains to undo"
    );

    // The transaction was closed: no ambient txn leaks onto the task.
    assert!(
        store.current_transaction().is_none(),
        "execute must close the transaction it opened"
    );
}
