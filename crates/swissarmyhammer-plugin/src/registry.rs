//! Registry of MCP servers registered with the plugin platform.
//!
//! Tracks the set of registered [`McpServer`]s by unique name. The platform
//! has a single global server namespace: the first registration of a name
//! wins, and a later attempt to reuse that name is rejected until the name
//! is freed by [`ServerRegistry::unregister`].
//!
//! Unregistering a name does not erase it: the registry keeps a *tombstone* —
//! a record that the name was once live and has since been disposed. A call
//! that resolves a tombstoned name learns the server was disposed out from
//! under it ([`ServerStatus::Disposed`]) rather than that the name never
//! existed ([`ServerStatus::Unknown`]). Re-registering the name clears its
//! tombstone.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use crate::error::{Error, Result};
use crate::server::McpServer;

/// The unique name an [`McpServer`] is registered under.
///
/// An alias for `String`: server names live in a single flat namespace, so a
/// plain string is the natural key. The alias names the role the string plays
/// throughout the registry API.
pub type ServerName = String;

/// The outcome of resolving a name against the [`ServerRegistry`].
///
/// Resolution distinguishes three cases so a caller can return the accurate
/// error: a name that resolves to a [`Live`](Self::Live) server, a name whose
/// server was registered and later [`Disposed`](Self::Disposed) — the
/// tombstone case — and a name the registry has never seen
/// ([`Unknown`](Self::Unknown)).
pub enum ServerStatus {
    /// The name resolves to a live, callable server.
    Live(Arc<dyn McpServer>),

    /// The name was registered and has since been unregistered. A consumer
    /// holding this name learns its server was disposed out from under it.
    Disposed,

    /// The registry has no record of the name — it was never registered.
    Unknown,
}

/// Tracks the MCP servers registered with the platform, keyed by name.
///
/// The registry owns the shared handles to every registered server. Callers
/// register a server under a name, look it up by name to dispatch work, and
/// unregister it when the backing plugin goes away.
///
/// Unregistering leaves a tombstone in [`disposed`](Self::disposed) so a later
/// resolution of the freed name reports [`ServerStatus::Disposed`] rather than
/// [`ServerStatus::Unknown`]. Re-registering the name clears its tombstone.
#[derive(Default)]
pub struct ServerRegistry {
    /// The registered servers, keyed by their unique [`ServerName`].
    servers: HashMap<ServerName, Arc<dyn McpServer>>,

    /// Tombstones: names that were registered and have since been
    /// unregistered. A name is in exactly one of `servers` or `disposed` —
    /// registering moves it into `servers` and clears any tombstone;
    /// unregistering moves it the other way.
    disposed: HashSet<ServerName>,
}

/// `Debug` is written by hand because the registered server values are
/// `Arc<dyn McpServer>`, and the [`McpServer`] trait deliberately carries no
/// `Debug` supertrait bound — adding one would burden every transport impl.
/// The trait objects are therefore not printable, so this impl reports the
/// registered server names instead, which is the registry's meaningful state.
impl fmt::Debug for ServerRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerRegistry")
            .field("servers", &self.servers.keys())
            .field("disposed", &self.disposed)
            .finish()
    }
}

impl ServerRegistry {
    /// Creates an empty registry with no servers registered.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `server` under `name`.
    ///
    /// The platform has a single global namespace and first registration
    /// wins: a name can be held by exactly one server at a time.
    ///
    /// # Parameters
    ///
    /// - `name` — the unique name to register the server under.
    /// - `server` — a shared handle to the server being registered.
    ///
    /// # Returns
    ///
    /// `Ok(())` when `name` was free and the server is now registered.
    /// Registering a name that carried a tombstone succeeds and clears that
    /// tombstone — the name is live again.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServerNameTaken`] — carrying `name` — when a server
    /// is already registered under that name. The existing registration is
    /// left untouched.
    pub fn register(&mut self, name: ServerName, server: Arc<dyn McpServer>) -> Result<()> {
        match self.servers.entry(name) {
            Entry::Vacant(slot) => {
                // A re-registration revives the name: clear any tombstone so
                // it resolves as live rather than disposed.
                self.disposed.remove(slot.key());
                slot.insert(server);
                Ok(())
            }
            Entry::Occupied(slot) => Err(Error::ServerNameTaken(slot.key().clone())),
        }
    }

    /// Removes the server registered under `name`, leaving a tombstone.
    ///
    /// The name is freed for re-registration, but the registry remembers it
    /// was once live: a later [`resolve`](Self::resolve) of the freed name
    /// reports [`ServerStatus::Disposed`] until the name is registered again.
    ///
    /// # Parameters
    ///
    /// - `name` — the name of the server to unregister.
    ///
    /// # Returns
    ///
    /// `Some` shared handle to the removed server when `name` was registered,
    /// freeing the name for reuse; `None` when no server held that name.
    pub fn unregister(&mut self, name: &str) -> Option<Arc<dyn McpServer>> {
        let removed = self.servers.remove(name);
        if removed.is_some() {
            // Leave a tombstone only for a name that was actually live, so a
            // never-registered name keeps resolving as `Unknown`.
            self.disposed.insert(name.to_string());
        }
        removed
    }

    /// Looks up the server registered under `name`.
    ///
    /// # Parameters
    ///
    /// - `name` — the name of the server to look up.
    ///
    /// # Returns
    ///
    /// `Some` clone of the shared handle when a server is registered under
    /// `name`; `None` otherwise. This does not distinguish a disposed name
    /// from one that never existed — use [`resolve`](Self::resolve) when that
    /// distinction matters.
    pub fn get(&self, name: &str) -> Option<Arc<dyn McpServer>> {
        self.servers.get(name).cloned()
    }

    /// Resolves `name` to its registration status.
    ///
    /// Unlike [`get`](Self::get), this distinguishes a live server from a name
    /// whose server was disposed (tombstoned) and from a name the registry has
    /// never seen — so a router can return [`Error::ServerUnavailable`] for the
    /// disposed case and [`Error::UnknownServer`] only for the never-registered
    /// case.
    ///
    /// # Parameters
    ///
    /// - `name` — the name of the server to resolve.
    ///
    /// # Returns
    ///
    /// [`ServerStatus::Live`] carrying the server handle when `name` is live,
    /// [`ServerStatus::Disposed`] when `name` carries a tombstone, and
    /// [`ServerStatus::Unknown`] when the registry has no record of `name`.
    pub fn resolve(&self, name: &str) -> ServerStatus {
        if let Some(server) = self.servers.get(name) {
            return ServerStatus::Live(server.clone());
        }
        if self.disposed.contains(name) {
            return ServerStatus::Disposed;
        }
        ServerStatus::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{CallerId, ToolMetadata};
    use async_trait::async_trait;
    use rmcp::model::Tool;
    use serde_json::{Map, Value};

    /// A trivial [`McpServer`] used to exercise the registry.
    ///
    /// It advertises a single fixed tool and its `invoke` echoes the input
    /// straight back, so registry tests can register and dispatch against a
    /// real trait object without a transport.
    struct FakeServer {
        tool_name: &'static str,
    }

    #[async_trait]
    impl McpServer for FakeServer {
        fn tools(&self) -> Vec<ToolMetadata> {
            let schema = Map::new();
            vec![ToolMetadata::new(Tool::new(
                self.tool_name,
                "a fixed fake tool",
                schema,
            ))]
        }

        async fn invoke(&self, _caller: CallerId, _tool: &str, input: Value) -> Result<Value> {
            Ok(input)
        }
    }

    #[test]
    fn register_two_distinct_names_succeeds() {
        let mut registry = ServerRegistry::new();

        let first: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "alpha" });
        let second: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "beta" });

        assert!(registry.register("alpha".to_string(), first).is_ok());
        assert!(registry.register("beta".to_string(), second).is_ok());

        assert!(registry.get("alpha").is_some());
        assert!(registry.get("beta").is_some());
    }

    #[test]
    fn register_taken_name_errors_with_server_name_taken() {
        let mut registry = ServerRegistry::new();

        let first: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "alpha" });
        let duplicate: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "other" });

        registry
            .register("alpha".to_string(), first)
            .expect("first registration of a fresh name should succeed");

        let err = registry
            .register("alpha".to_string(), duplicate)
            .expect_err("registering an already-taken name should fail");

        match err {
            Error::ServerNameTaken(name) => assert_eq!(name, "alpha"),
            other => panic!("expected ServerNameTaken, got {other:?}"),
        }
    }

    #[test]
    fn unregister_then_get_yields_none() {
        let mut registry = ServerRegistry::new();

        let server: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "alpha" });
        registry
            .register("alpha".to_string(), server)
            .expect("registration of a fresh name should succeed");

        let removed = registry.unregister("alpha");
        assert!(removed.is_some(), "unregister should return the server");

        assert!(
            registry.get("alpha").is_none(),
            "get should yield None after unregister"
        );
    }

    #[test]
    fn resolve_distinguishes_live_disposed_and_unknown() {
        let mut registry = ServerRegistry::new();

        // A name the registry has never seen resolves as Unknown.
        assert!(
            matches!(registry.resolve("never"), ServerStatus::Unknown),
            "a never-registered name must resolve as Unknown"
        );

        let server: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "alpha" });
        registry
            .register("alpha".to_string(), server)
            .expect("registration of a fresh name should succeed");

        // A live registration resolves to a callable server.
        assert!(
            matches!(registry.resolve("alpha"), ServerStatus::Live(_)),
            "a registered name must resolve as Live"
        );

        registry.unregister("alpha");

        // After unregister the name carries a tombstone: Disposed, not Unknown.
        assert!(
            matches!(registry.resolve("alpha"), ServerStatus::Disposed),
            "an unregistered name must resolve as Disposed, not Unknown"
        );
    }

    #[test]
    fn re_registering_a_disposed_name_clears_its_tombstone() {
        let mut registry = ServerRegistry::new();

        let first: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "alpha" });
        registry
            .register("alpha".to_string(), first)
            .expect("registration of a fresh name should succeed");
        registry.unregister("alpha");
        assert!(
            matches!(registry.resolve("alpha"), ServerStatus::Disposed),
            "the name should be tombstoned after unregister"
        );

        let second: Arc<dyn McpServer> = Arc::new(FakeServer { tool_name: "beta" });
        registry
            .register("alpha".to_string(), second)
            .expect("re-registering a tombstoned name should succeed");
        assert!(
            matches!(registry.resolve("alpha"), ServerStatus::Live(_)),
            "re-registration must clear the tombstone and resolve as Live"
        );
    }

    #[test]
    fn unregister_of_a_never_registered_name_leaves_no_tombstone() {
        let mut registry = ServerRegistry::new();

        let removed = registry.unregister("ghost");
        assert!(
            removed.is_none(),
            "unregister of an unknown name returns None"
        );
        assert!(
            matches!(registry.resolve("ghost"), ServerStatus::Unknown),
            "a name that was never live must not gain a tombstone"
        );
    }
}
