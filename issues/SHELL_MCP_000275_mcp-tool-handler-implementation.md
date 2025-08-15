# MCP Tool Handler Implementation

Refer to /Users/wballard/github/sah-shell/ideas/shell.md

## Overview

Implement the complete MCP tool handler that integrates the shell execution engine with the MCP protocol, following the established patterns used by other tools in the system.

## Objective

Create the MCP tool handler function that processes incoming tool requests, validates parameters, executes shell commands, and returns properly formatted MCP responses.

## Requirements

### MCP Tool Handler Function
- Implement the main tool handler following existing patterns
- Handle parameter parsing and validation
- Integrate with the shell execution engine
- Return properly formatted MCP responses

### Parameter Validation
- Validate all incoming parameters against the JSON schema
- Provide clear error messages for invalid parameters
- Handle optional parameters with appropriate defaults
- Sanitize input parameters for security

### Response Formatting
- Format successful execution responses per specification
- Handle error responses with appropriate metadata
- Include comprehensive execution information
- Follow existing MCP response patterns

### Tool Registration
- Register the tool with the MCP tool registry
- Provide proper tool metadata and descriptions
- Integrate with the existing tool discovery system
- Follow naming and registration conventions

## Implementation Details

### MCP Tool Handler Structure
```rust
use serde_json::{json, Value};

pub async fn shell_execute_handler(
    args: Value
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    // Parse and validate parameters
    let params = parse_shell_parameters(args)?;
    
    // Execute shell command
    let result = execute_shell_command(
        params.command,
        params.working_directory,
        params.timeout,
        params.environment
    ).await?;
    
    // Format MCP response
    format_shell_response(result)
}
```

### Parameter Structure
```rust
#[derive(Debug, Deserialize)]
struct ShellParameters {
    command: String,
    working_directory: Option<String>,
    #[serde(default = "default_timeout")]
    timeout: u64,
    environment: Option<HashMap<String, String>>,
}

fn default_timeout() -> u64 { 300 }
```

### Response Formatting
Follow the specification response format:
```rust
fn format_shell_response(
    result: ShellExecutionResult
) -> Result<Value, ShellError> {
    Ok(json!({
        "content": [{
            "type": "text",
            "text": if result.exit_code == 0 {
                "Command executed successfully"
            } else {
                format!("Command failed with exit code {}", result.exit_code)
            }
        }],
        "is_error": result.exit_code != 0,
        "metadata": {
            "command": result.command,
            "exit_code": result.exit_code,
            "stdout": result.stdout,
            "stderr": result.stderr,
            "execution_time_ms": result.execution_time_ms,
            "working_directory": result.working_directory.display().to_string()
        }
    }))
}
```

## Integration Points

### Existing Tool Patterns
- Study `issues/create/mod.rs` and other tool implementations
- Follow the same parameter parsing patterns
- Use consistent error handling approaches
- Maintain the same response formatting style

### Tool Registry Integration
- Add to `tool_registry.rs` following existing patterns
- Provide proper tool metadata
- Ensure tool appears in MCP tool lists
- Follow naming conventions for tool identification

### Error Handling Integration
- Integrate with existing MCP error handling patterns
- Use consistent error response formats
- Provide detailed error context
- Follow established error propagation patterns

## Security Integration

### Input Validation
- Validate command strings for basic safety
- Check parameter bounds and formats
- Sanitize directory and environment inputs
- Prevent obvious injection patterns

### Audit Logging
- Log all shell command executions
- Include user context and execution details
- Log security-relevant events
- Follow existing logging patterns

## Acceptance Criteria

- [ ] MCP tool handler implemented following existing patterns
- [ ] Parameter validation works correctly
- [ ] Response formatting matches specification
- [ ] Tool registration integrated with registry
- [ ] Error handling comprehensive and consistent
- [ ] Security validation prevents basic attacks
- [ ] Audit logging captures execution details
- [ ] Integration with existing tool architecture

## Testing Requirements

- [ ] Unit tests for parameter validation
- [ ] Tests for response formatting
- [ ] Integration tests with MCP protocol
- [ ] Security tests for input validation
- [ ] Error handling tests
- [ ] Tool registration tests

## Notes

- This step brings together all the previous shell execution components
- Focus on proper MCP protocol integration and response formatting
- Ensure consistency with existing tool implementations
- Security validation is basic at this stage - comprehensive security comes later
- The tool should be fully functional after this step