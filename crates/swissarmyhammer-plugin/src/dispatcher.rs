//! The single dispatcher every call flows through.
//!
//! Every call within the platform — host-to-host, plugin-to-host,
//! plugin-to-plugin, agent-to-host — is routed by one [`Dispatcher`]. The
//! dispatcher resolves a server name against the [`ServerRegistry`] and
//! forwards the call to that server's [`McpServer::invoke`].

use std::sync::Arc;

use serde_json::Value;

use crate::error::{Error, Result};
use crate::registry::ServerRegistry;
use crate::server::CallerId;

/// The single dispatcher every call within the platform flows through.
///
/// `Dispatcher` is the one routing point for all traffic — host-to-host,
/// plugin-to-host, plugin-to-plugin, and agent-to-host. It holds a shared
/// handle to the [`ServerRegistry`], resolves a request's target server name
/// against it, and forwards the request to that server's
/// [`McpServer::invoke`](crate::server::McpServer::invoke).
///
/// The dispatcher routes purely by `(server, tool)`. It forwards a single
/// arguments map and never inspects it: there is no verb/noun axis in the
/// routing signature. When an `op` key is present it is just an ordinary key
/// inside the arguments map that the target tool's own handler interprets —
/// the platform neither reads nor acts on it.
///
/// The registry handle is an `Arc`, so a `Dispatcher` is cheap to clone and
/// share across the platform's async tasks.
#[derive(Debug, Clone)]
pub struct Dispatcher {
    /// Shared handle to the registry of MCP servers calls are routed against.
    registry: Arc<ServerRegistry>,
}

impl Dispatcher {
    /// Creates a dispatcher that routes calls against `registry`.
    ///
    /// # Parameters
    ///
    /// - `registry` — a shared handle to the registry of MCP servers; every
    ///   call resolves its target server against this registry.
    pub fn new(registry: Arc<ServerRegistry>) -> Self {
        Self { registry }
    }

    /// Routes a call to a registered server and returns the server's result.
    ///
    /// Looks up `server` in the registry and forwards `caller`, `tool`, and
    /// `input` to that server's
    /// [`McpServer::invoke`](crate::server::McpServer::invoke), returning the
    /// server's result unchanged.
    ///
    /// Routing is by `(server, tool)` only. `input` is a single arguments map
    /// forwarded verbatim — the dispatcher never reads it. An `op` key, when
    /// present, is just an ordinary entry in `input` that the target tool's
    /// handler parses; the platform does not interpret it.
    ///
    /// `caller` is threaded through to `invoke` unchanged. The platform does
    /// not gate calls on the caller's identity; it simply carries the identity
    /// to the server for the server's own bookkeeping and access decisions.
    ///
    /// # Parameters
    ///
    /// - `caller` — identifies who issued the request; passed through to the
    ///   target server unchanged.
    /// - `server` — the name of the registered server to route to.
    /// - `tool` — the name of the tool to invoke on that server.
    /// - `input` — the `tools/call` arguments, forwarded verbatim.
    ///
    /// # Returns
    ///
    /// The target tool's result payload as a JSON value on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownServer`] when no server is registered under
    /// `server`. Any error the target server's `invoke` produces — an unknown
    /// tool, an unavailable server, a reloaded plugin, or a handler failure —
    /// is propagated to the caller unchanged.
    pub async fn call(
        &self,
        caller: CallerId,
        server: &str,
        tool: &str,
        input: Value,
    ) -> Result<Value> {
        let target = self.registry.get(server).ok_or(Error::UnknownServer)?;
        target.invoke(caller, tool, input).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use serde_json::{json, Value};

    use crate::error::{Error, Result};
    use crate::registry::ServerRegistry;
    use crate::server::{CallerId, McpServer, PluginId, ToolMetadata};

    use super::Dispatcher;

    /// An [`McpServer`] that records the [`CallerId`] and `tool` of the most
    /// recent `invoke`, so routing tests can assert what reached it.
    ///
    /// `invoke` echoes its `input` straight back, letting a test confirm the
    /// arguments map passed through the dispatcher untouched.
    struct RecordingServer {
        /// The `(caller, tool)` pair from the last `invoke`, if any.
        last_call: Mutex<Option<(CallerId, String)>>,
    }

    impl RecordingServer {
        /// Creates a [`RecordingServer`] that has not yet recorded a call.
        fn new() -> Self {
            Self {
                last_call: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl McpServer for RecordingServer {
        fn tools(&self) -> Vec<ToolMetadata> {
            Vec::new()
        }

        async fn invoke(&self, caller: CallerId, tool: &str, input: Value) -> Result<Value> {
            *self.last_call.lock().expect("recording lock poisoned") =
                Some((caller, tool.to_string()));
            Ok(input)
        }
    }

    #[tokio::test]
    async fn call_forwards_caller_tool_and_input_unchanged() {
        let server = Arc::new(RecordingServer::new());
        let mut registry = ServerRegistry::new();
        registry
            .register("srv".to_string(), server.clone())
            .expect("registering a fresh name should succeed");

        let dispatcher = Dispatcher::new(Arc::new(registry));

        let caller = CallerId::Plugin(PluginId::new("plugin-a"));
        let input = json!({ "op": "x" });
        let result = dispatcher
            .call(caller.clone(), "srv", "t", input.clone())
            .await
            .expect("call to a registered server should succeed");

        assert_eq!(
            result, input,
            "the input map should pass through the dispatcher untouched"
        );

        let recorded = server
            .last_call
            .lock()
            .expect("recording lock poisoned")
            .clone();
        assert_eq!(
            recorded,
            Some((caller, "t".to_string())),
            "the fake server should see the same caller and tool"
        );
    }

    #[test]
    fn dispatcher_is_clone_and_debug() {
        let dispatcher = Dispatcher::new(Arc::new(ServerRegistry::new()));

        let cloned = dispatcher.clone();
        assert!(
            !format!("{cloned:?}").is_empty(),
            "a cloned Dispatcher should be Debug-formattable"
        );
    }

    #[tokio::test]
    async fn call_on_unknown_server_yields_unknown_server() {
        let dispatcher = Dispatcher::new(Arc::new(ServerRegistry::new()));

        let err = dispatcher
            .call(CallerId::HostInternal, "missing", "t", json!({}))
            .await
            .expect_err("call to an unregistered server should fail");

        assert!(
            matches!(err, Error::UnknownServer),
            "expected UnknownServer, got {err:?}"
        );
    }
}
