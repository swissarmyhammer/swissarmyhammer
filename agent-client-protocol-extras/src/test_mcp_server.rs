//! TestMcpServer - Simple MCP server for testing with MCP notifications
//!
//! Provides list-files and create-plan tools with logging and progress notifications.

use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService,
};
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{json, Map, Value};
use std::sync::Arc;
use tokio::net::TcpListener;

/// TestMcpServer provides predictable tools for ACP conformance testing
#[derive(Clone)]
pub struct TestMcpServer {
    name: String,
    version: String,
}

impl TestMcpServer {
    pub fn new() -> Self {
        Self {
            name: "test-mcp-server".to_string(),
            version: "1.0.0".to_string(),
        }
    }

    fn get_tools() -> Vec<Tool> {
        vec![
            Tool {
                name: "list-files".into(),
                description: Some("List files in a directory".into()),
                input_schema: Arc::new({
                    let mut map = Map::new();
                    map.insert("type".to_string(), json!("object"));
                    map.insert(
                        "properties".to_string(),
                        json!({"path": {"type": "string", "description": "Directory path"}}),
                    );
                    map.insert("required".to_string(), json!(["path"]));
                    map
                }),
                annotations: None,
                output_schema: None,
                icons: None,
                title: Some("list-files".into()),
                meta: None,
                execution: None,
            },
            Tool {
                name: "create-plan".into(),
                description: Some("Create an execution plan".into()),
                input_schema: Arc::new({
                    let mut map = Map::new();
                    map.insert("type".to_string(), json!("object"));
                    map.insert(
                        "properties".to_string(),
                        json!({"goal": {"type": "string", "description": "Goal"}}),
                    );
                    map.insert("required".to_string(), json!(["goal"]));
                    map
                }),
                annotations: None,
                output_schema: None,
                icons: None,
                title: Some("create-plan".into()),
                meta: None,
                execution: None,
            },
        ]
    }

    fn execute_list_files(path: &str) -> Value {
        json!({
            "files": ["file1.txt", "file2.txt", "file3.txt"],
            "path": path,
            "count": 3
        })
    }

    fn execute_create_plan(goal: &str) -> Value {
        json!({
            "plan": {
                "goal": goal,
                "steps": [
                    {"id": 1, "description": "Analyze requirements", "status": "pending"},
                    {"id": 2, "description": "Design solution", "status": "pending"},
                    {"id": 3, "description": "Implement solution", "status": "pending"},
                    {"id": 4, "description": "Test and validate", "status": "pending"}
                ]
            }
        })
    }
}

impl Default for TestMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Start TestMcpServer as an in-process HTTP server
pub async fn start_test_mcp_server() -> Result<String, Box<dyn std::error::Error>> {
    let server = Arc::new(TestMcpServer::new());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let url = format!("http://{}/mcp", addr);

    tracing::info!("TestMcpServer starting on {}", url);

    let http_service = StreamableHttpService::new(
        move || Ok((*server).clone()),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let app = axum::Router::new().nest_service("/mcp", http_service);

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("TestMcpServer error: {}", e);
        }
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    tracing::info!("TestMcpServer running at {}", url);
    Ok(url)
}

impl ServerHandler for TestMcpServer {
    async fn initialize(
        &self,
        request: InitializeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<InitializeResult, McpError> {
        tracing::info!(
            "TestMcpServer: Client connecting: {} v{}",
            request.client_info.name,
            request.client_info.version
        );

        Ok(InitializeResult {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                prompts: None,
                resources: None,
                logging: None,
                completions: None,
                experimental: None,
                extensions: None,
                tasks: None,
            },
            instructions: Some("Test MCP server for ACP conformance testing".into()),
            server_info: Implementation {
                name: self.name.clone(),
                version: self.version.clone(),
                icons: None,
                title: None,
                description: None,
                website_url: None,
            },
        })
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        tracing::debug!("TestMcpServer: list_tools called");
        Ok(ListToolsResult {
            tools: Self::get_tools(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::info!("TestMcpServer: call_tool: {}", request.name);

        let arguments = request.arguments.unwrap_or_default();

        match request.name.as_ref() {
            "list-files" => {
                let path = arguments
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");

                // Send one logging notification at start
                let _ = context
                    .peer
                    .send_notification(
                        LoggingMessageNotification::new(LoggingMessageNotificationParam {
                            level: LoggingLevel::Info,
                            logger: Some("test-mcp-server".to_string()),
                            data: json!({"message": format!("Listing files in: {}", path)}),
                        })
                        .into(),
                    )
                    .await;

                // Send progress notifications
                let token = ProgressToken(NumberOrString::String("list-files-1".into()));

                let result = Self::execute_list_files(path);
                let files = result["files"].as_array().unwrap();

                // Progress notification for each file
                for (i, _file) in files.iter().enumerate() {
                    let _ = context
                        .peer
                        .send_notification(
                            ProgressNotification::new(ProgressNotificationParam {
                                progress_token: token.clone(),
                                progress: (i + 1) as f64,
                                total: Some(files.len() as f64),
                                message: Some(format!("Processing file {}/{}", i + 1, files.len())),
                            })
                            .into(),
                        )
                        .await;

                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }

                Ok(CallToolResult {
                    content: vec![Annotated::new(
                        RawContent::Text(RawTextContent {
                            text: serde_json::to_string_pretty(&result).unwrap(),
                            meta: None,
                        }),
                        None,
                    )],
                    is_error: Some(false),
                    structured_content: None,
                    meta: None,
                })
            }
            "create-plan" => {
                let goal = arguments
                    .get("goal")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");

                // Send one logging notification at start
                let _ = context
                    .peer
                    .send_notification(
                        LoggingMessageNotification::new(LoggingMessageNotificationParam {
                            level: LoggingLevel::Info,
                            logger: Some("test-mcp-server".to_string()),
                            data: json!({"message": format!("Creating plan for: {}", goal)}),
                        })
                        .into(),
                    )
                    .await;

                let token = ProgressToken(NumberOrString::String("create-plan-1".into()));
                let result = Self::execute_create_plan(goal);
                let steps = result["plan"]["steps"].as_array().unwrap();

                // Progress notification for each step
                for (i, _step) in steps.iter().enumerate() {
                    let _ = context
                        .peer
                        .send_notification(
                            ProgressNotification::new(ProgressNotificationParam {
                                progress_token: token.clone(),
                                progress: (i + 1) as f64,
                                total: Some(steps.len() as f64),
                                message: Some(format!("Creating step {}/{}", i + 1, steps.len())),
                            })
                            .into(),
                        )
                        .await;

                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }

                Ok(CallToolResult {
                    content: vec![Annotated::new(
                        RawContent::Text(RawTextContent {
                            text: serde_json::to_string_pretty(&result).unwrap(),
                            meta: None,
                        }),
                        None,
                    )],
                    is_error: Some(false),
                    structured_content: None,
                    meta: None,
                })
            }
            _ => Err(McpError::invalid_request(
                format!("Unknown tool: {}", request.name),
                None,
            )),
        }
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                prompts: None,
                resources: None,
                logging: None,
                completions: None,
                experimental: None,
                extensions: None,
                tasks: None,
            },
            server_info: Implementation {
                name: self.name.clone(),
                version: self.version.clone(),
                icons: None,
                title: None,
                description: None,
                website_url: None,
            },
            instructions: Some("Test MCP server for ACP conformance testing".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let server = TestMcpServer::new();
        assert_eq!(server.name, "test-mcp-server");
    }

    #[test]
    fn test_tools() {
        let tools = TestMcpServer::get_tools();
        assert_eq!(tools.len(), 2);
    }
}
