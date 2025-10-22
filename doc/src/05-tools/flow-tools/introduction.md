# Flow Operations

The Flow Operations tool provides dynamic workflow execution via the MCP protocol, enabling AI assistants to discover and execute workflows with real-time progress tracking.

## Overview

The Flow tool bridges workflows and MCP, allowing:

- Discovery of available workflows and their parameters
- Dynamic workflow execution with parameter passing
- Real-time progress notifications during execution
- Integration with Claude Code and other MCP clients

## Key Concepts

### Workflow Discovery

The Flow tool can list all available workflows from:

- **Builtin**: Workflows embedded in the SwissArmyHammer binary
- **User**: Personal workflows in `~/.swissarmyhammer/workflows/`
- **Local**: Project-specific workflows in `.swissarmyhammer/workflows/`

This enables AI assistants to discover and understand available workflows without hardcoding.

### Dynamic Execution

Workflows can be executed with parameters:

```json
{
  "flow_name": "plan",
  "parameters": {
    "plan_filename": "spec.md"
  }
}
```

Parameters are validated against workflow definitions and passed to workflow states.

### Progress Notifications

Long-running workflows send MCP progress notifications:

- **Flow Start**: Workflow begins execution
- **State Start**: Entering a workflow state
- **State Complete**: Exiting a workflow state
- **Flow Complete**: Workflow completes successfully
- **Flow Error**: Workflow fails with error

These notifications enable real-time progress tracking in AI assistants.

## Available Tools

- [`flow`](execute.md) - Execute or list workflows dynamically

## Use Cases

### Workflow Discovery

AI assistants can discover available workflows:

```json
{
  "flow_name": "list",
  "verbose": true
}
```

Response includes workflow names, descriptions, parameters, and source locations.

### Autonomous Execution

AI assistants can execute workflows based on user intent:

```
User: "Implement the feature described in requirements.md"