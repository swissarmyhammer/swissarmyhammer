//! HTTP MCP server implementation for serving MCP tools over HTTP
//!
//! This module provides HTTP transport for the existing MCP server, enabling
//! integration with web clients and in-process execution for LlamaAgent.

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use swissarmyhammer::Result;
use swissarmyhammer_config::McpServerConfig;
use tokio::sync::{Mutex, RwLock};

use super::server::McpServer;

/// Handle for managing HTTP MCP server lifecycle and providing port information
///
/// This is the key interface that AgentExecutor uses to get port information
/// and manage the server lifecycle.
#[derive(Debug, Clone)]
pub struct McpServerHandle {
    /// Actual bound port (important when using port 0 for random port)
    port: u16,
    /// Host the server is bound to
    host: String,
    /// Full HTTP URL for connecting to the server
    url: String,
    /// Shutdown sender for graceful shutdown
    shutdown_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl McpServerHandle {
    /// Create a new MCP server handle
    fn new(port: u16, host: String, shutdown_tx: tokio::sync::oneshot::Sender<()>) -> Self {
        let url = format!("http://{}:{}", host, port);
        Self {
            port,
            host,
            url,
            shutdown_tx: Arc::new(Mutex::new(Some(shutdown_tx))),
        }
    }

    /// Get the actual port the server is bound to
    /// This is crucial for AgentExecutor when using random ports
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the host the server is bound to
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Get the full HTTP URL for connecting to the server
    /// AgentExecutor will use this URL to connect LlamaAgent to MCP server
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Shutdown the server gracefully
    pub async fn shutdown(&self) -> Result<()> {
        let mut guard = self.shutdown_tx.lock().await;
        if let Some(tx) = guard.take() {
            if tx.send(()).is_err() {
                tracing::warn!("Server shutdown signal receiver already dropped");
            }
        }
        Ok(())
    }
}

/// Start in-process HTTP MCP server and return handle with port information
///
/// This is the primary function AgentExecutor will call to start an MCP server
/// and get the port information needed to connect LlamaAgent.
pub async fn start_in_process_mcp_server(config: &McpServerConfig) -> Result<McpServerHandle> {
    let host = "127.0.0.1";
    let bind_addr = format!("{}:{}", host, config.port);

    tracing::info!("Starting in-process MCP HTTP server on {}", bind_addr);

    // Create the underlying MCP server
    let library = swissarmyhammer::PromptLibrary::new();
    let mcp_server = McpServer::new(library)?;
    mcp_server.initialize().await?;

    // Start HTTP server with the MCP server
    start_http_server_with_mcp_server(host, config.port, mcp_server).await
}

/// Start standalone HTTP MCP server (for CLI usage)
pub async fn start_http_server(bind_addr: &str) -> Result<McpServerHandle> {
    let (host, port) = parse_bind_address(bind_addr)?;

    tracing::info!("Starting standalone MCP HTTP server on {}", bind_addr);

    // Create the underlying MCP server
    let library = swissarmyhammer::PromptLibrary::new();
    let mcp_server = McpServer::new(library)?;
    mcp_server.initialize().await?;

    start_http_server_with_mcp_server(&host, port, mcp_server).await
}

/// Internal function to start HTTP server with an existing MCP server
async fn start_http_server_with_mcp_server(
    host: &str,
    port: u16,
    mcp_server: McpServer,
) -> Result<McpServerHandle> {
    let bind_addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| {
            swissarmyhammer::SwissArmyHammerError::Other(format!(
                "Failed to bind to {}: {}",
                bind_addr, e
            ))
        })?;

    let actual_addr = listener.local_addr().map_err(|e| {
        swissarmyhammer::SwissArmyHammerError::Other(format!("Failed to get local address: {}", e))
    })?;

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    // Share MCP server in application state
    let app_state = HttpServerState {
        mcp_server: Arc::new(RwLock::new(mcp_server)),
    };

    // Build Axum router with all MCP endpoints
    let app = create_mcp_router(app_state.clone());

    // Spawn server task
    let server_future = axum::serve(listener, app);
    tokio::spawn(async move {
        let graceful = server_future.with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
            tracing::info!("HTTP MCP server shutting down gracefully");
        });

        if let Err(e) = graceful.await {
            tracing::error!("HTTP MCP server error: {}", e);
        }
    });

    let handle = McpServerHandle::new(
        actual_addr.port(),
        actual_addr.ip().to_string(),
        shutdown_tx,
    );

    tracing::info!("HTTP MCP server ready on {} for connections", handle.url());

    Ok(handle)
}

/// HTTP server state shared across handlers
#[derive(Clone)]
struct HttpServerState {
    mcp_server: Arc<RwLock<McpServer>>,
}

/// Create Axum router with MCP endpoints
fn create_mcp_router(state: HttpServerState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/", post(handle_mcp_request))
        .route("/mcp", post(handle_mcp_request))
        .with_state(state)
}

/// Health check endpoint
async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "service": "swissarmyhammer-mcp",
        "timestamp": Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Main MCP request handler
async fn handle_mcp_request(
    State(state): State<HttpServerState>,
    Json(payload): Json<Value>,
) -> std::result::Result<Json<Value>, StatusCode> {
    let method = payload
        .get("method")
        .and_then(|m| m.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let id = payload.get("id").cloned();
    let _params = payload.get("params").cloned().unwrap_or(Value::Null);

    tracing::info!("Processing MCP HTTP request: method={}", method);

    let mcp_server = state.mcp_server.read().await;

    // For now, provide basic responses for the main MCP methods
    // Full protocol integration will be completed in future iterations
    let result = match method {
        "initialize" => match mcp_server.initialize().await {
            Ok(_) => json!({
                "protocol_version": "2024-11-05",
                "capabilities": {
                    "prompts": { "list_changed": true },
                    "tools": { "list_changed": true }
                },
                "server_info": {
                    "name": "SwissArmyHammer",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
            Err(e) => {
                tracing::error!("Initialize failed: {}", e);
                return Ok(Json(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32603,
                        "message": format!("Initialization failed: {}", e)
                    }
                })));
            }
        },
        "prompts/list" => match mcp_server.list_prompts().await {
            Ok(prompts) => json!({
                "prompts": prompts.into_iter().map(|name| json!({
                    "name": name,
                    "description": format!("Prompt: {}", name)
                })).collect::<Vec<_>>()
            }),
            Err(e) => {
                tracing::error!("List prompts failed: {}", e);
                return Ok(Json(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32603,
                        "message": format!("Failed to list prompts: {}", e)
                    }
                })));
            }
        },
        "tools/list" => {
            // Get all registered tools from the MCP server's tool registry
            let tools = mcp_server.list_tools();
            let tools_json: Vec<Value> = tools.into_iter().map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description.as_ref().map(|s| s.as_ref()).unwrap_or("No description available"),
                    "inputSchema": *tool.input_schema
                })
            }).collect();

            json!({
                "tools": tools_json
            })
        }
        "tools/call" => {
            let params = payload.get("params").cloned().unwrap_or(Value::Null);
            let tool_name = params
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or(StatusCode::BAD_REQUEST)?;

            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            tracing::info!("Executing tool: {} with args: {}", tool_name, arguments);

            // Execute tool using the MCP server's public method
            match mcp_server.execute_tool(tool_name, arguments).await {
                Ok(result) => {
                    // Convert the CallToolResult to JSON
                    // Use serde_json to serialize the result automatically
                    match serde_json::to_value(&result) {
                        Ok(json_result) => json_result,
                        Err(e) => {
                            tracing::error!("Failed to serialize tool result: {}", e);
                            json!({
                                "content": [{
                                    "type": "text",
                                    "text": format!("Tool executed successfully but result serialization failed: {}", e)
                                }],
                                "isError": false
                            })
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Tool execution failed for {}: {}", tool_name, e);
                    return Ok(Json(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32603,
                            "message": format!("Tool execution failed: {}", e)
                        }
                    })));
                }
            }
        }
        _ => {
            tracing::warn!("Unsupported MCP method: {}", method);
            return Ok(Json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {}", method)
                }
            })));
        }
    };

    Ok(Json(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })))
}

/// Parse bind address string into host and port
fn parse_bind_address(bind_addr: &str) -> Result<(String, u16)> {
    let addr: SocketAddr = bind_addr.parse().map_err(|e| {
        swissarmyhammer::SwissArmyHammerError::Other(format!(
            "Invalid bind address '{}': {}",
            bind_addr, e
        ))
    })?;

    Ok((addr.ip().to_string(), addr.port()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_in_process_mcp_server() {
        let config = McpServerConfig {
            port: 0, // Random port
            timeout_seconds: 30,
        };

        // Start in-process server
        let server = start_in_process_mcp_server(&config).await.unwrap();

        // Verify we got a valid port
        assert!(server.port() > 0);
        assert!(server.url().starts_with("http://127.0.0.1:"));

        // Shutdown
        server.shutdown().await.unwrap();

        // Give server time to shutdown
        sleep(Duration::from_millis(100)).await;
    }

    #[tokio::test]
    async fn test_random_port_allocation() {
        let config = McpServerConfig {
            port: 0, // Request random port
            timeout_seconds: 30,
        };

        let server1 = start_in_process_mcp_server(&config).await.unwrap();
        let server2 = start_in_process_mcp_server(&config).await.unwrap();

        // Should get different random ports
        assert_ne!(server1.port(), server2.port());

        server1.shutdown().await.unwrap();
        server2.shutdown().await.unwrap();
    }

    #[test]
    fn test_parse_bind_address() {
        let (host, port) = parse_bind_address("127.0.0.1:8080").unwrap();
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 8080);

        let (host, port) = parse_bind_address("0.0.0.0:0").unwrap();
        assert_eq!(host, "0.0.0.0");
        assert_eq!(port, 0);

        // Test invalid address
        assert!(parse_bind_address("invalid").is_err());
    }
}
