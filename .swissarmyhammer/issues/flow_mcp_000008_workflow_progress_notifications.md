# Step 8: Implement Workflow Progress Notifications

Refer to ideas/flow_mcp.md

## Objective

Integrate notification sending into workflow execution, sending progress updates at key points.

## Context

With notification infrastructure in place, we now need to integrate it into the flow tool's workflow execution to send notifications at appropriate times during execution.

## Tasks

### 1. Update FlowTool to Use Notifications

Update `swissarmyhammer-tools/src/mcp/tools/flow/tool.rs`:

```rust
async fn execute_workflow(
    &self,
    flow_name: &str,
    parameters: serde_json::Map<String, serde_json::Value>,
    interactive: bool,
    dry_run: bool,
    quiet: bool,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let workflow = self.workflow_storage
        .get_workflow(&WorkflowName::new(flow_name))?;
    
    validate_required_parameters(&workflow, &parameters)?;
    
    // Generate unique run ID
    let run_id = Ulid::new().to_string();
    
    // Send flow start notification
    if let Some(sender) = &context.notification_sender {
        sender.send_flow_start(
            &run_id,
            flow_name,
            serde_json::to_value(&parameters)?,
            &workflow.initial_state,
        ).await?;
    }
    
    // Execute workflow with notification callbacks
    let executor = WorkflowExecutor::new();
    
    let result = match executor.execute_with_notifications(
        workflow,
        parameters,
        interactive,
        dry_run,
        quiet,
        &run_id,
        context.notification_sender.as_ref(),
    ).await {
        Ok(result) => {
            // Send completion notification
            if let Some(sender) = &context.notification_sender {
                sender.send_flow_complete(
                    &run_id,
                    flow_name,
                    &result.final_state,
                ).await?;
            }
            result
        }
        Err(e) => {
            // Send error notification
            if let Some(sender) = &context.notification_sender {
                sender.send_flow_error(
                    &run_id,
                    flow_name,
                    &e.state,
                    &e.message,
                ).await?;
            }
            return Err(McpError::internal_error(format!("Workflow failed: {}", e)));
        }
    };
    
    let output = format_workflow_result(&result)?;
    Ok(BaseToolImpl::create_success_response(output))
}
```

### 2. Add Notification Support to WorkflowExecutor

Update or create workflow executor with notification hooks:

```rust
impl WorkflowExecutor {
    pub async fn execute_with_notifications(
        &self,
        workflow: Workflow,
        parameters: HashMap<String, String>,
        interactive: bool,
        dry_run: bool,
        quiet: bool,
        run_id: &str,
        notification_sender: Option<&NotificationSender>,
    ) -> Result<WorkflowResult> {
        let total_states = workflow.states.len();
        let mut current_state_index = 0;
        
        for state in &workflow.states {
            // Send state start notification
            if let Some(sender) = notification_sender {
                let progress = (current_state_index * 100) / total_states;
                sender.send_state_start(
                    run_id,
                    &workflow.name,
                    &state.id,
                    &state.description,
                    progress as u32,
                ).await?;
            }
            
            // Execute state
            self.execute_state(state, &parameters, interactive, dry_run, quiet).await?;
            
            // Send state complete notification
            if let Some(sender) = notification_sender {
                let next_state = workflow.states.get(current_state_index + 1)
                    .map(|s| s.id.clone());
                sender.send_state_complete(
                    run_id,
                    &workflow.name,
                    &state.id,
                    next_state.as_deref(),
                ).await?;
            }
            
            current_state_index += 1;
        }
        
        Ok(WorkflowResult {
            workflow_name: workflow.name.clone(),
            duration: /* ... */,
            states: workflow.states.clone(),
            final_state: workflow.states.last().unwrap().id.clone(),
        })
    }
}
```

### 3. Add Progress Calculation

```rust
fn calculate_progress(current_state: usize, total_states: usize) -> u32 {
    if total_states == 0 {
        return 100;
    }
    ((current_state * 100) / total_states) as u32
}
```

### 4. Add Error Context

Update error types to include state information:

```rust
#[derive(Debug)]
pub struct WorkflowExecutionError {
    pub message: String,
    pub state: String,
    pub workflow: String,
}

impl std::fmt::Display for WorkflowExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Workflow '{}' failed in state '{}': {}", 
               self.workflow, self.state, self.message)
    }
}
```

### 5. Add Tests

```rust
#[tokio::test]
async fn test_workflow_sends_start_notification() {
    // Test flow start notification is sent
}

#[tokio::test]
async fn test_workflow_sends_state_notifications() {
    // Test state start/complete notifications sent for each state
}

#[tokio::test]
async fn test_workflow_sends_completion_notification() {
    // Test flow complete notification sent on success
}

#[tokio::test]
async fn test_workflow_sends_error_notification() {
    // Test error notification sent on failure
}

#[tokio::test]
async fn test_progress_calculation() {
    // Test progress percentages are calculated correctly
}
```

## Files to Modify

- `swissarmyhammer-tools/src/mcp/tools/flow/tool.rs`
- `swissarmyhammer-tools/src/workflow/executor.rs` (or equivalent)
- `swissarmyhammer-tools/src/mcp/tools/flow/tests.rs`

## Acceptance Criteria

- [ ] Flow start notification sent before execution
- [ ] State start notification sent for each state
- [ ] State complete notification sent after each state
- [ ] Flow complete notification sent on success
- [ ] Flow error notification sent on failure
- [ ] Progress percentages calculated correctly
- [ ] Run ID included in all notifications
- [ ] All tests pass
- [ ] Code compiles without warnings

## Estimated Changes

~240 lines of code
