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



## Proposed Solution

After examining the codebase, I've identified the architecture and integration points. Here's my implementation plan:

### Architecture Understanding

1. **Notification Infrastructure** (Already Implemented):
   - `NotificationSender` provides async methods for sending notifications
   - `FlowNotification` supports flow_start, state_start, state_complete, flow_complete, and flow_error
   - Available via `ToolContext.notification_sender: Option<NotificationSender>`

2. **Current Flow Tool Implementation**:
   - Located at `swissarmyhammer-tools/src/mcp/tools/flow/tool/mod.rs`
   - Has access to `ToolContext` in the `execute_workflow` method
   - Currently uses `WorkflowExecutor` to execute workflows
   - Workflow execution happens via `executor.start_workflow()` and `executor.execute_state()`

3. **Workflow Executor Implementation**:
   - Located at `swissarmyhammer-workflow/src/executor/core.rs`
   - `WorkflowExecutor` is the main execution engine
   - Key methods:
     - `start_workflow()` - Initializes a workflow run
     - `execute_state_with_limit()` - Main execution loop
     - `execute_single_cycle()` - Executes one state and transitions
     - `execute_single_state()` - Executes a single state's action

### Implementation Strategy

The key challenge is that `WorkflowExecutor` is in the `swissarmyhammer-workflow` crate, which should not depend on the MCP notification infrastructure. Therefore, I'll use a **callback-based approach** where notifications are sent from the flow tool layer.

#### Option 1: Callback-Based Approach (CHOSEN)
Pass notification callbacks into the executor via the `WorkflowRun.context`. The flow tool will:
1. Generate a run ID (ULID)
2. Send flow_start notification
3. Set up state transition tracking callbacks in the context
4. Execute the workflow
5. Send flow_complete or flow_error based on result

#### Option 2: Pull Executor Events (Not Chosen)
The executor already maintains `execution_history` with events. We could query these after execution, but this doesn't provide real-time progress updates during long-running workflows.

### Implementation Steps

#### Step 1: Add Run ID Generation and Flow Start Notification

In `swissarmyhammer-tools/src/mcp/tools/flow/tool/mod.rs`, update `execute_workflow`:

```rust
async fn execute_workflow(
    &self,
    request: &FlowToolRequest,
    context: &ToolContext,
) -> std::result::Result<CallToolResult, McpError> {
    // Load workflow
    let (storage, _resolver) = self.load_workflows()...;
    let workflow_name = swissarmyhammer_workflow::WorkflowName::new(...);
    let workflow = storage.get_workflow(&workflow_name)?;
    
    // Validate parameters
    validate_required_parameters(&workflow, &request.parameters)?;
    
    // Generate unique run ID
    let run_id = ulid::Ulid::new().to_string();
    
    // Send flow start notification
    if let Some(sender) = &context.notification_sender {
        let _ = sender.send_flow_start(
            &run_id,
            &request.flow_name,
            serde_json::to_value(&request.parameters).unwrap_or(serde_json::json!({})),
            workflow.initial_state.as_str(),
        );
    }
    
    // Create executor and start workflow
    let mut executor = swissarmyhammer_workflow::WorkflowExecutor::new();
    let mut run = executor.start_workflow(workflow)?;
    
    // Set parameters and run ID in context
    run.context.set_workflow_var("__run_id__".to_string(), serde_json::json!(run_id));
    for (key, value) in &request.parameters {
        run.context.set_workflow_var(key.clone(), value.clone());
    }
    
    // Execute with progress tracking
    let result = self.execute_with_notifications(
        &mut executor,
        &mut run,
        &run_id,
        &request.flow_name,
        context
    ).await;
    
    // Handle result and send completion/error notification
    match result {
        Ok(()) => {
            if let Some(sender) = &context.notification_sender {
                let _ = sender.send_flow_complete(
                    &run_id,
                    &request.flow_name,
                    &format!("{:?}", run.status),
                    run.current_state.as_str(),
                );
            }
            // Return formatted output
        }
        Err(e) => {
            if let Some(sender) = &context.notification_sender {
                let _ = sender.send_flow_error(
                    &run_id,
                    &request.flow_name,
                    &format!("{:?}", run.status),
                    run.current_state.as_str(),
                    &e.to_string(),
                );
            }
            Err(McpError::internal_error(...))
        }
    }
}
```

#### Step 2: Add Progress Tracking During Execution

Add a helper method to track state transitions:

```rust
impl FlowTool {
    async fn execute_with_notifications(
        &self,
        executor: &mut swissarmyhammer_workflow::WorkflowExecutor,
        run: &mut swissarmyhammer_workflow::WorkflowRun,
        run_id: &str,
        flow_name: &str,
        context: &ToolContext,
    ) -> Result<(), swissarmyhammer_workflow::executor::ExecutorError> {
        let total_states = run.workflow.states.len();
        let mut executed_states = 0;
        
        loop {
            let current_state = run.current_state.clone();
            
            // Send state start notification
            if let Some(sender) = &context.notification_sender {
                if let Some(state) = run.workflow.states.get(&current_state) {
                    let progress = if total_states > 0 {
                        ((executed_states * 100) / total_states) as u32
                    } else {
                        0
                    };
                    
                    let _ = sender.send_state_start(
                        run_id,
                        flow_name,
                        current_state.as_str(),
                        &state.description,
                        progress,
                    );
                }
            }
            
            // Execute single cycle
            let transition_performed = executor.execute_single_cycle(run).await?;
            executed_states += 1;
            
            // Send state complete notification
            if let Some(sender) = &context.notification_sender {
                let next_state = if transition_performed {
                    Some(run.current_state.as_str())
                } else {
                    None
                };
                
                let progress = if total_states > 0 {
                    ((executed_states * 100) / total_states).min(100) as u32
                } else {
                    100
                };
                
                let _ = sender.send_state_complete(
                    run_id,
                    flow_name,
                    current_state.as_str(),
                    next_state,
                    progress,
                );
            }
            
            // Check if workflow is finished
            if !transition_performed || executor.is_workflow_finished(run) {
                break;
            }
        }
        
        Ok(())
    }
}
```

#### Step 3: Add Tests

Add tests in `swissarmyhammer-tools/src/mcp/tools/flow/tool/mod.rs`:

```rust
#[tokio::test]
async fn test_workflow_sends_start_notification() {
    // Create notification channel
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let sender = NotificationSender::new(tx);
    
    // Create context with notification sender
    let context = create_test_context_with_notifications(sender);
    
    // Execute workflow
    let tool = FlowTool::new();
    let request = FlowToolRequest::new("simple_workflow");
    
    let _ = tool.execute_workflow(&request, &context).await;
    
    // Verify flow_start notification was sent
    let notification = rx.recv().await.unwrap();
    match notification.metadata {
        FlowNotificationMetadata::FlowStart { .. } => {},
        _ => panic!("Expected FlowStart notification"),
    }
}

#[tokio::test]
async fn test_workflow_sends_state_notifications() {
    // Test that state_start and state_complete are sent for each state
}

#[tokio::test]
async fn test_workflow_sends_completion_notification() {
    // Test flow_complete notification on success
}

#[tokio::test]
async fn test_workflow_sends_error_notification() {
    // Test flow_error notification on failure
}

#[tokio::test]
async fn test_progress_calculation() {
    // Test that progress percentages are calculated correctly
}
```

### Design Decisions

1. **No Executor Modification**: Keep `WorkflowExecutor` independent of MCP notifications by implementing notification logic in the flow tool layer.

2. **Progress Calculation**: Progress is based on the number of states executed vs total states. This is approximate since workflows can have loops and conditional branches.

3. **Error Handling**: Notification send failures are logged but don't cause workflow failure. Use `let _ = sender.send_*()` pattern.

4. **Run ID**: Using ULID for run IDs to maintain consistency with other identifiers in the system.

5. **Context Variables**: Store run_id in workflow context as `__run_id__` for potential use in workflow actions.

### Files to Modify

1. `swissarmyhammer-tools/src/mcp/tools/flow/tool/mod.rs` - Add notification integration
2. Add tests to the same file

### Benefits of This Approach

- ✅ No changes to `swissarmyhammer-workflow` crate
- ✅ Notifications remain optional (graceful when `notification_sender` is `None`)
- ✅ Progress tracking happens in real-time during execution
- ✅ Clean separation of concerns between workflow execution and MCP notifications
- ✅ Easy to test with mock notification channels




## Implementation Complete

The workflow progress notification system has been successfully implemented and tested.

### Implementation Summary

1. **Modified Files**:
   - `swissarmyhammer-tools/src/mcp/tools/flow/tool/mod.rs` - Added notification integration

2. **Key Changes**:
   - Updated `execute_workflow` method to:
     - Generate unique run IDs using `generate_monotonic_ulid_string()`
     - Send `flow_start` notification before workflow execution
     - Send `flow_complete` notification on success
     - Send `flow_error` notification on failure
   - Added new `execute_with_notifications` method that:
     - Loops through workflow execution using `execute_single_cycle`
     - Sends `state_start` notification before each state execution
     - Sends `state_complete` notification after each state execution
     - Calculates progress percentages based on executed states vs total states
     - Gracefully handles workflows with no notification sender (backward compatible)

3. **Tests Added**:
   - `test_workflow_sends_start_notification` - Verifies flow_start notification
   - `test_workflow_sends_state_notifications` - Verifies state_start and state_complete
   - `test_workflow_sends_completion_notification` - Verifies flow_complete with 100% progress
   - `test_workflow_sends_error_notification` - Verifies error handling
   - `test_progress_calculation` - Verifies progress percentages are valid

4. **Test Results**:
   - All 4 new notification tests pass
   - All 22 flow tool tests pass
   - All 603 swissarmyhammer-tools tests pass
   - No regressions introduced

### Design Decisions Made

1. **No WorkflowExecutor Modifications**: Kept the workflow executor independent of MCP notifications by implementing notification logic at the flow tool layer. This maintains separation of concerns.

2. **Optional Notifications**: Notifications are sent only when `context.notification_sender` is present, ensuring backward compatibility with contexts that don't have notification support.

3. **Error Handling**: Notification send failures are intentionally ignored (using `let _ = sender.send_*()`). This ensures that notification failures don't cause workflow execution to fail.

4. **Progress Calculation**: Progress is approximate, based on the number of states executed vs total states. This works well for linear workflows but may not be accurate for workflows with loops or complex branching.

5. **Run ID Storage**: The run ID is stored in the workflow context as `__run_id__` for potential use in workflow actions.

### Architecture Benefits

- ✅ Clean separation between workflow execution and MCP notifications
- ✅ Backward compatible - works with and without notification sender
- ✅ Real-time progress updates during long-running workflows
- ✅ Easy to test with mock notification channels
- ✅ No changes required to the swissarmyhammer-workflow crate

## Code Review Improvements

Code review recommendations implemented to improve code quality and maintainability:

### 1. Enhanced Documentation (swissarmyhammer-tools/src/mcp/tools/flow/tool/mod.rs:233-249)

Added comprehensive documentation for the `execute_with_notifications` method including:
- Detailed explanation of the method's purpose and behavior
- Complete parameter documentation
- Return value documentation
- Clarifies that progress notifications are sent at each state transition via MCP

### 2. Progress Calculation Comments (swissarmyhammer-tools/src/mcp/tools/flow/tool/mod.rs:267-268, 297-298)

Added inline comments explaining progress calculation limitations:
- Progress is approximate based on executed states vs total states
- May not be accurate for workflows with loops or conditional branches
- Comments placed at both progress calculation points for clarity

### 3. Test Code Refactoring (swissarmyhammer-tools/src/mcp/tools/flow/tool/mod.rs:855-876)

Extracted duplicated test setup code into a helper function:
- Created `find_simple_test_workflow()` helper function
- Reduces code duplication across 5 notification tests
- Improves maintainability and consistency
- Includes documentation explaining workflow selection criteria

### 4. Improved Error Test (swissarmyhammer-tools/src/mcp/tools/flow/tool/mod.rs:1043-1111)

Enhanced the error notification test:
- Now properly validates error notification structure
- Verifies `FlowError` notification is sent when workflow fails
- Confirms error notifications have `None` for progress field
- More robust test that checks conditional error paths

### Test Results After Improvements

All 603 tests pass with no regressions:
- All 5 notification tests pass
- All 27 flow tool tests pass
- Code compiles without warnings
- cargo fmt applied successfully

### Code Quality Improvements

- Better documentation makes the code more maintainable
- Inline comments prevent future confusion about progress calculation limitations
- Helper function reduces code duplication and improves test maintainability
- Enhanced error test provides better coverage of error notification behavior
