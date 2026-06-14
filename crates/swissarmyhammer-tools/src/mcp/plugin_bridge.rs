//! Exposing the in-process MCP tools to the plugin platform.
//!
//! The in-process tools keep their [`McpTool`]/[`ToolRegistry`] home; this
//! module is the thin glue that lets the plugin platform reach them. Each tool
//! is wrapped in a [`ToolModuleServer`], which implements the platform's
//! [`McpServer`] contract directly, and is handed to a
//! [`PluginHost`](swissarmyhammer_plugin::PluginHost) under the tool's name via
//! `expose_rust_module`.
//!
//! # Why a direct `McpServer` impl
//!
//! The platform's [`InProcessServer`](swissarmyhammer_plugin::InProcessServer)
//! wraps an `rmcp::ServerHandler`. An [`McpTool`] is *not* an
//! `rmcp::ServerHandler`: its [`execute`](McpTool::execute) takes a
//! [`ToolContext`] — the in-process tools' own state bundle — rather than an
//! `rmcp::service::RequestContext`. Routing an `McpTool` through a synthetic
//! `ServerHandler` would mean reconstructing a `ToolContext` from request
//! extensions on every call, which `rmcp` has no seam for. Implementing the
//! platform's [`McpServer`] trait directly is the clean path: `tools()` and
//! `invoke()` map one-to-one onto what an `McpTool` already exposes, and the
//! result is still a valid `Arc<dyn McpServer>` reachable through the
//! platform's `Dispatcher` — exactly what `expose_rust_module` requires.

use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::{Meta, Tool};
use serde_json::Value;
use swissarmyhammer_plugin::{
    CallerId, Error as PluginError, McpServer as PluginMcpServer, ToolMetadata,
};
use tokio::sync::RwLock;

use super::tool_registry::{McpTool, ToolContext, ToolRegistry};

/// The `_meta` key under which an operation tool publishes its discovery tree.
///
/// This is the same well-known key the `operation_tool!` macro writes; the
/// constant is shared so the bridge attaches `_meta` under the identical name.
const OPERATIONS_META_KEY: &str = "io.swissarmyhammer/operations";

/// A platform [`McpServer`](PluginMcpServer) backed by a single in-process
/// [`McpTool`].
///
/// One `ToolModuleServer` exposes exactly one tool: the platform addresses it
/// by the module id it was exposed under, and a `tools/call` for that tool
/// routes straight into the tool's [`McpTool::execute`]. The server keeps a
/// shared handle to the [`ToolRegistry`] the tool lives in and to the
/// [`ToolContext`] the tool needs, so a call resolves the live tool and runs
/// it with no copy of the tool itself.
///
/// The tool's published definition — name, description, `inputSchema`, and the
/// `io.swissarmyhammer/operations` `_meta` for operation tools — is built once
/// at construction and cached, because the platform's
/// [`tools`](PluginMcpServer::tools) is synchronous.
pub struct ToolModuleServer {
    /// The name of the wrapped tool, used to resolve it in the registry.
    tool_name: String,
    /// The registry the wrapped tool is registered in.
    tool_registry: Arc<RwLock<ToolRegistry>>,
    /// The context every tool execution is threaded through.
    tool_context: Arc<ToolContext>,
    /// The wrapped tool's published definition, enumerated once at construction.
    tool_metadata: ToolMetadata,
}

impl ToolModuleServer {
    /// Builds a module server for one tool already registered in `registry`.
    ///
    /// The tool's definition is snapshotted now: its schema becomes the
    /// platform `inputSchema`, and an operation tool — one whose
    /// [`McpTool::operations`] is non-empty — additionally gets the
    /// `io.swissarmyhammer/operations` `_meta` tree attached, generated from
    /// the very same operation slice the tool's schema is derived from.
    ///
    /// # Parameters
    ///
    /// - `tool` — the tool to wrap; only its definition is read here.
    /// - `tool_registry` — the registry the tool is registered in; consulted
    ///   on every call to resolve the live tool.
    /// - `tool_context` — the context every execution of the tool is threaded
    ///   through.
    fn new(
        tool: &dyn McpTool,
        tool_registry: Arc<RwLock<ToolRegistry>>,
        tool_context: Arc<ToolContext>,
    ) -> Self {
        Self {
            tool_name: McpTool::name(tool).to_string(),
            tool_registry,
            tool_context,
            tool_metadata: ToolMetadata::new(build_tool_definition(tool)),
        }
    }
}

/// Builds the platform [`Tool`] definition published for an [`McpTool`].
///
/// The flat wire schema comes from [`McpTool::schema`] unchanged. When the tool
/// is an operation tool — [`McpTool::operations`] returns a non-empty slice —
/// the discovery `_meta` is generated from that operation slice with
/// [`generate_operations_meta`](swissarmyhammer_operations::generate_operations_meta),
/// the very generator the `operation_tool!` macro uses, and attached under
/// [`OPERATIONS_META_KEY`]. The `_meta` is therefore produced by the generator
/// from the same operations the schema is built from — never hand-assembled,
/// and never able to drift from the operation definitions.
fn build_tool_definition(tool: &dyn McpTool) -> Tool {
    let schema = tool.schema();
    let schema_map = match schema {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };

    let mut definition = Tool::new(McpTool::name(tool), tool.description(), schema_map);

    let operations = tool.operations();
    if !operations.is_empty() {
        let ops_meta = swissarmyhammer_operations::generate_operations_meta(operations);
        let mut meta = Meta::new();
        meta.0.insert(OPERATIONS_META_KEY.to_string(), ops_meta);
        definition.meta = Some(meta);
    }

    definition
}

#[async_trait]
impl PluginMcpServer for ToolModuleServer {
    /// Returns the single wrapped tool's definition.
    ///
    /// The list always holds exactly one entry — the tool this module server
    /// was built for — snapshotted at construction.
    fn tools(&self) -> Vec<ToolMetadata> {
        vec![self.tool_metadata.clone()]
    }

    /// Routes a `tools/call` straight into the wrapped tool's [`McpTool::execute`].
    ///
    /// The wrapped tool is resolved live from the registry by name — so a tool
    /// disabled after exposure is no longer reachable — and run with the
    /// module server's [`ToolContext`]. The tool's [`CallToolResult`] is
    /// serialized to a `serde_json::Value`, the same shape an MCP `tools/call`
    /// response carries on the wire.
    ///
    /// `caller` is accepted to satisfy the platform contract; the in-process
    /// tools make no access decisions on caller identity, so it is not
    /// threaded further.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownTool`](PluginError::UnknownTool) when `tool`
    /// does not name this module's tool or the tool is no longer registered,
    /// and [`Error::ServerUnavailable`](PluginError::ServerUnavailable) when
    /// the tool's execution fails or its result cannot be serialized.
    async fn invoke(
        &self,
        _caller: CallerId,
        tool: &str,
        input: Value,
    ) -> swissarmyhammer_plugin::Result<Value> {
        if tool != self.tool_name {
            return Err(PluginError::UnknownTool);
        }

        let arguments = match input {
            Value::Object(map) => map,
            // A non-object body carries no `op` and no parameters; the tools
            // all expect an arguments object, so this is a missing tool shape.
            _ => serde_json::Map::new(),
        };

        let registry = self.tool_registry.read().await;
        let resolved = registry
            .get_tool(&self.tool_name)
            .ok_or(PluginError::UnknownTool)?;

        let result = resolved
            .execute(arguments, &self.tool_context)
            .await
            .map_err(|_| PluginError::ServerUnavailable)?;

        serde_json::to_value(result).map_err(|_| PluginError::ServerUnavailable)
    }
}

/// Builds one [`ToolModuleServer`] per tool in `registry`.
///
/// Every currently-enabled tool in the registry is wrapped, paired with the
/// tool's name as the module id a `register(name, { rust: id })` addresses.
/// Disabled tools are skipped — [`ToolRegistry::iter_tools`] already filters
/// them — so the exposed set matches the registry's live tool set.
///
/// # Parameters
///
/// - `tool_registry` — the registry whose tools are exposed.
/// - `tool_context` — the context every exposed tool is run with.
///
/// # Returns
///
/// `(module id, module server)` pairs, one per tool, ready to hand to
/// `PluginHost::expose_rust_module`.
pub async fn build_tool_modules(
    tool_registry: Arc<RwLock<ToolRegistry>>,
    tool_context: Arc<ToolContext>,
) -> Vec<(String, Arc<dyn PluginMcpServer>)> {
    let registry = tool_registry.read().await;
    registry
        .iter_tools()
        .map(|tool| {
            let module =
                ToolModuleServer::new(tool, Arc::clone(&tool_registry), Arc::clone(&tool_context));
            (
                McpTool::name(tool).to_string(),
                Arc::new(module) as Arc<dyn PluginMcpServer>,
            )
        })
        .collect()
}
