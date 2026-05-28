//! The MCP notification surface: the bridge, the subscription registry, and
//! the normalized, store-keyed notification model.
//!
//! This module is the platform-layer face of the app's in-process event
//! buses. The buses themselves (entity `EntityEvent`, `ViewEvent`,
//! `PerspectiveEvent`, UI-state changes, the command engine's action events)
//! stay the source of truth; this module gives them an **MCP face** so any
//! client — the kanban webview embedded in-process AND an external AI agent
//! over stdio/URL — subscribes to one normalized change stream of
//! server→client `notifications/…`.
//!
//! # The model
//!
//! Every event the app produces is normalized into an [`McpNotification`]: an
//! MCP notification `method` plus a JSON `params` object. There are four
//! *planes*:
//!
//! 1. **Data changes** — `notifications/store/changed`. One generic schema
//!    keyed by `store` ("task", "tag", "view", "perspective", …) covers
//!    entities (which carry field-level `changes`), views, and perspectives
//!    (which omit `changes` → reload-item).
//! 2. **Action/command events** — `notifications/commands/executed`.
//!    Emission is owned by the command engine's txn task; this bridge only
//!    **delivers** the event it is handed via [`NotificationBridge::publish`].
//! 3. **Registry / lifecycle** — `notifications/commands/changed`,
//!    `notifications/tools/list_changed`, plugin/board lifecycle.
//! 4. **Ephemeral UI state** — `notifications/ui_state/changed` and the undo
//!    stack's `notifications/store/undo_changed`.
//!
//! # Correlation + provenance
//!
//! Every notification carries two cross-cutting fields:
//!
//! - **`txn`** — the ambient transaction id grouping a command's writes, so a
//!   consumer can coalesce one command's N data changes into a single UI
//!   update, and an undo's inverse batch under one new `txn`.
//! - **`origin`** — provenance: `"user"`, `"agent:<id>"`, `"undo"`, `"redo"`,
//!   or `"watcher"`. Enables attribution and echo-suppression for
//!   multi-client / multi-agent scenarios.
//!
//! Both are stamped onto every [`McpNotification`] via [`Provenance`]. The
//! upstream bus structs do not yet carry `txn`/`origin` (a sibling
//! change-propagation task adds those fields and stamps them in the
//! reconcile/watcher); until then, the **wiring layer derives them at the
//! bridge** from the ambient transaction context and the [`CallerId`]. Once
//! the upstream structs carry the fields, the wiring passes them through
//! unchanged — the bridge shape does not change.
//!
//! # Mechanism
//!
//! [`NotificationBridge`] owns a single `tokio::sync::broadcast` channel — the
//! unified, post-normalization stream. The per-client **subscription
//! registry** is that channel's set of receivers, tracked with metadata so
//! the host can enumerate and reason about live subscribers:
//!
//! - **In-process clients** (the webview/host) call
//!   [`NotificationBridge::subscribe`] and read their
//!   [`NotificationSubscription`] receiver directly — zero IPC.
//! - **External clients** (a `CliServer`/`UrlServer`-backed agent) are served
//!   by a forwarder task that drains a subscription and pushes each
//!   notification to the connected MCP peer; see
//!   [`NotificationBridge::forward`].
//!
//! The concrete bus subscriptions live in a higher crate that depends on both
//! the event crates and this platform crate (the platform crate must not
//! depend on the domain crates — `swissarmyhammer-views` already depends on
//! it, so the edge would cycle). That wiring uses [`NotificationBridge::publish`]
//! to feed normalized notifications in. This keeps the bridge itself generic:
//! it knows the four planes and the correlation model, not the domain types.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tokio::sync::broadcast;

use crate::server::CallerId;

/// Default capacity of the bridge's unified broadcast channel.
///
/// Sized so a burst of a multi-write command's `store/changed` notifications
/// plus its `commands/executed` never laps a momentarily-behind subscriber.
/// A subscriber that still falls behind observes `broadcast`'s `Lagged`
/// signal and can resync — it never blocks the producer.
const NOTIFY_CHANNEL_CAPACITY: usize = 1024;

/// The provenance of a change: who caused it and which transaction it belongs
/// to.
///
/// `txn` is the ambient transaction id (the same id that groups undo
/// entries); `origin` is the actor classification. Both are stamped onto
/// every emitted [`McpNotification`]. A `None` `txn` means the change was not
/// made inside a transaction (a legacy per-write mutation, or an ephemeral
/// UI-state toggle that is not undoable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// The transaction / undo-group id this change belongs to, when one is
    /// active. `None` for changes made outside any transaction.
    pub txn: Option<String>,
    /// The actor classification: `"user"`, `"agent:<id>"`, `"undo"`,
    /// `"redo"`, or `"watcher"`.
    pub origin: String,
}

impl Provenance {
    /// The default provenance for a directly user-initiated change with no
    /// active transaction: `origin: "user"`, `txn: None`.
    pub fn user() -> Self {
        Self {
            txn: None,
            origin: "user".to_string(),
        }
    }

    /// Provenance for a change made inside transaction `txn` by `origin`.
    pub fn new(txn: Option<impl Into<String>>, origin: impl Into<String>) -> Self {
        Self {
            txn: txn.map(Into::into),
            origin: origin.into(),
        }
    }

    /// Derive the `origin` string for a [`CallerId`].
    ///
    /// - [`CallerId::HostInternal`] / [`CallerId::Unknown`] → `"user"` (the
    ///   host acts on the user's behalf; an unidentified caller is treated as
    ///   the user rather than invented as an agent).
    /// - [`CallerId::External`] → `"agent:<id>"` (an external MCP client is an
    ///   agent; the presented identity is carried through).
    /// - [`CallerId::Plugin`] → `"agent:<plugin-id>"` (a plugin acting on its
    ///   own is an agent for attribution purposes).
    pub fn origin_for_caller(caller: &CallerId) -> String {
        match caller {
            CallerId::HostInternal | CallerId::Unknown => "user".to_string(),
            CallerId::External(id) => format!("agent:{id}"),
            CallerId::Plugin(id) => format!("agent:{}", id.as_str()),
        }
    }

    /// Build provenance for a caller inside an (optional) transaction.
    ///
    /// `txn` is the ambient transaction id (the wiring reads it from the
    /// `store` server's `current_transaction()` / `RequestContext::extensions`);
    /// `origin` is derived from `caller` via [`origin_for_caller`](Self::origin_for_caller).
    pub fn for_caller(caller: &CallerId, txn: Option<impl Into<String>>) -> Self {
        Self {
            txn: txn.map(Into::into),
            origin: Self::origin_for_caller(caller),
        }
    }

    /// Stamp `txn` and `origin` into a params object, consuming `self`.
    fn stamp_into(self, params: &mut Map<String, Value>) {
        params.insert("txn".to_string(), json!(self.txn));
        params.insert("origin".to_string(), json!(self.origin));
    }
}

/// The kind of data-change op a [`McpNotification`] of the `store/changed`
/// plane reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeOp {
    /// The item was created.
    Created,
    /// The item was removed (trashed / deleted / archived out of view).
    Removed,
    /// The item was updated in place.
    Updated,
}

impl ChangeOp {
    /// The wire string for this op (`"created"` / `"removed"` / `"updated"`).
    fn as_str(self) -> &'static str {
        match self {
            ChangeOp::Created => "created",
            ChangeOp::Removed => "removed",
            ChangeOp::Updated => "updated",
        }
    }
}

/// A single field-level change carried in a `store/changed` notification's
/// `changes` array.
///
/// Mirrors the entity layer's `FieldChange`: a removed field is encoded as
/// `value: null`. Views and perspectives omit `changes` entirely (reload-item
/// semantics) — this struct is only populated for entity stores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldChange {
    /// The field that changed.
    pub field: String,
    /// The field's new value; `null` signals removal.
    pub value: Value,
}

/// A normalized MCP server→client notification, ready to deliver over any
/// transport.
///
/// Carries the MCP notification `method` (e.g. `"notifications/store/changed"`)
/// and a JSON `params` object. Construct one with the per-plane helpers
/// ([`store_changed`](Self::store_changed),
/// [`commands_executed`](Self::commands_executed), …) rather than building the
/// `params` by hand, so the schema and the correlation fields stay
/// consistent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpNotification {
    /// The MCP notification method, e.g. `"notifications/store/changed"`.
    pub method: String,
    /// The notification's parameter object, already carrying `txn`/`origin`
    /// where the plane requires them.
    pub params: Value,
}

impl McpNotification {
    /// Construct a raw notification from a method and params.
    ///
    /// Prefer the per-plane constructors; this is the escape hatch for
    /// lifecycle notifications a future task adds without a dedicated helper.
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            method: method.into(),
            params,
        }
    }

    /// Plane 1 — `notifications/store/changed`.
    ///
    /// The one generic data-change schema for entities, views, and
    /// perspectives. `store` names the store ("task", "tag", "view",
    /// "perspective", …); `item` is the item id; `op` is the change kind.
    /// `changes` is `Some` for entity stores (field-level diff) and `None`
    /// for views/perspectives (reload-item). `prov` stamps `txn`/`origin`.
    pub fn store_changed(
        store: impl Into<String>,
        item: impl Into<String>,
        op: ChangeOp,
        changes: Option<Vec<FieldChange>>,
        prov: Provenance,
    ) -> Self {
        let mut params = Map::new();
        params.insert("store".to_string(), json!(store.into()));
        params.insert("item".to_string(), json!(item.into()));
        params.insert("op".to_string(), json!(op.as_str()));
        if let Some(changes) = changes {
            params.insert("changes".to_string(), json!(changes));
        }
        prov.stamp_into(&mut params);
        Self::new("notifications/store/changed", Value::Object(params))
    }

    /// Plane 2 — `notifications/commands/executed`.
    ///
    /// **Delivery only.** The command engine's txn task owns *emission*; this
    /// constructor exists so that task (and the bridge's tests) can hand a
    /// fully-formed action event to [`NotificationBridge::publish`]. `id` is
    /// the command id, `ctx` the execution context, `result` the command's
    /// return value. Shares the command's `txn` with the data changes it
    /// produced.
    pub fn commands_executed(
        id: impl Into<String>,
        ctx: Value,
        result: Value,
        prov: Provenance,
    ) -> Self {
        let mut params = Map::new();
        params.insert("id".to_string(), json!(id.into()));
        params.insert("ctx".to_string(), ctx);
        params.insert("result".to_string(), result);
        prov.stamp_into(&mut params);
        Self::new("notifications/commands/executed", Value::Object(params))
    }

    /// Plane 3 — `notifications/commands/changed` (command registry changed).
    ///
    /// Signals the palette to refresh; carries no per-item payload beyond the
    /// correlation fields.
    pub fn commands_changed(prov: Provenance) -> Self {
        let mut params = Map::new();
        prov.stamp_into(&mut params);
        Self::new("notifications/commands/changed", Value::Object(params))
    }

    /// Plane 3 — `notifications/tools/list_changed` (server tool set changed).
    pub fn tools_list_changed() -> Self {
        Self::new("notifications/tools/list_changed", json!({}))
    }

    /// Plane 4 — `notifications/ui_state/changed` (ephemeral UI state).
    ///
    /// `window` scopes the change to one window (`None` for global state);
    /// `key` names the UI surface (`"palette_open"`, `"keymap_mode"`,
    /// `"app_mode"`, …); `value` is its new value. Ephemeral UI state is not
    /// a stored thing and not undoable, so it carries no `txn`.
    pub fn ui_state_changed(window: Option<String>, key: impl Into<String>, value: Value) -> Self {
        let mut params = Map::new();
        if let Some(window) = window {
            params.insert("window".to_string(), json!(window));
        }
        params.insert("key".to_string(), json!(key.into()));
        params.insert("value".to_string(), value);
        Self::new("notifications/ui_state/changed", Value::Object(params))
    }

    /// Plane 4 — `notifications/store/undo_changed` (undo-stack state).
    ///
    /// *Emission* of this family is owned by the change-propagation task; the
    /// bridge **delivers** it. This constructor is the seam that task emits
    /// into. Reports whether undo/redo are currently possible and the labels
    /// of the entries at the top of each stack.
    pub fn store_undo_changed(
        can_undo: bool,
        can_redo: bool,
        undo_label: Option<String>,
        redo_label: Option<String>,
    ) -> Self {
        let params = json!({
            "can_undo": can_undo,
            "can_redo": can_redo,
            "undo_label": undo_label,
            "redo_label": redo_label,
        });
        Self::new("notifications/store/undo_changed", params)
    }

    /// The `txn` field carried in this notification's params, if any.
    ///
    /// Returns `None` for planes that carry no transaction (ephemeral
    /// `ui_state/changed`, `tools/list_changed`) or when the field is JSON
    /// `null`. Used by tests and consumers to correlate a command's data
    /// changes.
    pub fn txn(&self) -> Option<&str> {
        self.params.get("txn").and_then(Value::as_str)
    }

    /// The `origin` field carried in this notification's params, if any.
    pub fn origin(&self) -> Option<&str> {
        self.params.get("origin").and_then(Value::as_str)
    }
}

/// A live, in-process subscription to the bridge's notification stream.
///
/// Wraps the `broadcast::Receiver` an in-process client (the kanban webview,
/// the host) reads from, plus the stable [`SubscriberId`] the registry tracks
/// it under. Dropping it removes the subscriber from the registry's live
/// count.
#[derive(Debug)]
pub struct NotificationSubscription {
    /// This subscriber's stable id within the bridge's registry.
    id: SubscriberId,
    /// The underlying broadcast receiver this subscriber drains.
    rx: broadcast::Receiver<McpNotification>,
    /// Shared registry handle so the subscriber deregisters on drop.
    registry: Arc<SubscriptionRegistry>,
}

impl NotificationSubscription {
    /// This subscription's stable id.
    pub fn id(&self) -> SubscriberId {
        self.id
    }

    /// Await the next notification on this subscription.
    ///
    /// Returns `Err(broadcast::error::RecvError::Lagged(n))` when the
    /// subscriber fell `n` notifications behind and the channel overwrote
    /// them — the caller should resync rather than treat it as fatal — and
    /// `Err(RecvError::Closed)` once the bridge is dropped.
    pub async fn recv(&mut self) -> Result<McpNotification, broadcast::error::RecvError> {
        self.rx.recv().await
    }

    /// Try to receive a notification without awaiting.
    pub fn try_recv(&mut self) -> Result<McpNotification, broadcast::error::TryRecvError> {
        self.rx.try_recv()
    }
}

impl Drop for NotificationSubscription {
    fn drop(&mut self) {
        self.registry.deregister(self.id);
    }
}

/// A stable identifier for one subscriber in the bridge's registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubscriberId(u64);

impl SubscriberId {
    /// The raw numeric value, for diagnostics.
    pub fn get(self) -> u64 {
        self.0
    }
}

/// How a subscriber receives its stream — the registry tracks this so the
/// host can reason about who is connected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriberKind {
    /// An in-process client draining a [`NotificationSubscription`] receiver
    /// directly (the webview, the host).
    InProcess,
    /// An external MCP client served by a forwarder that pushes
    /// notifications to its connected peer (a `CliServer`/`UrlServer` agent).
    /// The string is the client identity, mirroring [`CallerId::External`].
    External(String),
}

/// The per-client subscription registry.
///
/// Tracks every live subscriber by [`SubscriberId`] together with its
/// [`SubscriberKind`], so the host can enumerate connected clients and a
/// subscriber can deregister itself on drop. The actual fan-out is the
/// broadcast channel the [`NotificationBridge`] owns — a subscriber's
/// receiver is minted from that channel — so this registry is the *metadata*
/// face of the subscription set, not a second delivery path.
#[derive(Debug, Default)]
struct SubscriptionRegistry {
    /// Live subscribers: id → kind. Behind a `Mutex` because every mutation
    /// (register on subscribe, deregister on drop) is a brief, synchronous
    /// map edit never held across an `.await`.
    subscribers: Mutex<std::collections::HashMap<SubscriberId, SubscriberKind>>,
    /// Source of monotonically increasing subscriber ids.
    next_id: AtomicU64,
}

impl SubscriptionRegistry {
    /// Allocate and record a subscriber of `kind`, returning its id.
    fn register(&self, kind: SubscriberKind) -> SubscriberId {
        let id = SubscriberId(self.next_id.fetch_add(1, Ordering::Relaxed));
        self.subscribers
            .lock()
            .expect("subscription registry poisoned")
            .insert(id, kind);
        id
    }

    /// Drop the subscriber `id` from the live set.
    fn deregister(&self, id: SubscriberId) {
        self.subscribers
            .lock()
            .expect("subscription registry poisoned")
            .remove(&id);
    }

    /// The number of currently-live subscribers.
    fn len(&self) -> usize {
        self.subscribers
            .lock()
            .expect("subscription registry poisoned")
            .len()
    }
}

/// The notification bridge: one unified stream, fanned out to every client.
///
/// Holds the single `broadcast::Sender<McpNotification>` that carries the
/// normalized, post-fan-in stream, and the [`SubscriptionRegistry`] tracking
/// who is listening. Clone it freely — every clone shares the one channel and
/// the one registry, so a notification published on any clone reaches every
/// subscriber of every clone.
///
/// # The two delivery paths
///
/// - **In-process**: [`subscribe`](Self::subscribe) hands back a
///   [`NotificationSubscription`] the caller drains directly.
/// - **External**: [`forward`](Self::forward) spawns a task that drains a
///   fresh subscription and invokes a caller-supplied sink for each
///   notification — the sink is how a `CliServer`/`UrlServer` transport pushes
///   the notification to its connected MCP peer.
///
/// # Feeding it
///
/// [`publish`](Self::publish) is the single ingress. The wiring layer's
/// per-bus fan-in tasks normalize each bus event into an [`McpNotification`]
/// and call `publish`; the command engine's txn task publishes
/// `commands/executed`; the change-propagation task publishes
/// `store/undo_changed`. The bridge does not subscribe to the domain buses
/// itself — that wiring lives in a higher crate (see the module docs on why
/// the platform crate cannot depend on the domain crates).
#[derive(Clone)]
pub struct NotificationBridge {
    /// The unified, post-normalization stream every subscriber reads.
    sender: broadcast::Sender<McpNotification>,
    /// The live-subscriber registry, shared across clones.
    registry: Arc<SubscriptionRegistry>,
}

impl std::fmt::Debug for NotificationBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationBridge")
            .field("subscribers", &self.registry.len())
            .finish()
    }
}

impl Default for NotificationBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationBridge {
    /// Construct a fresh bridge with an empty subscription registry.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(NOTIFY_CHANNEL_CAPACITY);
        Self {
            sender,
            registry: Arc::new(SubscriptionRegistry::default()),
        }
    }

    /// Publish a normalized notification to every live subscriber.
    ///
    /// This is the bridge's single ingress — every plane flows through here.
    /// Returns the number of subscribers the notification reached. A return of
    /// `0` is not an error: it simply means no client is currently listening
    /// (the broadcast channel drops the value), which is the steady state
    /// before any webview or agent has subscribed.
    pub fn publish(&self, notification: McpNotification) -> usize {
        self.sender.send(notification).unwrap_or(0)
    }

    /// Subscribe an in-process client to the stream.
    ///
    /// Records an [`SubscriberKind::InProcess`] entry in the registry and
    /// returns a [`NotificationSubscription`] the caller drains with
    /// [`recv`](NotificationSubscription::recv). The subscription deregisters
    /// itself on drop.
    pub fn subscribe(&self) -> NotificationSubscription {
        self.subscribe_as(SubscriberKind::InProcess)
    }

    /// Subscribe a client of a specific [`SubscriberKind`].
    ///
    /// Used by [`forward`](Self::forward) to register an external subscriber,
    /// and available directly for callers that want to label an in-process
    /// subscriber (e.g. by window).
    pub fn subscribe_as(&self, kind: SubscriberKind) -> NotificationSubscription {
        let id = self.registry.register(kind);
        NotificationSubscription {
            id,
            rx: self.sender.subscribe(),
            registry: Arc::clone(&self.registry),
        }
    }

    /// Forward the stream to an external client via a caller-supplied sink.
    ///
    /// Spawns a task that drains a fresh [`SubscriberKind::External`]
    /// subscription and calls `sink` for each notification. `sink` is the
    /// transport-specific push to the connected MCP peer — for a
    /// `CliServer`/`UrlServer` it sends the notification's `method` and
    /// `params` to the agent over the wire. A `Lagged` is logged and the
    /// forwarder resyncs (keeps going); a `Closed` ends the task, which is
    /// also what happens when the bridge is dropped.
    ///
    /// The returned [`JoinHandle`](tokio::task::JoinHandle) lets the caller
    /// abort the forwarder when the client disconnects; dropping it detaches
    /// the task (it then ends on the next `Closed`).
    pub fn forward<F, Fut>(&self, identity: String, mut sink: F) -> tokio::task::JoinHandle<()>
    where
        F: FnMut(McpNotification) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let mut sub = self.subscribe_as(SubscriberKind::External(identity));
        tokio::spawn(async move {
            loop {
                match sub.recv().await {
                    Ok(notification) => sink(notification).await,
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(
                            skipped,
                            "notification forwarder lagged; external client may need to resync"
                        );
                    }
                    Err(broadcast::error::RecvError::Closed) => return,
                }
            }
        })
    }

    /// The number of currently-live subscribers (in-process + external).
    ///
    /// Exposed for the host and for tests asserting fan-out reach.
    pub fn subscriber_count(&self) -> usize {
        self.registry.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::PluginId;

    #[test]
    fn store_changed_carries_the_generic_schema_with_field_changes() {
        let note = McpNotification::store_changed(
            "task",
            "01ABC",
            ChangeOp::Updated,
            Some(vec![FieldChange {
                field: "title".to_string(),
                value: json!("New"),
            }]),
            Provenance::new(Some("txn-1"), "user"),
        );
        assert_eq!(note.method, "notifications/store/changed");
        assert_eq!(note.params["store"], "task");
        assert_eq!(note.params["item"], "01ABC");
        assert_eq!(note.params["op"], "updated");
        assert_eq!(note.params["changes"][0]["field"], "title");
        assert_eq!(note.txn(), Some("txn-1"));
        assert_eq!(note.origin(), Some("user"));
    }

    #[test]
    fn store_changed_omits_changes_for_reload_item_stores() {
        let note = McpNotification::store_changed(
            "perspective",
            "01XYZ",
            ChangeOp::Updated,
            None,
            Provenance::user(),
        );
        assert_eq!(note.params["store"], "perspective");
        assert!(
            note.params.get("changes").is_none(),
            "views/perspectives omit `changes` so the client reloads the item"
        );
    }

    #[test]
    fn origin_for_caller_classifies_actors() {
        assert_eq!(
            Provenance::origin_for_caller(&CallerId::HostInternal),
            "user"
        );
        assert_eq!(Provenance::origin_for_caller(&CallerId::Unknown), "user");
        assert_eq!(
            Provenance::origin_for_caller(&CallerId::External("a1".into())),
            "agent:a1"
        );
        assert_eq!(
            Provenance::origin_for_caller(&CallerId::Plugin(PluginId::new("p1"))),
            "agent:p1"
        );
    }

    #[tokio::test]
    async fn publish_fans_out_to_every_subscriber() {
        let bridge = NotificationBridge::new();
        let mut a = bridge.subscribe();
        let mut b = bridge.subscribe();
        assert_eq!(bridge.subscriber_count(), 2);

        let reached = bridge.publish(McpNotification::tools_list_changed());
        assert_eq!(reached, 2, "both subscribers should receive the notification");

        let from_a = a.recv().await.unwrap();
        let from_b = b.recv().await.unwrap();
        assert_eq!(from_a.method, "notifications/tools/list_changed");
        assert_eq!(from_b.method, "notifications/tools/list_changed");
    }

    #[tokio::test]
    async fn dropping_a_subscription_deregisters_it() {
        let bridge = NotificationBridge::new();
        let a = bridge.subscribe();
        assert_eq!(bridge.subscriber_count(), 1);
        drop(a);
        assert_eq!(bridge.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn forward_pushes_each_notification_to_the_external_sink() {
        use std::sync::Arc;
        use tokio::sync::Mutex as AsyncMutex;

        let bridge = NotificationBridge::new();
        let seen = Arc::new(AsyncMutex::new(Vec::<String>::new()));
        let seen_clone = Arc::clone(&seen);
        let handle = bridge.forward("agent-1".to_string(), move |note| {
            let seen = Arc::clone(&seen_clone);
            async move {
                seen.lock().await.push(note.method);
            }
        });

        // Give the forwarder task a moment to register its subscription.
        tokio::task::yield_now().await;
        for _ in 0..20 {
            if bridge.subscriber_count() == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }

        bridge.publish(McpNotification::commands_changed(Provenance::user()));
        bridge.publish(McpNotification::tools_list_changed());

        // Let the forwarder drain.
        for _ in 0..50 {
            if seen.lock().await.len() == 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let methods = seen.lock().await.clone();
        assert_eq!(
            methods,
            vec![
                "notifications/commands/changed".to_string(),
                "notifications/tools/list_changed".to_string(),
            ],
            "the external forwarder should receive every notification in order"
        );
        handle.abort();
    }
}
