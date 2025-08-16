# MCP Notify Tool Specification

## Overview

This specification defines a new MCP tool `notify` that enables LLMs to send messages directly to users. The tool provides a communication channel for LLMs to surface important information, status updates, and contextual feedback during workflow execution.

## Problem Statement

Currently, LLMs working within MCP workflows have limited ways to communicate important information to users beyond their final responses. There are scenarios where the LLM needs to:

1. Notify users of important discoveries or issues during execution
2. Provide status updates for long-running operations
3. Surface warnings or recommendations based on code analysis
4. Log contextual information that may be useful for debugging
5. Communicate workflow state changes or decision points

## Solution: MCP Notify Tool

### Tool Definition

**Tool Name**: `notify`  
**Purpose**: Send messages from LLM to user through the logging system  
**Usage Context**: Available to LLMs during MCP workflow execution and prompt processing

### Parameters

```json
{
  "type": "object",
  "properties": {
    "message": {
      "type": "string",
      "description": "The message to notify the user about",
      "minLength": 1
    },
    "level": {
      "type": "string", 
      "enum": ["info", "warn", "error"],
      "description": "The notification level (default: info)",
      "default": "info"
    },
    "context": {
      "type": "object",
      "description": "Optional structured JSON data for the notification",
      "default": {}
    }
  },
  "required": ["message"]
}
```

### Initial Implementation

The initial implementation will use the tracing system to log messages:

```rust
match request.level.as_deref().unwrap_or("info") {
    "info" => tracing::info!(target: "llm_notify", context = request.context, "{}", request.message),
    "warn" => tracing::warn!(target: "llm_notify", context = request.context, "{}", request.message),
    "error" => tracing::error!(target: "llm_notify", context = request.context, "{}", request.message),
    _ => tracing::info!(target: "llm_notify", context = request.context, "{}", request.message),
}
```

## Use Cases

### 1. Code Analysis Notifications
```
notify: "Found potential security vulnerability in authentication logic at line 145"
notify: "Detected deprecated API usage - recommend updating to newer version"
```

### 2. Workflow Status Updates
```
notify: "Processing large codebase - this may take a few minutes"  
notify: "Completed analysis of 47 files, found 3 issues requiring attention"
```

### 3. Decision Point Communication
```
notify: "Multiple merge conflicts detected - manual intervention may be required"
notify: "Automatically selected main branch as merge target based on git history"
```

### 4. Discovery and Insights
```
notify: "Identified common pattern that could be extracted to shared utility"
notify: "Found existing implementation of requested feature in legacy module"
```

### 5. Warning and Recommendations
```json
{
  "message": "Database migration detected - recommend backing up data before proceeding",
  "level": "warn",
  "context": "safety"
}
```

## Integration Points

### Prompt Usage
Prompts can include instructions to use the notify tool:

```liquid
{% if complexity > 5 %}
{{ notify "Complex refactoring detected - breaking into smaller steps for safety" }}
{% endif %}
```

### Workflow Integration
LLMs can use notify during workflow execution:

```
1. Analyze codebase structure
2. {{ notify "Found 3 potential refactoring opportunities" }}
3. Prioritize changes by impact
4. {{ notify "Starting with highest-impact changes first" }}
```

### CLI Context
When running CLI commands, notifications appear in the terminal output stream, providing real-time feedback to users.

## Technical Requirements

### Tool Implementation
- **Module**: `swissarmyhammer-tools/src/mcp/tools/notify/`
- **Struct**: `NotifyTool`
- **Trait**: Implement `McpTool`
- **Validation**: Ensure message is not empty
- **Logging Target**: Use `llm_notify` as logging target for filtering

### Response Format
```json
{
  "content": [{
    "text": "Notification sent: {message}",
    "type": "text"
  }],
  "is_error": false
}
```

### Error Handling
- **Empty Message**: Return validation error
- **Invalid Level**: Default to "info" level
- **Logging Failure**: Should not cause tool execution to fail


## Benefits

1. **Real-time Feedback**: Users get immediate insight into LLM decision-making
2. **Improved Transparency**: Clear communication of what the LLM is doing and why
3. **Better Debugging**: Contextual information helps diagnose issues
4. **Enhanced UX**: Users stay informed during long-running operations
5. **Workflow Visibility**: Clear understanding of workflow progress and state
6. **Prompt Integration**: Seamless notification capability in prompt templates

## Implementation Considerations

### Performance
- None

### Security
- None

## Testing Strategy

1. **Unit Tests**: Test tool parameter validation and response formatting
2. **Integration Tests**: Verify logging integration and message delivery
3. **Prompt Tests**: Test notification usage within prompt templates
4. **CLI Tests**: Verify notifications appear correctly in CLI output

## Conclusion

The `notify` tool provides a crucial communication channel between LLMs and users, enabling better transparency, feedback, and workflow visibility. The initial logging-based implementation provides immediate value while establishing the foundation for more sophisticated notification mechanisms in the future.