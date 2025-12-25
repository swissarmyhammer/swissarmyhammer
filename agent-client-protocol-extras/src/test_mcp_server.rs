//! Simple MCP server for testing tool calls and notifications
//!
//! Provides two tools via HTTP:
//! - list-files: Lists files in a directory
//! - create-plan: Creates a simple execution plan
//!
//! Run as in-process HTTP server for consistent testing.

use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;

/// Simple MCP server for testing
pub struct TestMcpServer {
    addr: SocketAddr,
}

impl TestMcpServer {
    /// Start test MCP server on random port
    pub async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        tokio::spawn(async move {
            // Simple HTTP server that responds to MCP requests
            loop {
                if let Ok((mut socket, _)) = listener.accept().await {
                    tokio::spawn(async move {
                        use tokio::io::{AsyncReadExt, AsyncWriteExt};

                        let mut buf = vec![0; 4096];
                        if let Ok(n) = socket.read(&mut buf).await {
                            // Parse request and send response
                            let response = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"tools\":[]}";
                            let _ = socket.write_all(response).await;
                        }
                    });
                }
            }
        });

        tracing::info!("TestMcpServer started on {}", addr);
        Ok(Self { addr })
    }

    /// Get server URL
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Get tools list
    pub fn tools() -> Value {
        json!({
            "tools": [
                {
                    "name": "list-files",
                    "description": "List files in a directory",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Directory path"
                            }
                        },
                        "required": ["path"]
                    }
                },
                {
                    "name": "create-plan",
                    "description": "Create an execution plan",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "goal": {
                                "type": "string",
                                "description": "Goal to plan for"
                            }
                        },
                        "required": ["goal"]
                    }
                }
            ]
        })
    }

    /// Execute tool call
    pub fn execute_tool(name: &str, arguments: Value) -> Value {
        match name {
            "list-files" => {
                let path = arguments.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                json!({
                    "files": ["file1.txt", "file2.txt", "file3.txt"],
                    "path": path
                })
            }
            "create-plan" => {
                let goal = arguments.get("goal").and_then(|v| v.as_str()).unwrap_or("unknown");
                json!({
                    "plan": {
                        "goal": goal,
                        "steps": [
                            {"id": 1, "description": "Analyze"},
                            {"id": 2, "description": "Design"},
                            {"id": 3, "description": "Implement"},
                            {"id": 4, "description": "Test"}
                        ]
                    }
                })
            }
            _ => json!({"error": "Unknown tool"})
        }
    }
}

