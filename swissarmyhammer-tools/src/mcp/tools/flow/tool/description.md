# Flow Tool

Execute or list workflows dynamically via MCP.

## Usage

### List Available Workflows

Set `flow_name` to "list" to discover available workflows:

```json
{
  "flow_name": "list",
  "format": "json",
  "verbose": true
}
```

Response:
```json
{
  "workflows": [
    {
      "name": "do",
      "description": "Autonomously work through all pending todo items",
      "source": "builtin",
      "parameters": []
    },
    {
      "name": "plan",
      "description": "Execute planning workflow for specific specification files",
      "source": "builtin",
      "parameters": [
        {
          "name": "plan_filename",
          "type": "string",
          "description": "Path to the specification file to process",
          "required": true
        }
      ]
    }
  ]
}
```

### Execute Workflow

Set `flow_name` to a workflow name and provide parameters:

```json
{
  "flow_name": "plan",
  "parameters": {
    "plan_filename": "spec.md"
  },
  "interactive": false
}
```

### Output Formats

When listing workflows, you can specify the output format:

- `json` (default): Structured JSON response
- `yaml`: YAML-formatted output
- `table`: Human-readable table format

### Common Parameters

- `flow_name` (required): Name of the workflow to execute, or "list" to show all workflows
- `parameters` (optional): Workflow-specific parameters as key-value pairs (ignored when flow_name='list')
- `format` (optional): Output format when flow_name='list' (json, yaml, or table)
- `verbose` (optional): Include detailed parameter information when flow_name='list'
- `interactive` (optional): Enable interactive mode for prompts (workflow execution only)
- `dry_run` (optional): Show execution plan without running (workflow execution only)
- `quiet` (optional): Suppress progress output (workflow execution only)

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

### Dry Run Mode

```json
{
  "flow_name": "plan",
  "parameters": {
    "plan_filename": "spec.md"
  },
  "dry_run": true
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

#### State Start
Sent when entering each workflow state:

```json
{
  "method": "notifications/progress", 
  "params": {
    "progressToken": "<workflow_run_id>",
    "value": 25,
    "message": "Entering state: parse_spec"
  }
}
```

The notification metadata includes:
- `flow_name`: Name of the workflow
- `state_id`: Identifier of the state being entered
- `state_description`: Human-readable description of the state

#### State Complete
Sent when exiting each workflow state:

```json
{
  "method": "notifications/progress",
  "params": {
    "progressToken": "<workflow_run_id>",
    "value": 50,
    "message": "Completed state: parse_spec"
  }
}
```

The notification metadata includes:
- `flow_name`: Name of the workflow
- `state_id`: Identifier of the completed state
- `next_state`: Identifier of the next state (if any)

#### Flow Complete
Sent on successful workflow completion:

```json
{
  "method": "notifications/progress",
  "params": {
    "progressToken": "<workflow_run_id>",
    "value": 100,
    "message": "Completed workflow: plan"
  }
}
```

The notification metadata includes:
- `flow_name`: Name of the workflow
- `status`: Final workflow status
- `final_state`: The final state reached

#### Flow Error
Sent if workflow execution fails:

```json
{
  "method": "notifications/progress",
  "params": {
    "progressToken": "<workflow_run_id>",
    "message": "Workflow failed: plan"
  }
}
```

Note: Progress value is omitted (null) for error notifications.

The notification metadata includes:
- `flow_name`: Name of the workflow
- `status`: Final workflow status
- `error_state`: State where the error occurred
- `error`: Error message describing the failure

### Progress Calculation

Progress percentages are approximate, calculated based on the number of executed states versus total states in the workflow. This may not be accurate for workflows with loops or conditional branches.

### Notification Usage

Clients should:
1. Subscribe to MCP notifications before executing workflows
2. Use the `progressToken` to correlate notifications with specific workflow runs
3. Track progress values to show execution status to users
4. Handle both success (Flow Complete) and failure (Flow Error) cases
5. Remember that notifications are informational - check the tool result for final status

### Example Notification Sequence

A typical workflow execution produces this notification sequence:

1. **Flow Start** (progress: 0%) - Workflow begins
2. **State Start** (progress: 20%) - First state begins
3. **State Complete** (progress: 40%) - First state completes
4. **State Start** (progress: 40%) - Second state begins
5. **State Complete** (progress: 60%) - Second state completes
6. **State Start** (progress: 60%) - Third state begins
7. **State Complete** (progress: 80%) - Third state completes
8. **Flow Complete** (progress: 100%) - Workflow succeeds

Or in case of failure:

1. **Flow Start** (progress: 0%)
2. **State Start** (progress: 33%)
3. **State Complete** (progress: 66%)
4. **State Start** (progress: 66%)
5. **Flow Error** (progress: null) - Workflow fails
