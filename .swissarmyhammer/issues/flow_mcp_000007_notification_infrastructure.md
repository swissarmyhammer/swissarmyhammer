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



## Proposed Solution

After reviewing the existing MCP infrastructure, I'll implement the notification system following these steps:

### Architecture Design

1. **Reuse existing NotifyRequest patterns**: The codebase already has `notify_types.rs` with `NotifyRequest` and `NotifyLevel`. We'll extend this pattern for flow notifications.

2. **Create FlowNotification types**: Define specialized notification types for workflow execution that extend the existing notification infrastructure with workflow-specific metadata.

3. **Channel-based sender**: Use `tokio::sync::mpsc` for asynchronous notification delivery without blocking workflow execution.

4. **ToolContext integration**: Add optional `NotificationSender` to `ToolContext` (already has structure for this pattern).

### Implementation Details

**Phase 1: Define notification types** (notifications.rs)
- Create `FlowNotification` struct with token, progress, message, and metadata
- Create `FlowNotificationMetadata` enum for different notification types:
  - FlowStart: workflow initiated with parameters
  - StateStart: entering a workflow state
  - StateComplete: exiting a workflow state
  - FlowComplete: workflow finished successfully
  - FlowError: workflow failed with error

**Phase 2: Notification sender** (notifications.rs)
- Create `NotificationSender` wrapping `mpsc::UnboundedSender<FlowNotification>`
- Implement convenience methods for each notification type
- Handle channel errors gracefully

**Phase 3: Builder utilities** (notifications.rs)
- Add builder methods to `FlowNotification` for each notification type
- Ensure consistent message formatting
- Calculate progress percentages based on workflow state position

**Phase 4: ToolContext update** (tool_registry.rs)
- Add `notification_sender: Option<NotificationSender>` field
- Update constructor to accept optional sender
- Maintain backward compatibility with existing code

**Phase 5: Comprehensive testing** (notifications_tests.rs)
- Test notification serialization/deserialization
- Test channel-based notification delivery
- Test builder methods produce correct structures
- Test error handling for closed channels

### Files to Modify
- `swissarmyhammer-tools/src/mcp/notifications.rs` (create)
- `swissarmyhammer-tools/src/mcp/tool_registry.rs` (update ToolContext)
- `swissarmyhammer-tools/src/mcp/mod.rs` (export notifications module)

### Testing Strategy
- Unit tests for each notification type
- Integration tests for notification channel
- Serialization tests to ensure MCP compatibility
- Error handling tests for edge cases



## Implementation Notes

### Completed Work

Successfully implemented the MCP notification infrastructure for workflow progress tracking.

### Files Created/Modified

1. **swissarmyhammer-tools/src/mcp/notifications.rs** (created, 729 lines)
   - Defined `FlowNotification` struct with token, progress, message, and metadata
   - Created `FlowNotificationMetadata` enum with 5 notification types:
     - FlowStart: workflow initiated with parameters
     - StateStart: entering a workflow state  
     - StateComplete: exiting a workflow state
     - FlowComplete: workflow finished successfully
     - FlowError: workflow failed with error
   - Implemented `NotificationSender` wrapping `mpsc::UnboundedSender`
   - Added convenience methods for each notification type
   - Created `SendError` type for channel error handling
   - Comprehensive test coverage (17 tests, all passing)

2. **swissarmyhammer-tools/src/mcp/tool_registry.rs** (modified)
   - Added `notification_sender: Option<NotificationSender>` field to `ToolContext`
   - Updated `ToolContext::new()` to initialize with `None`
   - Added `ToolContext::with_notifications()` constructor for notification-enabled contexts
   - Imported `NotificationSender` from notifications module

3. **swissarmyhammer-tools/src/mcp/mod.rs** (modified)
   - Added `notifications` module declaration
   - Re-exported key types: `FlowNotification`, `FlowNotificationMetadata`, `NotificationSender`, `SendError`

4. **swissarmyhammer-tools/src/mcp/tools/issues/show/mod.rs** (modified)
   - Fixed test to include `notification_sender: None` in ToolContext construction

### Design Decisions

1. **Channel-based async**: Used `tokio::sync::mpsc::UnboundedSender` for non-blocking notification delivery
2. **Optional sender**: Made notification sender optional in ToolContext for backward compatibility
3. **Builder pattern**: Added builder methods to `FlowNotification` for convenience
4. **Serde integration**: Full serialization/deserialization support for MCP compatibility
5. **Type safety**: Used enums for notification metadata to ensure type-safe handling

### Testing Results

- All 17 notification-specific tests passing
- Full test suite: 598 tests passing, 0 failures
- No clippy warnings
- Clean compilation with no errors

### API Examples

```rust
// Create notification channel
let (tx, rx) = mpsc::unbounded_channel();
let sender = NotificationSender::new(tx);

// Send flow start notification
sender.send_flow_start(
    "run_123",
    "implement",
    json!({"issue": "bug-456"}),
    "parse_issue"
)?;

// Send state notifications
sender.send_state_start("run_123", "implement", "state1", "Description", 25)?;
sender.send_state_complete("run_123", "implement", "state1", Some("state2"), 50)?;

// Send completion
sender.send_flow_complete("run_123", "implement", "completed", "done")?;
```

### Next Steps

This infrastructure is ready for integration with workflow execution in subsequent issues. The notification sender can be passed to ToolContext when creating contexts for workflow tools, enabling progress tracking during long-running operations.
