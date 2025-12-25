//! TestMcpServer - Simple MCP server for testing
//!
//! Provides list-files and create-plan tools for testing.
//! Actual MCP implementation TBD - for now just provides tool definitions.

use serde_json::{json, Value};

/// TestMcpServer provides predictable tools for testing
pub struct TestMcpServer;

impl TestMcpServer {
    /// Get MCP tools manifest
    pub fn tools() -> Value {
        json!({
            "tools": [
                {
                    "name": "list-files",
                    "description": "List files in a directory",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string", "description": "Directory path"}
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
                            "goal": {"type": "string", "description": "Goal"}
                        },
                        "required": ["goal"]
                    }
                }
            ]
        })
    }

    /// Execute list-files tool
    pub fn list_files(path: &str) -> Value {
        json!({
            "files": ["file1.txt", "file2.txt", "file3.txt"],
            "path": path
        })
    }

    /// Execute create-plan tool
    pub fn create_plan(goal: &str) -> Value {
        json!({
            "plan": {
                "goal": goal,
                "steps": [
                    {"id": 1, "description": "Analyze", "status": "pending"},
                    {"id": 2, "description": "Design", "status": "pending"},
                    {"id": 3, "description": "Implement", "status": "pending"},
                    {"id": 4, "description": "Test", "status": "pending"}
                ]
            }
        })
    }
}
