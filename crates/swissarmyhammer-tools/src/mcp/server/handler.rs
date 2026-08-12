// sah rule ignore acp/capability-enforcement
//! The [`ServerHandler`] implementation — where an MCP request becomes a call
//! on the rest of the server.
//!
//! Handshake, tool listing, tool dispatch, and the diagnostics resource
//! subscription all land here. The handler holds no state of its own; each
//! method reads the request, resolves the session and connecting host from it,
//! and delegates.
//!
//! Note: This is an MCP server, not an ACP agent. ACP capability checking
//! happens at the agent layer (claude-agent), not at the MCP layer.

use super::instructions::{
    build_instructions_with_health, create_server_capabilities, create_server_implementation,
};
use super::McpServer;
use crate::mcp::host::Host;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use std::sync::Arc;

use crate::mcp::tool_registry::ToolContext;

impl McpServer {
    /// Prepare tool context with peer for elicitation support.
    ///
    /// # Arguments
    ///
    /// * `peer` - The MCP peer connection
    ///
    /// # Returns
    ///
    /// * `ToolContext` - Tool context with peer configured
    fn prepare_tool_context(&self, peer: rmcp::Peer<RoleServer>) -> ToolContext {
        (*self.tool_context)
            .clone()
            .with_peer(Arc::new(peer.clone()))
    }
}

/// Extract the MCP session id from a [`RequestContext`].
///
/// For HTTP transports (the in-process validator MCP server), `rmcp` injects
/// [`http::request::Parts`] into the request context's extensions. The
/// `mcp-session-id` header on every per-session request carries the session
/// id assigned by the streamable-HTTP server. Returns `None` for stdio
/// transports (which have no session id concept) or when the header is
/// absent.
fn session_id_from_context(context: &RequestContext<RoleServer>) -> Option<String> {
    let parts = context.extensions.get::<http::request::Parts>()?;
    let value = parts.headers.get("mcp-session-id")?;
    value.to_str().ok().map(|s| s.to_string())
}

/// Identify the connecting client's [`Host`] from a request context.
///
/// Reads the client `Implementation` captured at the `initialize` handshake —
/// `rmcp` stores it on the peer, retrievable via [`Peer::peer_info`] — and maps
/// its name through [`Host::from_client_info`]. Resolves to [`Host::Other`]
/// (the conservative default) when no client info is available yet (e.g. a
/// `tools/list` that somehow precedes `initialize`).
///
/// [`Peer::peer_info`]: rmcp::Peer::peer_info
fn connecting_host_from_context(context: &RequestContext<RoleServer>) -> Host {
    context
        .peer
        .peer_info()
        .map(|client| Host::from_client_info(&client.client_info))
        .unwrap_or(Host::Other)
}

/// Render a [`CallToolResult`] as its full diagnostic text.
///
/// Joins all text content blocks; non-text blocks (images, audio, etc.) are
/// summarized as `<image>` / `<audio>` / `<resource>` placeholders so the log
/// line stays readable even for tools that return mixed content.
///
/// Returns `(total_text_bytes, joined_text)` — the complete joined text, never
/// truncated. Log truncation is forbidden in this codebase, so callers emit the
/// returned string in full.
pub(super) fn format_call_result_text(result: &rmcp::model::CallToolResult) -> (usize, String) {
    use rmcp::model::RawContent;

    let mut joined = String::new();
    for block in &result.content {
        match &block.raw {
            RawContent::Text(t) => {
                if !joined.is_empty() {
                    joined.push('\n');
                }
                joined.push_str(&t.text);
            }
            RawContent::Image(_) => {
                if !joined.is_empty() {
                    joined.push('\n');
                }
                joined.push_str("<image>");
            }
            RawContent::Audio(_) => {
                if !joined.is_empty() {
                    joined.push('\n');
                }
                joined.push_str("<audio>");
            }
            RawContent::Resource(_) => {
                if !joined.is_empty() {
                    joined.push('\n');
                }
                joined.push_str("<resource>");
            }
            RawContent::ResourceLink(_) => {
                if !joined.is_empty() {
                    joined.push('\n');
                }
                joined.push_str("<resource_link>");
            }
        }
    }
    let bytes = joined.len();
    (bytes, joined)
}

impl ServerHandler for McpServer {
    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<InitializeResult, McpError> {
        let session_id = session_id_from_context(&context);

        // Distinct from the `event=session_open` line emitted by
        // [`unified_server::request_observer`] on first sight of a new
        // `mcp-session-id` header. That earlier line marks the *transport*
        // assigning a session id; this one fires when the MCP `initialize`
        // request actually arrives, carrying client name + version. Keeping
        // the two events distinct prevents `grep 'event=session_open' | wc -l`
        // from double-counting per session.
        tracing::info!(
            session_id = session_id.as_deref().unwrap_or("<stdio>"),
            client = %request.client_info.name,
            client_version = %request.client_info.version,
            event = "session_initialized",
            "🚀 MCP client connecting (initialize received)"
        );

        // Install the peer on the process-wide diagnostics resource so the
        // diagnostics fan-out can push `notifications/resources/updated` to this
        // connected client. Best-effort: a foreign host that never subscribes
        // simply ignores the notifications.
        crate::mcp::diagnostics_resource::diagnostics_resources()
            .set_peer(context.peer.clone())
            .await;

        self.spawn_background_file_watcher(context.peer);

        // Auto-create agent actor for the connecting MCP client
        self.ensure_agent_actor(&request.client_info.name).await;

        // Suppress the native host tool(s) the served `Replacement` tools
        // supersede (e.g. Claude's `Bash`, replaced by `shell`) so the served
        // tool truly replaces the native rather than competing with it. Gated on
        // the connecting client being Claude.
        self.apply_serve_time_native_deny(&request.client_info)
            .await;

        // Start code-context background work (LSP, indexing, file watcher)
        // only when an MCP client actually connects — not in the constructor.
        if let Some(ref work_dir) = self.work_dir {
            Self::initialize_code_context(work_dir);
        }

        // Run Initializable::start() on all registered tools
        {
            let registry = self.tool_registry.read().await;
            for tool in registry.iter_tools() {
                let results = tool.start();
                for r in &results {
                    if r.status == swissarmyhammer_common::lifecycle::InitStatus::Error {
                        tracing::warn!("Tool start error: {} — {}", r.name, r.message);
                    }
                }
            }
        }

        Ok(InitializeResult::new(create_server_capabilities())
            .with_server_info(create_server_implementation())
            .with_instructions(build_instructions_with_health(self.work_dir.as_deref())))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        // Hot reload: check if tools.yaml changed since last call.
        // Acquire the write lock once and read from it directly — avoids a
        // second lock acquisition for the list_tools() call below.
        let mut registry = self.tool_registry.write().await;
        {
            let mut watcher = self.tool_config_watcher.lock().await;
            watcher.check_and_reload(&mut registry);
        }

        // Compose the advertised set per connecting client. The full server
        // filters by the client's host identity (from the `initialize`
        // handshake's client `Implementation`) and each tool's `category()`:
        // Claude gets `Shared` + `Replacement`; unknown clients get `Shared`
        // only. The validator server (`compose_per_client == false`)
        // serves its already-scoped registry verbatim.
        let host = connecting_host_from_context(&context);
        let tools = if self.compose_per_client {
            registry.list_tools_for_host(host)
        } else {
            registry.list_tools()
        };

        // Per-session log of which tools were advertised — answers the
        // grep-able question "which tools were exposed to the validator
        // agent?". Joins names rather than dumping schemas because schemas
        // are large and rarely the failure mode; trace-level callers can opt
        // in to per-tool schema bytes via the `tool_count` and tool-name
        // list below.
        let session_id = session_id_from_context(&context);
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        tracing::info!(
            session_id = session_id.as_deref().unwrap_or("<stdio>"),
            host = ?host,
            compose_per_client = self.compose_per_client,
            tool_count = tools.len(),
            tools = %tool_names.join(","),
            event = "tools_listed",
            "Advertised tool list to MCP client"
        );

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListResourcesResult, McpError> {
        Ok(crate::mcp::diagnostics_resource::diagnostics_resources().list())
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ReadResourceResult, McpError> {
        crate::mcp::diagnostics_resource::diagnostics_resources()
            .read(&request.uri)
            .await
            .ok_or_else(|| {
                McpError::resource_not_found(
                    format!("no resource with uri '{}'", request.uri),
                    None,
                )
            })
    }

    async fn subscribe(
        &self,
        request: SubscribeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<(), McpError> {
        // The diagnostics resource pushes to the connected peer unconditionally
        // (best-effort), so a subscribe is acknowledged for the known resource
        // and rejected for any other uri. There is no per-uri subscriber set to
        // maintain: the one aggregate resource notifies the captured peer.
        if request.uri == crate::mcp::diagnostics_resource::DIAGNOSTICS_RESOURCE_URI {
            Ok(())
        } else {
            Err(McpError::resource_not_found(
                format!("cannot subscribe to unknown resource '{}'", request.uri),
                None,
            ))
        }
    }

    async fn unsubscribe(
        &self,
        _request: UnsubscribeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<(), McpError> {
        // Symmetric with `subscribe`: nothing per-uri to tear down.
        Ok(())
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        use tracing::Instrument;

        let tool_name = request.name.to_string();
        let arg_count = request.arguments.as_ref().map_or(0, |a| a.len());
        let session_id = session_id_from_context(&context);
        let session_field = session_id.clone().unwrap_or_else(|| "<stdio>".to_string());
        // The dispatched op rides on the span so per-op log aggregation works
        // on any line of the call — including `tool_call complete` — without
        // joining back to the args line (which breaks under concurrent calls).
        let op_field = request
            .arguments
            .as_ref()
            .and_then(|args| args.get("op"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let span = tracing::info_span!(
            "tool_call",
            tool = %tool_name,
            op = op_field.as_str(),
            args = arg_count,
            session_id = %session_field,
            caller = "mcp",
            status = tracing::field::Empty,
        );

        async move {
            // The wall-clock total still anchors the `tool_call complete`
            // line, but we also break it into the four phases the original
            // task description called out so a slow call's bottleneck is
            // visible without re-running with more verbose tracing:
            //   parse_ms     — pre-call args logging + lookup of the tool.
            //   dispatch_ms  — building the tool context for this peer.
            //   handler_ms   — `tool.execute()` itself.
            //   response_ms  — formatting the response text.
            let total_start = std::time::Instant::now();
            let parse_start = total_start;

            // Pre-call args logging — answers "what did rule X actually
            // call?". The FULL serialized args are logged at info level, never
            // truncated. JSON serialization is gated behind `tracing::enabled!`
            // so info-disabled runs do not allocate.
            if let Some(args) = request.arguments.as_ref() {
                if tracing::enabled!(tracing::Level::INFO) {
                    let full = serde_json::to_string(args)
                        .unwrap_or_else(|_| "<unserializable>".to_string());
                    tracing::info!(
                        tool = %tool_name,
                        args_bytes = full.len(),
                        args = %full,
                        "tool_call args"
                    );
                }
            }

            let registry = self.tool_registry.read().await;
            let tool = registry.get_tool(&request.name).ok_or_else(|| {
                tracing::error!(tool = %request.name, "unknown tool requested");
                McpError::invalid_request(format!("unknown tool: {}", request.name), None)
            })?;
            let parse_ms = parse_start.elapsed().as_millis() as u64;

            let dispatch_start = std::time::Instant::now();
            // Plumb the JSON-RPC `_meta.progressToken` (when the client
            // supplied one) into the tool context so long-running tools
            // like `code_context` `rebuild index` can forward
            // `IndexProgress` events as `notifications/progress`. Absent
            // tokens leave the field as `None`, which tools treat as
            // "use a no-op reporter — don't emit progress notifications".
            // rmcp's `Request<M, P>` Deserialize impl extracts `_meta`
            // from the wire-level `params._meta` into the
            // `RequestContext.meta` field — by the time we see
            // `CallToolRequestParams` here, `params.meta` is always
            // `None`. The token lives on `context.meta` instead. We fall
            // back to `request.meta` as a belt-and-braces second source
            // for any custom transport that may populate the params-level
            // meta directly.
            let progress_token = context
                .meta
                .get_progress_token()
                .or_else(|| request.meta.as_ref().and_then(|m| m.get_progress_token()));
            let mut tool_context_with_peer = self.prepare_tool_context(context.peer.clone());
            if let Some(token) = progress_token {
                tool_context_with_peer = tool_context_with_peer.with_progress_token(token);
            }
            let arguments = request.arguments.unwrap_or_default();
            let dispatch_ms = dispatch_start.elapsed().as_millis() as u64;

            let handler_start = std::time::Instant::now();
            let result = tool.execute(arguments, &tool_context_with_peer).await;
            let handler_ms = handler_start.elapsed().as_millis() as u64;

            let is_error = match &result {
                Ok(r) => r.is_error.unwrap_or(false),
                Err(_) => true,
            };
            tracing::Span::current().record("status", if is_error { "error" } else { "ok" });

            // Post-call response text — answers "what did the tool return?".
            // The FULL joined result text is logged at info level, never
            // truncated. Computed only when info is enabled so info-disabled
            // runs do not allocate.
            let response_start = std::time::Instant::now();
            let (result_bytes, result_text): (usize, String) =
                if tracing::enabled!(tracing::Level::INFO) {
                    match &result {
                        Ok(call_result) => format_call_result_text(call_result),
                        Err(e) => {
                            let s = e.to_string();
                            (s.len(), s)
                        }
                    }
                } else {
                    (0, String::new())
                };
            let response_ms = response_start.elapsed().as_millis() as u64;

            let total_ms = total_start.elapsed().as_millis() as u64;
            tracing::info!(
                duration_ms = total_ms,
                parse_ms,
                dispatch_ms,
                handler_ms,
                response_ms,
                error = is_error,
                result_bytes,
                result = %result_text,
                "tool_call complete"
            );

            result
        }
        .instrument(span)
        .await
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(create_server_capabilities())
            .with_server_info(create_server_implementation())
            .with_instructions(build_instructions_with_health(self.work_dir.as_deref()))
    }
}
