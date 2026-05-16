//! Registry of MCP servers registered with the plugin platform.
//!
//! Tracks the set of registered [`McpServer`]s by unique name. The platform
//! has a single global server namespace: the first registration of a name
//! wins, and a later attempt to reuse that name is rejected until the name
//! is freed by [`ServerRegistry::unregister`].

use std::collections::hash_map::Entry;
use std::collections::HashMap;
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

/// Tracks the MCP servers registered with the platform, keyed by name.
///
/// The registry owns the shared handles to every registered server. Callers
/// register a server under a name, look it up by name to dispatch work, and
/// unregister it when the backing plugin goes away.
#[derive(Default)]
pub struct ServerRegistry {
    /// The registered servers, keyed by their unique [`ServerName`].
    servers: HashMap<ServerName, Arc<dyn McpServer>>,
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
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServerNameTaken`] — carrying `name` — when a server
    /// is already registered under that name. The existing registration is
    /// left untouched.
    pub fn register(&mut self, name: ServerName, server: Arc<dyn McpServer>) -> Result<()> {
        match self.servers.entry(name) {
            Entry::Vacant(slot) => {
                slot.insert(server);
                Ok(())
            }
            Entry::Occupied(slot) => Err(Error::ServerNameTaken(slot.key().clone())),
        }
    }

    /// Removes the server registered under `name`.
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
        self.servers.remove(name)
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
    /// `name`; `None` otherwise.
    pub fn get(&self, name: &str) -> Option<Arc<dyn McpServer>> {
        self.servers.get(name).cloned()
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
}
