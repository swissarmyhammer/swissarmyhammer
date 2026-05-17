//! The per-plugin registration ledger.
//!
//! When a plugin runs, it registers servers, installs callbacks, and otherwise
//! hands the platform long-lived state. The platform must be able to reclaim
//! every piece of that state on unload *without the plugin's cooperation* — a
//! plugin cannot be trusted to dispose what it created, and an unloaded plugin
//! is no longer running to be asked.
//!
//! The [`PluginLedger`] solves this by recording one [`RegistrationHandle`] for
//! every long-lived registration a plugin makes, keyed by the plugin's
//! [`PluginId`]. The handles are appended in registration order. On unload the
//! host drains the plugin's vec **in reverse** — last registration disposed
//! first — so disposal unwinds the registrations the way a stack unwinds
//! scopes.
//!
//! This module owns only the bookkeeping. *Acting* on a drained handle —
//! unregistering a server from the [`ServerRegistry`], dropping a callback,
//! running an opaque dispose function — is the host's job; see
//! [`crate::host`].
//!
//! [`ServerRegistry`]: crate::registry::ServerRegistry

use std::collections::HashMap;
use std::fmt;

use crate::registry::ServerName;
use crate::server::PluginId;

/// Identifier for a host-side callback registered on behalf of a plugin.
///
/// A newtype over `String`. This is the live id type the callback primitive
/// uses: the seam by which a plugin hands the host a function reference. When
/// the SDK marshals a function it mints a `cb_`-prefixed id; the host receives
/// that id as a `$callback` marker and [`crate::host`]'s `callback_dispatch`
/// records it as a [`RegistrationHandle::Callback`] so unload can dispose the
/// stored function.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallbackId(pub String);

impl CallbackId {
    /// Creates a [`CallbackId`] from anything that converts into a `String`.
    ///
    /// # Parameters
    ///
    /// - `id` — the callback identifier.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One long-lived registration a plugin made, recorded so unload can undo it.
///
/// Every variant names a distinct kind of platform state a plugin can acquire.
/// The host disposes each variant differently — see [`crate::host`] — but the
/// ledger only needs to *carry* enough to identify what to dispose.
pub enum RegistrationHandle {
    /// A server the plugin registered with the [`ServerRegistry`], identified
    /// by the name it was registered under. Disposed by unregistering it.
    ///
    /// [`ServerRegistry`]: crate::registry::ServerRegistry
    Server(ServerName),

    /// A host-side callback registered on the plugin's behalf, identified by
    /// its [`CallbackId`]. Disposed by dropping the stored function.
    ///
    /// This variant is live: [`crate::host`]'s `callback_dispatch` produces one
    /// on every `callbackDispatch` envelope, recording one handle per
    /// `$callback` marker the SDK marshalled.
    Callback(CallbackId),

    /// An arbitrary disposable: a boxed closure run once at unload time.
    ///
    /// This is the escape hatch for any long-lived resource that is neither a
    /// registered server nor a callback. Disposed by calling the closure.
    Opaque(Box<dyn FnOnce() + Send>),
}

/// `RegistrationHandle` is written by hand because the [`Opaque`] variant holds
/// a `Box<dyn FnOnce()>`, which is not `Debug`. The impl reports each variant's
/// kind — and, where it has one, its identifying name — without requiring a
/// `Debug` bound on the boxed closure.
///
/// [`Opaque`]: RegistrationHandle::Opaque
impl fmt::Debug for RegistrationHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Server(name) => f.debug_tuple("Server").field(name).finish(),
            Self::Callback(id) => f.debug_tuple("Callback").field(id).finish(),
            Self::Opaque(_) => f.debug_tuple("Opaque").field(&"<dispose-fn>").finish(),
        }
    }
}

/// The per-plugin registration ledger.
///
/// A map from a loaded plugin's [`PluginId`] to the ordered list of
/// [`RegistrationHandle`]s it has accumulated. The host appends a handle each
/// time the plugin makes a long-lived registration and drains the whole vec —
/// in reverse — when the plugin is unloaded.
#[derive(Default)]
pub struct PluginLedger {
    /// The handles each loaded plugin has accumulated, in registration order.
    entries: HashMap<PluginId, Vec<RegistrationHandle>>,
}

/// `PluginLedger` is written by hand because [`RegistrationHandle`]'s `Debug`
/// is itself hand-written (its `Opaque` variant blocks a derive); the impl
/// reports, per plugin, how many handles are recorded.
impl fmt::Debug for PluginLedger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let counts: HashMap<&PluginId, usize> = self
            .entries
            .iter()
            .map(|(id, handles)| (id, handles.len()))
            .collect();
        f.debug_struct("PluginLedger")
            .field("handle_counts", &counts)
            .finish()
    }
}

impl PluginLedger {
    /// Creates an empty ledger with no plugins tracked.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts tracking `plugin` with an empty handle list.
    ///
    /// Called once when a plugin is loaded, before it can register anything.
    /// Re-tracking a plugin already present resets its handle list to empty;
    /// the host never does this, but the operation is total rather than
    /// panicking.
    ///
    /// # Parameters
    ///
    /// - `plugin` — the id of the newly loaded plugin.
    pub fn track(&mut self, plugin: PluginId) {
        self.entries.entry(plugin).or_default();
    }

    /// Appends `handle` to `plugin`'s ordered list of registrations.
    ///
    /// The plugin must already be tracked via [`track`](Self::track); a handle
    /// recorded for an untracked plugin is dropped and `false` is returned, so
    /// a stray append cannot silently create an orphan ledger entry.
    ///
    /// # Parameters
    ///
    /// - `plugin` — the id of the plugin that made the registration.
    /// - `handle` — the handle describing what was registered.
    ///
    /// # Returns
    ///
    /// `true` when the handle was appended; `false` when `plugin` is not
    /// tracked and the handle was therefore discarded.
    pub fn record(&mut self, plugin: &PluginId, handle: RegistrationHandle) -> bool {
        match self.entries.get_mut(plugin) {
            Some(handles) => {
                handles.push(handle);
                true
            }
            None => false,
        }
    }

    /// Removes a server handle matching `name` from `plugin`'s list.
    ///
    /// A plugin's `unregister` frees a server it earlier registered; the
    /// matching [`RegistrationHandle::Server`] is consumed from the ledger so a
    /// later [`drain`](Self::drain) does not dispose it a second time. Only the
    /// first match is removed, mirroring the registry's single-name namespace.
    ///
    /// # Parameters
    ///
    /// - `plugin` — the id of the plugin that unregistered the server.
    /// - `name` — the registry name the server was registered under.
    ///
    /// # Returns
    ///
    /// `true` when a matching server handle was found and removed; `false`
    /// otherwise.
    pub fn consume_server(&mut self, plugin: &PluginId, name: &str) -> bool {
        let Some(handles) = self.entries.get_mut(plugin) else {
            return false;
        };
        let position = handles
            .iter()
            .position(|handle| matches!(handle, RegistrationHandle::Server(n) if n == name));
        match position {
            Some(index) => {
                handles.remove(index);
                true
            }
            None => false,
        }
    }

    /// Stops tracking `plugin` and returns its handles in disposal order.
    ///
    /// The handles are returned **reversed** — last registered first — so the
    /// caller disposes them the way a stack unwinds: a registration made later
    /// is undone before one it may have depended on. After this call `plugin`
    /// is no longer tracked.
    ///
    /// # Parameters
    ///
    /// - `plugin` — the id of the plugin being unloaded.
    ///
    /// # Returns
    ///
    /// `Some` vec of handles in disposal (reverse-registration) order when
    /// `plugin` was tracked; `None` when it was not.
    pub fn drain(&mut self, plugin: &PluginId) -> Option<Vec<RegistrationHandle>> {
        let mut handles = self.entries.remove(plugin)?;
        handles.reverse();
        Some(handles)
    }

    /// Returns the number of handles currently recorded for `plugin`.
    ///
    /// `None` when `plugin` is not tracked — distinguishing an unloaded plugin
    /// from a loaded plugin that has registered nothing (which returns
    /// `Some(0)`).
    ///
    /// # Parameters
    ///
    /// - `plugin` — the id of the plugin to inspect.
    pub fn len(&self, plugin: &PluginId) -> Option<usize> {
        self.entries.get(plugin).map(Vec::len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tracked plugin's recorded handles drain in reverse registration order.
    #[test]
    fn drain_returns_handles_in_reverse_registration_order() {
        let mut ledger = PluginLedger::new();
        let plugin = PluginId::new("p");
        ledger.track(plugin.clone());

        ledger.record(&plugin, RegistrationHandle::Server("first".to_string()));
        ledger.record(&plugin, RegistrationHandle::Server("second".to_string()));
        ledger.record(&plugin, RegistrationHandle::Server("third".to_string()));

        let drained = ledger.drain(&plugin).expect("a tracked plugin drains");
        let names: Vec<String> = drained
            .into_iter()
            .map(|handle| match handle {
                RegistrationHandle::Server(name) => name,
                other => panic!("expected a Server handle, got {other:?}"),
            })
            .collect();
        assert_eq!(
            names,
            vec!["third", "second", "first"],
            "drain must unwind the registrations last-to-first"
        );
    }

    /// Draining a plugin removes it from the ledger entirely.
    #[test]
    fn drain_stops_tracking_the_plugin() {
        let mut ledger = PluginLedger::new();
        let plugin = PluginId::new("p");
        ledger.track(plugin.clone());
        ledger.record(&plugin, RegistrationHandle::Server("s".to_string()));

        assert_eq!(ledger.len(&plugin), Some(1));
        ledger.drain(&plugin).expect("a tracked plugin drains");
        assert_eq!(
            ledger.len(&plugin),
            None,
            "a drained plugin must no longer be tracked"
        );
    }

    /// `consume_server` removes the matching server handle so it is not
    /// disposed twice.
    #[test]
    fn consume_server_removes_only_the_matching_handle() {
        let mut ledger = PluginLedger::new();
        let plugin = PluginId::new("p");
        ledger.track(plugin.clone());
        ledger.record(&plugin, RegistrationHandle::Server("keep".to_string()));
        ledger.record(&plugin, RegistrationHandle::Server("drop".to_string()));

        assert!(
            ledger.consume_server(&plugin, "drop"),
            "consuming a present server handle should report success"
        );
        assert_eq!(ledger.len(&plugin), Some(1));
        assert!(
            !ledger.consume_server(&plugin, "drop"),
            "consuming an already-consumed handle should report failure"
        );

        let drained = ledger.drain(&plugin).expect("a tracked plugin drains");
        assert!(
            matches!(drained.as_slice(), [RegistrationHandle::Server(name)] if name == "keep"),
            "only the un-consumed handle should remain, got {drained:?}"
        );
    }

    /// Recording a handle for an untracked plugin is rejected, not silently
    /// orphaned.
    #[test]
    fn record_for_an_untracked_plugin_is_rejected() {
        let mut ledger = PluginLedger::new();
        let plugin = PluginId::new("ghost");
        assert!(
            !ledger.record(&plugin, RegistrationHandle::Server("s".to_string())),
            "recording against an untracked plugin must fail"
        );
        assert_eq!(ledger.len(&plugin), None);
    }

    /// An `Opaque` handle's closure is the caller's to run; the ledger only
    /// carries it. This confirms the variant round-trips through a drain.
    #[test]
    fn opaque_handle_round_trips_through_drain() {
        let mut ledger = PluginLedger::new();
        let plugin = PluginId::new("p");
        ledger.track(plugin.clone());
        ledger.record(
            &plugin,
            RegistrationHandle::Opaque(Box::new(|| { /* dispose */ })),
        );

        let drained = ledger.drain(&plugin).expect("a tracked plugin drains");
        assert!(
            matches!(drained.as_slice(), [RegistrationHandle::Opaque(_)]),
            "an Opaque handle must survive a drain, got {drained:?}"
        );
    }
}
