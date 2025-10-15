# Step 7: Add MCP Notification Infrastructure

Refer to ideas/flow_mcp.md

## Objective

Create the notification infrastructure to support sending progress updates during long-running workflow execution.

## Context

Workflows are long-running operations. MCP supports notifications to keep clients informed of progress. We need infrastructure to send flow start, state transition, completion, and error notifications.

## Tasks

### 1. Define Notification Message Types

Create `swissarmyhammer-tools/src/mcp/notifications.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowNotification {
    pub token: String,  // Workflow run ID
    pub progress: Option<u32>,  // 0-100, None for errors
    pub message: String,
    pub metadata: FlowNotificationMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FlowNotificationMetadata {
    FlowStart {
        flow_name: String,
        parameters: serde_json::Value,
        initial_state: String,
    },
    StateStart {
        flow_name: String,
        state_id: String,
        state_description: String,
    },
    StateComplete {
        flow_name: String,
        state_id: String,
        next_state: Option<String>,
    },
    FlowComplete {
        flow_name: String,
        status: String,
        final_state: String,
    },
    FlowError {
        flow_name: String,
        status: String,
        error_state: String,
        error: String,
    },
}
```

### 2. Create Notification Sender

```rust
use tokio::sync::mpsc;

pub struct NotificationSender {
    sender: mpsc::UnboundedSender<FlowNotification>,
}

impl NotificationSender {
    pub fn new(sender: mpsc::UnboundedSender<FlowNotification>) -> Self {
        Self { sender }
    }
    
    pub fn send(&self, notification: FlowNotification) -> Result<(), SendError> {
        self.sender.send(notification)
            .map_err(|e| SendError::ChannelClosed(e.to_string()))
    }
    
    pub async fn send_flow_start(
        &self,
        run_id: &str,
        flow_name: &str,
        parameters: serde_json::Value,
        initial_state: &str,
    ) -> Result<(), SendError> {
        let notification = FlowNotification {
            token: run_id.to_string(),
            progress: Some(0),
            message: format!("Starting workflow: {}", flow_name),
            metadata: FlowNotificationMetadata::FlowStart {
                flow_name: flow_name.to_string(),
                parameters,
                initial_state: initial_state.to_string(),
            },
        };
        self.send(notification)
    }
    
    // Similar methods for other notification types...
}
```

### 3. Update ToolContext

Update `swissarmyhammer-tools/src/mcp/tool_registry.rs`:

```rust
pub struct ToolContext {
    pub working_directory: PathBuf,
    pub notification_sender: Option<NotificationSender>,
}

impl ToolContext {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            notification_sender: None,
        }
    }
    
    pub fn with_notifications(
        working_directory: PathBuf,
        sender: NotificationSender,
    ) -> Self {
        Self {
            working_directory,
            notification_sender: Some(sender),
        }
    }
}
```

### 4. Create Notification Builder Utilities

```rust
impl FlowNotification {
    pub fn flow_start(
        run_id: &str,
        flow_name: &str,
        parameters: serde_json::Value,
        initial_state: &str,
    ) -> Self {
        Self {
            token: run_id.to_string(),
            progress: Some(0),
            message: format!("Starting workflow: {}", flow_name),
            metadata: FlowNotificationMetadata::FlowStart {
                flow_name: flow_name.to_string(),
                parameters,
                initial_state: initial_state.to_string(),
            },
        }
    }
    
    pub fn state_start(
        run_id: &str,
        flow_name: &str,
        state_id: &str,
        state_description: &str,
        progress: u32,
    ) -> Self {
        Self {
            token: run_id.to_string(),
            progress: Some(progress),
            message: format!("Entering state: {}", state_id),
            metadata: FlowNotificationMetadata::StateStart {
                flow_name: flow_name.to_string(),
                state_id: state_id.to_string(),
                state_description: state_description.to_string(),
            },
        }
    }
    
    // Similar builder methods for other notification types...
}
```

### 5. Add Tests

Create `swissarmyhammer-tools/src/mcp/notifications_tests.rs`:

```rust
#[tokio::test]
async fn test_notification_sender() {
    // Test notification sender works
}

#[test]
fn test_flow_notification_serialization() {
    // Test notifications serialize correctly to JSON
}

#[tokio::test]
async fn test_notification_channel() {
    // Test notification channel works end-to-end
}
```

## Files to Create/Modify

- `swissarmyhammer-tools/src/mcp/notifications.rs` (create)
- `swissarmyhammer-tools/src/mcp/notifications_tests.rs` (create)
- `swissarmyhammer-tools/src/mcp/tool_registry.rs` (update)
- `swissarmyhammer-tools/src/mcp/mod.rs` (update)

## Acceptance Criteria

- [ ] FlowNotification types defined for all notification scenarios
- [ ] NotificationSender can send notifications
- [ ] ToolContext includes optional notification sender
- [ ] Builder methods create valid notifications
- [ ] Notifications serialize to correct JSON format
- [ ] All tests pass
- [ ] Code compiles without warnings

## Estimated Changes

~220 lines of code
