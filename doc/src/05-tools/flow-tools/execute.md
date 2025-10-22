# flow

Execute or list workflows dynamically via MCP.

## Purpose

The `flow` tool provides a unified interface for workflow discovery and execution through the MCP protocol. It enables AI assistants to:

- Discover available workflows without hardcoding
- Understand workflow parameters and requirements
- Execute workflows with appropriate parameters
- Track workflow progress in real-time

## Parameters

### flow_name (required)

The name of the workflow to execute, or "list" to discover available workflows.

- **Type**: String
- **Required**: Yes
- **Special Values**:
  - `"list"` - Discover all available workflows
  - Any workflow name (e.g., `"implement"`, `"plan"`)

### parameters (optional)

Workflow-specific parameters as key-value pairs. Ignored when `flow_name='list'`.

- **Type**: Object (key-value map)
- **Required**: No (depends on workflow requirements)
- **Example**: `{"plan_filename": "spec.md", "target_branch": "main"}`

### format (optional)

Output format when listing workflows. Only applies when `flow_name='list'`.

- **Type**: String (enum)
- **Required**: No
- **Values**: `"json"`, `"yaml"`, `"table"`
- **Default**: `"json"`

### verbose (optional)

Include detailed parameter information when listing workflows.

- **Type**: Boolean
- **Required**: No
- **Default**: `false`
- **Note**: Only applies when `flow_name='list'`

### interactive (optional)

Enable interactive mode for prompts during workflow execution.

- **Type**: Boolean
- **Required**: No
- **Default**: `false`
- **Note**: Only applies during workflow execution

### dry_run (optional)

Show execution plan without actually running the workflow.

- **Type**: Boolean
- **Required**: No
- **Default**: `false`
- **Note**: Only applies during workflow execution

### quiet (optional)

Suppress progress output during workflow execution.

- **Type**: Boolean
- **Required**: No
- **Default**: `false`
- **Note**: Only applies during workflow execution

## Response Format

### Listing Workflows (flow_name='list')

```json
{
  "workflows": [
    {
      "name": "implement",
      "description": "Execute the implement workflow for autonomous issue resolution",
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

### Executing Workflow

```json
{
  "workflow": "plan",
  "status": "completed",
  "result": {
    "final_state": "complete",
    "outputs": {
      "issues_created": 3,
      "files_generated": ["plan.md", "architecture.md"]
    }
  },
  "execution_time_ms": 15342
}
```

## Examples

### MCP Usage (Claude Code)

#### Discover Available Workflows

```json
{
  "tool": "flow",
  "parameters": {
    "flow_name": "list",
    "verbose": true
  }
}
```

#### Execute Workflow Without Parameters

```json
{
  "tool": "flow",
  "parameters": {
    "flow_name": "implement",
    "quiet": false
  }
}
```

#### Execute Workflow With Parameters

```json
{
  "tool": "flow",
  "parameters": {
    "flow_name": "plan",
    "parameters": {
      "plan_filename": "requirements.md"
    },
    "interactive": false
  }
}
```

#### Dry Run Mode

```json
{
  "tool": "flow",
  "parameters": {
    "flow_name": "plan",
    "parameters": {
      "plan_filename": "spec.md"
    },
    "dry_run": true
  }
}
```

### CLI Usage

```bash
# List all workflows
sah flow run list

# List with details
sah flow run list --verbose

# Execute workflow
sah flow run implement

# Execute with parameters
sah flow run plan --param plan_filename=requirements.md

# Dry run
sah flow run plan --param plan_filename=spec.md --dry-run

# Quiet mode
sah flow run implement --quiet
```

## Progress Notifications

Long-running workflows send MCP progress notifications to track execution state. These notifications enable clients to monitor workflow progress without blocking.

### Notification Types

#### 1. Flow Start

Sent when workflow execution begins:

```json
{
  "method": "notifications/progress",
  "params": {
    "progressToken": "01H5ABCDEF1234567890WXYZ",
    "value": 0,
    "message": "Starting workflow: plan"
  }
}
```

Metadata includes:
- `flow_name`: Name of the workflow being executed
- `parameters`: The parameters provided to the workflow
- `initial_state`: The starting state of the workflow

#### 2. State Start

Sent when entering each workflow state:

```json
{
  "method": "notifications/progress",
  "params": {
    "progressToken": "01H5ABCDEF1234567890WXYZ",
    "value": 25,
    "message": "Entering state: parse_spec"
  }
}
```

Metadata includes:
- `flow_name`: Name of the workflow
- `state_id`: Identifier of the state being entered
- `state_description`: Human-readable description of the state

#### 3. State Complete

Sent when exiting each workflow state:

```json
{
  "method": "notifications/progress",
  "params": {
    "progressToken": "01H5ABCDEF1234567890WXYZ",
    "value": 50,
    "message": "Completed state: parse_spec"
  }
}
```

Metadata includes:
- `flow_name`: Name of the workflow
- `state_id`: Identifier of the completed state
- `next_state`: Identifier of the next state (if any)

#### 4. Flow Complete

Sent on successful workflow completion:

```json
{
  "method": "notifications/progress",
  "params": {
    "progressToken": "01H5ABCDEF1234567890WXYZ",
    "value": 100,
    "message": "Completed workflow: plan"
  }
}
```

Metadata includes:
- `flow_name`: Name of the workflow
- `status`: Final workflow status
- `final_state`: The final state reached

#### 5. Flow Error

Sent if workflow execution fails:

```json
{
  "method": "notifications/progress",
  "params": {
    "progressToken": "01H5ABCDEF1234567890WXYZ",
    "message": "Workflow failed: plan"
  }
}
```

Note: Progress value is omitted (null) for error notifications.

Metadata includes:
- `flow_name`: Name of the workflow
- `status`: Final workflow status
- `error_state`: State where the error occurred
- `error`: Error message describing the failure

### Progress Calculation

Progress percentages are approximate, calculated based on the number of executed states versus total states in the workflow. This may not be accurate for workflows with loops or conditional branches.

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

### Using Notifications

Clients should:

1. Subscribe to MCP notifications before executing workflows
2. Use the `progressToken` to correlate notifications with specific workflow runs
3. Track progress values to show execution status to users
4. Handle both success (Flow Complete) and failure (Flow Error) cases
5. Remember that notifications are informational - check the tool result for final status

## Use Cases

### AI-Driven Workflow Selection

AI assistants can analyze user intent and select appropriate workflows:

```
User: "I need to implement the feature in requirements.md"
Assistant: (lists workflows, identifies "plan" workflow)
Assistant: (executes flow tool with plan workflow and parameters)
```

This enables natural language workflow invocation without hardcoding.

### Progress Tracking

Display real-time progress in UI:

```typescript
// Claude Code integration
client.onNotification("notifications/progress", (params) => {
  if (params.progressToken === currentWorkflowToken) {
    updateProgressBar(params.value);
    showMessage(params.message);
  }
});
```

### Workflow Validation

Test workflows before execution:

```bash
# Dry run to validate workflow
sah flow run complex-deployment \
  --param environment=production \
  --param version=2.0.0 \
  --dry-run

# Shows execution plan without running
```

## Best Practices

### Discovery Before Execution

Always list workflows first to understand available options:

```bash
# Discover workflows with details
sah flow run list --verbose
```

This helps understand parameters and requirements.

### Parameter Validation

Workflows validate parameters before execution. Provide all required parameters:

```json
{
  "flow_name": "deploy",
  "parameters": {
    "environment": "production",
    "version": "2.0.0",
    "notify": true
  }
}
```

### Handle Progress Notifications

Subscribe to notifications for long-running workflows:

```javascript
// Track workflow progress
const progressHandler = (notification) => {
  console.log(`Progress: ${notification.value}% - ${notification.message}`);
};

mcpClient.on("notifications/progress", progressHandler);
```

### Error Handling

Always check workflow execution results:

```typescript
const result = await flowTool.execute({
  flow_name: "implement",
  parameters: {}
});

if (result.status === "failed") {
  console.error(`Workflow failed at state: ${result.error_state}`);
  console.error(`Error: ${result.error}`);
}
```

## Limitations

### Workflow Availability

The tool can only execute workflows that are:
- Installed in the system (builtin, user, or local)
- Properly formatted with valid YAML syntax
- Accessible from the current working directory

### Parameter Types

Workflow parameters are passed as key-value pairs. Complex nested structures may require special handling.

### Progress Accuracy

Progress percentages are approximate for workflows with:
- Conditional branches
- Loops
- Variable-length operations

### Interactive Mode Limitations

Interactive mode (`interactive: true`) may not work well in all MCP clients. Use with caution.

## See Also

- [Workflows Introduction](../../04-workflows/workflows.md) - Learn about workflow structure
- [Creating Workflows](../../04-workflows/creating.md) - Build custom workflows
- [Workflow Parameters](../../04-workflows/workflow-parameters.md) - Parameter system details

