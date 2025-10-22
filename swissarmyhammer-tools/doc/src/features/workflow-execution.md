# Workflow Execution

SwissArmyHammer provides workflow execution capabilities using YAML specifications with AI coordination.

## Overview

Workflows enable you to define and execute multi-step development processes. Each workflow is specified in YAML and can coordinate AI agents to perform complex tasks.

## Available Tool

### flow

Execute or list workflows dynamically via MCP.

**Parameters:**
- `flow_name` (required): Name of the workflow to execute, or "list" to show all workflows
- `parameters` (optional): Workflow-specific parameters as key-value pairs
- `format` (optional): Output format when flow_name='list' (json, yaml, or table)
- `verbose` (optional): Include detailed parameter information when flow_name='list'
- `interactive` (optional): Enable interactive mode for prompts (workflow execution only)
- `dry_run` (optional): Show execution plan without running (workflow execution only)
- `quiet` (optional): Suppress progress output (workflow execution only)

## Listing Workflows

Discover available workflows:

```json
{
  "flow_name": "list",
  "format": "json",
  "verbose": true
}
```

Returns:
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

## Executing Workflows

### Basic Execution

Execute a workflow without parameters:

```json
{
  "flow_name": "implement",
  "quiet": true
}
```

### With Parameters

Execute a workflow with parameters:

```json
{
  "flow_name": "plan",
  "parameters": {
    "plan_filename": "spec.md"
  },
  "interactive": false
}
```

### Dry Run Mode

See what a workflow will do without executing:

```json
{
  "flow_name": "plan",
  "parameters": {
    "plan_filename": "spec.md"
  },
  "dry_run": true
}
```

## Built-in Workflows

### implement

Execute autonomous issue resolution workflow.

**Purpose:** Automatically work through issues

**Parameters:** None

**Example:**
```json
{
  "flow_name": "implement"
}
```

### plan

Execute planning workflow for specification files.

**Purpose:** Convert specifications into actionable plans

**Parameters:**
- `plan_filename` (required): Path to specification file

**Example:**
```json
{
  "flow_name": "plan",
  "parameters": {
    "plan_filename": "requirements.md"
  }
}
```

## Progress Notifications

Long-running workflows send MCP notifications to track execution progress.

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

### Progress Calculation

Progress percentages are approximate, calculated based on the number of executed states versus total states in the workflow. This may not be accurate for workflows with loops or conditional branches.

## Workflow Integration Patterns

### Issue-Driven Workflow

1. Create issue with requirements
2. Execute `implement` workflow
3. Workflow processes issue automatically
4. Review and complete

### Specification-Driven Workflow

1. Write specification document
2. Execute `plan` workflow with specification
3. Review generated plan
4. Execute implementation

### Custom Workflow Pattern

1. Define workflow in YAML
2. Register with workflow system
3. Execute via `flow` tool
4. Monitor progress notifications

## Best Practices

### Defining Workflows

1. **Clear Steps**: Define discrete, testable steps
2. **Error Handling**: Include error states and recovery
3. **Progress Tracking**: Emit progress at key points
4. **Parameterization**: Make workflows flexible with parameters

### Executing Workflows

1. **Dry Run First**: Use dry_run to preview execution
2. **Monitor Progress**: Watch notifications for status
3. **Handle Errors**: Be prepared for failures
4. **Review Results**: Always review workflow output

### Interactive Mode

1. **Prompts**: Interactive mode enables user prompts
2. **Decisions**: Make decisions during execution
3. **Validation**: Validate steps before proceeding
4. **Flexibility**: Adjust workflow based on context

## Error Handling

Workflows can fail at any state. When a failure occurs:

1. **Flow Error Notification**: Sent with error details
2. **Error State**: Identifies where failure occurred
3. **Error Message**: Describes what went wrong
4. **Recovery**: Workflow system handles cleanup

## Workflow State Management

Workflows maintain state through execution:

- **Initial State**: Starting point of workflow
- **Transitions**: Movement between states
- **Current State**: Where workflow is now
- **Final State**: Completion or error state

## Performance Considerations

- **Long-Running**: Some workflows may take minutes to complete
- **Resource Usage**: Monitor CPU and memory during execution
- **Notifications**: Can be numerous for complex workflows
- **Concurrency**: Only one workflow per flow tool instance

## Use Cases

### Autonomous Development

Execute `implement` workflow to autonomously work through issues:

```json
{
  "flow_name": "implement",
  "quiet": false
}
```

### Specification Processing

Convert requirements into actionable plans:

```json
{
  "flow_name": "plan",
  "parameters": {
    "plan_filename": "requirements.md"
  }
}
```

### Testing Workflows

Preview workflow execution without changes:

```json
{
  "flow_name": "test",
  "dry_run": true
}
```

## Limitations

- **No Persistence**: Workflow state is not persisted between runs
- **No Pause/Resume**: Cannot pause and resume workflows
- **Single Instance**: One workflow execution at a time per tool instance
- **Fixed Workflows**: Cannot modify workflows at runtime

## Next Steps

- [Issue Management](./issue-management.md): Track work with issues
- [File Operations](./file-operations.md): File manipulation
- [Git Integration](./git-integration.md): Track changes
