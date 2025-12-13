//! Enhanced MCP server connection error handling for ACP compliance
//!
//! This module provides comprehensive error handling for MCP server connections
//! following ACP specification requirements with detailed error reporting.

use crate::{
    config::McpServerConfig,
    mcp::{McpServerConnection, TransportConnection},
    session_errors::{SessionSetupError, SessionSetupResult},
    session_validation::validate_mcp_server_config,
};
use reqwest::{Client, Url};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::Command;
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;

/// Enhanced MCP server connection manager with comprehensive error handling
pub struct EnhancedMcpServerManager {
    /// Map of server name to connection
    connections: Arc<RwLock<HashMap<String, McpServerConnection>>>,
    /// Connection timeout in milliseconds
    connection_timeout_ms: u64,
    /// Protocol negotiation timeout in milliseconds  
    protocol_timeout_ms: u64,
}

impl Default for EnhancedMcpServerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EnhancedMcpServerManager {
    /// Create a new enhanced MCP server manager with default timeouts
    pub fn new() -> Self {
        Self::with_timeouts(30000, 10000) // 30s connection, 10s protocol
    }

    /// Create a new enhanced MCP server manager with custom timeouts
    pub fn with_timeouts(connection_timeout_ms: u64, protocol_timeout_ms: u64) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_timeout_ms,
            protocol_timeout_ms,
        }
    }

    /// Connect to all configured MCP servers with comprehensive error handling
    ///
    /// This method validates each server configuration before attempting connection
    /// and provides detailed error information for each failure while continuing
    /// to connect to other servers.
    pub async fn connect_servers_with_validation(
        &mut self,
        configs: Vec<McpServerConfig>,
    ) -> SessionSetupResult<HashMap<String, Result<String, SessionSetupError>>> {
        let mut results = HashMap::new();

        for config in configs {
            let server_name = config.name().to_string();

            // Step 1: Validate server configuration before attempting connection
            match validate_mcp_server_config(&config) {
                Ok(()) => {
                    // Step 2: Attempt connection with comprehensive error handling
                    match self.connect_server_enhanced(config).await {
                        Ok(connection) => {
                            let connection_name = connection.name.clone();
                            tracing::info!(
                                "Successfully connected to MCP server: {} with {} tools",
                                connection_name,
                                connection.tools.len()
                            );
                            let mut connections = self.connections.write().await;
                            connections.insert(connection_name, connection);
                            results.insert(server_name, Ok("Connected successfully".to_string()));
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to connect to MCP server {}: {}",
                                server_name,
                                e
                            );
                            results.insert(server_name, Err(e));
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Invalid MCP server configuration for {}: {}",
                        server_name,
                        e
                    );
                    results.insert(server_name, Err(e));
                }
            }
        }

        Ok(results)
    }

    /// Connect to a single MCP server with enhanced error handling
    async fn connect_server_enhanced(
        &self,
        config: McpServerConfig,
    ) -> SessionSetupResult<McpServerConnection> {
        let start_time = Instant::now();

        match config.clone() {
            McpServerConfig::Stdio(stdio_config) => {
                self.connect_stdio_server_enhanced(config, &stdio_config, start_time)
                    .await
            }
            McpServerConfig::Http(http_config) => {
                self.connect_http_server_enhanced(config, &http_config, start_time)
                    .await
            }
            McpServerConfig::Sse(sse_config) => {
                self.connect_sse_server_enhanced(config, &sse_config, start_time)
                    .await
            }
        }
    }

    /// Connect to STDIO MCP server with comprehensive error handling
    async fn connect_stdio_server_enhanced(
        &self,
        config: McpServerConfig,
        stdio_config: &crate::config::StdioTransport,
        start_time: Instant,
    ) -> SessionSetupResult<McpServerConnection> {
        tracing::info!(
            "Attempting STDIO connection to MCP server: {} ({})",
            stdio_config.name,
            stdio_config.command
        );

        // Build the command with comprehensive error handling
        let mut command = Command::new(&stdio_config.command);
        command.args(&stdio_config.args);

        // Set working directory if provided with validation
        if let Some(cwd_str) = &stdio_config.cwd {
            let cwd_path = std::path::Path::new(cwd_str);
            if !cwd_path.exists() {
                return Err(SessionSetupError::McpServerConnectionFailed {
                    server_name: stdio_config.name.clone(),
                    error: format!("Working directory does not exist: {}", cwd_path.display()),
                    transport_type: "stdio".to_string(),
                });
            }
            command.current_dir(cwd_path);
        }

        // Set environment variables
        for env_var in &stdio_config.env {
            command.env(&env_var.name, &env_var.value);
        }

        // Configure process stdio
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Spawn the process with detailed error handling
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                return match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        Err(SessionSetupError::McpServerExecutableNotFound {
                            server_name: stdio_config.name.clone(),
                            command: Path::new(&stdio_config.command).to_path_buf(),
                            suggestion: if Path::new(&stdio_config.command).is_absolute() {
                                "Check that the executable exists and has proper permissions"
                                    .to_string()
                            } else {
                                format!(
                                    "Install {} or provide the full path to the executable",
                                    stdio_config.command
                                )
                            },
                        })
                    }
                    std::io::ErrorKind::PermissionDenied => {
                        Err(SessionSetupError::McpServerConnectionFailed {
                            server_name: stdio_config.name.clone(),
                            error: "Permission denied: insufficient permissions to execute server"
                                .to_string(),
                            transport_type: "stdio".to_string(),
                        })
                    }
                    _ => Err(SessionSetupError::McpServerStartupFailed {
                        server_name: stdio_config.name.clone(),
                        exit_code: -1,
                        stderr: format!("Process spawn failed: {}", e),
                        suggestion: "Check server installation, permissions, and system resources"
                            .to_string(),
                    }),
                }
            }
        };

        // Check if process started successfully (hasn't exited immediately)
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Ok(Some(exit_status)) = child.try_wait() {
            // Process exited immediately - likely a startup failure
            let stderr_output = if let Some(stderr) = child.stderr.take() {
                let mut stderr_reader = BufReader::new(stderr);
                let mut stderr_content = Vec::new();
                let _ = stderr_reader.read_to_end(&mut stderr_content).await;
                String::from_utf8_lossy(&stderr_content).to_string()
            } else {
                "No stderr available".to_string()
            };

            return Err(SessionSetupError::McpServerStartupFailed {
                server_name: stdio_config.name.clone(),
                exit_code: exit_status.code().unwrap_or(-1),
                stderr: stderr_output,
                suggestion: "Check server logs and configuration".to_string(),
            });
        }

        // Get stdio handles
        let stdin =
            child
                .stdin
                .take()
                .ok_or_else(|| SessionSetupError::McpServerConnectionFailed {
                    server_name: stdio_config.name.clone(),
                    error: "Failed to get stdin handle from child process".to_string(),
                    transport_type: "stdio".to_string(),
                })?;

        let stdout =
            child
                .stdout
                .take()
                .ok_or_else(|| SessionSetupError::McpServerConnectionFailed {
                    server_name: stdio_config.name.clone(),
                    error: "Failed to get stdout handle from child process".to_string(),
                    transport_type: "stdio".to_string(),
                })?;

        let mut stdin_writer = BufWriter::new(stdin);
        let mut stdout_reader = BufReader::new(stdout);

        // Initialize MCP protocol with timeout and comprehensive error handling
        let tools = match timeout(
            Duration::from_millis(self.protocol_timeout_ms),
            self.initialize_mcp_protocol_enhanced(
                &mut stdin_writer,
                &mut stdout_reader,
                &stdio_config.name,
            ),
        )
        .await
        {
            Ok(Ok(tools)) => tools,
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                // Kill the child process on timeout
                let _ = child.kill().await;
                return Err(SessionSetupError::McpServerTimeout {
                    server_name: stdio_config.name.clone(),
                    timeout_ms: self.protocol_timeout_ms,
                    transport_type: "stdio_protocol".to_string(),
                });
            }
        };

        let transport = TransportConnection::Stdio {
            process: Arc::new(RwLock::new(Some(child))),
            stdin_writer: Arc::new(RwLock::new(Some(stdin_writer))),
            stdout_reader: Arc::new(RwLock::new(Some(stdout_reader))),
        };

        let connection = McpServerConnection {
            name: stdio_config.name.clone(),
            tools,
            prompts: Vec::new(),
            config,
            transport,
        };

        let connection_time = start_time.elapsed();
        tracing::info!(
            "Successfully connected to STDIO MCP server {} in {:?}",
            stdio_config.name,
            connection_time
        );

        Ok(connection)
    }

    /// Connect to HTTP MCP server with comprehensive error handling
    async fn connect_http_server_enhanced(
        &self,
        config: McpServerConfig,
        http_config: &crate::config::HttpTransport,
        _start_time: Instant,
    ) -> SessionSetupResult<McpServerConnection> {
        tracing::info!(
            "Attempting HTTP connection to MCP server: {} ({})",
            http_config.name,
            http_config.url
        );

        // Validate and parse URL
        let parsed_url = Url::parse(&http_config.url).map_err(|_| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: http_config.name.clone(),
                error: "Invalid URL format".to_string(),
                transport_type: "http".to_string(),
            }
        })?;

        // Build HTTP client with headers
        let mut headers = reqwest::header::HeaderMap::new();
        for header in &http_config.headers {
            let name =
                reqwest::header::HeaderName::from_bytes(header.name.as_bytes()).map_err(|_| {
                    SessionSetupError::McpServerConnectionFailed {
                        server_name: http_config.name.clone(),
                        error: format!("Invalid header name: {}", header.name),
                        transport_type: "http".to_string(),
                    }
                })?;

            let value = reqwest::header::HeaderValue::from_str(&header.value).map_err(|_| {
                SessionSetupError::McpServerConnectionFailed {
                    server_name: http_config.name.clone(),
                    error: format!("Invalid header value: {}", header.value),
                    transport_type: "http".to_string(),
                }
            })?;

            headers.insert(name, value);
        }

        let client = Client::builder()
            .timeout(Duration::from_millis(self.connection_timeout_ms))
            .default_headers(headers)
            .build()
            .map_err(|e| SessionSetupError::McpServerConnectionFailed {
                server_name: http_config.name.clone(),
                error: format!("Failed to create HTTP client: {}", e),
                transport_type: "http".to_string(),
            })?;

        // Test connection and initialize protocol
        let session_id = Arc::new(RwLock::new(None));
        let tools = self
            .initialize_http_mcp_protocol_enhanced(&client, http_config, Arc::clone(&session_id))
            .await?;

        let transport = TransportConnection::Http {
            client: Arc::new(client),
            url: parsed_url.to_string(),
            headers: http_config.headers.clone(),
            session_id,
        };

        let connection = McpServerConnection {
            name: http_config.name.clone(),
            tools,
            prompts: Vec::new(),
            config,
            transport,
        };

        Ok(connection)
    }

    /// Connect to SSE MCP server with comprehensive error handling
    async fn connect_sse_server_enhanced(
        &self,
        config: McpServerConfig,
        sse_config: &crate::config::SseTransport,
        _start_time: Instant,
    ) -> SessionSetupResult<McpServerConnection> {
        tracing::info!(
            "Attempting SSE connection to MCP server: {} ({})",
            sse_config.name,
            sse_config.url
        );

        // Validate URL format
        Url::parse(&sse_config.url).map_err(|_| SessionSetupError::McpServerConnectionFailed {
            server_name: sse_config.name.clone(),
            error: "Invalid URL format".to_string(),
            transport_type: "sse".to_string(),
        })?;

        // Create SSE connection channels
        let (message_tx, _message_rx) = mpsc::unbounded_channel();
        let (response_tx, response_rx) = mpsc::unbounded_channel();

        // Initialize SSE connection
        let tools = self
            .initialize_sse_mcp_protocol_enhanced(sse_config, response_tx)
            .await?;

        let transport = TransportConnection::Sse {
            url: sse_config.url.clone(),
            headers: sse_config.headers.clone(),
            message_tx: Arc::new(RwLock::new(Some(message_tx))),
            response_rx: Arc::new(RwLock::new(Some(response_rx))),
        };

        let connection = McpServerConnection {
            name: sse_config.name.clone(),
            tools,
            prompts: Vec::new(),
            config,
            transport,
        };

        Ok(connection)
    }

    /// Initialize MCP protocol with comprehensive error handling
    async fn initialize_mcp_protocol_enhanced(
        &self,
        writer: &mut BufWriter<tokio::process::ChildStdin>,
        reader: &mut BufReader<tokio::process::ChildStdout>,
        server_name: &str,
    ) -> SessionSetupResult<Vec<String>> {
        // Send initialize request
        let initialize_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "roots": {
                        "listChanged": true
                    },
                    "sampling": {}
                },
                "clientInfo": {
                    "name": "claude-agent",
                    "version": "1.0.0"
                }
            }
        });

        let request_line = format!("{}\n", initialize_request);

        writer
            .write_all(request_line.as_bytes())
            .await
            .map_err(|e| SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Failed to send initialize request: {}", e),
                transport_type: "stdio".to_string(),
            })?;

        writer
            .flush()
            .await
            .map_err(|e| SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Failed to flush initialize request: {}", e),
                transport_type: "stdio".to_string(),
            })?;

        // Read initialize response
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await.map_err(|_e| {
            SessionSetupError::McpServerProtocolNegotiationFailed {
                server_name: server_name.to_string(),
                expected_version: "2024-11-05".to_string(),
                actual_version: None,
            }
        })?;

        if response_line.trim().is_empty() {
            return Err(SessionSetupError::McpServerProtocolNegotiationFailed {
                server_name: server_name.to_string(),
                expected_version: "2024-11-05".to_string(),
                actual_version: Some("No response".to_string()),
            });
        }

        let response: Value = serde_json::from_str(response_line.trim()).map_err(|e| {
            SessionSetupError::McpServerProtocolNegotiationFailed {
                server_name: server_name.to_string(),
                expected_version: "2024-11-05".to_string(),
                actual_version: Some(format!("Invalid JSON: {}", e)),
            }
        })?;

        // Validate initialize response
        if let Some(error) = response.get("error") {
            return Err(SessionSetupError::McpServerProtocolNegotiationFailed {
                server_name: server_name.to_string(),
                expected_version: "2024-11-05".to_string(),
                actual_version: Some(format!("Server error: {}", error)),
            });
        }

        // Send initialized notification
        let initialized_notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        let notification_line = format!("{}\n", initialized_notification);
        writer
            .write_all(notification_line.as_bytes())
            .await
            .map_err(|e| SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Failed to send initialized notification: {}", e),
                transport_type: "stdio".to_string(),
            })?;

        writer
            .flush()
            .await
            .map_err(|e| SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Failed to flush initialized notification: {}", e),
                transport_type: "stdio".to_string(),
            })?;

        // Request list of tools
        let tools_request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });

        let tools_line = format!("{}\n", tools_request);
        writer.write_all(tools_line.as_bytes()).await.map_err(|e| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Failed to send tools/list request: {}", e),
                transport_type: "stdio".to_string(),
            }
        })?;

        writer
            .flush()
            .await
            .map_err(|e| SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Failed to flush tools/list request: {}", e),
                transport_type: "stdio".to_string(),
            })?;

        // Read tools response
        let mut tools_response_line = String::new();
        reader
            .read_line(&mut tools_response_line)
            .await
            .map_err(|e| SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: format!("Failed to read tools/list response: {}", e),
                transport_type: "stdio".to_string(),
            })?;

        let tools_response: Value =
            serde_json::from_str(tools_response_line.trim()).map_err(|e| {
                SessionSetupError::McpServerConnectionFailed {
                    server_name: server_name.to_string(),
                    error: format!("Invalid tools/list response JSON: {}", e),
                    transport_type: "stdio".to_string(),
                }
            })?;

        // Extract tool names from response
        let tools = if let Some(result) = tools_response.get("result") {
            if let Some(tools_array) = result.get("tools").and_then(|t| t.as_array()) {
                tools_array
                    .iter()
                    .filter_map(|tool| tool.get("name").and_then(|name| name.as_str()))
                    .map(|name| name.to_string())
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        tracing::info!("MCP server {} reported {} tools", server_name, tools.len());
        Ok(tools)
    }

    /// Initialize HTTP MCP protocol with comprehensive error handling
    async fn initialize_http_mcp_protocol_enhanced(
        &self,
        client: &Client,
        http_config: &crate::config::HttpTransport,
        session_id: Arc<RwLock<Option<String>>>,
    ) -> SessionSetupResult<Vec<String>> {
        tracing::info!("Initializing HTTP MCP protocol for {}", http_config.name);

        // Step 1: Send initialize request
        let initialize_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "roots": {
                        "listChanged": true
                    },
                    "sampling": {}
                },
                "clientInfo": {
                    "name": "claude-agent",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });

        let response = client
            .post(&http_config.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .json(&initialize_request)
            .send()
            .await
            .map_err(|e| SessionSetupError::McpServerConnectionFailed {
                server_name: http_config.name.clone(),
                error: format!("Failed to send initialize request: {}", e),
                transport_type: "http".to_string(),
            })?;

        if !response.status().is_success() {
            return Err(SessionSetupError::McpServerConnectionFailed {
                server_name: http_config.name.clone(),
                error: format!(
                    "Initialize request failed with status: {}",
                    response.status()
                ),
                transport_type: "http".to_string(),
            });
        }

        // Extract session ID if present
        if let Some(session_id_header) = response.headers().get("Mcp-Session-Id") {
            if let Ok(session_id_str) = session_id_header.to_str() {
                let mut session_id_write = session_id.write().await;
                *session_id_write = Some(session_id_str.to_string());
                tracing::debug!("Stored session ID: {}", session_id_str);
            }
        }

        // Parse response body
        let content_type = response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json");

        let initialize_response: Value = if content_type.contains("text/event-stream") {
            // Handle SSE response
            let body = response.text().await.map_err(|e| {
                SessionSetupError::McpServerConnectionFailed {
                    server_name: http_config.name.clone(),
                    error: format!("Failed to read SSE response: {}", e),
                    transport_type: "http".to_string(),
                }
            })?;

            // Parse SSE format - look for data: lines
            let mut json_data = String::new();
            for line in body.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    json_data = data.to_string();
                    break;
                }
            }

            if json_data.is_empty() {
                return Err(SessionSetupError::McpServerProtocolNegotiationFailed {
                    server_name: http_config.name.clone(),
                    expected_version: "2024-11-05".to_string(),
                    actual_version: Some("No data in SSE response".to_string()),
                });
            }

            serde_json::from_str(&json_data).map_err(|e| {
                SessionSetupError::McpServerProtocolNegotiationFailed {
                    server_name: http_config.name.clone(),
                    expected_version: "2024-11-05".to_string(),
                    actual_version: Some(format!("Invalid JSON in SSE: {}", e)),
                }
            })?
        } else {
            // Handle JSON response
            response
                .json()
                .await
                .map_err(|e| SessionSetupError::McpServerConnectionFailed {
                    server_name: http_config.name.clone(),
                    error: format!("Failed to parse initialize response: {}", e),
                    transport_type: "http".to_string(),
                })?
        };

        // Validate initialize response
        if let Some(error) = initialize_response.get("error") {
            return Err(SessionSetupError::McpServerProtocolNegotiationFailed {
                server_name: http_config.name.clone(),
                expected_version: "2024-11-05".to_string(),
                actual_version: Some(format!("Server error: {}", error)),
            });
        }

        // Step 2: Send initialized notification
        let initialized_notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        let mut notify_request = client
            .post(&http_config.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json");

        // Include session ID if present
        if let Some(session_id_value) = session_id.read().await.as_ref() {
            notify_request = notify_request.header("Mcp-Session-Id", session_id_value);
        }

        let notif_response = notify_request
            .json(&initialized_notification)
            .send()
            .await
            .map_err(|e| SessionSetupError::McpServerConnectionFailed {
                server_name: http_config.name.clone(),
                error: format!("Failed to send initialized notification: {}", e),
                transport_type: "http".to_string(),
            })?;

        // Expect 202 Accepted for notification
        if notif_response.status() != reqwest::StatusCode::ACCEPTED {
            tracing::warn!(
                "Initialized notification returned status: {}",
                notif_response.status()
            );
        }

        // Step 3: Request tools list
        let tools_request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });

        let mut tools_request_builder = client
            .post(&http_config.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json");

        // Include session ID if present
        if let Some(session_id_value) = session_id.read().await.as_ref() {
            tools_request_builder =
                tools_request_builder.header("Mcp-Session-Id", session_id_value);
        }

        let tools_response = tools_request_builder
            .json(&tools_request)
            .send()
            .await
            .map_err(|e| SessionSetupError::McpServerConnectionFailed {
                server_name: http_config.name.clone(),
                error: format!("Failed to send tools/list request: {}", e),
                transport_type: "http".to_string(),
            })?;

        if !tools_response.status().is_success() {
            return Err(SessionSetupError::McpServerConnectionFailed {
                server_name: http_config.name.clone(),
                error: format!(
                    "Tools list request failed with status: {}",
                    tools_response.status()
                ),
                transport_type: "http".to_string(),
            });
        }

        // Parse tools response
        let tools_content_type = tools_response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json");

        let tools_result: Value = if tools_content_type.contains("text/event-stream") {
            // Handle SSE response
            let body = tools_response.text().await.map_err(|e| {
                SessionSetupError::McpServerConnectionFailed {
                    server_name: http_config.name.clone(),
                    error: format!("Failed to read tools SSE response: {}", e),
                    transport_type: "http".to_string(),
                }
            })?;

            // Parse SSE format - look for data: lines
            let mut json_data = String::new();
            for line in body.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    json_data = data.to_string();
                    break;
                }
            }

            if json_data.is_empty() {
                return Err(SessionSetupError::McpServerConnectionFailed {
                    server_name: http_config.name.clone(),
                    error: "No data in tools SSE response".to_string(),
                    transport_type: "http".to_string(),
                });
            }

            serde_json::from_str(&json_data).map_err(|e| {
                SessionSetupError::McpServerConnectionFailed {
                    server_name: http_config.name.clone(),
                    error: format!("Invalid JSON in tools SSE: {}", e),
                    transport_type: "http".to_string(),
                }
            })?
        } else {
            // Handle JSON response
            tools_response.json().await.map_err(|e| {
                SessionSetupError::McpServerConnectionFailed {
                    server_name: http_config.name.clone(),
                    error: format!("Failed to parse tools response: {}", e),
                    transport_type: "http".to_string(),
                }
            })?
        };

        // Extract tool names from response
        let tools = if let Some(result) = tools_result.get("result") {
            if let Some(tools_array) = result.get("tools").and_then(|t| t.as_array()) {
                tools_array
                    .iter()
                    .filter_map(|tool| tool.get("name").and_then(|name| name.as_str()))
                    .map(|name| name.to_string())
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        tracing::info!(
            "MCP server {} reported {} tools",
            http_config.name,
            tools.len()
        );
        Ok(tools)
    }

    /// Initialize SSE MCP protocol with comprehensive error handling
    ///
    /// SSE transport uses a persistent GET connection for receiving server events
    /// and POST requests for sending client requests (initialize, tools/call, etc).
    ///
    /// # Arguments
    /// * `sse_config` - SSE transport configuration including URL and headers
    /// * `response_tx` - Channel for sending responses from the event stream
    ///
    /// # Returns
    /// List of available tool names from the MCP server
    ///
    /// # Errors
    /// Returns SessionSetupError if:
    /// - Connection fails
    /// - Server returns non-success status
    /// - Response parsing fails
    /// - Protocol negotiation fails
    async fn initialize_sse_mcp_protocol_enhanced(
        &self,
        sse_config: &crate::config::SseTransport,
        response_tx: mpsc::UnboundedSender<String>,
    ) -> SessionSetupResult<Vec<String>> {
        tracing::info!("Initializing SSE MCP protocol for {}", sse_config.name);

        // Create HTTP client with headers
        let mut headers = reqwest::header::HeaderMap::new();
        for header in &sse_config.headers {
            if let (Ok(name), Ok(value)) = (
                reqwest::header::HeaderName::from_bytes(header.name.as_bytes()),
                reqwest::header::HeaderValue::from_str(&header.value),
            ) {
                headers.insert(name, value);
            }
        }

        let client = Client::builder()
            .default_headers(headers.clone())
            .build()
            .map_err(|e| SessionSetupError::McpServerConnectionFailed {
                server_name: sse_config.name.clone(),
                error: format!("Failed to create HTTP client: {}", e),
                transport_type: "sse".to_string(),
            })?;

        // Step 1: Send initialize request via POST
        let initialize_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "clientInfo": {
                    "name": "claude-agent",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });

        let response = timeout(
            Duration::from_millis(self.protocol_timeout_ms),
            client
                .post(&sse_config.url)
                .header("Accept", "text/event-stream")
                .header("Content-Type", "application/json")
                .json(&initialize_request)
                .send(),
        )
        .await
        .map_err(|_| SessionSetupError::McpServerConnectionFailed {
            server_name: sse_config.name.clone(),
            error: format!(
                "Initialize request timed out after {}ms",
                self.protocol_timeout_ms
            ),
            transport_type: "sse".to_string(),
        })?
        .map_err(|e| SessionSetupError::McpServerConnectionFailed {
            server_name: sse_config.name.clone(),
            error: format!("Failed to send initialize request: {}", e),
            transport_type: "sse".to_string(),
        })?;

        if !response.status().is_success() {
            return Err(SessionSetupError::McpServerConnectionFailed {
                server_name: sse_config.name.clone(),
                error: format!(
                    "Initialize request failed with status: {}",
                    response.status()
                ),
                transport_type: "sse".to_string(),
            });
        }

        // Parse SSE response for initialize
        let body =
            response
                .text()
                .await
                .map_err(|e| SessionSetupError::McpServerConnectionFailed {
                    server_name: sse_config.name.clone(),
                    error: format!("Failed to read SSE response: {}", e),
                    transport_type: "sse".to_string(),
                })?;

        let initialize_response = Self::parse_sse_response_enhanced(&sse_config.name, &body)?;

        // Validate initialize response
        if let Some(error) = initialize_response.get("error") {
            return Err(SessionSetupError::McpServerConnectionFailed {
                server_name: sse_config.name.clone(),
                error: format!("Server returned error: {}", error),
                transport_type: "sse".to_string(),
            });
        }

        // Step 2: Send initialized notification
        let initialized_notification = json!({
            "jsonrpc": "2.0",
            "method": "initialized"
        });

        let notify_response = timeout(
            Duration::from_millis(self.protocol_timeout_ms),
            client
                .post(&sse_config.url)
                .header("Accept", "text/event-stream")
                .header("Content-Type", "application/json")
                .json(&initialized_notification)
                .send(),
        )
        .await
        .map_err(|_| SessionSetupError::McpServerConnectionFailed {
            server_name: sse_config.name.clone(),
            error: format!(
                "Initialized notification timed out after {}ms",
                self.protocol_timeout_ms
            ),
            transport_type: "sse".to_string(),
        })?
        .map_err(|e| SessionSetupError::McpServerConnectionFailed {
            server_name: sse_config.name.clone(),
            error: format!("Failed to send initialized notification: {}", e),
            transport_type: "sse".to_string(),
        })?;

        if notify_response.status() != reqwest::StatusCode::ACCEPTED {
            tracing::warn!(
                "Initialized notification returned status: {} (expected 202 Accepted)",
                notify_response.status()
            );
        }

        // Step 3: Request list of available tools
        let tools_list_request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });

        let tools_response = timeout(
            Duration::from_millis(self.protocol_timeout_ms),
            client
                .post(&sse_config.url)
                .header("Accept", "text/event-stream")
                .header("Content-Type", "application/json")
                .json(&tools_list_request)
                .send(),
        )
        .await
        .map_err(|_| SessionSetupError::McpServerConnectionFailed {
            server_name: sse_config.name.clone(),
            error: format!(
                "Tools list request timed out after {}ms",
                self.protocol_timeout_ms
            ),
            transport_type: "sse".to_string(),
        })?
        .map_err(|e| SessionSetupError::McpServerConnectionFailed {
            server_name: sse_config.name.clone(),
            error: format!("Failed to send tools/list request: {}", e),
            transport_type: "sse".to_string(),
        })?;

        if !tools_response.status().is_success() {
            return Err(SessionSetupError::McpServerConnectionFailed {
                server_name: sse_config.name.clone(),
                error: format!(
                    "Tools list request failed with status: {}",
                    tools_response.status()
                ),
                transport_type: "sse".to_string(),
            });
        }

        let tools_body = tools_response.text().await.map_err(|e| {
            SessionSetupError::McpServerConnectionFailed {
                server_name: sse_config.name.clone(),
                error: format!("Failed to read tools SSE response: {}", e),
                transport_type: "sse".to_string(),
            }
        })?;

        let tools_response_json = Self::parse_sse_response_enhanced(&sse_config.name, &tools_body)?;
        let final_tools =
            self.extract_tools_from_list_response_enhanced(&sse_config.name, &tools_response_json)?;

        tracing::info!(
            "SSE MCP server {} provides {} tools: {:?}",
            sse_config.name,
            final_tools.len(),
            final_tools
        );

        // Spawn background task to maintain SSE event stream for server-initiated events
        let event_url = sse_config.url.clone();
        let event_client = client.clone();
        let server_name = sse_config.name.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::handle_sse_event_stream_enhanced(
                &server_name,
                event_client,
                &event_url,
                response_tx,
            )
            .await
            {
                tracing::error!("SSE event stream error for {}: {}", server_name, e);
            }
        });

        Ok(final_tools)
    }

    /// Parse SSE response body to extract JSON data
    ///
    /// SSE format specification (https://html.spec.whatwg.org/multipage/server-sent-events.html):
    /// - Lines starting with "data: " contain the actual message data
    /// - Multiple data lines can be sent for a single event
    /// - Empty line indicates end of event
    ///
    /// This implementation extracts only the first "data: " line for simplicity.
    ///
    /// # Example SSE stream format
    /// ```text
    /// data: {"jsonrpc":"2.0","id":1,"result":{"tools":[]}}
    ///
    /// data: {"jsonrpc":"2.0","method":"notification"}
    ///
    /// ```
    fn parse_sse_response_enhanced(server_name: &str, body: &str) -> SessionSetupResult<Value> {
        let mut json_data = String::new();
        for line in body.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                json_data = data.to_string();
                break;
            }
        }

        if json_data.is_empty() {
            return Err(SessionSetupError::McpServerConnectionFailed {
                server_name: server_name.to_string(),
                error: "No data in SSE response".to_string(),
                transport_type: "sse".to_string(),
            });
        }

        serde_json::from_str(&json_data).map_err(|e| SessionSetupError::McpServerConnectionFailed {
            server_name: server_name.to_string(),
            error: format!("Invalid JSON in SSE response: {}", e),
            transport_type: "sse".to_string(),
        })
    }

    /// Handle persistent SSE event stream for server-initiated events
    ///
    /// This runs in a background task and maintains a persistent connection to the SSE endpoint.
    /// It buffers incoming bytes and processes complete SSE messages line by line.
    ///
    /// The buffer size is limited to prevent memory exhaustion attacks.
    async fn handle_sse_event_stream_enhanced(
        server_name: &str,
        client: Client,
        url: &str,
        response_tx: mpsc::UnboundedSender<String>,
    ) -> SessionSetupResult<()> {
        use futures::StreamExt;

        const MAX_BUFFER_SIZE: usize = 1024 * 1024; // 1MB limit

        loop {
            tracing::debug!(
                "Establishing SSE event stream connection for {}",
                server_name
            );

            let response = client
                .get(url)
                .header("Accept", "text/event-stream")
                .send()
                .await
                .map_err(|e| SessionSetupError::McpServerConnectionFailed {
                    server_name: server_name.to_string(),
                    error: format!("Failed to establish SSE event stream: {}", e),
                    transport_type: "sse".to_string(),
                })?;

            if !response.status().is_success() {
                return Err(SessionSetupError::McpServerConnectionFailed {
                    server_name: server_name.to_string(),
                    error: format!("SSE event stream failed with status: {}", response.status()),
                    transport_type: "sse".to_string(),
                });
            }

            let mut bytes_stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = bytes_stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        let text = String::from_utf8_lossy(&chunk);

                        // Prevent unbounded buffer growth
                        if buffer.len() + text.len() > MAX_BUFFER_SIZE {
                            tracing::error!(
                                "SSE buffer exceeded maximum size for {}, resetting",
                                server_name
                            );
                            buffer.clear();
                            continue;
                        }

                        buffer.push_str(&text);

                        // Process complete lines from buffer
                        while let Some(newline_pos) = buffer.find('\n') {
                            let line = buffer[..newline_pos].to_string();
                            buffer = buffer[newline_pos + 1..].to_string();

                            if let Some(data) = line.strip_prefix("data: ") {
                                if response_tx.send(data.to_string()).is_err() {
                                    tracing::warn!(
                                        "SSE response channel closed for {}",
                                        server_name
                                    );
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error reading SSE stream for {}: {}", server_name, e);
                        break;
                    }
                }
            }

            // Connection closed, wait before reconnecting
            tracing::info!(
                "SSE connection closed for {}, reconnecting in 5 seconds",
                server_name
            );
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    /// Extract tools from tools/list response
    fn extract_tools_from_list_response_enhanced(
        &self,
        server_name: &str,
        response: &Value,
    ) -> SessionSetupResult<Vec<String>> {
        let mut tools = Vec::new();

        if let Some(result) = response.get("result") {
            if let Some(tools_array) = result.get("tools") {
                if let Some(tool_list) = tools_array.as_array() {
                    for tool in tool_list {
                        if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                            tools.push(name.to_string());
                        }
                    }
                }
            }
        }

        // If no tools found, log warning but don't fail
        if tools.is_empty() {
            tracing::warn!(
                "No tools found in MCP tools/list response for {}",
                server_name
            );
        }

        Ok(tools)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enhanced_manager_creation() {
        let manager = EnhancedMcpServerManager::new();
        let connections = manager.connections.read().await;
        assert!(connections.is_empty());
    }

    #[tokio::test]
    async fn test_connect_servers_with_invalid_config() {
        let mut manager = EnhancedMcpServerManager::new();

        let invalid_config = McpServerConfig::Stdio(crate::config::StdioTransport {
            name: "invalid_server".to_string(),
            command: "/nonexistent/command".to_string(),
            args: vec![],
            env: vec![],
            cwd: None,
        });

        let results = manager
            .connect_servers_with_validation(vec![invalid_config])
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results.get("invalid_server").unwrap().is_err());
    }

    #[tokio::test]
    async fn test_stdio_server_nonexistent_command() {
        let manager = EnhancedMcpServerManager::new();

        let config = McpServerConfig::Stdio(crate::config::StdioTransport {
            name: "test_server".to_string(),
            command: "/absolutely/nonexistent/command".to_string(),
            args: vec![],
            env: vec![],
            cwd: None,
        });

        let result = manager.connect_server_enhanced(config).await;
        assert!(result.is_err());

        if let Err(SessionSetupError::McpServerExecutableNotFound { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected McpServerExecutableNotFound error");
        }
    }

    #[tokio::test]
    async fn test_http_server_invalid_url() {
        let manager = EnhancedMcpServerManager::new();

        let config = McpServerConfig::Http(crate::config::HttpTransport {
            transport_type: "http".to_string(),
            name: "test_server".to_string(),
            url: "not-a-valid-url".to_string(),
            headers: vec![],
        });

        let result = manager.connect_server_enhanced(config).await;
        assert!(result.is_err());

        if let Err(SessionSetupError::McpServerConnectionFailed { .. }) = result {
            // Expected error type
        } else {
            panic!("Expected McpServerConnectionFailed error");
        }
    }

    #[test]
    fn test_parse_sse_response_valid() {
        let sse_body = "data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"tools\":[]}}\n\n";
        let result = EnhancedMcpServerManager::parse_sse_response_enhanced("test_server", sse_body);
        assert!(result.is_ok());
        let json = result.unwrap();
        assert_eq!(json.get("jsonrpc").and_then(|v| v.as_str()), Some("2.0"));
        assert_eq!(json.get("id").and_then(|v| v.as_i64()), Some(1));
    }

    #[test]
    fn test_parse_sse_response_multiple_data_lines() {
        let sse_body = "data: {\"jsonrpc\":\"2.0\",\"id\":1}\ndata: {\"other\":\"data\"}\n\n";
        let result = EnhancedMcpServerManager::parse_sse_response_enhanced("test_server", sse_body);
        assert!(result.is_ok());
        let json = result.unwrap();
        // Should parse only the first data line
        assert_eq!(json.get("jsonrpc").and_then(|v| v.as_str()), Some("2.0"));
        assert!(json.get("other").is_none());
    }

    #[test]
    fn test_parse_sse_response_no_data() {
        let sse_body = "event: message\n\n";
        let result = EnhancedMcpServerManager::parse_sse_response_enhanced("test_server", sse_body);
        assert!(result.is_err());
        if let Err(SessionSetupError::McpServerConnectionFailed { error, .. }) = result {
            assert!(error.contains("No data in SSE response"));
        } else {
            panic!("Expected McpServerConnectionFailed error");
        }
    }

    #[test]
    fn test_parse_sse_response_invalid_json() {
        let sse_body = "data: not valid json\n\n";
        let result = EnhancedMcpServerManager::parse_sse_response_enhanced("test_server", sse_body);
        assert!(result.is_err());
        if let Err(SessionSetupError::McpServerConnectionFailed { error, .. }) = result {
            assert!(error.contains("Invalid JSON in SSE response"));
        } else {
            panic!("Expected McpServerConnectionFailed error");
        }
    }

    #[test]
    fn test_extract_tools_from_list_response_valid() {
        let manager = EnhancedMcpServerManager::new();
        let response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": [
                    {"name": "tool1", "description": "First tool"},
                    {"name": "tool2", "description": "Second tool"}
                ]
            }
        });

        let result = manager.extract_tools_from_list_response_enhanced("test_server", &response);
        assert!(result.is_ok());
        let tools = result.unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0], "tool1");
        assert_eq!(tools[1], "tool2");
    }

    #[test]
    fn test_extract_tools_from_list_response_empty() {
        let manager = EnhancedMcpServerManager::new();
        let response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": []
            }
        });

        let result = manager.extract_tools_from_list_response_enhanced("test_server", &response);
        assert!(result.is_ok());
        let tools = result.unwrap();
        assert_eq!(tools.len(), 0);
    }

    #[test]
    fn test_extract_tools_from_list_response_no_result() {
        let manager = EnhancedMcpServerManager::new();
        let response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "error": {
                "code": -32600,
                "message": "Invalid request"
            }
        });

        let result = manager.extract_tools_from_list_response_enhanced("test_server", &response);
        assert!(result.is_ok());
        let tools = result.unwrap();
        assert_eq!(tools.len(), 0);
    }

    #[test]
    fn test_extract_tools_from_list_response_malformed() {
        let manager = EnhancedMcpServerManager::new();
        let response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": [
                    {"name": "tool1"},
                    {"description": "tool without name"},
                    {"name": "tool2"}
                ]
            }
        });

        let result = manager.extract_tools_from_list_response_enhanced("test_server", &response);
        assert!(result.is_ok());
        let tools = result.unwrap();
        // Should skip the tool without a name
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0], "tool1");
        assert_eq!(tools[1], "tool2");
    }

    #[tokio::test]
    async fn test_manager_with_custom_timeouts() {
        let manager = EnhancedMcpServerManager::with_timeouts(5000, 2000);
        assert_eq!(manager.connection_timeout_ms, 5000);
        assert_eq!(manager.protocol_timeout_ms, 2000);
    }

    #[tokio::test]
    async fn test_manager_default_trait() {
        let manager = EnhancedMcpServerManager::default();
        assert_eq!(manager.connection_timeout_ms, 30000);
        assert_eq!(manager.protocol_timeout_ms, 10000);
    }

    #[tokio::test]
    async fn test_connect_servers_multiple_configs() {
        let mut manager = EnhancedMcpServerManager::new();

        let config1 = McpServerConfig::Stdio(crate::config::StdioTransport {
            name: "server1".to_string(),
            command: "/nonexistent1".to_string(),
            args: vec![],
            env: vec![],
            cwd: None,
        });

        let config2 = McpServerConfig::Stdio(crate::config::StdioTransport {
            name: "server2".to_string(),
            command: "/nonexistent2".to_string(),
            args: vec![],
            env: vec![],
            cwd: None,
        });

        let results = manager
            .connect_servers_with_validation(vec![config1, config2])
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.get("server1").unwrap().is_err());
        assert!(results.get("server2").unwrap().is_err());
    }

    #[tokio::test]
    async fn test_stdio_invalid_working_directory() {
        let manager = EnhancedMcpServerManager::new();

        let config = McpServerConfig::Stdio(crate::config::StdioTransport {
            name: "test_server".to_string(),
            command: "echo".to_string(),
            args: vec![],
            env: vec![],
            cwd: Some("/nonexistent/directory/path".to_string()),
        });

        let result = manager.connect_server_enhanced(config).await;
        assert!(result.is_err());

        if let Err(SessionSetupError::McpServerConnectionFailed { error, .. }) = result {
            assert!(error.contains("Working directory does not exist"));
        } else {
            panic!("Expected McpServerConnectionFailed with working directory error");
        }
    }

    #[tokio::test]
    async fn test_http_invalid_header_name() {
        let manager = EnhancedMcpServerManager::new();

        let config = McpServerConfig::Http(crate::config::HttpTransport {
            transport_type: "http".to_string(),
            name: "test_server".to_string(),
            url: "http://localhost:8080".to_string(),
            headers: vec![crate::config::HttpHeader {
                name: "Invalid\nHeader".to_string(), // Invalid header with newline
                value: "value".to_string(),
            }],
        });

        let result = manager.connect_server_enhanced(config).await;
        assert!(result.is_err());

        if let Err(SessionSetupError::McpServerConnectionFailed { error, .. }) = result {
            assert!(error.contains("Invalid header"));
        } else {
            panic!("Expected McpServerConnectionFailed with invalid header error");
        }
    }

    #[tokio::test]
    async fn test_sse_invalid_url() {
        let manager = EnhancedMcpServerManager::new();

        let config = McpServerConfig::Sse(crate::config::SseTransport {
            transport_type: "sse".to_string(),
            name: "test_server".to_string(),
            url: "not-a-url".to_string(),
            headers: vec![],
        });

        let result = manager.connect_server_enhanced(config).await;
        assert!(result.is_err());

        if let Err(SessionSetupError::McpServerConnectionFailed { error, .. }) = result {
            assert!(error.contains("Invalid URL format"));
        } else {
            panic!("Expected McpServerConnectionFailed with URL error");
        }
    }

    #[test]
    fn test_parse_sse_response_with_whitespace() {
        let sse_body = "  \ndata: {\"test\": \"value\"}\n  \n";
        let result = EnhancedMcpServerManager::parse_sse_response_enhanced("test", sse_body);
        assert!(result.is_ok());
        let json = result.unwrap();
        assert_eq!(json.get("test").and_then(|v| v.as_str()), Some("value"));
    }

    #[test]
    fn test_parse_sse_response_empty_body() {
        let sse_body = "";
        let result = EnhancedMcpServerManager::parse_sse_response_enhanced("test", sse_body);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_tools_missing_tools_array() {
        let manager = EnhancedMcpServerManager::new();
        let response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {}
        });

        let result = manager.extract_tools_from_list_response_enhanced("test", &response);
        assert!(result.is_ok());
        let tools = result.unwrap();
        assert_eq!(tools.len(), 0);
    }

    #[test]
    fn test_extract_tools_tools_not_array() {
        let manager = EnhancedMcpServerManager::new();
        let response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": "not an array"
            }
        });

        let result = manager.extract_tools_from_list_response_enhanced("test", &response);
        assert!(result.is_ok());
        let tools = result.unwrap();
        assert_eq!(tools.len(), 0);
    }
}
