//! The MCP server abstraction and its transports.
//!
//! Defines the [`McpServer`] trait the platform dispatches against, the
//! [`ToolMetadata`] description it advertises, and the [`CallerId`] that
//! identifies who issued a request. The concrete transports
//! (`InProcessServer`, `CliServer`, `UrlServer`) that implement this trait
//! are filled in by later tasks.

use async_trait::async_trait;
use rmcp::model::Tool;
use serde_json::Value;

use crate::error::Result;

/// A registered MCP server the platform can dispatch work to.
///
/// Every transport — an in-process server backed by a JavaScript plugin, a
/// child process speaking MCP over stdio, a remote server reached over HTTP —
/// implements this single trait. The platform itself only ever sees an
/// `McpServer`; it does not care which transport carries the traffic.
///
/// The trait is `Send + Sync` so that an `Arc<dyn McpServer>` can be shared
/// across the platform's async tasks and held in the [`ServerRegistry`].
///
/// [`ServerRegistry`]: crate::registry::ServerRegistry
#[async_trait]
pub trait McpServer: Send + Sync {
    /// Lists the tools this server exposes, as an MCP `tools/list` would.
    ///
    /// # Returns
    ///
    /// One [`ToolMetadata`] per tool. The list reflects the server's current
    /// tool set; a server whose tools change over its lifetime returns the
    /// up-to-date set on each call.
    fn tools(&self) -> Vec<ToolMetadata>;

    /// Invokes a tool on this server, exactly as an MCP `tools/call` request.
    ///
    /// This is a plain `tools/call`: the platform passes `input` straight
    /// through to the server without inspecting it. In particular, the
    /// platform never reads `input` to make routing decisions — when an
    /// `op` key is present inside `input`, it is just an ordinary argument
    /// that the tool's own handler parses. Operation routing is the tool's
    /// concern, not the platform's.
    ///
    /// # Parameters
    ///
    /// - `caller` — identifies who issued the request, for the server's
    ///   bookkeeping and access decisions.
    /// - `tool` — the name of the tool to invoke, matching a
    ///   [`ToolMetadata::name`] returned by [`tools`](McpServer::tools).
    /// - `input` — the `tools/call` arguments, forwarded verbatim.
    ///
    /// # Returns
    ///
    /// The tool's result payload as a JSON value on success.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`](crate::error::Error) when the named tool does not
    /// exist, the server is unavailable, the backing plugin was reloaded
    /// mid-call, or the tool's handler itself reports a failure.
    async fn invoke(&self, caller: CallerId, tool: &str, input: Value) -> Result<Value>;
}

/// A tool's definition as it appears in an MCP `tools/list` response.
///
/// This is the platform's view of a single tool: its `name`, optional
/// `description`, `inputSchema`, and optional `_meta`. Operation tools carry
/// their `io.swissarmyhammer/operations` tree under `_meta`.
///
/// `ToolMetadata` wraps [`rmcp::model::Tool`] directly rather than mirroring
/// its fields. `rmcp::model::Tool` already models the exact wire shape of an
/// MCP tool (`name`, `description`, `inputSchema`, `_meta`, and more), and it
/// is `#[non_exhaustive]` — so a transport such as a future `InProcessServer`,
/// which already holds `rmcp` `Tool` values, can produce a `ToolMetadata` with
/// a single cheap move instead of copying field by field. Wrapping in a
/// newtype keeps the platform's public surface stable and gives the platform
/// a place to hang its own focused accessors.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolMetadata {
    /// The underlying `rmcp` tool definition.
    tool: Tool,
}

impl ToolMetadata {
    /// Wraps an [`rmcp::model::Tool`] as platform [`ToolMetadata`].
    ///
    /// # Parameters
    ///
    /// - `tool` — the `rmcp` tool definition to expose through the platform.
    pub fn new(tool: Tool) -> Self {
        Self { tool }
    }

    /// Returns the tool's name, as used to address it in a `tools/call`.
    pub fn name(&self) -> &str {
        &self.tool.name
    }

    /// Returns the tool's human-readable description, if it declares one.
    pub fn description(&self) -> Option<&str> {
        self.tool.description.as_deref()
    }

    /// Borrows the underlying [`rmcp::model::Tool`].
    ///
    /// This exposes the full tool definition — `inputSchema`, `_meta`, and
    /// every other `rmcp` field — for callers that need the complete shape.
    pub fn as_tool(&self) -> &Tool {
        &self.tool
    }

    /// Consumes this `ToolMetadata`, returning the wrapped [`rmcp::model::Tool`].
    pub fn into_tool(self) -> Tool {
        self.tool
    }
}

impl From<Tool> for ToolMetadata {
    /// Wraps an [`rmcp::model::Tool`] as [`ToolMetadata`]; see [`ToolMetadata::new`].
    fn from(tool: Tool) -> Self {
        Self::new(tool)
    }
}

/// Identifier for a plugin within the platform.
///
/// A newtype over `String`: the platform assigns each loaded plugin a stable
/// identifier, and [`CallerId::Plugin`] uses it to attribute requests that
/// originate from one plugin calling another.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginId(pub String);

impl PluginId {
    /// Creates a [`PluginId`] from anything that converts into a `String`.
    ///
    /// # Parameters
    ///
    /// - `id` — the plugin identifier.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Identifies who issued a request to an [`McpServer`].
///
/// A server uses the caller identity for bookkeeping and access decisions.
/// This is a placeholder: the dispatcher task refines how callers are
/// resolved and used, but the [`McpServer::invoke`] signature needs the type
/// now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallerId {
    /// The request originates from the platform host itself.
    HostInternal,
    /// The request originates from a loaded plugin, identified by its [`PluginId`].
    Plugin(PluginId),
    /// The request originates from an external client; the string carries
    /// whatever identity that client presented.
    External(String),
    /// The caller could not be identified.
    Unknown,
}
