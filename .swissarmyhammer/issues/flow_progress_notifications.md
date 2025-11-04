# Add Progress Notifications to flow MCP Tool

## Background

The MCP specification includes support for progress notifications (`notifications/progress`) that allow long-running operations to send real-time updates to clients. Our `flow` tool (workflow execution) currently lacks this capability, which means users have no feedback during long-running workflow executions that may involve multiple states and actions.

## Problem

Workflows can execute multiple states, each potentially taking significant time:
- State transitions
- Action executions within states
- Conditional branching
- Loop iterations

Users currently have no visibility into:
- Which state is currently executing
- How many states have completed
- Overall workflow progress
- Whether the workflow is still running or hung

## Proposed Solution

Implement batched progress notifications for workflow execution that report:
- Workflow start with initial state
- State transitions as they occur
- Completion with final state and outcome

### Progress Notification Flow

1. **Start**: Send progress=0, message="Starting workflow: {workflow_name}"
2. **State Transitions**: Send progress updates as states execute
3. **Completion**: Send final progress with execution summary

### Metadata to Include

Completion notification should include:
- `workflow_name`: Name of executed workflow
- `final_state`: Final state reached
- `states_executed`: Number of states executed
- `duration_ms`: Execution time in milliseconds
- `outcome`: Success, failure, or error status

## Implementation Notes

- Follow the pattern established in `shell_execute` tool
- Use the existing `ProgressSender` from `ToolContext`
- Ensure notifications don't block workflow execution
- Handle cases where progress_sender is None gracefully

## Success Criteria

- ✅ Workflow executions send start notification
- ✅ State transitions send progress updates
- ✅ Completion notification includes execution summary
- ✅ Notification failures don't affect workflow execution
- ✅ Tests verify progress notification behavior
- ✅ No performance regression in workflow execution

## References

- [MCP Progress Specification](https://modelcontextprotocol.io/specification/2025-06-18/schema#notifications%2Fprogress)
- Existing implementation: `swissarmyhammer-tools/src/mcp/tools/shell/execute/mod.rs` (lines 846-1318)
- Workflow executor: `swissarmyhammer-workflow/src/executor.rs`
- Tool implementation: `swissarmyhammer-tools/src/mcp/tools/flow/tool/mod.rs`