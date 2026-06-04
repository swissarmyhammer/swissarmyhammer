//! Per-bus **fan-in adapters**: subscribe to the in-process domain event
//! buses and the store's undo-stack-state sender, normalize each event into a
//! platform-layer [`McpNotification`], and publish it into the
//! [`NotificationBridge`].
//!
//! # Why this lives here
//!
//! The bridge (`swissarmyhammer-plugin::notify`) is deliberately generic: it
//! knows the four notification planes and the correlation model, not the
//! domain event types. It cannot subscribe to the entity / view / perspective
//! buses itself, because `swissarmyhammer-views` already depends on the
//! platform crate, so a platform→domain edge would cycle.
//!
//! This crate (`swissarmyhammer-kanban`) is the natural fan-in home: it
//! already depends on `-entity` / `-views` / `-perspectives` (for their event
//! buses) **and** on `-store` (for the stack-state sender), and
//! `swissarmyhammer-plugin` does not depend on it — so the edge
//! kanban→plugin is acyclic. The adapters subscribe to each bus, translate,
//! and call [`NotificationBridge::publish`].
//!
//! # The translation
//!
//! - [`EntityEvent::EntityChanged`] → `store/changed` `op: updated` with the
//!   field-level `changes`. (`EntityChanged` covers both first-write and
//!   update; the consumer reads `changes` either way.)
//! - [`EntityEvent::EntityDeleted`] → `store/changed` `op: removed`, no
//!   `changes`.
//! - [`EntityEvent::AttachmentChanged`] → no data-plane mapping (attachments
//!   are not store items); skipped here.
//! - [`ViewEvent`] / [`PerspectiveEvent`] → `store/changed` for the `view` /
//!   `perspective` store, `op: created` when `is_create` else `updated`, with
//!   no `changes` (reload-item semantics).
//! - [`StackState`] → `store/undo_changed`.
//!
//! Every data-plane notification carries the `txn` + `origin` the upstream
//! event now stamps (a sibling change-propagation task added those fields):
//! the adapter passes them through unchanged rather than re-deriving them.

use std::sync::Arc;

use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use swissarmyhammer_entity::EntityEvent;
use swissarmyhammer_operations::Notification;
use swissarmyhammer_perspectives::events::PerspectiveEvent;
use swissarmyhammer_plugin::notify::{
    ChangeOp, FieldChange as NotifyFieldChange, McpNotification, NotificationBridge, Provenance,
};
use swissarmyhammer_store::operations::{StoreChanged, StoreUndoChanged};
use swissarmyhammer_store::StackState;
use swissarmyhammer_views::events::ViewEvent;

/// The store-name string the entity bus uses is the entity type itself
/// (`"task"`, `"tag"`, …); views and perspectives use these fixed names,
/// matching `ViewStore::store_name()` / `PerspectiveStore::store_name()`.
const VIEW_STORE: &str = swissarmyhammer_views::VIEW_STORE_NAME;
const PERSPECTIVE_STORE: &str = swissarmyhammer_perspectives::PERSPECTIVE_STORE_NAME;

/// Translate an [`EntityEvent`] into a bridge [`McpNotification`].
///
/// Returns `None` for events that have no data-plane mapping
/// (`AttachmentChanged` — attachments are not store items).
pub fn entity_event_to_notification(event: &EntityEvent) -> Option<McpNotification> {
    match event {
        EntityEvent::EntityChanged {
            entity_type,
            id,
            changes,
            txn,
            origin,
            ..
        } => {
            let mapped: Vec<NotifyFieldChange> = changes
                .iter()
                .map(|c| NotifyFieldChange {
                    field: c.field.clone(),
                    value: c.value.clone(),
                })
                .collect();
            Some(store_changed_notification(
                entity_type.clone(),
                id.clone(),
                ChangeOp::Updated,
                Some(mapped),
                Provenance::new(txn.clone(), origin.clone()),
            ))
        }
        EntityEvent::EntityDeleted {
            entity_type,
            id,
            txn,
            origin,
        } => Some(store_changed_notification(
            entity_type.clone(),
            id.clone(),
            ChangeOp::Removed,
            None,
            Provenance::new(txn.clone(), origin.clone()),
        )),
        EntityEvent::AttachmentChanged { .. } => None,
    }
}

/// Build a `notifications/store/changed` notification from the declared
/// [`StoreChanged`] payload struct — the single-source-of-truth publish path.
///
/// Serializing the struct produces the wire `params`; `prov` stamps
/// `txn`/`origin` on top via
/// [`McpNotification::from_declared`]. Because the SAME struct both declares the
/// notification's params (on the `store` tool) and produces them here, the
/// declared schema and the emitted payload cannot drift.
fn store_changed_notification(
    store: String,
    item: String,
    op: ChangeOp,
    changes: Option<Vec<NotifyFieldChange>>,
    prov: Provenance,
) -> McpNotification {
    let payload = StoreChanged {
        store,
        item,
        op: op.as_str().to_string(),
        changes,
    };
    McpNotification::from_declared(payload.method(), &payload, prov)
}

/// Translate a [`ViewEvent`] into a bridge [`McpNotification`] for the `view`
/// store. Views carry no field diff (reload-item semantics), so `changes` is
/// always `None`.
pub fn view_event_to_notification(event: &ViewEvent) -> McpNotification {
    match event {
        ViewEvent::ViewChanged {
            id,
            is_create,
            txn,
            origin,
            ..
        } => store_changed_notification(
            VIEW_STORE.to_string(),
            id.clone(),
            if *is_create {
                ChangeOp::Created
            } else {
                ChangeOp::Updated
            },
            None,
            Provenance::new(txn.clone(), origin.clone()),
        ),
        ViewEvent::ViewDeleted { id, txn, origin } => store_changed_notification(
            VIEW_STORE.to_string(),
            id.clone(),
            ChangeOp::Removed,
            None,
            Provenance::new(txn.clone(), origin.clone()),
        ),
    }
}

/// Translate a [`PerspectiveEvent`] into a bridge [`McpNotification`] for the
/// `perspective` store (reload-item semantics, no `changes`).
pub fn perspective_event_to_notification(event: &PerspectiveEvent) -> McpNotification {
    match event {
        PerspectiveEvent::PerspectiveChanged {
            id,
            is_create,
            txn,
            origin,
            ..
        } => store_changed_notification(
            PERSPECTIVE_STORE.to_string(),
            id.clone(),
            if *is_create {
                ChangeOp::Created
            } else {
                ChangeOp::Updated
            },
            None,
            Provenance::new(txn.clone(), origin.clone()),
        ),
        PerspectiveEvent::PerspectiveDeleted { id, txn, origin } => store_changed_notification(
            PERSPECTIVE_STORE.to_string(),
            id.clone(),
            ChangeOp::Removed,
            None,
            Provenance::new(txn.clone(), origin.clone()),
        ),
    }
}

/// Translate a [`StackState`] into the `store/undo_changed` notification.
///
/// Built from the declared [`StoreUndoChanged`] payload struct so the wire
/// `params` and the schema declared on the `store` tool share one source.
/// Undo-stack state is ephemeral and not itself an undoable thing, so this
/// plane carries no provenance — the struct is serialized directly.
pub fn stack_state_to_notification(state: &StackState) -> McpNotification {
    let payload = StoreUndoChanged {
        can_undo: state.can_undo,
        can_redo: state.can_redo,
        undo_label: state.undo_label.clone(),
        redo_label: state.redo_label.clone(),
    };
    let params = serde_json::to_value(&payload).unwrap_or_else(|_| serde_json::json!({}));
    McpNotification::new(payload.method(), params)
}

/// Handles to the forwarder tasks spawned by [`spawn_notification_fanin`].
///
/// Dropping it detaches the tasks (they end naturally when their upstream
/// channel closes — i.e. when the producing context is dropped). Keep it
/// alive for the lifetime of the board to keep the fan-in running, or
/// `abort()` it to tear the fan-in down early.
#[must_use = "dropping the handle leaves the forwarders detached; hold it for the board lifetime"]
pub struct NotificationFanin {
    handles: Vec<JoinHandle<()>>,
}

impl NotificationFanin {
    /// Abort every forwarder task immediately.
    pub fn abort(&self) {
        for h in &self.handles {
            h.abort();
        }
    }
}

impl Drop for NotificationFanin {
    fn drop(&mut self) {
        // Detach: the tasks end on the next `Closed` from their upstream bus.
        // We do not abort on drop so an accidental early drop does not silently
        // kill live notification delivery — callers that want teardown call
        // `abort()` explicitly.
    }
}

/// Spawn one forwarder task per supplied bus, each draining its receiver,
/// normalizing every event into an [`McpNotification`], and publishing it into
/// `bridge`.
///
/// Pass `None` for a bus that is not present on this board (e.g. no views /
/// perspectives sub-context yet) to skip wiring it. A `Lagged` on any bus is
/// logged and the forwarder resyncs (keeps going); a `Closed` ends that one
/// forwarder.
pub fn spawn_notification_fanin(
    bridge: NotificationBridge,
    entity_rx: Option<broadcast::Receiver<EntityEvent>>,
    view_rx: Option<broadcast::Receiver<ViewEvent>>,
    perspective_rx: Option<broadcast::Receiver<PerspectiveEvent>>,
    stack_state_rx: Option<broadcast::Receiver<StackState>>,
) -> NotificationFanin {
    let mut handles = Vec::new();

    if let Some(rx) = entity_rx {
        handles.push(spawn_forwarder(
            bridge.clone(),
            rx,
            "entity",
            entity_event_to_notification,
        ));
    }
    if let Some(rx) = view_rx {
        handles.push(spawn_forwarder(bridge.clone(), rx, "view", |e| {
            Some(view_event_to_notification(e))
        }));
    }
    if let Some(rx) = perspective_rx {
        handles.push(spawn_forwarder(bridge.clone(), rx, "perspective", |e| {
            Some(perspective_event_to_notification(e))
        }));
    }
    if let Some(rx) = stack_state_rx {
        handles.push(spawn_forwarder(bridge, rx, "undo_stack", |s| {
            Some(stack_state_to_notification(s))
        }));
    }

    NotificationFanin { handles }
}

/// Spawn one forwarder draining a single broadcast bus through `map` into the
/// bridge. `map` returns `None` to drop an event with no data-plane mapping.
fn spawn_forwarder<E, F>(
    bridge: NotificationBridge,
    mut rx: broadcast::Receiver<E>,
    bus: &'static str,
    map: F,
) -> JoinHandle<()>
where
    E: Clone + Send + 'static,
    F: Fn(&E) -> Option<McpNotification> + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Some(note) = map(&event) {
                        bridge.publish(note);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(bus, skipped, "notification fan-in lagged; resyncing");
                }
                Err(broadcast::error::RecvError::Closed) => return,
            }
        }
    })
}

/// Hold this `Arc` to keep a fan-in alive for the board lifetime without the
/// caller threading the [`NotificationFanin`] handle through every owner.
pub type SharedFanin = Arc<NotificationFanin>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeSet;
    use swissarmyhammer_entity::events::FieldChange;
    use swissarmyhammer_operations::generate_notifications_meta;
    use swissarmyhammer_store::operations::store_notifications;

    /// The set of `notifications/store/*` methods the `store` tool DECLARES,
    /// read from the `_meta` generator.
    fn declared_store_methods() -> BTreeSet<String> {
        generate_notifications_meta(store_notifications())
            .as_object()
            .expect("notifications meta is an object")
            .values()
            .map(|leaf| {
                leaf["method"]
                    .as_str()
                    .expect("each notification leaf carries a method")
                    .to_string()
            })
            .collect()
    }

    /// The set of `notifications/store/*` methods the fan-in actually PUBLISHES,
    /// gathered by exercising every translator with a representative event.
    fn published_store_methods() -> BTreeSet<String> {
        let entity = entity_event_to_notification(&EntityEvent::EntityChanged {
            entity_type: "task".to_string(),
            id: "t1".to_string(),
            version: 1,
            changes: vec![],
            txn: None,
            origin: "user".to_string(),
        })
        .expect("entity changed maps");
        let view = view_event_to_notification(&ViewEvent::ViewChanged {
            id: "v1".to_string(),
            changed_fields: vec![],
            is_create: true,
            txn: None,
            origin: "user".to_string(),
        });
        let perspective =
            perspective_event_to_notification(&PerspectiveEvent::PerspectiveChanged {
                id: "p1".to_string(),
                changed_fields: vec![],
                is_create: true,
                txn: None,
                origin: "user".to_string(),
            });
        let undo = stack_state_to_notification(&StackState {
            can_undo: true,
            can_redo: false,
            undo_label: None,
            redo_label: None,
        });
        [entity, view, perspective, undo]
            .into_iter()
            .map(|n| n.method)
            .collect()
    }

    /// Coverage guard (declared ⟺ published), namespace-based. Every
    /// `notifications/store/*` method the fan-in publishes MUST be declared on
    /// the `store` tool, and every declared store method MUST be one the fan-in
    /// publishes — so a store notification can never be raised without appearing
    /// in `_meta`, nor declared without this fan-in producing it. The owner
    /// (`store` tool) and the publisher (this fan-in) live in different crates;
    /// this test is the seam that keeps them in lockstep.
    #[test]
    fn published_store_methods_equal_declared() {
        let declared = declared_store_methods();
        let published = published_store_methods();
        assert_eq!(
            published, declared,
            "fan-in published store methods {published:?} must equal the methods \
             declared on the store tool {declared:?}",
        );
        assert_eq!(
            declared,
            BTreeSet::from([
                "notifications/store/changed".to_string(),
                "notifications/store/undo_changed".to_string(),
            ]),
        );
    }

    #[test]
    fn entity_changed_maps_to_store_changed_updated_with_changes_and_provenance() {
        let event = EntityEvent::EntityChanged {
            entity_type: "task".to_string(),
            id: "01ABC".to_string(),
            version: 1,
            changes: vec![FieldChange {
                field: "title".to_string(),
                value: json!("New"),
            }],
            txn: Some("txn-1".to_string()),
            origin: "undo".to_string(),
        };
        let note = entity_event_to_notification(&event).expect("entity changed maps");
        assert_eq!(note.method, "notifications/store/changed");
        assert_eq!(note.params["store"], "task");
        assert_eq!(note.params["item"], "01ABC");
        assert_eq!(note.params["op"], "updated");
        assert_eq!(note.params["changes"][0]["field"], "title");
        assert_eq!(note.txn(), Some("txn-1"));
        assert_eq!(note.origin(), Some("undo"));
    }

    #[test]
    fn entity_deleted_maps_to_removed_without_changes() {
        let event = EntityEvent::EntityDeleted {
            entity_type: "task".to_string(),
            id: "01ABC".to_string(),
            txn: Some("txn-9".to_string()),
            origin: "redo".to_string(),
        };
        let note = entity_event_to_notification(&event).expect("entity deleted maps");
        assert_eq!(note.params["op"], "removed");
        assert!(note.params.get("changes").is_none());
        assert_eq!(note.origin(), Some("redo"));
    }

    #[test]
    fn attachment_changed_has_no_data_plane_mapping() {
        let event = EntityEvent::AttachmentChanged {
            entity_type: "task".to_string(),
            filename: "x.png".to_string(),
            removed: false,
        };
        assert!(entity_event_to_notification(&event).is_none());
    }

    #[test]
    fn view_create_maps_to_created_for_view_store() {
        let event = ViewEvent::ViewChanged {
            id: "01V".to_string(),
            changed_fields: vec![],
            is_create: true,
            txn: None,
            origin: "user".to_string(),
        };
        let note = view_event_to_notification(&event);
        assert_eq!(note.params["store"], VIEW_STORE);
        assert_eq!(note.params["op"], "created");
        assert!(note.params.get("changes").is_none(), "views omit changes");
    }

    #[test]
    fn perspective_update_maps_to_updated_for_perspective_store() {
        let event = PerspectiveEvent::PerspectiveChanged {
            id: "01P".to_string(),
            changed_fields: vec![],
            is_create: false,
            txn: Some("t".to_string()),
            origin: "undo".to_string(),
        };
        let note = perspective_event_to_notification(&event);
        assert_eq!(note.params["store"], PERSPECTIVE_STORE);
        assert_eq!(note.params["op"], "updated");
        assert_eq!(note.origin(), Some("undo"));
    }

    #[test]
    fn stack_state_maps_to_undo_changed() {
        let state = StackState {
            can_undo: true,
            can_redo: false,
            undo_label: Some("create task".to_string()),
            redo_label: None,
        };
        let note = stack_state_to_notification(&state);
        assert_eq!(note.method, "notifications/store/undo_changed");
        assert_eq!(note.params["can_undo"], true);
        assert_eq!(note.params["can_redo"], false);
        assert_eq!(note.params["undo_label"], "create task");
    }

    #[tokio::test]
    async fn fanin_forwards_entity_events_into_the_bridge() {
        let bridge = NotificationBridge::new();
        let mut sub = bridge.subscribe();
        let (tx, rx) = broadcast::channel(16);

        let fanin = spawn_notification_fanin(bridge.clone(), Some(rx), None, None, None);

        // Give the forwarder a moment to register its subscription.
        tokio::task::yield_now().await;

        tx.send(EntityEvent::EntityChanged {
            entity_type: "tag".to_string(),
            id: "t1".to_string(),
            version: 1,
            changes: vec![],
            txn: Some("txn-1".to_string()),
            origin: "user".to_string(),
        })
        .unwrap();

        let note = tokio::time::timeout(std::time::Duration::from_secs(2), sub.recv())
            .await
            .expect("fan-in must forward within timeout")
            .expect("a notification");
        assert_eq!(note.params["store"], "tag");
        assert_eq!(note.txn(), Some("txn-1"));
        fanin.abort();
    }
}
