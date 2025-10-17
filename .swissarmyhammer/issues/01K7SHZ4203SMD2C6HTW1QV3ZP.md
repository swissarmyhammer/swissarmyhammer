# Phase 1: Implement MCP Progress Notification Infrastructure

## Overview

Implement the foundational infrastructure for MCP progress notifications that will enable real-time streaming updates during long-running tool operations. This is Phase 1 of the notification system implementation and must be completed before any tools can send progress notifications.

## Goals

1. Create generic progress notification types compatible with MCP specification
2. Add notification channel infrastructure to tool execution context
3. Integrate notification delivery from server to MCP clients
4. Establish patterns and best practices for notification usage

## Implementation Tasks

### 1. Create Progress Notification Types

**Location**: `swissarmyhammer-tools/src/mcp/progress_notifications.rs` (new file)

```rust
/// Progress notification for MCP tool operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressNotification {
    /// Unique token for this operation (ULID)
    pub progress_token: String,

    /// Progress percentage (0-100), None for indeterminate
    pub progress: Option<u32>,

    /// Human-readable progress message
    pub message: String,

    /// Tool-specific metadata
    #[serde(flatten)]
    pub metadata: Option<serde_json::Value>,
}

/// Progress notification sender with channel-based async delivery
#[derive(Clone)]
pub struct ProgressSender {
    sender: mpsc::UnboundedSender<ProgressNotification>,
}

impl ProgressSender {
    pub fn new(sender: mpsc::UnboundedSender<ProgressNotification>) -> Self {
        Self { sender }
    }

    /// Send a progress notification
    pub fn send(&self, notification: ProgressNotification) -> Result<(), SendError> {
        self.sender
            .send(notification)
            .map_err(|e| SendError::ChannelClosed(e.to_string()))
    }

    /// Convenience method to send progress with token, progress %, and message
    pub fn send_progress(
        &self,
        token: &str,
        progress: Option<u32>,
        message: impl Into<String>,
    ) -> Result<(), SendError> {
        self.send(ProgressNotification {
            progress_token: token.to_string(),
            progress,
            message: message.into(),
            metadata: None,
        })
    }

    /// Send progress with metadata
    pub fn send_progress_with_metadata(
        &self,
        token: &str,
        progress: Option<u32>,
        message: impl Into<String>,
        metadata: serde_json::Value,
    ) -> Result<(), SendError> {
        self.send(ProgressNotification {
            progress_token: token.to_string(),
            progress,
            message: message.into(),
            metadata: Some(metadata),
        })
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum SendError {
    #[error("Notification channel closed: {0}")]
    ChannelClosed(String),
}
```

### 2. Add Progress Sender to ToolContext

**Location**: `swissarmyhammer-tools/src/mcp/tool_registry.rs`

Update `ToolContext` to include optional progress sender:

```rust
pub struct ToolContext {
    // ... existing fields ...
    
    /// Optional progress notification sender
    pub progress_sender: Option<Arc<ProgressSender>>,
}

impl ToolContext {
    pub fn new(
        tool_handlers: Arc<ToolHandlers>,
        issue_storage: Arc<RwLock<Box<dyn IssueStorage>>>,
        git_ops: Arc<Mutex<Option<GitOperations>>>,
        memo_storage: Arc<RwLock<Box<dyn MemoStorage>>>,
        agent_config: Arc<AgentConfiguration>,
    ) -> Self {
        Self {
            tool_handlers,
            issue_storage,
            git_ops,
            memo_storage,
            agent_config,
            mcp_server_port: Arc::new(RwLock::new(None)),
            progress_sender: None, // Initially None
        }
    }

    /// Set the progress sender for this context
    pub fn with_progress_sender(mut self, sender: Arc<ProgressSender>) -> Self {
        self.progress_sender = Some(sender);
        self
    }
}
```

### 3. Integrate with MCP Server

**Location**: `swissarmyhammer-tools/src/mcp/server.rs`

Update server to create notification channel and pass to tools:

```rust
impl ServerHandler for McpServer {
    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        // Create progress notification channel
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
        let progress_sender = Arc::new(ProgressSender::new(progress_tx));

        // Add progress sender to tool context
        let tool_context_with_progress = Arc::new(
            (*self.tool_context).clone().with_progress_sender(progress_sender)
        );

        // Spawn task to forward progress notifications to MCP client
        let peer = context.peer.clone();
        let tool_name = request.name.clone();
        tokio::spawn(async move {
            while let Some(notification) = progress_rx.recv().await {
                // Convert to MCP progress notification format
                let progress_params = serde_json::json!({
                    "progressToken": notification.progress_token,
                    "progress": notification.progress,
                    "message": notification.message,
                    "metadata": notification.metadata,
                });

                // Send to MCP client (via peer notification)
                if let Err(e) = peer.send_progress_notification(progress_params).await {
                    tracing::warn!(
                        "Failed to send progress notification for {}: {}",
                        tool_name,
                        e
                    );
                    break;
                }
            }
        });

        // Execute tool with progress-enabled context
        if let Some(tool) = self.tool_registry.get_tool(&request.name) {
            tool.execute(
                request.arguments.unwrap_or_default(),
                &tool_context_with_progress,
            )
            .await
        } else {
            Err(McpError::invalid_request(
                format!("Unknown tool: {}", request.name),
                None,
            ))
        }
    }
}
```

### 4. Create Utility Functions

**Location**: `swissarmyhammer-tools/src/mcp/progress_notifications.rs`

```rust
/// Generate a unique progress token using ULID
pub fn generate_progress_token() -> String {
    ulid::Ulid::new().to_string()
}

/// Create a progress notification for operation start
pub fn start_notification(
    token: &str,
    operation: impl Into<String>,
) -> ProgressNotification {
    ProgressNotification {
        progress_token: token.to_string(),
        progress: Some(0),
        message: format!("Starting: {}", operation.into()),
        metadata: None,
    }
}

/// Create a progress notification for operation completion
pub fn complete_notification(
    token: &str,
    operation: impl Into<String>,
) -> ProgressNotification {
    ProgressNotification {
        progress_token: token.to_string(),
        progress: Some(100),
        message: format!("Completed: {}", operation.into()),
        metadata: None,
    }
}
```

### 5. Add Comprehensive Tests

**Location**: `swissarmyhammer-tools/src/mcp/progress_notifications.rs` (test module)

Test coverage for:
- Progress notification creation and serialization
- Progress sender sending and error handling
- Channel-based notification delivery
- Metadata handling
- Error cases (channel closed, invalid data)

### 6. Update Module Exports

**Location**: `swissarmyhammer-tools/src/mcp/mod.rs`

```rust
pub mod progress_notifications;

pub use progress_notifications::{
    ProgressNotification, ProgressSender, SendError,
    generate_progress_token, start_notification, complete_notification,
};
```

## Design Decisions

### Why Separate from FlowNotification?

`FlowNotification` is specifically for workflow state machine transitions. `ProgressNotification` is for general tool progress updates. They serve different purposes:

- **FlowNotification**: Workflow lifecycle (FlowStart, StateStart, StateComplete, FlowComplete, FlowError)
- **ProgressNotification**: Generic progress updates for any long-running operation

### Why Use Channels?

Channels allow async notification sending without blocking tool execution. The spawned task handles delivery to the MCP client independently.

### Why Optional ProgressSender?

Not all tool executions need progress notifications (e.g., fast operations, CLI usage). Making it optional avoids overhead when not needed.

## Success Criteria

1. Progress notification types compile and pass tests
2. ToolContext can be created with optional progress sender
3. MCP server creates notification channel per tool execution
4. Test tool can send progress notifications successfully
5. Notifications are delivered to MCP client in correct format
6. No performance degradation for tools without progress notifications
7. Documentation explains when and how to use progress notifications

## Testing Strategy

### Unit Tests
- Test progress notification serialization/deserialization
- Test progress sender success and error cases
- Test ULID generation for progress tokens
- Test utility functions for creating notifications

### Integration Tests
- Create test tool that sends progress notifications
- Verify notifications are received by MCP client
- Test channel cleanup when tool execution completes
- Test error handling when client disconnects

### Manual Testing
- Use MCP Inspector to view progress notifications
- Test with real long-running operation (like file indexing)
- Verify progress bars update correctly in clients

## Documentation

Create or update:
- `doc/src/architecture/progress-notifications.md` - Architecture overview
- `doc/src/reference/progress-notifications.md` - API reference
- Update `doc/src/features.md` with progress notification feature
- Add examples in `doc/src/examples.md`

## Timeline

- **Week 1, Days 1-2**: Create notification types and utility functions with tests
- **Week 1, Days 3-4**: Integrate with ToolContext and server
- **Week 1, Day 5**: Integration testing and bug fixes
- **Week 2, Day 1**: Documentation and examples

## Related Issues

This is a prerequisite for:
- Issue: Eliminate notify_create tool (01K7SHXK4XKMDREMXG7BFJ8YJ7)
- Phase 2: Add progress notifications to shell_execute
- Phase 2: Add progress notifications to search_index
- Phase 3: Add progress notifications to web_search, web_fetch, outline_generate
- Phase 4: Add progress notifications to rules_check and file operations

## References

- MCP Notification Recommendations: `specification/mcp_notifications_recommendations.md`
- Existing FlowNotification: `swissarmyhammer-tools/src/mcp/notifications.rs`
- MCP Specification: https://spec.modelcontextprotocol.io/
- RMCP Library: v0.6.4
