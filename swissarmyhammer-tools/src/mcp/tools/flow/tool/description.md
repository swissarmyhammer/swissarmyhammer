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
  "flow_name": "implement",
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
