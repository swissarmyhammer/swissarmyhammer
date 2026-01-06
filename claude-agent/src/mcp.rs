//! Model Context Protocol (MCP) server integration
//!
//! This module provides the infrastructure for connecting to and communicating
//! with external MCP servers to extend the agent's tool capabilities beyond
//! the built-in file system and terminal operations.

use crate::{config::McpServerConfig, error::McpError, tools::InternalToolRequest};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_common::is_prompt_visible;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, RwLock};

/// Transport-specific connection details
#[derive(Debug)]
pub enum TransportConnection {
    /// Stdio transport using child process
    Stdio {
        process: Arc<RwLock<Option<Child>>>,
        stdin_writer: Arc<RwLock<Option<BufWriter<tokio::process::ChildStdin>>>>,
        stdout_reader: Arc<RwLock<Option<BufReader<tokio::process::ChildStdout>>>>,
    },
    /// HTTP transport using reqwest client
    Http {
        client: Arc<Client>,
        url: String,
        headers: Vec<crate::config::HttpHeader>,
        session_id: Arc<RwLock<Option<String>>>,
    },
    /// SSE transport using WebSocket connection
    Sse {
        url: String,
        headers: Vec<crate::config::HttpHeader>,
        message_tx: Arc<RwLock<Option<mpsc::UnboundedSender<String>>>>,
        response_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<String>>>>,
    },
}

/// MCP prompt argument metadata
#[derive(Debug, Clone)]
pub struct McpPromptArgument {
    /// The name of the argument
    pub name: String,
    /// Optional description of what the argument is for
    pub description: Option<String>,
    /// Whether this argument is required or optional
    pub required: bool,
}

/// MCP prompt metadata
#[derive(Debug, Clone)]
pub struct McpPrompt {
    /// The name of the prompt (used as slash command)
    pub name: String,
    /// Optional human-readable description
    pub description: Option<String>,
    /// List of arguments this prompt accepts
    pub arguments: Vec<McpPromptArgument>,
}

/// Represents a connection to an MCP server
#[derive(Debug)]
pub struct McpServerConnection {
    /// Name of the MCP server
    pub name: String,
    /// List of tools available from this server
    pub tools: Vec<String>,
    /// List of prompts available from this server (for slash commands)
    pub prompts: Vec<McpPrompt>,
    /// Configuration used to create this connection
    pub config: McpServerConfig,
    /// Transport-specific connection details
    pub transport: TransportConnection,
}

/// Manages connections to multiple MCP servers
#[derive(Debug)]
pub struct McpServerManager {
    /// Map of server name to connection
    connections: Arc<RwLock<HashMap<String, McpServerConnection>>>,
}

impl McpServerManager {
    /// Create a new MCP server manager
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Connect to all configured MCP servers
    pub async fn connect_servers(&mut self, configs: Vec<McpServerConfig>) -> crate::Result<()> {
        for config in configs {
            let config_name = config.name().to_string();
            match self.connect_server(config).await {
                Ok(connection) => {
                    let connection_name = connection.name.clone();
                    tracing::info!("Connected to MCP server: {}", connection_name);
                    let mut connections = self.connections.write().await;
                    connections.insert(connection_name, connection);
                }
                Err(e) => {
                    tracing::error!("Failed to connect to MCP server {}: {}", config_name, e);
                    // Continue with other servers instead of failing completely
                }
            }
        }
        Ok(())
    }

    /// Connect to a single MCP server
    async fn connect_server(&self, config: McpServerConfig) -> crate::Result<McpServerConnection> {
        // Only stdio transport is currently implemented in the connection logic
        match &config {
            McpServerConfig::Stdio(stdio_config) => {
                tracing::info!(
                    "Connecting to MCP server: {} ({})",
                    stdio_config.name,
                    stdio_config.command
                );

                // Start the MCP server process with environment variables
                let mut command = Command::new(&stdio_config.command);
                command.args(&stdio_config.args);

                // Set working directory if provided
                if let Some(cwd) = &stdio_config.cwd {
                    command.current_dir(cwd);
                }

                // Set environment variables
                for env_var in &stdio_config.env {
                    command.env(&env_var.name, &env_var.value);
                }

                let mut child = command
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
                    .map_err(|e| McpError::ProcessSpawnFailed(stdio_config.name.clone(), e))?;

                // Get stdio handles
                let stdin = child.stdin.take().ok_or(McpError::StdinNotAvailable)?;
                let stdout = child.stdout.take().ok_or(McpError::StdoutNotAvailable)?;

                let mut stdin_writer = BufWriter::new(stdin);
                let mut stdout_reader = BufReader::new(stdout);

                // Initialize MCP protocol
                let (tools, prompts) = self
                    .initialize_mcp_connection(
                        &mut stdin_writer,
                        &mut stdout_reader,
                        &stdio_config.name,
                        stdio_config,
                    )
                    .await?;

                let transport = TransportConnection::Stdio {
                    process: Arc::new(RwLock::new(Some(child))),
                    stdin_writer: Arc::new(RwLock::new(Some(stdin_writer))),
                    stdout_reader: Arc::new(RwLock::new(Some(stdout_reader))),
                };

                let connection = McpServerConnection {
                    name: stdio_config.name.clone(),
                    tools,
                    prompts,
                    config,
                    transport,
                };

                Ok(connection)
            }
            McpServerConfig::Http(http_config) => {
                tracing::info!(
                    "Connecting to HTTP MCP server: {} ({})",
                    http_config.name,
                    http_config.url
                );

                // Create HTTP client with headers
                let client_builder = Client::builder();
                let mut headers = reqwest::header::HeaderMap::new();

                for header in &http_config.headers {
                    if let (Ok(name), Ok(value)) = (
                        reqwest::header::HeaderName::from_bytes(header.name.as_bytes()),
                        reqwest::header::HeaderValue::from_str(&header.value),
                    ) {
                        headers.insert(name, value);
                    }
                }

                let client = client_builder
                    .default_headers(headers)
                    .build()
                    .map_err(|e| {
                        crate::AgentError::ToolExecution(format!(
                            "Failed to create HTTP client for MCP server {}: {}",
                            http_config.name, e
                        ))
                    })?;

                // Initialize MCP connection via HTTP
                let session_id = Arc::new(RwLock::new(None));
                let (tools, prompts) = self
                    .initialize_http_mcp_connection(&client, http_config, Arc::clone(&session_id))
                    .await?;

                let transport = TransportConnection::Http {
                    client: Arc::new(client),
                    url: http_config.url.clone(),
                    headers: http_config.headers.clone(),
                    session_id,
                };

                let connection = McpServerConnection {
                    name: http_config.name.clone(),
                    tools,
                    prompts,
                    config,
                    transport,
                };

                Ok(connection)
            }
            McpServerConfig::Sse(sse_config) => {
                tracing::info!(
                    "Connecting to SSE MCP server: {} ({})",
                    sse_config.name,
                    sse_config.url
                );

                // Create SSE connection channels
                let (message_tx, _message_rx) = mpsc::unbounded_channel();
                let (response_tx, response_rx) = mpsc::unbounded_channel();

                // Initialize SSE connection
                let (tools, prompts) = self
                    .initialize_sse_mcp_connection(sse_config, response_tx)
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
                    prompts,
                    config,
                    transport,
                };

                Ok(connection)
            }
        }
    }

    /// Initialize the MCP protocol connection
    async fn initialize_mcp_connection(
        &self,
        writer: &mut BufWriter<tokio::process::ChildStdin>,
        reader: &mut BufReader<tokio::process::ChildStdout>,
        server_name: &str,
        _config: &crate::config::StdioTransport,
    ) -> crate::Result<(Vec<String>, Vec<McpPrompt>)> {
        // Send initialize request
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

        let request_line = format!("{}\n", initialize_request);
        writer
            .write_all(request_line.as_bytes())
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to write initialize request to MCP server: {}",
                    e
                ))
            })?;
        writer.flush().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Failed to flush initialize request to MCP server: {}",
                e
            ))
        })?;

        // Read initialize response
        let mut response_line = String::new();
        let bytes_read = reader
            .read_line(&mut response_line)
            .await
            .map_err(McpError::IoError)?;

        if bytes_read == 0 {
            return Err(McpError::ConnectionClosed.into());
        }

        let response: Value =
            serde_json::from_str(response_line.trim()).map_err(McpError::SerializationFailed)?;

        // Extract available tools from response
        let _tools = self.extract_tools_from_initialize_response(&response)?;

        // Send initialized notification
        let initialized_notification = json!({
            "jsonrpc": "2.0",
            "method": "initialized"
        });

        let notification_line = format!("{}\n", initialized_notification);
        writer
            .write_all(notification_line.as_bytes())
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to send initialized notification to MCP server: {}",
                    e
                ))
            })?;
        writer.flush().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Failed to flush initialized notification to MCP server: {}",
                e
            ))
        })?;

        // Request list of available tools
        let tools_list_request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });

        let tools_request_line = format!("{}\n", tools_list_request);
        writer
            .write_all(tools_request_line.as_bytes())
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to write tools/list request to MCP server: {}",
                    e
                ))
            })?;
        writer.flush().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Failed to flush tools/list request to MCP server: {}",
                e
            ))
        })?;

        // Read tools list response
        let mut tools_response_line = String::new();
        let tools_bytes_read = reader
            .read_line(&mut tools_response_line)
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to read tools/list response from MCP server: {}",
                    e
                ))
            })?;

        if tools_bytes_read == 0 {
            return Err(crate::AgentError::ToolExecution(
                "MCP server closed connection during tools/list request".to_string(),
            ));
        }

        let tools_response: Value =
            serde_json::from_str(tools_response_line.trim()).map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Invalid JSON from MCP server tools/list response: {}",
                    e
                ))
            })?;

        let final_tools = self.extract_tools_from_list_response(&tools_response)?;

        tracing::info!(
            "MCP server {} provides {} tools: {:?}",
            server_name,
            final_tools.len(),
            final_tools
        );

        // Request list of available prompts
        let prompts_list_request = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "prompts/list"
        });

        let prompts_request_line = format!("{}\n", prompts_list_request);
        writer
            .write_all(prompts_request_line.as_bytes())
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to write prompts/list request to MCP server: {}",
                    e
                ))
            })?;
        writer.flush().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Failed to flush prompts/list request to MCP server: {}",
                e
            ))
        })?;

        // Read prompts list response
        let mut prompts_response_line = String::new();
        let prompts_bytes_read =
            reader
                .read_line(&mut prompts_response_line)
                .await
                .map_err(|e| {
                    crate::AgentError::ToolExecution(format!(
                        "Failed to read prompts/list response from MCP server: {}",
                        e
                    ))
                })?;

        let final_prompts = if prompts_bytes_read == 0 {
            tracing::warn!("MCP server closed connection during prompts/list request");
            Vec::new()
        } else {
            let prompts_response: Value = serde_json::from_str(prompts_response_line.trim())
                .map_err(|e| {
                    crate::AgentError::ToolExecution(format!(
                        "Invalid JSON from MCP server prompts/list response: {}",
                        e
                    ))
                })?;

            self.extract_prompts_from_list_response(&prompts_response)?
        };

        tracing::info!(
            "MCP server {} provides {} prompts: {:?}",
            server_name,
            final_prompts.len(),
            final_prompts.iter().map(|p| &p.name).collect::<Vec<_>>()
        );

        Ok((final_tools, final_prompts))
    }

    /// Initialize HTTP MCP connection using the MCP Streamable HTTP transport protocol.
    ///
    /// Implements the three-step initialization handshake:
    /// 1. Send initialize request and parse response (JSON or SSE)
    /// 2. Send initialized notification (expects HTTP 202)
    /// 3. Request tools list and extract tool names
    ///
    /// # Arguments
    /// * `client` - HTTP client to use for requests
    /// * `config` - HTTP transport configuration including URL and headers
    /// * `session_id` - Arc-wrapped session ID storage for subsequent requests
    ///
    /// # Returns
    /// List of available tool names from the MCP server
    ///
    /// # Errors
    /// Returns error if:
    /// - Connection fails
    /// - Server returns non-success status
    /// - Response parsing fails
    /// - Protocol negotiation fails
    async fn initialize_http_mcp_connection(
        &self,
        client: &Client,
        config: &crate::config::HttpTransport,
        session_id: Arc<RwLock<Option<String>>>,
    ) -> crate::Result<(Vec<String>, Vec<McpPrompt>)> {
        tracing::info!("Initializing HTTP MCP protocol for {}", config.name);

        // Step 1: Send initialize request via HTTP POST
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

        let response = client
            .post(&config.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .json(&initialize_request)
            .send()
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to send initialize request to HTTP MCP server: {}",
                    e
                ))
            })?;

        if !response.status().is_success() {
            return Err(crate::AgentError::ToolExecution(format!(
                "Initialize request failed with status: {}",
                response.status()
            )));
        }

        // Extract session ID if present
        if let Some(session_id_header) = response.headers().get("Mcp-Session-Id") {
            if let Ok(session_id_str) = session_id_header.to_str() {
                let mut session_id_write = session_id.write().await;
                *session_id_write = Some(session_id_str.to_string());
                tracing::debug!("Stored session ID: {}", session_id_str);
            }
        }

        // Parse response body - handle both JSON and SSE formats
        let content_type = response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json");

        let initialize_response: Value = if content_type.contains("text/event-stream") {
            // Handle SSE response format
            let body = response.text().await.map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to read SSE response from HTTP MCP server: {}",
                    e
                ))
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
                return Err(crate::AgentError::ToolExecution(
                    "No data in SSE response from HTTP MCP server".to_string(),
                ));
            }

            serde_json::from_str(&json_data).map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Invalid JSON in SSE response from HTTP MCP server: {}",
                    e
                ))
            })?
        } else {
            // Handle JSON response format
            response.json().await.map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to parse initialize response from HTTP MCP server: {}",
                    e
                ))
            })?
        };

        // Validate initialize response
        if let Some(error) = initialize_response.get("error") {
            return Err(crate::AgentError::ToolExecution(format!(
                "HTTP MCP server returned error: {}",
                error
            )));
        }

        // Step 2: Send initialized notification
        let initialized_notification = json!({
            "jsonrpc": "2.0",
            "method": "initialized"
        });

        let mut notify_request = client
            .post(&config.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json");

        // Include session ID if present
        if let Some(session_id_value) = session_id.read().await.as_ref() {
            notify_request = notify_request.header("Mcp-Session-Id", session_id_value);
        }

        let notify_response = notify_request
            .json(&initialized_notification)
            .send()
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to send initialized notification to HTTP MCP server: {}",
                    e
                ))
            })?;

        // Expect 202 Accepted for notification
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

        let mut tools_request = client
            .post(&config.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json");

        // Include session ID if present
        if let Some(session_id_value) = session_id.read().await.as_ref() {
            tools_request = tools_request.header("Mcp-Session-Id", session_id_value);
        }

        let tools_response = tools_request
            .json(&tools_list_request)
            .send()
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to send tools/list request to HTTP MCP server: {}",
                    e
                ))
            })?;

        if !tools_response.status().is_success() {
            return Err(crate::AgentError::ToolExecution(format!(
                "Tools list request failed with status: {}",
                tools_response.status()
            )));
        }

        // Parse tools response - handle both JSON and SSE formats
        let tools_content_type = tools_response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json");

        let tools_response_json: Value = if tools_content_type.contains("text/event-stream") {
            // Handle SSE response format
            let body = tools_response.text().await.map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to read tools SSE response from HTTP MCP server: {}",
                    e
                ))
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
                return Err(crate::AgentError::ToolExecution(
                    "No data in tools SSE response from HTTP MCP server".to_string(),
                ));
            }

            serde_json::from_str(&json_data).map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Invalid JSON in tools SSE response from HTTP MCP server: {}",
                    e
                ))
            })?
        } else {
            // Handle JSON response format
            tools_response.json().await.map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to parse tools/list response from HTTP MCP server: {}",
                    e
                ))
            })?
        };

        let final_tools = self.extract_tools_from_list_response(&tools_response_json)?;

        tracing::info!(
            "HTTP MCP server {} provides {} tools: {:?}",
            config.name,
            final_tools.len(),
            final_tools
        );

        // Step 4: Request list of available prompts
        let prompts_list_request = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "prompts/list"
        });

        let mut prompts_request = client
            .post(&config.url)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json");

        // Include session ID if present
        if let Some(session_id_value) = session_id.read().await.as_ref() {
            prompts_request = prompts_request.header("Mcp-Session-Id", session_id_value);
        }

        let prompts_response = prompts_request
            .json(&prompts_list_request)
            .send()
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to send prompts/list request to HTTP MCP server: {}",
                    e
                ))
            })?;

        let final_prompts = if !prompts_response.status().is_success() {
            tracing::warn!(
                "Prompts list request failed with status: {} (skipping prompts)",
                prompts_response.status()
            );
            Vec::new()
        } else {
            // Parse prompts response - handle both JSON and SSE formats
            let prompts_content_type = prompts_response
                .headers()
                .get("Content-Type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/json");

            let prompts_response_json: Value = if prompts_content_type.contains("text/event-stream")
            {
                // Handle SSE response format
                let body = prompts_response.text().await.map_err(|e| {
                    crate::AgentError::ToolExecution(format!(
                        "Failed to read prompts SSE response from HTTP MCP server: {}",
                        e
                    ))
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
                    tracing::warn!("No data in prompts SSE response from HTTP MCP server");
                    serde_json::json!({"result": {"prompts": []}})
                } else {
                    serde_json::from_str(&json_data).map_err(|e| {
                        crate::AgentError::ToolExecution(format!(
                            "Invalid JSON in prompts SSE response from HTTP MCP server: {}",
                            e
                        ))
                    })?
                }
            } else {
                // Handle JSON response format
                prompts_response.json().await.map_err(|e| {
                    crate::AgentError::ToolExecution(format!(
                        "Failed to parse prompts/list response from HTTP MCP server: {}",
                        e
                    ))
                })?
            };

            self.extract_prompts_from_list_response(&prompts_response_json)?
        };

        tracing::info!(
            "HTTP MCP server {} provides {} prompts: {:?}",
            config.name,
            final_prompts.len(),
            final_prompts.iter().map(|p| &p.name).collect::<Vec<_>>()
        );

        Ok((final_tools, final_prompts))
    }

    /// Initialize SSE MCP connection using persistent event stream.
    ///
    /// SSE transport uses a persistent GET connection for receiving server events
    /// and POST requests for sending client requests (initialize, tools/call, etc).
    ///
    /// # Arguments
    /// * `config` - SSE transport configuration including URL and headers
    /// * `response_tx` - Channel for sending responses from the event stream
    ///
    /// # Returns
    /// List of available tool names from the MCP server
    ///
    /// # Errors
    /// Returns error if:
    /// - Connection fails
    /// - Server returns non-success status
    /// - Response parsing fails
    /// - Protocol negotiation fails
    async fn initialize_sse_mcp_connection(
        &self,
        config: &crate::config::SseTransport,
        response_tx: mpsc::UnboundedSender<String>,
    ) -> crate::Result<(Vec<String>, Vec<McpPrompt>)> {
        tracing::info!("Initializing SSE MCP protocol for {}", config.name);

        // Create HTTP client with headers
        let mut headers = reqwest::header::HeaderMap::new();
        for header in &config.headers {
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
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to create HTTP client for SSE MCP server {}: {}",
                    config.name, e
                ))
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

        let response = client
            .post(&config.url)
            .header("Accept", "text/event-stream")
            .header("Content-Type", "application/json")
            .json(&initialize_request)
            .send()
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to send initialize request to SSE MCP server: {}",
                    e
                ))
            })?;

        if !response.status().is_success() {
            return Err(crate::AgentError::ToolExecution(format!(
                "Initialize request failed with status: {}",
                response.status()
            )));
        }

        // Parse SSE response for initialize
        let body = response.text().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Failed to read SSE response from MCP server: {}",
                e
            ))
        })?;

        let initialize_response = Self::parse_sse_response(&body)?;

        // Validate initialize response
        if let Some(error) = initialize_response.get("error") {
            return Err(crate::AgentError::ToolExecution(format!(
                "SSE MCP server returned error: {}",
                error
            )));
        }

        // Step 2: Send initialized notification
        let initialized_notification = json!({
            "jsonrpc": "2.0",
            "method": "initialized"
        });

        let notify_response = client
            .post(&config.url)
            .header("Accept", "text/event-stream")
            .header("Content-Type", "application/json")
            .json(&initialized_notification)
            .send()
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to send initialized notification to SSE MCP server: {}",
                    e
                ))
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

        let tools_response = client
            .post(&config.url)
            .header("Accept", "text/event-stream")
            .header("Content-Type", "application/json")
            .json(&tools_list_request)
            .send()
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to send tools/list request to SSE MCP server: {}",
                    e
                ))
            })?;

        if !tools_response.status().is_success() {
            return Err(crate::AgentError::ToolExecution(format!(
                "Tools list request failed with status: {}",
                tools_response.status()
            )));
        }

        let tools_body = tools_response.text().await.map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Failed to read tools SSE response from MCP server: {}",
                e
            ))
        })?;

        let tools_response_json = Self::parse_sse_response(&tools_body)?;
        let final_tools = self.extract_tools_from_list_response(&tools_response_json)?;

        tracing::info!(
            "SSE MCP server {} provides {} tools: {:?}",
            config.name,
            final_tools.len(),
            final_tools
        );

        // Step 4: Request list of available prompts
        let prompts_list_request = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "prompts/list"
        });

        let prompts_response = client
            .post(&config.url)
            .header("Accept", "text/event-stream")
            .header("Content-Type", "application/json")
            .json(&prompts_list_request)
            .send()
            .await
            .map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to send prompts/list request to SSE MCP server: {}",
                    e
                ))
            })?;

        let final_prompts = if !prompts_response.status().is_success() {
            tracing::warn!(
                "Prompts list request failed with status: {} (skipping prompts)",
                prompts_response.status()
            );
            Vec::new()
        } else {
            let prompts_body = prompts_response.text().await.map_err(|e| {
                crate::AgentError::ToolExecution(format!(
                    "Failed to read prompts SSE response from MCP server: {}",
                    e
                ))
            })?;

            let prompts_response_json = Self::parse_sse_response(&prompts_body)?;
            self.extract_prompts_from_list_response(&prompts_response_json)?
        };

        tracing::info!(
            "SSE MCP server {} provides {} prompts: {:?}",
            config.name,
            final_prompts.len(),
            final_prompts.iter().map(|p| &p.name).collect::<Vec<_>>()
        );

        // Spawn background task to maintain SSE event stream for server-initiated events
        let event_url = config.url.clone();
        let event_client = client.clone();
        tokio::spawn(async move {
            if let Err(e) =
                Self::handle_sse_event_stream(event_client, &event_url, response_tx).await
            {
                tracing::error!("SSE event stream error: {}", e);
            }
        });

        Ok((final_tools, final_prompts))
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
    fn parse_sse_response(body: &str) -> crate::Result<Value> {
        let mut json_data = String::new();
        for line in body.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                json_data = data.to_string();
                break;
            }
        }

        if json_data.is_empty() {
            return Err(crate::AgentError::ToolExecution(
                "No data in SSE response from MCP server".to_string(),
            ));
        }

        serde_json::from_str(&json_data).map_err(|e| {
            crate::AgentError::ToolExecution(format!(
                "Invalid JSON in SSE response from MCP server: {}",
                e
            ))
        })
    }

    /// Handle persistent SSE event stream for server-initiated events
    ///
    /// This runs in a background task and maintains a persistent connection to the SSE endpoint.
    /// It buffers incoming bytes and processes complete SSE messages line by line.
    ///
    /// The buffer size is limited to prevent memory exhaustion attacks.
    ///
    /// # Example SSE stream
    /// ```text
    /// data: {"jsonrpc":"2.0","method":"tools/list_changed"}
    ///
    /// data: {"jsonrpc":"2.0","method":"progress","params":{"progressToken":"token1"}}
    ///
    /// ```
    async fn handle_sse_event_stream(
        client: Client,
        url: &str,
        response_tx: mpsc::UnboundedSender<String>,
    ) -> crate::Result<()> {
        use futures::StreamExt;

        const MAX_BUFFER_SIZE: usize = 1024 * 1024; // 1MB limit

        loop {
            tracing::debug!("Establishing SSE event stream connection");

            let response = client
                .get(url)
                .header("Accept", "text/event-stream")
                .send()
                .await
                .map_err(|e| {
                    crate::AgentError::ToolExecution(format!(
                        "Failed to establish SSE event stream: {}",
                        e
                    ))
                })?;

            if !response.status().is_success() {
                return Err(crate::AgentError::ToolExecution(format!(
                    "SSE event stream failed with status: {}",
                    response.status()
                )));
            }

            let mut bytes_stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = bytes_stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        let text = String::from_utf8_lossy(&chunk);

                        // Prevent unbounded buffer growth
                        if buffer.len() + text.len() > MAX_BUFFER_SIZE {
                            tracing::error!("SSE buffer exceeded maximum size, resetting");
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
                                    tracing::warn!("SSE response channel closed");
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error reading SSE stream: {}", e);
                        break;
                    }
                }
            }

            // Connection closed, wait before reconnecting
            tracing::info!("SSE connection closed, reconnecting in 5 seconds");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    /// Extract tools from initialize response
    ///
    /// The MCP initialize response may include server capabilities, but does not
    /// directly list available tools. Tools are obtained via the separate tools/list
    /// request. This function validates the response structure and extracts any
    /// capability information for future use.
    ///
    /// According to the MCP specification, capabilities.tools is metadata about
    /// tool support (like listChanged notification support), not the actual tool list.
    fn extract_tools_from_initialize_response(
        &self,
        response: &Value,
    ) -> crate::Result<Vec<String>> {
        // Validate response structure
        if let Some(result) = response.get("result") {
            if let Some(capabilities) = result.get("capabilities") {
                if let Some(tools_capability) = capabilities.get("tools") {
                    tracing::debug!(
                        "Server supports tools capability: {}",
                        serde_json::to_string(tools_capability).unwrap_or_default()
                    );
                }
            }
        }

        // According to MCP specification, tools are obtained from tools/list request,
        // not from the initialize response. Return empty list here.
        Ok(Vec::new())
    }

    /// Extract tools from tools/list response
    fn extract_tools_from_list_response(&self, response: &Value) -> crate::Result<Vec<String>> {
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
            tracing::warn!("No tools found in MCP tools/list response");
        }

        Ok(tools)
    }

    /// Extract prompts from prompts/list response
    fn extract_prompts_from_list_response(
        &self,
        response: &Value,
    ) -> crate::Result<Vec<McpPrompt>> {
        tracing::trace!(
            "Extracting prompts from response: {}",
            serde_json::to_string_pretty(response).unwrap_or_default()
        );

        let mut prompts = Vec::new();

        if let Some(result) = response.get("result") {
            if let Some(prompts_array) = result.get("prompts") {
                if let Some(prompt_list) = prompts_array.as_array() {
                    for prompt in prompt_list {
                        if let Some(name) = prompt.get("name").and_then(|n| n.as_str()) {
                            let description = prompt
                                .get("description")
                                .and_then(|d| d.as_str())
                                .map(|s| s.to_string());

                            let mut arguments = Vec::new();
                            if let Some(args_array) =
                                prompt.get("arguments").and_then(|a| a.as_array())
                            {
                                for arg in args_array {
                                    if let Some(arg_name) = arg.get("name").and_then(|n| n.as_str())
                                    {
                                        let arg_description = arg
                                            .get("description")
                                            .and_then(|d| d.as_str())
                                            .map(|s| s.to_string());
                                        let required = arg
                                            .get("required")
                                            .and_then(|r| r.as_bool())
                                            .unwrap_or(false);

                                        arguments.push(McpPromptArgument {
                                            name: arg_name.to_string(),
                                            description: arg_description,
                                            required,
                                        });
                                    }
                                }
                            }

                            prompts.push(McpPrompt {
                                name: name.to_string(),
                                description,
                                arguments,
                            });
                        }
                    }
                }
            }
        }

        // If no prompts found, log debug message (prompts are optional)
        if prompts.is_empty() {
            tracing::debug!("No prompts found in MCP prompts/list response");
        } else {
            tracing::debug!(
                "Extracted {} prompts: {:?}",
                prompts.len(),
                prompts.iter().map(|p| &p.name).collect::<Vec<_>>()
            );
        }

        Ok(prompts)
    }

    /// Execute a tool call on the specified MCP server
    pub async fn execute_tool_call(
        &self,
        server_name: &str,
        tool_call: &InternalToolRequest,
    ) -> crate::Result<String> {
        let connections = self.connections.read().await;
        let connection = connections.get(server_name).ok_or_else(|| {
            McpError::InvalidConfiguration(format!("MCP server '{}' not found", server_name))
        })?;

        // Send tool call to the server
        let response_content = self.send_tool_call_to_server(connection, tool_call).await?;

        // Convert MCP response to string result
        self.process_tool_call_response(&response_content)
    }

    /// Send a tool call request to an MCP server
    async fn send_tool_call_to_server(
        &self,
        connection: &McpServerConnection,
        tool_call: &InternalToolRequest,
    ) -> crate::Result<Value> {
        // Create MCP tool call request
        let mcp_request = json!({
            "jsonrpc": "2.0",
            "id": tool_call.id.clone(),
            "method": "tools/call",
            "params": {
                "name": tool_call.name.split(':').nth(1).unwrap_or(&tool_call.name),
                "arguments": tool_call.arguments
            }
        });

        tracing::info!(
            "Sending tool call to MCP server {}: {}",
            connection.name,
            tool_call.name
        );

        match &connection.transport {
            TransportConnection::Stdio {
                stdin_writer,
                stdout_reader,
                ..
            } => {
                // Get writer and reader
                let mut writer_guard = stdin_writer.write().await;
                let writer = writer_guard.as_mut().ok_or(McpError::StdinNotAvailable)?;

                let mut reader_guard = stdout_reader.write().await;
                let reader = reader_guard.as_mut().ok_or(McpError::StdoutNotAvailable)?;

                // Send request
                let request_line = format!("{}\n", mcp_request);
                writer
                    .write_all(request_line.as_bytes())
                    .await
                    .map_err(McpError::IoError)?;
                writer.flush().await.map_err(McpError::IoError)?;

                // Read response
                let mut response_line = String::new();
                let bytes_read = reader
                    .read_line(&mut response_line)
                    .await
                    .map_err(McpError::IoError)?;

                if bytes_read == 0 {
                    return Err(McpError::ConnectionClosed.into());
                }

                let response: Value = serde_json::from_str(response_line.trim())
                    .map_err(McpError::SerializationFailed)?;

                Ok(response)
            }
            TransportConnection::Http {
                client,
                url,
                session_id,
                ..
            } => {
                // Send HTTP request with session ID if available
                let mut request = client
                    .post(url)
                    .header("Accept", "application/json, text/event-stream")
                    .header("Content-Type", "application/json");

                // Include session ID if present
                if let Some(session_id_value) = session_id.read().await.as_ref() {
                    request = request.header("Mcp-Session-Id", session_id_value);
                }

                let response = request.json(&mcp_request).send().await.map_err(|e| {
                    crate::AgentError::ToolExecution(format!(
                        "Failed to send HTTP tool call request to MCP server: {}",
                        e
                    ))
                })?;

                let response_json: Value = response.json().await.map_err(|e| {
                    McpError::ProtocolError(format!(
                        "Failed to parse HTTP tool call response from MCP server: {}",
                        e
                    ))
                })?;

                Ok(response_json)
            }
            TransportConnection::Sse { url, headers, .. } => {
                // Create HTTP client with headers
                let mut header_map = reqwest::header::HeaderMap::new();
                for header in headers {
                    if let (Ok(name), Ok(value)) = (
                        reqwest::header::HeaderName::from_bytes(header.name.as_bytes()),
                        reqwest::header::HeaderValue::from_str(&header.value),
                    ) {
                        header_map.insert(name, value);
                    }
                }

                let client = Client::builder()
                    .default_headers(header_map)
                    .build()
                    .map_err(|e| {
                        crate::AgentError::ToolExecution(format!(
                            "Failed to create HTTP client for SSE tool call: {}",
                            e
                        ))
                    })?;

                // Send tool call request via POST
                let response = client
                    .post(url)
                    .header("Accept", "text/event-stream")
                    .header("Content-Type", "application/json")
                    .json(&mcp_request)
                    .send()
                    .await
                    .map_err(|e| {
                        crate::AgentError::ToolExecution(format!(
                            "Failed to send SSE tool call request to MCP server: {}",
                            e
                        ))
                    })?;

                if !response.status().is_success() {
                    return Err(crate::AgentError::ToolExecution(format!(
                        "SSE tool call request failed with status: {}",
                        response.status()
                    )));
                }

                // Parse SSE response
                let body = response.text().await.map_err(|e| {
                    crate::AgentError::ToolExecution(format!(
                        "Failed to read SSE tool call response from MCP server: {}",
                        e
                    ))
                })?;

                let response_json = Self::parse_sse_response(&body)?;
                Ok(response_json)
            }
        }
    }

    /// Process MCP tool call response into string result
    fn process_tool_call_response(&self, response: &Value) -> crate::Result<String> {
        if let Some(result) = response.get("result") {
            // Try to extract content array first (standard MCP format)
            if let Some(content) = result.get("content") {
                if let Some(content_array) = content.as_array() {
                    let mut result_text = String::new();
                    for item in content_array {
                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                            result_text.push_str(text);
                            result_text.push('\n');
                        }
                    }
                    return Ok(result_text.trim().to_string());
                }
            }

            // Fallback: handle different result types intelligently
            match result {
                Value::String(s) => {
                    tracing::debug!("MCP response result is a simple string");
                    Ok(s.clone())
                }
                Value::Number(n) => {
                    tracing::debug!("MCP response result is a number");
                    Ok(n.to_string())
                }
                Value::Bool(b) => {
                    tracing::debug!("MCP response result is a boolean");
                    Ok(b.to_string())
                }
                Value::Null => {
                    tracing::debug!("MCP response result is null");
                    Ok(String::new())
                }
                Value::Object(_) | Value::Array(_) => {
                    tracing::debug!("MCP response result is a complex type, formatting as JSON");
                    serde_json::to_string_pretty(result).map_err(|e| {
                        crate::AgentError::ToolExecution(format!(
                            "Failed to serialize MCP result to JSON: {}",
                            e
                        ))
                    })
                }
            }
        } else if let Some(error) = response.get("error") {
            Err(McpError::ServerError(error.clone()).into())
        } else {
            Err(McpError::MissingResult.into())
        }
    }

    /// List all available tools from all connected MCP servers
    pub async fn list_available_tools(&self) -> Vec<String> {
        let connections = self.connections.read().await;
        let mut all_tools = Vec::new();

        for connection in connections.values() {
            for tool in &connection.tools {
                all_tools.push(format!("{}:{}", connection.name, tool));
            }
        }

        all_tools
    }

    /// List all available prompts from all connected MCP servers
    ///
    /// Filters out partial templates and hidden prompts using the shared
    /// visibility logic from `swissarmyhammer_common::is_prompt_visible`.
    pub async fn list_available_prompts(&self) -> Vec<McpPrompt> {
        let connections = self.connections.read().await;
        let mut all_prompts = Vec::new();

        for connection in connections.values() {
            for prompt in &connection.prompts {
                // Filter out partial templates and hidden prompts
                if is_prompt_visible(&prompt.name, prompt.description.as_deref(), None) {
                    all_prompts.push(prompt.clone());
                }
            }
        }

        all_prompts
    }

    /// Start monitoring MCP server notifications for capability changes
    ///
    /// Spawns background tasks to monitor each MCP server connection for
    /// `tools/list_changed` and `prompts/list_changed` notifications.
    /// When these notifications are received, invokes the provided callback.
    ///
    /// # Arguments
    ///
    /// * `on_commands_changed` - Async callback invoked when commands change
    ///
    /// # Returns
    ///
    /// Returns a vector of join handles for the spawned monitoring tasks
    pub fn start_monitoring_notifications<F>(
        self: Arc<Self>,
        on_commands_changed: F,
    ) -> Vec<tokio::task::JoinHandle<()>>
    where
        F: Fn() -> futures::future::BoxFuture<'static, ()> + Send + Sync + 'static,
    {
        let callback = Arc::new(on_commands_changed);
        let mut handles = Vec::new();

        // For each SSE connection, spawn a task to monitor notifications
        let connections = futures::executor::block_on(self.connections.read());

        for (server_name, connection) in connections.iter() {
            if let TransportConnection::Sse { response_rx, .. } = &connection.transport {
                let response_rx = Arc::clone(response_rx);
                let callback = Arc::clone(&callback);
                let server_name = server_name.clone();

                let handle = tokio::spawn(async move {
                    loop {
                        let mut rx_guard = response_rx.write().await;

                        if let Some(rx) = rx_guard.as_mut() {
                            match rx.recv().await {
                                Some(message) => {
                                    // Try to parse as JSON-RPC notification
                                    if let Ok(json) =
                                        serde_json::from_str::<serde_json::Value>(&message)
                                    {
                                        if let Some(method) =
                                            json.get("method").and_then(|m| m.as_str())
                                        {
                                            match method {
                                                "notifications/tools/list_changed" => {
                                                    tracing::info!(
                                                        "MCP server {} reported tools changed, refreshing commands",
                                                        server_name
                                                    );
                                                    callback().await;
                                                }
                                                "notifications/prompts/list_changed" => {
                                                    tracing::info!(
                                                        "MCP server {} reported prompts changed, refreshing commands",
                                                        server_name
                                                    );
                                                    callback().await;
                                                }
                                                _ => {
                                                    // Other notifications are ignored
                                                    tracing::trace!(
                                                        "Ignoring MCP notification: {} from {}",
                                                        method,
                                                        server_name
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                                None => {
                                    tracing::debug!(
                                        "MCP notification channel closed for server: {}",
                                        server_name
                                    );
                                    break;
                                }
                            }
                        } else {
                            break;
                        }
                    }
                });

                handles.push(handle);
            }
        }

        handles
    }

    /// Shutdown all MCP server connections
    pub async fn shutdown(&self) -> crate::Result<()> {
        let mut connections = self.connections.write().await;

        for (name, connection) in connections.iter_mut() {
            tracing::info!("Shutting down MCP server: {}", name);

            match &connection.transport {
                TransportConnection::Stdio {
                    process,
                    stdin_writer,
                    stdout_reader,
                } => {
                    // Close stdio handles first
                    {
                        let mut writer_guard = stdin_writer.write().await;
                        *writer_guard = None;
                    }
                    {
                        let mut reader_guard = stdout_reader.write().await;
                        *reader_guard = None;
                    }

                    // Kill and wait for the process
                    let mut process_guard = process.write().await;
                    if let Some(mut proc) = process_guard.take() {
                        let _ = proc.kill().await;
                        let _ = proc.wait().await;
                    }
                }
                TransportConnection::Http { .. } => {
                    // HTTP connections don't need explicit cleanup
                    tracing::debug!("HTTP MCP server connection closed: {}", name);
                }
                TransportConnection::Sse {
                    message_tx,
                    response_rx,
                    ..
                } => {
                    // Close SSE channels
                    {
                        let mut tx_guard = message_tx.write().await;
                        *tx_guard = None;
                    }
                    {
                        let mut rx_guard = response_rx.write().await;
                        *rx_guard = None;
                    }
                    tracing::debug!("SSE MCP server connection closed: {}", name);
                }
            }
        }

        connections.clear();
        Ok(())
    }
}

impl Default for McpServerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mcp_manager_creation() {
        let manager = McpServerManager::new();
        let tools = manager.list_available_tools().await;
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_mcp_manager_connect_empty_servers() {
        let mut manager = McpServerManager::new();
        let result = manager.connect_servers(vec![]).await;
        assert!(result.is_ok());

        let tools = manager.list_available_tools().await;
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_mcp_manager_connect_invalid_server() {
        let mut manager = McpServerManager::new();

        let invalid_config = McpServerConfig::Stdio(crate::config::StdioTransport {
            name: "invalid_server".to_string(),
            command: "nonexistent_command_12345".to_string(),
            args: vec![],
            env: vec![],
            cwd: None,
        });

        // Should succeed but log errors for individual server failures
        let result = manager.connect_servers(vec![invalid_config]).await;
        assert!(result.is_ok());

        // No tools should be available since server failed to start
        let tools = manager.list_available_tools().await;
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_tool_name_extraction() {
        let _manager = McpServerManager::new();

        let tool_call = InternalToolRequest {
            id: "test-123".to_string(),
            name: "server:read_file".to_string(),
            arguments: json!({}),
        };

        // Test that we extract the tool name correctly in the request
        let mcp_request = json!({
            "jsonrpc": "2.0",
            "id": tool_call.id,
            "method": "tools/call",
            "params": {
                "name": tool_call.name.split(':').nth(1).unwrap_or(&tool_call.name),
                "arguments": tool_call.arguments
            }
        });

        assert_eq!(mcp_request["params"]["name"].as_str().unwrap(), "read_file");
    }

    #[test]
    fn test_extract_tools_from_list_response() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": [
                    {
                        "name": "read_file",
                        "description": "Read a file"
                    },
                    {
                        "name": "write_file",
                        "description": "Write a file"
                    }
                ]
            }
        });

        let tools = manager.extract_tools_from_list_response(&response).unwrap();
        assert_eq!(tools.len(), 2);
        assert!(tools.contains(&"read_file".to_string()));
        assert!(tools.contains(&"write_file".to_string()));
    }

    #[test]
    fn test_extract_tools_from_empty_response() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": {
                "tools": []
            }
        });

        let tools = manager.extract_tools_from_list_response(&response).unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_extract_tools_from_initialize_response_with_tools() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {
                        "listChanged": true
                    }
                },
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            }
        });

        let tools = manager
            .extract_tools_from_initialize_response(&response)
            .unwrap();
        // For now this should return empty as capabilities.tools is just metadata
        assert!(tools.is_empty());
    }

    #[test]
    fn test_extract_tools_from_initialize_response_empty() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "serverInfo": {
                    "name": "test-server",
                    "version": "1.0.0"
                }
            }
        });

        let tools = manager
            .extract_tools_from_initialize_response(&response)
            .unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_process_tool_call_response_success() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": "test-123",
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": "File contents here"
                    }
                ]
            }
        });

        let result = manager.process_tool_call_response(&response).unwrap();
        assert_eq!(result, "File contents here");
    }

    #[test]
    fn test_process_tool_call_response_error() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": "test-123",
            "error": {
                "code": -1,
                "message": "File not found"
            }
        });

        let result = manager.process_tool_call_response(&response);
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("MCP server error:"));
        assert!(error_message.contains("File not found"));
    }

    #[test]
    fn test_process_tool_call_response_multiple_content() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": "test-123",
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": "Line 1"
                    },
                    {
                        "type": "text",
                        "text": "Line 2"
                    }
                ]
            }
        });

        let result = manager.process_tool_call_response(&response).unwrap();
        assert_eq!(result, "Line 1\nLine 2");
    }

    #[test]
    fn test_process_tool_call_response_fallback_string() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": "test-123",
            "result": "Simple string result"
        });

        let result = manager.process_tool_call_response(&response).unwrap();
        assert_eq!(result, "Simple string result");
    }

    #[test]
    fn test_process_tool_call_response_fallback_number() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": "test-123",
            "result": 42
        });

        let result = manager.process_tool_call_response(&response).unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn test_process_tool_call_response_fallback_bool() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": "test-123",
            "result": true
        });

        let result = manager.process_tool_call_response(&response).unwrap();
        assert_eq!(result, "true");
    }

    #[test]
    fn test_process_tool_call_response_fallback_object() {
        let manager = McpServerManager::new();

        let response = json!({
            "jsonrpc": "2.0",
            "id": "test-123",
            "result": {
                "status": "success",
                "data": {
                    "value": 123
                }
            }
        });

        let result = manager.process_tool_call_response(&response).unwrap();
        // Should return formatted JSON
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["status"], "success");
        assert_eq!(parsed["data"]["value"], 123);
    }

    #[tokio::test]
    async fn test_shutdown() {
        let manager = McpServerManager::new();

        // Test shutdown with no connections
        let result = manager.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_stdio_transport_connection_invalid_command() {
        let mut manager = McpServerManager::new();

        let stdio_config = McpServerConfig::Stdio(crate::config::StdioTransport {
            name: "invalid_server".to_string(),
            command: "nonexistent_command_12345".to_string(),
            args: vec!["--stdio".to_string()],
            env: vec![crate::config::EnvVariable {
                name: "TEST_VAR".to_string(),
                value: "test_value".to_string(),
            }],
            cwd: None,
        });

        // Should fail gracefully and log errors
        let result = manager.connect_servers(vec![stdio_config]).await;
        assert!(result.is_ok()); // Manager should continue despite individual failures

        // No tools should be available since server failed to start
        let tools = manager.list_available_tools().await;
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_http_transport_configuration() {
        let _manager = McpServerManager::new();

        let http_config = crate::config::HttpTransport {
            transport_type: "http".to_string(),
            name: "test-http-server".to_string(),
            url: "https://api.example.com/mcp".to_string(),
            headers: vec![
                crate::config::HttpHeader {
                    name: "Authorization".to_string(),
                    value: "Bearer token123".to_string(),
                },
                crate::config::HttpHeader {
                    name: "Content-Type".to_string(),
                    value: "application/json".to_string(),
                },
            ],
        };

        // Test validation
        assert!(http_config.validate().is_ok());

        let mcp_config = McpServerConfig::Http(http_config);
        assert_eq!(mcp_config.name(), "test-http-server");
        assert_eq!(mcp_config.transport_type(), "http");
    }

    #[tokio::test]
    async fn test_sse_transport_configuration() {
        let _manager = McpServerManager::new();

        let sse_config = crate::config::SseTransport {
            transport_type: "sse".to_string(),
            name: "test-sse-server".to_string(),
            url: "https://events.example.com/mcp".to_string(),
            headers: vec![crate::config::HttpHeader {
                name: "X-API-Key".to_string(),
                value: "apikey456".to_string(),
            }],
        };

        // Test validation
        assert!(sse_config.validate().is_ok());

        let mcp_config = McpServerConfig::Sse(sse_config);
        assert_eq!(mcp_config.name(), "test-sse-server");
        assert_eq!(mcp_config.transport_type(), "sse");
    }

    #[test]
    fn test_transport_type_detection() {
        let stdio_config = McpServerConfig::Stdio(crate::config::StdioTransport {
            name: "stdio-test".to_string(),
            command: "/bin/echo".to_string(),
            args: vec!["hello".to_string()],
            env: vec![],
            cwd: None,
        });

        let http_config = McpServerConfig::Http(crate::config::HttpTransport {
            transport_type: "http".to_string(),
            name: "http-test".to_string(),
            url: "https://example.com".to_string(),
            headers: vec![],
        });

        let sse_config = McpServerConfig::Sse(crate::config::SseTransport {
            transport_type: "sse".to_string(),
            name: "sse-test".to_string(),
            url: "https://example.com".to_string(),
            headers: vec![],
        });

        assert_eq!(stdio_config.transport_type(), "stdio");
        assert_eq!(http_config.transport_type(), "http");
        assert_eq!(sse_config.transport_type(), "sse");

        assert_eq!(stdio_config.name(), "stdio-test");
        assert_eq!(http_config.name(), "http-test");
        assert_eq!(sse_config.name(), "sse-test");
    }

    #[test]
    fn test_transport_validation_error_cases() {
        // Test stdio with empty command
        let invalid_stdio = crate::config::StdioTransport {
            name: "test".to_string(),
            command: String::new(),
            args: vec![],
            env: vec![],
            cwd: None,
        };
        assert!(invalid_stdio.validate().is_err());

        // Test HTTP with invalid URL
        let invalid_http = crate::config::HttpTransport {
            transport_type: "http".to_string(),
            name: "test".to_string(),
            url: "ftp://invalid-protocol.com".to_string(),
            headers: vec![],
        };
        assert!(invalid_http.validate().is_err());

        // Test SSE with empty name
        let invalid_sse = crate::config::SseTransport {
            transport_type: "sse".to_string(),
            name: String::new(),
            url: "https://example.com".to_string(),
            headers: vec![],
        };
        assert!(invalid_sse.validate().is_err());

        // Test env var with empty name
        let invalid_stdio_env = crate::config::StdioTransport {
            name: "test".to_string(),
            command: "/bin/echo".to_string(),
            args: vec![],
            env: vec![crate::config::EnvVariable {
                name: String::new(),
                value: "value".to_string(),
            }],
            cwd: None,
        };
        assert!(invalid_stdio_env.validate().is_err());

        // Test HTTP header with empty name
        let invalid_http_header = crate::config::HttpTransport {
            transport_type: "http".to_string(),
            name: "test".to_string(),
            url: "https://example.com".to_string(),
            headers: vec![crate::config::HttpHeader {
                name: String::new(),
                value: "value".to_string(),
            }],
        };
        assert!(invalid_http_header.validate().is_err());
    }

    #[test]
    fn test_env_variable_and_http_header_equality() {
        let env1 = crate::config::EnvVariable {
            name: "API_KEY".to_string(),
            value: "secret123".to_string(),
        };
        let env2 = crate::config::EnvVariable {
            name: "API_KEY".to_string(),
            value: "secret123".to_string(),
        };
        let env3 = crate::config::EnvVariable {
            name: "API_KEY".to_string(),
            value: "different_secret".to_string(),
        };

        assert_eq!(env1, env2);
        assert_ne!(env1, env3);

        let header1 = crate::config::HttpHeader {
            name: "Authorization".to_string(),
            value: "Bearer token".to_string(),
        };
        let header2 = crate::config::HttpHeader {
            name: "Authorization".to_string(),
            value: "Bearer token".to_string(),
        };
        let header3 = crate::config::HttpHeader {
            name: "Authorization".to_string(),
            value: "Bearer different_token".to_string(),
        };

        assert_eq!(header1, header2);
        assert_ne!(header1, header3);
    }

    #[tokio::test]
    async fn test_mixed_transport_configurations() {
        let mut manager = McpServerManager::new();

        let configs = vec![
            McpServerConfig::Stdio(crate::config::StdioTransport {
                name: "stdio-server".to_string(),
                command: "/bin/echo".to_string(),
                args: vec!["stdio".to_string()],
                env: vec![crate::config::EnvVariable {
                    name: "TRANSPORT".to_string(),
                    value: "stdio".to_string(),
                }],
                cwd: None,
            }),
            // Note: HTTP and SSE will likely fail to connect in tests
            // but the manager should handle this gracefully
        ];

        let result = manager.connect_servers(configs).await;
        assert!(result.is_ok());

        // Should be able to shutdown cleanly regardless of connection failures
        let shutdown_result = manager.shutdown().await;
        assert!(shutdown_result.is_ok());
    }

    #[test]
    fn test_parse_sse_response_with_data() {
        let sse_body = "data: {\"jsonrpc\":\"2.0\",\"result\":{\"tools\":[]}}\n\n";

        let mut json_data = String::new();
        for line in sse_body.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                json_data = data.to_string();
                break;
            }
        }

        assert!(!json_data.is_empty());
        let parsed: Value = serde_json::from_str(&json_data).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
    }

    #[test]
    fn test_parse_sse_response_empty() {
        let sse_body = "event: message\n\n";

        let mut json_data = String::new();
        for line in sse_body.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                json_data = data.to_string();
                break;
            }
        }

        assert!(json_data.is_empty());
    }

    #[test]
    fn test_session_id_storage() {
        use tokio::runtime::Runtime;

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let session_id = Arc::new(RwLock::new(None));

            // Test initial state
            assert!(session_id.read().await.is_none());

            // Test storing session ID
            {
                let mut write_lock = session_id.write().await;
                *write_lock = Some("test-session-123".to_string());
            }

            // Test reading session ID
            let read_lock = session_id.read().await;
            assert_eq!(read_lock.as_ref().unwrap(), "test-session-123");
        });
    }

    #[test]
    fn test_http_transport_connection_has_session_id() {
        use tokio::runtime::Runtime;

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = reqwest::Client::new();
            let session_id = Arc::new(RwLock::new(Some("session-abc".to_string())));

            let transport = TransportConnection::Http {
                client: Arc::new(client),
                url: "http://localhost:8080".to_string(),
                headers: vec![],
                session_id: session_id.clone(),
            };

            match transport {
                TransportConnection::Http {
                    session_id: sid, ..
                } => {
                    assert_eq!(sid.read().await.as_ref().unwrap(), "session-abc");
                }
                _ => panic!("Expected HTTP transport"),
            }
        });
    }

    #[test]
    fn test_extract_prompts_from_valid_response() {
        let manager = McpServerManager::new();

        let response = json!({
            "result": {
                "prompts": [
                    {
                        "name": "test_prompt",
                        "description": "A test prompt",
                        "arguments": [
                            {
                                "name": "required_arg",
                                "description": "A required argument",
                                "required": true
                            },
                            {
                                "name": "optional_arg",
                                "description": "An optional argument",
                                "required": false
                            }
                        ]
                    }
                ]
            }
        });

        let prompts = manager
            .extract_prompts_from_list_response(&response)
            .unwrap();

        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].name, "test_prompt");
        assert_eq!(prompts[0].description, Some("A test prompt".to_string()));
        assert_eq!(prompts[0].arguments.len(), 2);
        assert_eq!(prompts[0].arguments[0].name, "required_arg");
        assert!(prompts[0].arguments[0].required);
        assert_eq!(prompts[0].arguments[1].name, "optional_arg");
        assert!(!prompts[0].arguments[1].required);
    }

    #[test]
    fn test_extract_prompts_without_description() {
        let manager = McpServerManager::new();

        let response = json!({
            "result": {
                "prompts": [
                    {
                        "name": "no_description_prompt",
                        "arguments": []
                    }
                ]
            }
        });

        let prompts = manager
            .extract_prompts_from_list_response(&response)
            .unwrap();

        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].name, "no_description_prompt");
        assert_eq!(prompts[0].description, None);
        assert_eq!(prompts[0].arguments.len(), 0);
    }

    #[test]
    fn test_extract_prompts_without_arguments() {
        let manager = McpServerManager::new();

        let response = json!({
            "result": {
                "prompts": [
                    {
                        "name": "simple_prompt",
                        "description": "Simple prompt without arguments"
                    }
                ]
            }
        });

        let prompts = manager
            .extract_prompts_from_list_response(&response)
            .unwrap();

        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].name, "simple_prompt");
        assert_eq!(
            prompts[0].description,
            Some("Simple prompt without arguments".to_string())
        );
        assert_eq!(prompts[0].arguments.len(), 0);
    }

    #[test]
    fn test_extract_prompts_from_empty_array() {
        let manager = McpServerManager::new();

        let response = json!({
            "result": {
                "prompts": []
            }
        });

        let prompts = manager
            .extract_prompts_from_list_response(&response)
            .unwrap();

        assert_eq!(prompts.len(), 0);
    }

    #[test]
    fn test_extract_prompts_from_missing_result() {
        let manager = McpServerManager::new();

        let response = json!({});

        let prompts = manager
            .extract_prompts_from_list_response(&response)
            .unwrap();

        assert_eq!(prompts.len(), 0);
    }

    #[test]
    fn test_extract_prompts_from_missing_prompts_field() {
        let manager = McpServerManager::new();

        let response = json!({
            "result": {}
        });

        let prompts = manager
            .extract_prompts_from_list_response(&response)
            .unwrap();

        assert_eq!(prompts.len(), 0);
    }

    #[test]
    fn test_extract_prompts_with_multiple_prompts() {
        let manager = McpServerManager::new();

        let response = json!({
            "result": {
                "prompts": [
                    {
                        "name": "prompt1",
                        "description": "First prompt"
                    },
                    {
                        "name": "prompt2",
                        "description": "Second prompt"
                    },
                    {
                        "name": "prompt3",
                        "description": "Third prompt"
                    }
                ]
            }
        });

        let prompts = manager
            .extract_prompts_from_list_response(&response)
            .unwrap();

        assert_eq!(prompts.len(), 3);
        assert_eq!(prompts[0].name, "prompt1");
        assert_eq!(prompts[1].name, "prompt2");
        assert_eq!(prompts[2].name, "prompt3");
    }

    #[test]
    fn test_extract_prompts_skips_entries_without_name() {
        let manager = McpServerManager::new();

        let response = json!({
            "result": {
                "prompts": [
                    {
                        "name": "valid_prompt",
                        "description": "Valid prompt"
                    },
                    {
                        "description": "Missing name field"
                    },
                    {
                        "name": "another_valid_prompt",
                        "description": "Another valid prompt"
                    }
                ]
            }
        });

        let prompts = manager
            .extract_prompts_from_list_response(&response)
            .unwrap();

        // Should only extract the two prompts with names
        assert_eq!(prompts.len(), 2);
        assert_eq!(prompts[0].name, "valid_prompt");
        assert_eq!(prompts[1].name, "another_valid_prompt");
    }

    #[test]
    fn test_extract_prompts_with_argument_without_description() {
        let manager = McpServerManager::new();

        let response = json!({
            "result": {
                "prompts": [
                    {
                        "name": "prompt_with_arg",
                        "arguments": [
                            {
                                "name": "arg_without_desc",
                                "required": true
                            }
                        ]
                    }
                ]
            }
        });

        let prompts = manager
            .extract_prompts_from_list_response(&response)
            .unwrap();

        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].arguments.len(), 1);
        assert_eq!(prompts[0].arguments[0].name, "arg_without_desc");
        assert_eq!(prompts[0].arguments[0].description, None);
        assert!(prompts[0].arguments[0].required);
    }

    #[test]
    fn test_extract_prompts_with_argument_missing_required_field() {
        let manager = McpServerManager::new();

        let response = json!({
            "result": {
                "prompts": [
                    {
                        "name": "prompt_with_arg",
                        "arguments": [
                            {
                                "name": "arg_default_optional",
                                "description": "Should default to not required"
                            }
                        ]
                    }
                ]
            }
        });

        let prompts = manager
            .extract_prompts_from_list_response(&response)
            .unwrap();

        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].arguments.len(), 1);
        // When required field is missing, it should default to false
        assert!(!prompts[0].arguments[0].required);
    }
}
