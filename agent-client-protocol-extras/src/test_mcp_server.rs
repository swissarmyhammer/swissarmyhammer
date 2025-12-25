//! TestMcpServer - MCP server for testing tool calls and plan notifications
//!
//! Implements list-files and create-plan tools with proper ACP notifications:
//! - tool_call notifications when tools are invoked
//! - tool_call_update notifications during execution
//! - Plan notifications for create-plan tool
//! - Per-file notifications for list-files tool

use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Notification callback for sending ACP notifications during tool execution
pub type NotificationCallback = Arc<dyn Fn(agent_client_protocol::SessionNotification) + Send + Sync>;

/// TestMcpServer provides predictable tools for testing
pub struct TestMcpServer {
    notification_callback: Option<NotificationCallback>,
}

impl TestMcpServer {
    /// Create new test MCP server
    pub fn new() -> Self {
        Self {
            notification_callback: None,
        }
    }

    /// Set notification callback
    pub fn set_notification_callback(&mut self, callback: NotificationCallback) {
        self.notification_callback = Some(callback);
    }

    /// Execute list-files tool with notifications
    ///
    /// Sends:
    /// - tool_call notification (status: pending)
    /// - tool_call_update (status: in_progress)
    /// - Notification for each file found
    /// - tool_call_update (status: completed)
    pub async fn execute_list_files(&self, path: &str, session_id: agent_client_protocol::SessionId) -> Value {
        let tool_call_id = format!("tool_call_{}", uuid::Uuid::new_v4());

        // Send initial tool_call notification
        if let Some(ref callback) = self.notification_callback {
            let notif = agent_client_protocol::SessionNotification::new(
                session_id.clone(),
                agent_client_protocol::SessionUpdate::ToolCall(
                    agent_client_protocol::ToolCallUpdate::new(tool_call_id.clone())
                        .title("List files")
                        .kind(agent_client_protocol::ToolCallKind::Read)
                        .status(agent_client_protocol::ToolCallStatus::Pending)
                )
            );
            callback(notif);
        }

        // Send in_progress update
        if let Some(ref callback) = self.notification_callback {
            let notif = agent_client_protocol::SessionNotification::new(
                session_id.clone(),
                agent_client_protocol::SessionUpdate::ToolCallUpdate(
                    agent_client_protocol::ToolCallUpdateNotification::new(tool_call_id.clone())
                        .status(agent_client_protocol::ToolCallStatus::InProgress)
                )
            );
            callback(notif);
        }

        let files = vec!["file1.txt", "file2.txt", "file3.txt"];

        // Send notification for each file
        for file in &files {
            if let Some(ref callback) = self.notification_callback {
                let content = agent_client_protocol::ContentBlock::Text(
                    agent_client_protocol::TextContent::new(format!("Found: {}", file))
                );
                let notif = agent_client_protocol::SessionNotification::new(
                    session_id.clone(),
                    agent_client_protocol::SessionUpdate::ToolCallUpdate(
                        agent_client_protocol::ToolCallUpdateNotification::new(tool_call_id.clone())
                            .content(vec![agent_client_protocol::ToolCallContent::new(content)])
                    )
                );
                callback(notif);
            }
        }

        // Send completed update
        if let Some(ref callback) = self.notification_callback {
            let notif = agent_client_protocol::SessionNotification::new(
                session_id.clone(),
                agent_client_protocol::SessionUpdate::ToolCallUpdate(
                    agent_client_protocol::ToolCallUpdateNotification::new(tool_call_id.clone())
                        .status(agent_client_protocol::ToolCallStatus::Completed)
                )
            );
            callback(notif);
        }

        json!({
            "files": files,
            "path": path
        })
    }

    /// Execute create-plan tool with plan notifications
    ///
    /// Sends Plan notifications per https://agentclientprotocol.com/protocol/agent-plan
    pub async fn execute_create_plan(&self, goal: &str, session_id: agent_client_protocol::SessionId) -> Value {
        // Send Plan notification with steps
        if let Some(ref callback) = self.notification_callback {
            let entries = vec![
                agent_client_protocol::PlanEntry::new("Analyze requirements")
                    .priority(agent_client_protocol::PlanPriority::High)
                    .status(agent_client_protocol::PlanStatus::Pending),
                agent_client_protocol::PlanEntry::new("Design solution")
                    .priority(agent_client_protocol::PlanPriority::High)
                    .status(agent_client_protocol::PlanStatus::Pending),
                agent_client_protocol::PlanEntry::new("Implement")
                    .priority(agent_client_protocol::PlanPriority::Medium)
                    .status(agent_client_protocol::PlanStatus::Pending),
                agent_client_protocol::PlanEntry::new("Test")
                    .priority(agent_client_protocol::PlanPriority::Low)
                    .status(agent_client_protocol::PlanStatus::Pending),
            ];

            let plan_notif = agent_client_protocol::SessionNotification::new(
                session_id.clone(),
                agent_client_protocol::SessionUpdate::Plan(
                    agent_client_protocol::PlanUpdate::new(entries)
                )
            );
            callback(plan_notif);
        }

        json!({
            "plan": {
                "goal": goal,
                "steps": [
                    {"id": 1, "description": "Analyze requirements", "status": "pending"},
                    {"id": 2, "description": "Design solution", "status": "pending"},
                    {"id": 3, "description": "Implement", "status": "pending"},
                    {"id": 4, "description": "Test", "status": "pending"}
                ]
            }
        })
    }

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
                            "goal": {"type": "string", "description": "Goal to plan for"}
                        },
                        "required": ["goal"]
                    }
                }
            ]
        })
    }
}

impl Default for TestMcpServer {
    fn default() -> Self {
        Self::new()
    }
}
