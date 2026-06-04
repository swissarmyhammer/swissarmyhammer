//! Per-host registry of plugin interest in notification methods.
//!
//! The platform's [`NotificationBridge`](crate::notify::NotificationBridge) is a
//! host-wide pub/sub stream every in-process and external client reads. This
//! registry is the seam that lets a *plugin isolate* be one of those clients:
//! when a plugin subscribes (`this.<server>.on(event, cb)` in the SDK), the SDK
//! marshals `cb` into a callback id and the host records
//! `(method → {plugin, callback})` here. The host's event pump (see
//! [`crate::host`]) drains the bridge and, for each notification, invokes every
//! callback registered against the notification's `method` with the
//! notification's `params`.
//!
//! The registry is *host-owned* state — not service-owned — so the host cleans a
//! plugin's entries directly on unload via [`EventSubscriptions::remove_plugin`]
//! rather than through the per-plugin ledger's opaque-hook path the command
//! service uses for its own resources. (The isolate's callback table is still
//! drained by the ledger, because each subscribe records the callback id as a
//! ledger `Callback` handle.)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::server::PluginId;

/// One plugin's interest in a notification method: which isolate, which callback.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Subscription {
    /// The plugin whose isolate holds the callback.
    plugin_id: PluginId,
    /// The SDK-assigned callback id (e.g. `"cb_42"`) to invoke on delivery.
    callback_id: String,
}

/// Maps a notification `method` to the plugin callbacks subscribed to it.
///
/// Cloned freely — every clone shares one `Arc<Mutex<…>>`, so the host, its
/// `subscribe`/`unsubscribe` envelope handlers, and the event pump all see one
/// registry. Each mutation is a brief synchronous map edit never held across an
/// `.await`, so the `Mutex` never blocks async progress.
#[derive(Clone, Default)]
pub(crate) struct EventSubscriptions {
    /// `method → subscribers`, shared across clones.
    inner: Arc<Mutex<HashMap<String, Vec<Subscription>>>>,
}

impl EventSubscriptions {
    /// Construct an empty registry.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Record that `plugin_id`'s `callback_id` is interested in `method`.
    ///
    /// Idempotent: a duplicate `(plugin_id, callback_id)` for the same method is
    /// not appended twice, so a plugin re-subscribing the same callback does not
    /// receive doubled deliveries.
    pub(crate) fn subscribe(&self, method: String, plugin_id: PluginId, callback_id: String) {
        let sub = Subscription {
            plugin_id,
            callback_id,
        };
        let mut map = self.lock();
        let entries = map.entry(method).or_default();
        if !entries.contains(&sub) {
            entries.push(sub);
        }
    }

    /// Remove one `(plugin_id, callback_id)` subscription from `method`.
    ///
    /// A no-op when the subscription is absent. Drops the method's entry from
    /// the map once its last subscriber is gone, so the map does not accumulate
    /// empty vecs.
    pub(crate) fn unsubscribe(&self, method: &str, plugin_id: &PluginId, callback_id: &str) {
        let mut map = self.lock();
        if let Some(entries) = map.get_mut(method) {
            entries.retain(|s| !(s.plugin_id == *plugin_id && s.callback_id == callback_id));
            if entries.is_empty() {
                map.remove(method);
            }
        }
    }

    /// Remove every subscription belonging to `plugin_id`, across all methods.
    ///
    /// Called on the host's unload path so a torn-down plugin's callbacks are no
    /// longer targeted by the pump.
    pub(crate) fn remove_plugin(&self, plugin_id: &PluginId) {
        let mut map = self.lock();
        map.retain(|_, entries| {
            entries.retain(|s| s.plugin_id != *plugin_id);
            !entries.is_empty()
        });
    }

    /// The `(plugin_id, callback_id)` pairs subscribed to `method`.
    ///
    /// Returns owned clones so the caller (the event pump) can drop the lock
    /// before awaiting any callback invocation.
    pub(crate) fn subscribers(&self, method: &str) -> Vec<(PluginId, String)> {
        self.lock()
            .get(method)
            .map(|entries| {
                entries
                    .iter()
                    .map(|s| (s.plugin_id.clone(), s.callback_id.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Lock the inner map, panicking on poison consistent with the host mutex.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Vec<Subscription>>> {
        self.inner
            .lock()
            .expect("event subscriptions registry poisoned")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A subscribe then a `subscribers` lookup returns exactly that subscriber.
    #[test]
    fn subscribe_then_lookup_returns_the_subscriber() {
        let subs = EventSubscriptions::new();
        subs.subscribe(
            "notifications/commands/executed".to_string(),
            PluginId::new("plugin-1"),
            "cb_1".to_string(),
        );

        assert_eq!(
            subs.subscribers("notifications/commands/executed"),
            vec![(PluginId::new("plugin-1"), "cb_1".to_string())]
        );
        // An unrelated method has no subscribers.
        assert!(subs.subscribers("notifications/store/changed").is_empty());
    }

    /// Two distinct callbacks on one method both deliver; the same callback
    /// subscribed twice is deduplicated.
    #[test]
    fn distinct_callbacks_accumulate_but_duplicates_are_deduped() {
        let subs = EventSubscriptions::new();
        let method = "notifications/commands/executed";
        subs.subscribe(method.to_string(), PluginId::new("p"), "cb_1".to_string());
        subs.subscribe(method.to_string(), PluginId::new("p"), "cb_2".to_string());
        // Exact duplicate — must not double up.
        subs.subscribe(method.to_string(), PluginId::new("p"), "cb_1".to_string());

        let mut got = subs.subscribers(method);
        got.sort_by(|a, b| a.1.cmp(&b.1));
        assert_eq!(
            got,
            vec![
                (PluginId::new("p"), "cb_1".to_string()),
                (PluginId::new("p"), "cb_2".to_string()),
            ]
        );
    }

    /// Unsubscribe removes only the named callback and prunes the method entry
    /// once empty.
    #[test]
    fn unsubscribe_removes_only_the_named_callback() {
        let subs = EventSubscriptions::new();
        let method = "notifications/commands/executed";
        subs.subscribe(method.to_string(), PluginId::new("p"), "cb_1".to_string());
        subs.subscribe(method.to_string(), PluginId::new("p"), "cb_2".to_string());

        subs.unsubscribe(method, &PluginId::new("p"), "cb_1");
        assert_eq!(
            subs.subscribers(method),
            vec![(PluginId::new("p"), "cb_2".to_string())]
        );

        // Removing the last one prunes the method entry entirely.
        subs.unsubscribe(method, &PluginId::new("p"), "cb_2");
        assert!(subs.subscribers(method).is_empty());

        // Unsubscribing an absent pair is a no-op, not a panic.
        subs.unsubscribe(method, &PluginId::new("p"), "cb_absent");
    }

    /// `remove_plugin` drops every subscription of one plugin across methods,
    /// leaving other plugins' subscriptions intact.
    #[test]
    fn remove_plugin_drops_all_of_one_plugins_subscriptions() {
        let subs = EventSubscriptions::new();
        let m1 = "notifications/commands/executed";
        let m2 = "notifications/store/changed";
        subs.subscribe(m1.to_string(), PluginId::new("a"), "cb_a1".to_string());
        subs.subscribe(m2.to_string(), PluginId::new("a"), "cb_a2".to_string());
        subs.subscribe(m1.to_string(), PluginId::new("b"), "cb_b1".to_string());

        subs.remove_plugin(&PluginId::new("a"));

        // Plugin `a` is gone from every method; `b` remains.
        assert_eq!(
            subs.subscribers(m1),
            vec![(PluginId::new("b"), "cb_b1".to_string())]
        );
        assert!(subs.subscribers(m2).is_empty());
    }
}
