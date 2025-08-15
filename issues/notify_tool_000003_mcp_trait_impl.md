# Implement McpTool Trait for NotifyTool

Refer to /Users/wballard/github/swissarmyhammer/ideas/notify_tool.md

## Objective
Implement the `McpTool` trait for the NotifyTool, defining the core MCP interface including name, schema, and basic execute structure.

## Tasks
1. Implement `McpTool` trait for `NotifyTool`
2. Define tool name as "notify" 
3. Create JSON schema for parameter validation
4. Implement basic execute method structure (without logging implementation)
5. Define proper response format

## McpTool Implementation Requirements

### Schema Definition
JSON schema matching the specification:
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

## Implementation Notes
- Follow existing patterns from other MCP tools like `issues/create/` or `memoranda/create/`
- Use proper async/await patterns
- Include comprehensive parameter validation
- Ensure proper error handling and response formatting
- Leave actual logging implementation as TODO for next step

## Success Criteria
- `McpTool` trait is fully implemented
- JSON schema validates correctly
- Basic execute method structure is in place
- Tool integrates with MCP protocol without errors
- Response format matches specification

## Dependencies
- Build on data types from step 000002