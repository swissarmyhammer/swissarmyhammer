# Flow Tool

Execute workflows dynamically via MCP.


## Examples

### Discover Available Workflows

```json
{
  "flow_name": "list",
  "verbose": true
}
```

### Execute Workflow Without Parameters

```json
{
  "flow_name": "do",
  "quiet": true
}
```

### Execute Workflow With Parameters

```json
{
  "flow_name": "plan",
  "parameters": {
    "plan_filename": "requirements.md"
  },
  "interactive": false,
  "dry_run": false
}
```

## Progress Notifications

Long-running workflows send MCP notifications to track execution progress. These notifications enable clients to monitor workflow state without blocking.

### Notification Types

#### Flow Start
Sent when workflow execution begins:

```json
{
  "method": "notifications/progress",
  "params": {
    "progressToken": "<workflow_run_id>",
    "value": 0,
    "message": "Starting workflow: plan"
  }
}
```

The notification metadata includes:
- `flow_name`: Name of the workflow being executed
- `parameters`: The parameters provided to the workflow
- `initial_state`: The starting state of the workflow

### Notification Usage

Clients should:
1. Subscribe to MCP notifications before executing workflows
2. Use the `progressToken` to correlate notifications with specific workflow runs
3. Track progress values to show execution status to users
4. Handle both success (Flow Complete) and failure (Flow Error) cases
5. Remember that notifications are informational - check the tool result for final status
